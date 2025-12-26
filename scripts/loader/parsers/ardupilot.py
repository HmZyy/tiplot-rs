import struct
from enum import IntEnum
from dataclasses import dataclass
from typing import Dict, List, Any, Optional, BinaryIO
from collections import defaultdict


class MessageType(IntEnum):
    FMT = 0x80
    PARM = 0x81
    MSG = 0x82


@dataclass
class FormatDefinition:
    msg_type: int
    msg_length: int
    name: str
    format_chars: str
    field_names: List[str]
    
    def get_struct_format(self) -> str:
        format_map = {
            'a': '64s', 'b': 'b', 'B': 'B', 'h': 'h', 'H': 'H',
            'i': 'i', 'I': 'I', 'f': 'f', 'd': 'd',
            'n': '4s', 'N': '16s', 'Z': '64s',
            'c': 'h', 'C': 'H', 'e': 'i', 'E': 'I',
            'L': 'i', 'M': 'B', 'q': 'q', 'Q': 'Q',
        }
        
        struct_format = '<'
        for char in self.format_chars:
            struct_format += format_map.get(char, 'B')
        
        return struct_format


class ArduPilotBinParser:
    HEAD1 = 0xA3
    HEAD2 = 0x95
    
    def __init__(self, filepath: str):
        self.filepath = filepath
        self.formats: Dict[int, FormatDefinition] = {}
        self.messages_by_type: Dict[str, List[Dict[str, Any]]] = defaultdict(list)
        self.parameters: Dict[str, float] = {}
        self.text_messages: List[str] = []
        self.version_info: Dict[str, str] = {}
        
    def parse(self) -> None:
        with open(self.filepath, 'rb') as f:
            while True:
                msg = self._read_message(f)
                if msg is None:
                    break
                
                if msg['type'] == 'FMT':
                    self._process_fmt_message(msg)
                elif msg['type'] == 'PARM':
                    self._process_parm_message(msg)
                elif msg['type'] == 'MSG':
                    self._process_text_message(msg)
                elif msg['type'] in ['UNIT', 'MULT', 'FMTU', 'UNKNOWN']:
                    continue
                else:
                    self.messages_by_type[msg['type']].append(msg)
    
    def _read_message(self, f: BinaryIO) -> Optional[Dict[str, Any]]:
        while True:
            byte = f.read(1)
            if not byte:
                return None
            
            if byte[0] == self.HEAD1:
                head2 = f.read(1)
                if head2 and head2[0] == self.HEAD2:
                    break
        
        msg_type_byte = f.read(1)
        if not msg_type_byte:
            return None
        
        msg_type = msg_type_byte[0]
        
        if msg_type == ord('Y'):
            actual_type = f.read(1)
            if actual_type and actual_type[0] == MessageType.FMT:
                return self._read_fmt_message(f)
        
        if msg_type == MessageType.FMT:
            return self._read_fmt_message(f)
        
        if msg_type == MessageType.MSG:
            return self._read_text_message(f)
        
        if msg_type in self.formats:
            return self._read_data_message(f, msg_type)
        else:
            return {'type': 'UNKNOWN', 'msg_type': msg_type}
    
    def _read_fmt_message(self, f: BinaryIO) -> Dict[str, Any]:
        fmt_data = f.read(86)
        if len(fmt_data) < 86:
            return None
        
        msg_type, msg_length = struct.unpack_from('<BB', fmt_data, 0)
        name = fmt_data[2:6].rstrip(b'\x00').decode('ascii', errors='ignore')
        format_str = fmt_data[6:22].rstrip(b'\x00').decode('ascii', errors='ignore')
        columns = fmt_data[22:86].rstrip(b'\x00').decode('ascii', errors='ignore')
        
        field_names = [col.strip() for col in columns.split(',') if col.strip()]
        
        return {
            'type': 'FMT',
            'msg_type': msg_type,
            'msg_length': msg_length,
            'name': name,
            'format': format_str,
            'columns': field_names
        }
    
    def _read_text_message(self, f: BinaryIO) -> Dict[str, Any]:
        msg_data = f.read(72)
        if len(msg_data) < 72:
            return None
        
        time_us, message = struct.unpack('<Q64s', msg_data)
        message = message.rstrip(b'\x00').decode('ascii', errors='ignore')
        
        return {
            'type': 'MSG',
            'TimeUS': time_us,
            'Message': message
        }
    
    def _read_data_message(self, f: BinaryIO, msg_type: int) -> Optional[Dict[str, Any]]:
        fmt_def = self.formats[msg_type]
        data_length = fmt_def.msg_length - 3
        
        if data_length <= 0:
            return None
        
        data = f.read(data_length)
        if len(data) < data_length:
            return None
        
        try:
            struct_format = fmt_def.get_struct_format()
            expected_size = struct.calcsize(struct_format)
            
            if expected_size > data_length:
                return None
            
            values = struct.unpack(struct_format, data[:expected_size])
            message = {'type': fmt_def.name}
            
            for i, (field_name, value) in enumerate(zip(fmt_def.field_names, values)):
                if isinstance(value, bytes):
                    value = value.rstrip(b'\x00').decode('ascii', errors='ignore')
                
                format_char = fmt_def.format_chars[i] if i < len(fmt_def.format_chars) else ''
                if format_char in ['c', 'C', 'e', 'E']:
                    value = value / 100.0
                elif format_char == 'L':
                    value = value / 1e7
                
                message[field_name] = value
            
            return message
            
        except struct.error:
            return None
    
    def _process_fmt_message(self, msg: Dict[str, Any]) -> None:
        fmt_def = FormatDefinition(
            msg_type=msg['msg_type'],
            msg_length=msg['msg_length'],
            name=msg['name'],
            format_chars=msg['format'],
            field_names=msg['columns']
        )
        self.formats[msg['msg_type']] = fmt_def
    
    def _process_parm_message(self, msg: Dict[str, Any]) -> None:
        if 'Name' in msg and 'Value' in msg:
            self.parameters[msg['Name']] = msg['Value']
    
    def _process_text_message(self, msg: Dict[str, Any]) -> None:
        if 'Message' in msg:
            message = msg['Message']
            self.text_messages.append(message)
            
            if any(x in message for x in ['ArduPlane', 'ArduCopter', 'ArduRover', 'ArduSub']):
                self.version_info['sw_version'] = message
    
    def get_messages_by_type(self, msg_type: str) -> List[Dict[str, Any]]:
        return self.messages_by_type.get(msg_type, [])
    
    def get_available_message_types(self) -> List[str]:
        return sorted(list(self.messages_by_type.keys()))
    
    def get_timeline_range(self) -> tuple:
        min_timestamp = None
        max_timestamp = None
        
        for msg_list in self.messages_by_type.values():
            for msg in msg_list:
                if 'TimeUS' in msg:
                    ts = msg['TimeUS']
                    if min_timestamp is None or ts < min_timestamp:
                        min_timestamp = ts
                    if max_timestamp is None or ts > max_timestamp:
                        max_timestamp = ts
        
        return min_timestamp, max_timestamp
