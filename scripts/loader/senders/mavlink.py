import json
import socket
import struct
import time
import threading
from collections import defaultdict, deque

import pyarrow as pa
import pyarrow.ipc as ipc
from pymavlink import mavutil
from PyQt6.QtCore import QObject, pyqtSignal


class MAVLinkStreamer(QObject):
    log_signal = pyqtSignal(str)
    status_signal = pyqtSignal(str)
    
    # Format: {message_type: id_field_name}
    SPLIT_BY_ID = {
        'BATTERY_STATUS': 'id',
        # 'ESC_STATUS': 'index',
        # 'SERVO_OUTPUT_RAW': 'port',
    }
    
    # Format: {message_type: {field_name: array_length}}
    EXPAND_ARRAYS = {
        'BATTERY_STATUS': {
            'voltages': 10,
            'voltages_ext': 4,
        },
        # 'ESC_STATUS': {
        #     'rpm': 4,
        #     'voltage': 4,
        #     'current': 4,
        # },
        # 'RC_CHANNELS': {
        #     'channels': 18,
        # },
    }
    
    def __init__(self, connection_string, baudrate, host, port, update_rate_hz):
        super().__init__()
        self.connection_string = connection_string
        self.baudrate = baudrate
        self.host = host
        self.port = port
        self.update_rate_hz = update_rate_hz
        self.update_interval = 1.0 / update_rate_hz
        
        self.start_time_us = int(time.time() * 1_000_000)
        self.last_sent_time_us = self.start_time_us
        
        self.sock = None
        self.mav = None
        self.running = False
        
        self.buffer_lock = threading.Lock()
        self.data_buffers = defaultdict(lambda: deque(maxlen=1000))
        
        self.parameters = {
            "mavlink_connection": connection_string,
            "buffer_size": 1000,
            "update_rate_hz": update_rate_hz
        }
        
        self.version_info = {"sw_version": "v1.0.0-mavlink-streamer"}
        
        self.mavlink_thread = None
        self.seen_message_types = set()
        
        self.total_messages_sent = 0
        self.total_tables_sent = 0
    
    def get_current_time_us(self):
        return int(time.time() * 1_000_000)
    
    def get_topic_name(self, msg):
        msg_type = msg.get_type()
        
        if msg_type in self.SPLIT_BY_ID:
            id_field = self.SPLIT_BY_ID[msg_type]
            try:
                id_value = getattr(msg, id_field, 0)
                return f"{msg_type}_{id_value}"
            except AttributeError:
                return msg_type
        
        return msg_type
    
    def get_array_fields_for_message(self, msg_type):
        return self.EXPAND_ARRAYS.get(msg_type, {})
    
    def should_expand_field(self, msg_type, field_name):
        """Check if a field should be expanded into individual columns"""
        array_config = self.get_array_fields_for_message(msg_type)
        return field_name in array_config
    
    def connect_mavlink(self):
        try:
            self.log_signal.emit(f"Connecting to MAVLink on {self.connection_string}...")
            
            if self.baudrate:
                self.mav = mavutil.mavlink_connection(self.connection_string, baud=self.baudrate)
            else:
                self.mav = mavutil.mavlink_connection(self.connection_string)
            
            self.log_signal.emit("Waiting for heartbeat...")
            return True
        except Exception as e:
            self.log_signal.emit(f"✗ Failed to connect to MAVLink: {e}")
            return False
    
    def mavlink_receive_loop(self):
        while self.running:
            try:
                msg = self.mav.recv_match(blocking=True)
                if msg is None:
                    continue
                
                msg_type = msg.get_type()
                if msg_type == 'BAD_DATA':
                    continue
                
                current_time_us = self.get_current_time_us()
                topic_name = self.get_topic_name(msg)
                
                with self.buffer_lock:
                    self.data_buffers[topic_name].append({
                        'timestamp': current_time_us,
                        'msg': msg
                    })
                    
                    if topic_name not in self.seen_message_types:
                        self.seen_message_types.add(topic_name)
                        
            except Exception as e:
                if self.running:
                    time.sleep(0.1)
    
    def convert_value_for_arrow(self, value):
        if isinstance(value, (list, tuple)):
            return json.dumps(value)
        elif isinstance(value, bytes):
            return value.hex()
        elif value is None:
            return 0
        return value
    
    def create_table_from_messages(self, topic_name, messages):
        if not messages:
            return None
        
        first_msg = messages[0]['msg']
        msg_type = first_msg.get_type()
        fieldnames = first_msg.get_fieldnames()
        
        array_config = self.get_array_fields_for_message(msg_type)
        
        data = defaultdict(list)
        data['timestamp'] = []
        
        for item in messages:
            msg = item['msg']
            relative_timestamp = item['timestamp'] - self.start_time_us
            data['timestamp'].append(relative_timestamp)
            
            for field in fieldnames:
                try:
                    value = getattr(msg, field)
                    
                    # Handle array expansion for configured fields
                    if field in array_config and isinstance(value, (list, tuple)):
                        expected_length = array_config[field]
                        for i in range(expected_length):
                            expanded_field_name = f"{field}_{i}"
                            if i < len(value):
                                data[expanded_field_name].append(value[i])
                            else:
                                # Pad with None if array is shorter than expected
                                data[expanded_field_name].append(None)
                    else:
                        # Normal field handling
                        data[field].append(value)
                        
                except AttributeError:
                    data[field].append(None)
        
        arrays = []
        names = []
        
        # Add timestamp column
        arrays.append(pa.array(data['timestamp'], type=pa.int64()))
        names.append('timestamp')
        
        # Process all other fields
        for field in fieldnames:
            # Skip fields that were expanded
            if field in array_config:
                expected_length = array_config[field]
                for i in range(expected_length):
                    expanded_field_name = f"{field}_{i}"
                    if expanded_field_name in data:
                        values = data[expanded_field_name]
                        try:
                            arrays.append(pa.array(values))
                            names.append(expanded_field_name)
                        except (pa.ArrowInvalid, pa.ArrowTypeError):
                            # Fallback to string conversion
                            string_values = [str(v) if v is not None else "" for v in values]
                            arrays.append(pa.array(string_values, type=pa.string()))
                            names.append(expanded_field_name)
                continue
            
            # Handle non-expanded fields
            values = data[field]
            
            if not values or all(v is None for v in values):
                continue
            
            try:
                first_value = next((v for v in values if v is not None), None)
                if first_value is None:
                    continue
                
                if isinstance(first_value, (list, tuple, bytes)):
                    converted_values = [self.convert_value_for_arrow(v) for v in values]
                    arrays.append(pa.array(converted_values, type=pa.string()))
                    names.append(field)
                else:
                    try:
                        arrays.append(pa.array(values))
                        names.append(field)
                    except (pa.ArrowInvalid, pa.ArrowTypeError):
                        string_values = [str(v) if v is not None else "" for v in values]
                        arrays.append(pa.array(string_values, type=pa.string()))
                        names.append(field)
                        
            except Exception:
                continue
        
        if not arrays:
            return None
        
        try:
            return pa.Table.from_arrays(arrays, names=names)
        except Exception:
            return None
    
    def generate_tables_from_buffers(self, start_time_us, end_time_us):
        tables = {}
        
        with self.buffer_lock:
            for topic_name, buffer in self.data_buffers.items():
                if not buffer:
                    continue
                
                messages = [
                    item for item in buffer
                    if start_time_us <= item['timestamp'] <= end_time_us
                ]
                
                if not messages:
                    continue
                
                try:
                    table = self.create_table_from_messages(topic_name, messages)
                    if table is not None:
                        table_name = topic_name.lower()
                        tables[table_name] = table
                except Exception:
                    pass
        
        return tables
    
    def send_update(self):
        if not self.sock:
            raise ConnectionError("Not connected to receiver")
        
        current_time_us = self.get_current_time_us()
        time_delta_us = current_time_us - self.last_sent_time_us
        
        if time_delta_us < 1000:
            return
        
        tables = self.generate_tables_from_buffers(self.last_sent_time_us, current_time_us)
        
        if not tables:
            self.last_sent_time_us = current_time_us
            return
        
        all_timestamps = []
        for table in tables.values():
            ts_array = table.column('timestamp').to_numpy()
            all_timestamps.extend(ts_array)
        
        if not all_timestamps:
            self.last_sent_time_us = current_time_us
            return
        
        import numpy as np
        min_ts = int(np.min(all_timestamps))
        max_ts = int(np.max(all_timestamps))
        
        if min_ts < 0:
            min_ts = 0
        
        metadata = {
            'parameters': self.parameters,
            'version_info': self.version_info,
            'table_count': len(tables),
            'table_names': list(tables.keys()),
            'timeline_range': {
                'min_timestamp': min_ts,
                'max_timestamp': max_ts
            }
        }
        
        metadata_json = json.dumps(metadata).encode('utf-8')
        metadata_len = struct.pack('<I', len(metadata_json))
        self.sock.sendall(metadata_len + metadata_json)
        
        total_rows = 0
        for table_name, table in tables.items():
            name_bytes = table_name.encode('utf-8')
            name_len = struct.pack('<I', len(name_bytes))
            self.sock.sendall(name_len + name_bytes)
            
            sink = pa.BufferOutputStream()
            with ipc.new_stream(sink, table.schema) as writer:
                writer.write_table(table)
            arrow_buffer = sink.getvalue()
            
            table_size = struct.pack('<Q', len(arrow_buffer))
            self.sock.sendall(table_size)
            self.sock.sendall(arrow_buffer)
            
            total_rows += table.num_rows
        
        self.last_sent_time_us = current_time_us
        self.total_messages_sent += total_rows
        self.total_tables_sent += len(tables)
        
        elapsed_sec = (current_time_us - self.start_time_us) / 1_000_000.0
        if total_rows > 0:
            status = f"{elapsed_sec:>7.1f}s | Tables: {len(tables):>2} | Rows: {total_rows:>4} | Total: {self.total_messages_sent:>7,} msgs"
            self.status_signal.emit(status)
    
    def connect_socket(self):
        max_retries = 5
        retry_delay = 2.0
        
        for attempt in range(max_retries):
            try:
                self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                self.sock.connect((self.host, self.port))
                return True
            except (ConnectionRefusedError, OSError):
                if self.sock:
                    self.sock.close()
                    self.sock = None
                if attempt < max_retries - 1:
                    time.sleep(retry_delay)
                else:
                    return False
        return False
    
    def disconnect_socket(self):
        if self.sock:
            try:
                self.sock.close()
            except:
                pass
            self.sock = None
    
    def run(self):
        try:
            if not self.connect_mavlink():
                self.log_signal.emit("Could not connect to MAVLink device. Exiting.")
                return
            
            self.running = True
            self.mavlink_thread = threading.Thread(target=self.mavlink_receive_loop, daemon=True)
            self.mavlink_thread.start()
            
            time.sleep(0.5)
            
            self.log_signal.emit(f"Connecting to receiver at {self.host}:{self.port}...")
            if not self.connect_socket():
                self.log_signal.emit("Could not establish connection to receiver.")
                self.running = False
                return
            
            self.log_signal.emit("✓ Connected to receiver\n")
            
            consecutive_errors = 0
            max_consecutive_errors = 3
            
            while self.running:
                loop_start = time.time()
                
                try:
                    self.send_update()
                    consecutive_errors = 0
                    
                except (BrokenPipeError, ConnectionResetError, OSError):
                    consecutive_errors += 1
                    self.disconnect_socket()
                    
                    if consecutive_errors >= max_consecutive_errors:
                        self.log_signal.emit("\n✗ Too many consecutive errors.")
                        break
                    
                    if not self.connect_socket():
                        time.sleep(2.0)
                        continue
                    
                    consecutive_errors = 0
                    continue
                
                elapsed = time.time() - loop_start
                sleep_time = max(0, self.update_interval - elapsed)
                time.sleep(sleep_time)
            
            self.log_signal.emit(f"\n✓ Total messages sent: {self.total_messages_sent:,}")
            
        except Exception as e:
            self.log_signal.emit(f"\n✗ Unexpected error: {e}")
        finally:
            self.running = False
            if self.mavlink_thread:
                self.mavlink_thread.join(timeout=2.0)
            self.disconnect_socket()
    
    def stop(self):
        self.running = False
