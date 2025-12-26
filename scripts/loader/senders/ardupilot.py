import json
import socket
import struct
from typing import List, Dict, Any
from collections import defaultdict

import pyarrow as pa
import pyarrow.ipc as ipc
from PyQt6.QtCore import QObject, pyqtSignal

from parsers.ardupilot import ArduPilotBinParser


class ArduPilotSender(QObject):
    log_signal = pyqtSignal(str)
    finished_signal = pyqtSignal(bool, str)
    
    def __init__(self, filename, host, port):
        super().__init__()
        self.filename = filename
        self.host = host
        self.port = port
    
    def messages_to_arrow_table(self, msg_type: str, messages: List[Dict[str, Any]]):
        if not messages:
            return None
        
        data = defaultdict(list)
        
        for msg in messages:
            for key, value in msg.items():
                if key == 'type':
                    continue
                data[key].append(value)
        
        arrays = []
        names = []
        
        if 'TimeUS' in data:
            arrays.append(pa.array(data['TimeUS'], type=pa.int64()))
            names.append('timestamp')
        
        for field_name, values in data.items():
            if field_name == 'TimeUS':
                continue
                
            try:
                arrays.append(pa.array(values))
                names.append(field_name)
            except (pa.ArrowInvalid, pa.ArrowTypeError):
                string_values = [str(v) if v is not None else "" for v in values]
                arrays.append(pa.array(string_values, type=pa.string()))
                names.append(field_name)
        
        if not arrays:
            return None
        
        try:
            return pa.Table.from_arrays(arrays, names=names)
        except Exception:
            return None
    
    def run(self):
        try:
            self.log_signal.emit(f"Parsing ArduPilot .BIN file: {self.filename}")
            
            parser = ArduPilotBinParser(self.filename)
            parser.parse()
            
            tables = {}
            for msg_type in parser.get_available_message_types():
                messages = parser.get_messages_by_type(msg_type)
                if messages:
                    table = self.messages_to_arrow_table(msg_type, messages)
                    if table is not None:
                        tables[msg_type.lower()] = table
            
            self.log_signal.emit(f"\nData Topics: {len(tables)}")
            for name, table in tables.items():
                self.log_signal.emit(f"  - {name}: {table.num_rows} rows, {table.num_columns} columns")
            
            self.log_signal.emit(f"\nConnecting to {self.host}:{self.port}...")
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.connect((self.host, self.port))
            self.log_signal.emit("✓ Connected successfully!")
            
            try:
                min_timestamp, max_timestamp = parser.get_timeline_range()
                
                metadata = {
                    'parameters': {k: float(v) if isinstance(v, (int, float)) else str(v) 
                                  for k, v in parser.parameters.items()},
                    'version_info': parser.version_info,
                    'table_count': len(tables),
                    'table_names': list(tables.keys()),
                    'timeline_range': {
                        'min_timestamp': int(min_timestamp) if min_timestamp is not None else None,
                        'max_timestamp': int(max_timestamp) if max_timestamp is not None else None
                    }
                }
                
                if min_timestamp is not None and max_timestamp is not None:
                    duration = max_timestamp - min_timestamp
                    duration_sec = duration / 1e6
                    self.log_signal.emit(f"\nTimeline Range:")
                    self.log_signal.emit(f"  Min: {min_timestamp} ({min_timestamp/1e6:.2f}s)")
                    self.log_signal.emit(f"  Max: {max_timestamp} ({max_timestamp/1e6:.2f}s)")
                    self.log_signal.emit(f"  Duration: {duration_sec:.2f}s")
                
                metadata_json = json.dumps(metadata).encode('utf-8')
                metadata_len = struct.pack('<I', len(metadata_json))
                sock.sendall(metadata_len + metadata_json)
                self.log_signal.emit(f"\nSent metadata ({len(metadata_json)} bytes)")
                
                for table_name, table in tables.items():
                    name_bytes = table_name.encode('utf-8')
                    name_len = struct.pack('<I', len(name_bytes))
                    sock.sendall(name_len + name_bytes)
                    
                    sink = pa.BufferOutputStream()
                    with ipc.new_stream(sink, table.schema) as writer:
                        writer.write_table(table)
                    
                    arrow_buffer = sink.getvalue()
                    table_size = struct.pack('<Q', len(arrow_buffer))
                    sock.sendall(table_size)
                    sock.sendall(arrow_buffer)
                
                self.log_signal.emit("\n✓ All data sent successfully!")
                self.finished_signal.emit(True, "Success")
                
            finally:
                sock.close()
                
        except ConnectionRefusedError:
            msg = f"Could not connect to {self.host}:{self.port}\nMake sure the receiver is running."
            self.log_signal.emit(f"\n✗ {msg}")
            self.finished_signal.emit(False, msg)
        except Exception as e:
            msg = f"Error: {e}"
            self.log_signal.emit(f"\n✗ {msg}")
            import traceback
            traceback.print_exc()
            self.finished_signal.emit(False, msg)
