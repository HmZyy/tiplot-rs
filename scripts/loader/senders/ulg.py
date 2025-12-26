import json
import socket
import struct

import pyarrow as pa
import pyarrow.ipc as ipc
from pyulog import ULog
from PyQt6.QtCore import QObject, pyqtSignal


class ULGSender(QObject):
    log_signal = pyqtSignal(str)
    finished_signal = pyqtSignal(bool, str)
    
    def __init__(self, filename, host, port):
        super().__init__()
        self.filename = filename
        self.host = host
        self.port = port
    
    def parse_ulg_file(self):
        ulg = ULog(self.filename)
        tables = {}
        
        for data in ulg.data_list:
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
        
        parameters = ulg.initial_parameters
        version_info = ulg.msg_info_dict
        
        return tables, parameters, version_info
    
    def run(self):
        try:
            self.log_signal.emit(f"Parsing ULG file: {self.filename}")
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
