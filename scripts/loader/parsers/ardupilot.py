import mmap
import struct
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Any, BinaryIO, Dict, List, Optional, Tuple
from collections import defaultdict


FMT_MESSAGE_TYPE = 0x80
GPS_EPOCH = datetime(1980, 1, 6, tzinfo=timezone.utc)
GPS_UTC_LEAP_SECONDS = 18


@dataclass
class FormatDefinition:
    msg_type: int
    msg_length: int
    name: str
    format_chars: str
    field_names: List[str]
    normalized_field_names: Tuple[str, ...]
    decode_plan: Tuple[Tuple[str, int, int], ...]
    payload_length: int


SCALAR_STRUCTS = {
    "b": struct.Struct("<b"),
    "B": struct.Struct("<B"),
    "h": struct.Struct("<h"),
    "H": struct.Struct("<H"),
    "i": struct.Struct("<i"),
    "I": struct.Struct("<I"),
    "q": struct.Struct("<q"),
    "Q": struct.Struct("<Q"),
    "f": struct.Struct("<f"),
    "d": struct.Struct("<d"),
}

FORMAT_SIZES = {
    "b": 1,
    "B": 1,
    "h": 2,
    "H": 2,
    "i": 4,
    "I": 4,
    "q": 8,
    "Q": 8,
    "f": 4,
    "d": 8,
    "c": 2,
    "C": 2,
    "e": 4,
    "E": 4,
    "L": 4,
    "n": 4,
    "N": 16,
    "Z": 64,
    "a": 64,
    "M": 1,
}

HEADER_BYTES = b"\xA3\x95"


