import json
import os
import socket
import struct
import time

import pyarrow as pa
import pyarrow.ipc as ipc
from pyulog.core import ULog
from PyQt6.QtCore import QObject, pyqtSignal


class ProgressULog(ULog):
    def __init__(self, log_file, progress_callback=None, *args, **kwargs):
        self._progress_callback = progress_callback
        self._progress_span = (15, 80)
        self._last_progress = -1
        self._last_progress_emit = 0.0
        self._file_size = 0
        super().__init__(log_file, *args, **kwargs)

    def _load_file(self, log_file, message_name_filter_list, parse_header_only=False):
        if isinstance(log_file, str):
            self._file_size = os.path.getsize(log_file)
        else:
            current_pos = log_file.tell()
            log_file.seek(0, os.SEEK_END)
            self._file_size = log_file.tell()
            log_file.seek(current_pos)

        self._emit_file_progress(0)
        super()._load_file(log_file, message_name_filter_list, parse_header_only)
        self._emit_percent(self._progress_span[1])

    def _read_file_header(self):
        super()._read_file_header()
        self._emit_percent(5)

    def _read_file_definitions(self):
        super()._read_file_definitions()
        self._emit_percent(15)

    def _read_file_data(self, message_name_filter_list, read_until=None):
        if read_until is None:
            read_until = 1 << 50

        try:
            header = self._MessageHeader()
            msg_data = self._MessageData()

            curr_file_pos = self._file_handle.tell()

            while True:
                data = self._file_handle.read(3)
                curr_file_pos += len(data)
                header.initialize(data)
                data = self._file_handle.read(header.msg_size)
                curr_file_pos += len(data)
                if len(data) < header.msg_size:
                    break

                if curr_file_pos > read_until:
                    if self._debug:
                        print('read until offset=%i done, current pos=%i' %
                              (read_until, curr_file_pos))
                    break

                self._emit_file_progress(curr_file_pos)

                try:
                    if header.msg_type == self.MSG_TYPE_INFO:
                        msg_info = self._MessageInfo(data, header)
                        self._msg_info_dict[msg_info.key] = msg_info.value
                        self._msg_info_dict_types[msg_info.key] = msg_info.type
                    elif header.msg_type == self.MSG_TYPE_INFO_MULTIPLE:
                        msg_info = self._MessageInfo(data, header, is_info_multiple=True)
                        self._add_message_info_multiple(msg_info)
                    elif header.msg_type == self.MSG_TYPE_PARAMETER:
                        msg_info = self._MessageInfo(data, header)
                        self._changed_parameters.append((self._last_timestamp,
                                                         msg_info.key, msg_info.value))
                    elif header.msg_type == self.MSG_TYPE_PARAMETER_DEFAULT:
                        msg_param = self._MessageParameterDefault(data, header)
                        self._add_parameter_default(msg_param)
                    elif header.msg_type == self.MSG_TYPE_ADD_LOGGED_MSG:
                        msg_add_logged = self._MessageAddLogged(data, header,
                                                                self._message_formats)
                        if (message_name_filter_list is None or
                                msg_add_logged.message_name in message_name_filter_list):
                            self._subscriptions[msg_add_logged.msg_id] = msg_add_logged
                        else:
                            self._filtered_message_ids.add(msg_add_logged.msg_id)
                    elif header.msg_type == self.MSG_TYPE_LOGGING:
                        msg_logging = self.MessageLogging(data, header)
                        self._logged_messages.append(msg_logging)
                    elif header.msg_type == self.MSG_TYPE_LOGGING_TAGGED:
                        msg_log_tagged = self.MessageLoggingTagged(data, header)
                        if msg_log_tagged.tag in self._logged_messages_tagged:
                            self._logged_messages_tagged[msg_log_tagged.tag].append(msg_log_tagged)
                        else:
                            self._logged_messages_tagged[msg_log_tagged.tag] = [msg_log_tagged]
                    elif header.msg_type == self.MSG_TYPE_DATA:
                        msg_data.initialize(data, header, self._subscriptions, self)
                        if msg_data.timestamp != 0 and msg_data.timestamp > self._last_timestamp:
                            self._last_timestamp = msg_data.timestamp
                    elif header.msg_type == self.MSG_TYPE_DROPOUT:
                        msg_dropout = self.MessageDropout(data, header,
                                                          self._last_timestamp)
                        self._dropouts.append(msg_dropout)
                    elif header.msg_type == self.MSG_TYPE_SYNC:
                        self._sync_seq_cnt = self._sync_seq_cnt + 1
                    else:
                        if self._debug:
                            print('_read_file_data: unknown message type: %i (%s)' %
                                  (header.msg_type, chr(header.msg_type)))
                            print('file position: %i msg size: %i' % (
                                curr_file_pos, header.msg_size))

                        if self._check_packet_corruption(header):
                            curr_file_pos = self._file_handle.seek(-2-header.msg_size, 1)

                            if self._has_sync:
                                self._find_sync()
                        else:
                            if self._has_sync:
                                self._find_sync(header.msg_size)

                except IndexError:
                    if not self._file_corrupt:
                        print("File corruption detected while reading file data!")
                        self._file_corrupt = True

        except struct.error:
            pass

        while self._subscriptions:
            _, value = self._subscriptions.popitem()
            if len(value.buffer) > 0:
                data_item = ULog.Data(value)
                self._data_list.append(data_item)
        self.data_list.sort(key=lambda ds: (ds.name, ds.multi_id))
        self._emit_percent(self._progress_span[1])

    def _emit_file_progress(self, file_pos: int) -> None:
        start, end = self._progress_span
        if self._file_size <= 0:
            self._emit_percent(start)
            return

        percent = start + int((file_pos / self._file_size) * (end - start))
        self._emit_percent(percent)

    def _emit_percent(self, percent: int) -> None:
        if self._progress_callback is None:
            return

        clamped = max(0, min(100, percent))
        now = time.monotonic()
        if clamped == self._last_progress and now - self._last_progress_emit < 0.1:
            return

        self._last_progress = clamped
        self._last_progress_emit = now
        self._progress_callback(clamped)


