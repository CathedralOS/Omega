#!/usr/bin/env python3
"""Copy the emitted private entry and its image at their original ELF addresses."""
import argparse
import hashlib
import json
from pathlib import Path
import struct


def require(condition, message):
    if not condition:
        raise ValueError(message)


def extract(directory, entry_control="InterruptReturn"):
    data = (directory / "native-image.bin").read_bytes()
    entries = [
        path for path in directory.glob("private-entry-*.txt")
        if f"entry_control=Some({entry_control})" in path.read_text()
    ]
    require(len(entries) == 1, "exactly one compiler-recorded entry of the selected kind required")
    record = entries[0].read_text()
    require("body_foreign_calls=0\n" in record, "the CPU witness cannot call host imports")
    if entry_control == "InterruptReturn":
        require("body_footprint=Some(" in record, "actual body footprint required")
    require(len(data) >= 64 and data[:6] == b"\x7fELF\x02\x01", "64-bit little-endian ELF required")
    require(struct.unpack_from("<H", data, 18)[0] == 62, "x86-64 ELF required")
    header_offset = struct.unpack_from("<Q", data, 32)[0]
    header_size, header_count = struct.unpack_from("<HH", data, 54)
    require(header_size >= 56, "truncated ELF program header")
    require(header_offset + header_size * header_count <= len(data), "program headers outside ELF")
    segments = []
    for ordinal in range(header_count):
        kind, flags, offset, address, _, size, memory, _ = struct.unpack_from(
            "<IIQQQQQQ", data, header_offset + ordinal * header_size
        )
        if kind == 1:
            require(size <= memory and offset + size <= len(data), "invalid ELF load extent")
            segments.append(dict(flags=flags, offset=offset, address=address, size=size, memory=memory))
    require(bool(segments), "ELF has no load segments")
    segments.sort(key=lambda segment: segment["address"])
    start = segments[0]["address"]
    end = max(segment["address"] + segment["memory"] for segment in segments)
    require(0x200000 <= start < end < 0x2000000, "ELF exceeds the harness payload range")
    for left, right in zip(segments, segments[1:]):
        require(left["address"] + left["memory"] <= right["address"], "overlapping ELF load segments")
    fields = dict(line.split("=", 1) for line in record.splitlines())
    entry = int(fields["entry_address"], 0)
    require(any(
        segment["flags"] & 1
        and segment["address"] <= entry < segment["address"] + segment["size"]
        for segment in segments
    ), "exact compiler entry outside executable bytes")
    payload = bytearray(end - start)
    for segment in segments:
        offset = segment["address"] - start
        payload[offset:offset + segment["size"]] = data[segment["offset"]:segment["offset"] + segment["size"]]
    receipt = dict(
        entry_control=entry_control,
        elf_sha256=hashlib.sha256(data).hexdigest(),
        entry_record_sha256=hashlib.sha256(entries[0].read_bytes()).hexdigest(),
        payload_sha256=hashlib.sha256(payload).hexdigest(),
        load_address=start, entry_address=entry, entry_offset=entry - start,
        segments=segments,
        placement="original ELF addresses; file bytes copied and BSS zero initialized; no relocations added",
    )
    (directory / "payload.bin").write_bytes(payload)
    (directory / "placement.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("export", type=Path)
    parser.add_argument("--entry-control", choices=["InterruptReturn", "CallReturn"], default="InterruptReturn")
    arguments = parser.parse_args()
    print(json.dumps(extract(arguments.export, arguments.entry_control), indent=2))


if __name__ == "__main__":
    main()