class ArduPilotBinParser:
    HEAD1 = 0xA3
    HEAD2 = 0x95

    def __init__(self, filepath: str):
        self.filepath = filepath
        self.formats: Dict[int, FormatDefinition] = {}
        self.formats_by_name: Dict[str, FormatDefinition] = {}
        self.messages_by_type: Dict[str, List[Dict[str, Any]]] = defaultdict(list)
        self.parameters: Dict[str, float] = {}
        self.text_messages: List[str] = []
        self.version_info: Dict[str, str] = {}

        self.line_start_offsets: List[int] = []
        self.message_index: Dict[int, List[int]] = defaultdict(list)
        self.message_index_line: Dict[int, List[int]] = defaultdict(list)

        self.fmtu_by_type: Dict[int, Tuple[str, str]] = {}
        self.unit_by_id: Dict[str, str] = {}
        self.mult_by_id: Dict[str, str] = {}
        self.unit_mult_list: List[Tuple[str, str, str, float]] = []
        self.instance_field_by_type: Dict[int, Tuple[int, List[str]]] = {}

        self.gps_start_time: Optional[datetime] = None
        self.ms_offset: Optional[float] = None
        self.firmware_type: Optional[str] = None

    def parse(self) -> None:
        with open(self.filepath, "rb") as handle:
            with mmap.mmap(handle.fileno(), 0, access=mmap.ACCESS_READ) as mm:
                if not self._looks_like_binary_log(mm):
                    raise ValueError("File is not an ArduPilot DataFlash binary log")

                data_length = len(mm)
                cursor = 0
                line_number = 0

                while True:
                    offset = self._sync_to_next_header(mm, cursor)
                    if offset is None:
                        break

                    if offset + 3 > data_length:
                        break

                    msg_type = mm[offset + 2]
                    self.line_start_offsets.append(offset)
                    self.message_index[msg_type].append(offset)
                    self.message_index_line[msg_type].append(line_number)
                    line_number += 1

                    if msg_type == FMT_MESSAGE_TYPE:
                        next_cursor = self._parse_fmt_packet(mm, offset)
                        if next_cursor is None:
                            break
                        cursor = next_cursor
                        continue

                    fmt_def = self.formats.get(msg_type)
                    if fmt_def is None:
                        cursor = offset + 1
                        continue

                    payload_start = offset + 3
                    payload_end = payload_start + fmt_def.payload_length
                    if payload_end > data_length:
                        break

                    try:
                        message = self._decode_message(mm, payload_start, msg_type, offset, fmt_def)
                    except (struct.error, ValueError):
                        cursor = offset + 1
                        continue

                    self._process_message(fmt_def.name, message)
                    cursor = payload_end

        self._build_unit_multi_list()
        self._discover_instances()
        self._detect_gps_time_anchor()

    def _looks_like_binary_log(self, data: Any) -> bool:
        return len(data) >= 2 and data[0] == self.HEAD1 and data[1] == self.HEAD2

    def _sync_to_next_header(self, data: Any, start: int) -> Optional[int]:
        offset = data.find(HEADER_BYTES, start)
        if offset < 0:
            return None
        return offset

    def _parse_fmt_packet(self, data: Any, offset: int) -> Optional[int]:
        payload_start = offset + 3
        payload_end = payload_start + 86
        if payload_end > len(data):
            return None

        payload = data[payload_start:payload_end]
        msg_type, msg_length = struct.unpack_from("<BB", payload, 0)
        name = payload[2:6].rstrip(b"\x00").decode("ascii", errors="ignore")
        format_chars = payload[6:22].rstrip(b"\x00").decode("ascii", errors="ignore")
        columns = payload[22:86].rstrip(b"\x00").decode("ascii", errors="ignore")
        field_names = [column.strip() for column in columns.split(",") if column.strip()]

        fmt_def = FormatDefinition(
            msg_type=msg_type,
            msg_length=msg_length,
            name=name,
            format_chars=format_chars,
            field_names=field_names,
            normalized_field_names=self._normalize_field_names(field_names, len(format_chars)),
            decode_plan=self._build_decode_plan(format_chars),
            payload_length=msg_length - 3,
        )
        self.formats[fmt_def.msg_type] = fmt_def
        self.formats_by_name[fmt_def.name] = fmt_def
        return payload_end

    def _build_decode_plan(self, format_chars: str) -> Tuple[Tuple[str, int, int], ...]:
        offset = 0
        plan = []
        for code in format_chars:
            size = FORMAT_SIZES.get(code)
            if size is None:
                raise ValueError(f"Unsupported format code: {code}")
            plan.append((code, offset, size))
            offset += size
        return tuple(plan)

    def _normalize_field_names(self, field_names: List[str], value_count: int) -> Tuple[str, ...]:
        normalized = list(field_names[:value_count])
        if len(normalized) < value_count:
            for index in range(len(normalized), value_count):
                normalized.append(f"field_{index}")
        return tuple(normalized)

    def _decode_message(
        self,
        data: Any,
        payload_start: int,
        msg_type: int,
        offset: int,
        fmt_def: FormatDefinition,
    ) -> Dict[str, Any]:
        message = {
            "type": fmt_def.name,
            "offset": offset,
            "msg_type": msg_type,
        }
        names = fmt_def.normalized_field_names

        for index, (code, field_offset, size) in enumerate(fmt_def.decode_plan):
            field_name = names[index]
            start = payload_start + field_offset
            message[field_name] = self._decode_value(data, start, code, size)

        return message

    def _decode_value(self, data: Any, start: int, code: str, size: int) -> Any:
        scalar = SCALAR_STRUCTS.get(code)
        if scalar is not None:
            return scalar.unpack_from(data, start)[0]
        if code == "c":
            return SCALAR_STRUCTS["h"].unpack_from(data, start)[0] / 100.0
        if code == "C":
            return SCALAR_STRUCTS["H"].unpack_from(data, start)[0] / 100.0
        if code == "e":
            return SCALAR_STRUCTS["i"].unpack_from(data, start)[0] / 100.0
        if code == "E":
            return SCALAR_STRUCTS["I"].unpack_from(data, start)[0] / 100.0
        if code == "L":
            return SCALAR_STRUCTS["i"].unpack_from(data, start)[0] / 10000000.0
        if code == "n":
            return data[start:start + size].rstrip(b"\x00").decode("ascii", errors="ignore")
        if code == "N":
            return data[start:start + size].rstrip(b"\x00").decode("ascii", errors="ignore")
        if code == "Z":
            return data[start:start + size]
        if code == "a":
            return list(struct.unpack_from("<32h", data, start))
        if code == "M":
            return data[start]
        raise ValueError(f"Unsupported format code: {code}")

    def _process_message(self, msg_type: str, message: Dict[str, Any]) -> None:
        if msg_type == "PARM":
            self._process_parm_message(message)
        elif msg_type == "MSG":
            self._process_text_message(message)
        elif msg_type == "FMTU":
            self._process_fmtu_message(message)
        elif msg_type == "UNIT":
            self._process_unit_message(message)
        elif msg_type == "MULT":
            self._process_mult_message(message)
        else:
            self.messages_by_type[msg_type].append(message)

    def _process_parm_message(self, message: Dict[str, Any]) -> None:
        name = message.get("Name")
        value = message.get("Value")
        if isinstance(name, str) and value is not None:
            self.parameters[name] = value
            self._update_firmware_hint_from_text(name)

    def _process_text_message(self, message: Dict[str, Any]) -> None:
        text = self._extract_text_message(message)
        if not text:
            return

        self.text_messages.append(text)
        self._update_firmware_hint_from_text(text)
        if any(marker in text for marker in ("ArduPlane", "ArduCopter", "ArduRover", "ArduSub", "ArduTracker")):
            self.version_info["sw_version"] = text

    def _process_fmtu_message(self, message: Dict[str, Any]) -> None:
        fmt_type = message.get("FmtType")
        unit_ids = message.get("UnitIds")
        mult_ids = message.get("MultIds")
        if isinstance(fmt_type, int) and isinstance(unit_ids, str) and isinstance(mult_ids, str):
            self.fmtu_by_type[fmt_type] = (unit_ids, mult_ids)
            instance_index = unit_ids.find("#")
            if instance_index >= 0:
                self.instance_field_by_type[fmt_type] = (instance_index, [])

    def _process_unit_message(self, message: Dict[str, Any]) -> None:
        unit_id = message.get("Id")
        label = message.get("Label")
        if isinstance(unit_id, int) and isinstance(label, str):
            self.unit_by_id[chr(unit_id)] = label

    def _process_mult_message(self, message: Dict[str, Any]) -> None:
        mult_id = message.get("Id")
        mult = message.get("Mult")
        if isinstance(mult_id, int):
            self.mult_by_id[chr(mult_id)] = str(mult).strip()

    def _extract_text_message(self, message: Dict[str, Any]) -> str:
        for key, value in message.items():
            if key in {"type", "offset", "msg_type"}:
                continue
            if isinstance(value, bytes):
                text = value.decode("ascii", errors="ignore").rstrip("\x00").strip()
                if text:
                    return text
            if isinstance(value, str):
                text = value.strip()
                if text:
                    return text
        return ""

    def _update_firmware_hint_from_text(self, text: str) -> None:
        if "RATE_RLL_P" in text or "ArduCopter" in text or "Copter" in text:
            self.firmware_type = "ArduCopter2"
        elif "H_SWASH_PLATE" in text:
            self.firmware_type = "ArduCopter2"
        elif "PTCH2SRV_P" in text or "ArduPlane" in text or "Plane" in text:
            self.firmware_type = "ArduPlane"
        elif "SKID_STEER_OUT" in text or "ArduRover" in text or "Rover" in text:
            self.firmware_type = "ArduRover"
        elif "AntennaTracker" in text or "Tracker" in text:
            self.firmware_type = "ArduTracker"

    def _build_unit_multi_list(self) -> None:
        self.unit_mult_list = []

        for msg_type, fmt_def in self.formats.items():
            fmtu = self.fmtu_by_type.get(msg_type)
            if fmtu is None:
                continue

            unit_ids, mult_ids = fmtu
            for index, field_name in enumerate(fmt_def.normalized_field_names):
                unit = self.unit_by_id.get(unit_ids[index], "") if index < len(unit_ids) else ""
                multiplier = 1.0

                if index < len(mult_ids):
                    try:
                        multiplier = float(self.mult_by_id.get(mult_ids[index], "1"))
                    except ValueError:
                        multiplier = 1.0

                if fmt_def.format_chars[index] in {"c", "C", "e", "E", "L"}:
                    multiplier = 1.0

                self.unit_mult_list.append((fmt_def.name, field_name, unit, multiplier))

    def _discover_instances(self) -> None:
        for msg_type, (instance_index, values) in self.instance_field_by_type.items():
            fmt_def = self.formats.get(msg_type)
            if fmt_def is None:
                continue

            messages = self.messages_by_type.get(fmt_def.name, [])
            if instance_index >= len(fmt_def.normalized_field_names):
                continue

            field_name = fmt_def.normalized_field_names[instance_index]
            for message in messages[:2000]:
                value = message.get(field_name)
                if value is None:
                    continue
                value_text = str(value)
                if value_text not in values:
                    values.append(value_text)

    def _detect_gps_time_anchor(self) -> None:
        for msg_type in ("GPS", "GPS2", "GPSB"):
            for message in self.messages_by_type.get(msg_type, []):
                status = message.get("Status")
                if not isinstance(status, int) or status < 3:
                    continue

                week = message.get("Week", message.get("GWk"))
                time_ms = message.get("TimeMS", message.get("GMS"))
                if not isinstance(week, int) or not isinstance(time_ms, int):
                    continue

                gps_time = self._gps_time_to_utc(week, time_ms / 1000.0)
                if gps_time is None:
                    continue

                record_ms = self._extract_record_time_ms(message)
                if record_ms is None:
                    continue

                self.gps_start_time = gps_time
                self.ms_offset = record_ms
                return

    def _gps_time_to_utc(self, week: int, seconds: float) -> Optional[datetime]:
        if week < 0 or week > 5000:
            return None
        if seconds < 0 or seconds > 7 * 24 * 60 * 60:
            return None
        return GPS_EPOCH + timedelta(weeks=week, seconds=seconds - GPS_UTC_LEAP_SECONDS)

    def _extract_record_time_ms(self, message: Dict[str, Any]) -> Optional[float]:
        time_us = message.get("TimeUS")
        if isinstance(time_us, int):
            return time_us / 1000.0

        time_ms = message.get("TimeMS")
        if isinstance(time_ms, int):
            return float(time_ms)

        board_time = message.get("T")
        if isinstance(board_time, int):
            return float(board_time)

        return None

    def get_messages_by_type(self, msg_type: str) -> List[Dict[str, Any]]:
        return self.messages_by_type.get(msg_type, [])

    def get_available_message_types(self) -> List[str]:
        return sorted(self.messages_by_type.keys())

    def get_timeline_range(self) -> Tuple[Optional[int], Optional[int]]:
        min_timestamp = None
        max_timestamp = None

        for messages in self.messages_by_type.values():
            for message in messages:
                timestamp = self._extract_timeline_timestamp_us(message)
                if timestamp is None:
                    continue
                if min_timestamp is None or timestamp < min_timestamp:
                    min_timestamp = timestamp
                if max_timestamp is None or timestamp > max_timestamp:
                    max_timestamp = timestamp

        return min_timestamp, max_timestamp

    def _extract_timeline_timestamp_us(self, message: Dict[str, Any]) -> Optional[int]:
        time_us = message.get("TimeUS")
        if isinstance(time_us, int):
            return time_us

        time_ms = message.get("TimeMS")
        if isinstance(time_ms, int):
            return time_ms * 1000

        board_time = message.get("T")
        if isinstance(board_time, int):
            return board_time * 1000

        return None
