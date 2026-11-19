"""Check both published target directories against the recorded compiler comparison."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess


RECORD = Path(__file__).resolve().parents[1] / "checkpoints/target-execution-4650.json"


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def macho_signature(data):
    """Check this arm64 executable's entry, signed pages and special slots."""
    magic, cpu, _, kind, count = struct.unpack_from("<5I", data)
    require((magic, cpu, kind) == (0xfeedfacf, 0x0100000c, 2), "expected an arm64 Mach-O executable")
    offset = 32
    text = entry = signature = None
    for _ in range(count):
        command, size = struct.unpack_from("<II", data, offset)
        require(8 <= size <= len(data) - offset, "invalid Mach-O load command")
        if command == 0x19 and data[offset + 8:offset + 24].rstrip(b"\0") == b"__TEXT":
            location, length = struct.unpack_from("<QQ", data, offset + 40)
            flags, = struct.unpack_from("<I", data, offset + 60)
            require(text is None and flags & 4 and location + length <= len(data), "invalid executable text")
            text = range(location, location + length)
        elif command == 0x80000028:
            require(entry is None, "duplicate entry")
            entry, = struct.unpack_from("<Q", data, offset + 8)
        elif command == 0x1d:
            require(signature is None, "duplicate signature")
            signature = struct.unpack_from("<II", data, offset + 8)
        offset += size
    require(text is not None and entry in text and signature is not None, "missing entry, text or signature")
    position, length = signature
    blob = data[position:position + length]
    magic, actual, count = struct.unpack_from(">III", blob)
    require(magic == 0xfade0cc0 and len(blob) == actual == length, "invalid signature superblob")
    slots = {}
    for index in range(count):
        slot, start = struct.unpack_from(">II", blob, 12 + 8 * index)
        require(slot not in slots and 12 + 8 * count <= start <= length - 8, "invalid signature slot")
        size, = struct.unpack_from(">I", blob, start + 4)
        require(8 <= size <= length - start, "invalid signature slot length")
        slots[slot] = blob[start:start + size]
    directory = slots[0]
    magic, size, version, flags, hashes, _, special, pages, limit, width, algorithm, _, shift = struct.unpack_from(">9I4B", directory)
    require(magic == 0xfade0c02 and size == len(directory) and flags & 2, "invalid ad-hoc code directory")
    if limit == 0xffffffff and version >= 0x20300:
        limit, = struct.unpack_from(">Q", directory, 56)
    require((width, algorithm) == (32, 2) and 8 <= shift <= 20 and limit == position, "invalid hash geometry")
    page_size = 1 << shift
    require((limit + page_size - 1) // page_size == pages, "invalid signed page count")
    require(hashes >= special * width and hashes + pages * width <= size, "hash slots exceed code directory")
    for index in range(-special, pages):
        if index < 0:
            expected = hashlib.sha256(slots[-index]).digest() if -index in slots else bytes(width)
        else:
            expected = hashlib.sha256(data[index * page_size:min(limit, (index + 1) * page_size)]).digest()
        start = hashes + index * width
        require(directory[start:start + width] == expected, "signed digest mismatch")
    return dict(architecture="arm64", entry_file_offset=entry, signed_bytes=limit,
                code_directory_sha256=sha256(directory), code_pages=pages,
                page_size=page_size, verified_special_slots=special)


def check_macho(data):
    result = macho_signature(data)
    changed = bytearray(data)
    changed[result["entry_file_offset"]] ^= 1
    try:
        macho_signature(changed)
    except ValueError as error:
        require(str(error) == "signed digest mismatch", "tamper hit another validation rule")
    else:
        raise ValueError("modified entry passed signature verification")
    return dict(result, modified_entry_fails_signature=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path, help="four published target directories from the official compiler")
    parser.add_argument("candidate", type=Path, help="four published target directories from the migration compiler")
    parser.add_argument("--run", action="store_true", help="run the ELF and PE images from each compiler")
    parser.add_argument("--qemu", default="qemu-aarch64", help="AArch64 Linux user-mode runner")
    parser.add_argument("--wine", default="wine64", help="x86-64 Wine runner or wrapper")
    args = parser.parse_args()
    record = json.loads(RECORD.read_text())
    for target, expected in record["targets"].items():
        filename = "omega-program.exe" if target == "windows_x86_64" else "omega-program"
        for lane, root in (("baseline", args.baseline), ("candidate", args.candidate)):
            path = (root / target / filename).resolve()
            image = path.read_bytes()
            require(len(image) == expected["image_bytes"] and sha256(image) == expected["image_sha256"],
                    f"{lane} {target}: executable bytes differ from the checkpoint")
            if target == "macos_arm64":
                require(check_macho(image) == expected["macho"], f"{lane}: Mach-O inspection differs")
                observation = "Mach-O entry, signature and tamper check verified"
            elif args.run:
                prefix = {"linux_x86_64": [], "linux_arm64": [args.qemu], "windows_x86_64": [args.wine]}[target]
                result = subprocess.run([*prefix, str(path)], env=dict(os.environ, WINEDEBUG="-all"))
                require(result.returncode == expected["native_exit"][lane], f"{lane} {target}: exit {result.returncode}")
                observation = f"exit {result.returncode}"
            else:
                observation = "matching image bytes"
            print(lane, target, observation)


if __name__ == "__main__":
    main()