class ULGSender(QObject):
    log_signal = pyqtSignal(str)
    progress_signal = pyqtSignal(int)
    finished_signal = pyqtSignal(bool, str)
    
    def __init__(self, filename, host, port):
        super().__init__()
        self.filename = filename
        self.host = host
        self.port = port
        self._last_progress = -1
        self._last_progress_emit = 0.0
    
    def parse_ulg_file(self):
        ulg = ProgressULog(self.filename, progress_callback=self._emit_progress)
        tables = {}

        total_topics = max(1, len(ulg.data_list))
        for index, data in enumerate(ulg.data_list, start=1):
            if data.multi_id > 0:
                name = f"{data.name}_{data.multi_id}"
            else:
                name = data.name
            
            arrays = []
            field_names = []
            
            for field_name, field_data in data.data.items():
                arrays.append(pa.array(field_data))
                field_names.append(field_name)
            
            table = pa.Table.from_arrays(arrays, names=field_names)
            tables[name] = table
            self._emit_progress(80 + int((index / total_topics) * 20))
        
        parameters = ulg.initial_parameters
        version_info = ulg.msg_info_dict
        
        return tables, parameters, version_info
    
    def run(self):
        try:
            self.log_signal.emit(f"Parsing ULG file: {self.filename}")
            self._emit_progress(0)
            tables, parameters, version_info = self.parse_ulg_file()
            
            self.log_signal.emit(f"\nData Topics: {len(tables)}")
            for name, table in tables.items():
                self.log_signal.emit(f"  - {name}: {table.num_rows} rows, {table.num_columns} columns")
            
            self.log_signal.emit(f"\nConnecting to {self.host}:{self.port}...")
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.connect((self.host, self.port))
            self.log_signal.emit("✓ Connected successfully!")
            
            try:
                min_timestamp = None
                max_timestamp = None
                
                for table_name, table in tables.items():
                    if 'timestamp' in table.column_names:
                        timestamps = table.column('timestamp').to_pylist()
                        if timestamps:
                            valid_timestamps = [ts for ts in timestamps if ts != 0]
                            if valid_timestamps:
                                table_min = min(valid_timestamps)
                                table_max = max(valid_timestamps)
                                
                                if min_timestamp is None or table_min < min_timestamp:
                                    min_timestamp = table_min
                                if max_timestamp is None or table_max > max_timestamp:
                                    max_timestamp = table_max
                
                metadata = {
                    'parameters': {k: float(v) if isinstance(v, (int, float)) else str(v) 
                                  for k, v in parameters.items()},
                    'version_info': {k: str(v) for k, v in version_info.items()},
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
                self._emit_progress(100)
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
            self.finished_signal.emit(False, msg)

    def _emit_progress(self, percent: int) -> None:
        clamped = max(0, min(100, percent))
        now = time.monotonic()
        if clamped == self._last_progress and now - self._last_progress_emit < 0.1:
            return

        self._last_progress = clamped
        self._last_progress_emit = now
        self.progress_signal.emit(clamped)
