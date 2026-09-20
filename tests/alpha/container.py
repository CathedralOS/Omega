#!/usr/bin/env python3
"""Native-container structure validation for the audited Alpha seeds.

The Alpha seeds are native executables carrying a 16 MiB stamping hole:
`alpha_x64_windows.exe` is a PE32+ x86-64 image and `alpha_arm64_macos` is an
arm64 Mach-O. `tools/bootstrap/alpha/seed_env.sh` stamps a raw tape by writing
[4-byte LE length][tape] at a recorded file offset. This check binds those
recorded offsets to the container's actual native structure and validates the
contract the runtime loader depends on:

- the container parses as its documented native format;
- the entry point lands inside executable code;
- the loader imports the host calls the runtime uses (kernel32 I/O +
  VirtualAlloc on Windows; libSystem linkage plus a code signature blob on
  arm64 macOS, where execution of an unsigned image is refused);
- a tape section exists whose raw extent is exactly the profile's hole —
  `HOLE_OFF`/`ALPHA_SEED_HOLE_SIZE` — so stamping cannot land outside it;
- a pristine container's hole is all zero; a stamped container's hole is
  exactly [length][tape] followed by zeros, and a `--pristine` comparison
  proves every byte outside the hole is unchanged.

Inspection is host-free: this script needs only Python 3 and runs on hosts
where neither container can execute. Execution validation stays on the native
hosts (`tests/alpha/conformance.sh`); the SHA-256 identity binding stays in
`seed_env.sh` (`tests/bootstrap/alpha-identity.sh`).

Usage:
    container.py CONTAINER --format {pe,macho} --hole-off OFF \
        --hole-size BYTES --max-tape BYTES [--stamped] [--tape FILE] \
        [--pristine FILE]
"""

import argparse
import struct
import sys

PE32_MAGIC = 0x20B
IMAGE_FILE_MACHINE_AMD64 = 0x8664
IMAGE_FILE_EXECUTABLE_IMAGE = 0x0002
IMAGE_FILE_LARGE_ADDRESS_AWARE = 0x0020
IMAGE_FILE_DLL = 0x2000
IMAGE_SCN_MEM_EXECUTE = 0x20000000
IMAGE_DIRECTORY_ENTRY_IMPORT = 1
REQUIRED_KERNEL32_IMPORTS = {
    b"getstdhandle": b"GetStdHandle",
    b"readfile": b"ReadFile",
    b"writefile": b"WriteFile",
    b"virtualalloc": b"VirtualAlloc",
}

MACHO_MAGIC_64 = 0xFEEDFACF
CPU_TYPE_ARM64 = 0x0100000C
MH_EXECUTE = 0x2
LC_SEGMENT_64 = 0x19
LC_LOAD_DYLIB = 0x0C
LC_CODE_SIGNATURE = 0x1D
LC_REEXPORT_DYLIB = 0x1F
LC_LOAD_WEAK_DYLIB = 0x80000018
LC_LOAD_UPWARD_DYLIB = 0x80000023
LC_MAIN = 0x80000028
SECTION_TYPE_ZEROFILL = 0x01
S_ATTR_SOME_INSTRUCTIONS = 0x00000400
S_ATTR_PURE_INSTRUCTIONS = 0x80000000
CS_SUPERBLOB_MAGIC = 0xFADE0CC0


class ContainerError(Exception):
    """A container failed a structural check."""


class Reader:
    """Bounds-checked little-endian cursor over the container bytes."""

    def __init__(self, data):
        self.data = data
        self.size = len(data)

    def slice(self, offset, length, what):
        if offset < 0 or length < 0 or offset + length > self.size:
            raise ContainerError(
                f"{what} lies outside the container (offset {offset:#x}, "
                f"size {length:#x}, file {self.size:#x})")
        return self.data[offset:offset + length]

    def u16(self, offset, what):
        return struct.unpack_from("<H", self.slice(offset, 2, what))[0]

    def u32(self, offset, what):
        return struct.unpack_from("<I", self.slice(offset, 4, what))[0]

    def u64(self, offset, what):
        return struct.unpack_from("<Q", self.slice(offset, 8, what))[0]

    def cstring(self, offset, what):
        end = self.data.find(b"\x00", offset)
        if end < 0:
            raise ContainerError(f"{what}: unterminated string at {offset:#x}")
        return self.data[offset:end]

    def expect(self, condition, message):
        if not condition:
            raise ContainerError(message)


class Section:
    def __init__(self, name, address, span, raw_offset, raw_size, flags):
        self.name = name
        self.address = address
        self.span = span
        self.raw_offset = raw_offset
        self.raw_size = raw_size
        self.flags = flags


def parse_pe(reader):
    reader.expect(reader.u16(0, "DOS header") == 0x5A4D,
                  "PE32+ container lacks the MZ signature")
    pe = reader.u32(0x3C, "DOS header")
    reader.expect(pe >= 0x40, "PE header offset collides with the DOS stub")
    reader.expect(reader.slice(pe, 4, "PE signature") == b"PE\x00\x00",
                  "PE32+ container lacks the PE signature")
    coff = pe + 4
    machine = reader.u16(coff, "COFF header")
    reader.expect(machine == IMAGE_FILE_MACHINE_AMD64,
                  f"PE32+ machine is {machine:#06x}, not x86-64")
    section_count = reader.u16(coff + 2, "COFF header")
    optional_size = reader.u16(coff + 16, "COFF header")
    characteristics = reader.u16(coff + 18, "COFF header")
    reader.expect(characteristics & IMAGE_FILE_EXECUTABLE_IMAGE,
                  "PE32+ container is not marked executable")
    reader.expect(not characteristics & IMAGE_FILE_DLL,
                  "PE32+ container is a DLL, not an executable image")
    reader.expect(characteristics & IMAGE_FILE_LARGE_ADDRESS_AWARE,
                  "PE32+ container lacks large-address-awareness")
    optional = coff + 20
    reader.expect(optional_size >= 0xF0,
                  f"PE32+ optional header is {optional_size:#x} bytes, "
                  "below the PE32+ floor")
    reader.expect(reader.u16(optional, "optional header") == PE32_MAGIC,
                  "PE32+ optional header magic mismatch")
    entry_rva = reader.u32(optional + 16, "optional header")
    section_alignment = reader.u32(optional + 32, "optional header")
    file_alignment = reader.u32(optional + 36, "optional header")
    size_of_image = reader.u32(optional + 56, "optional header")
    size_of_headers = reader.u32(optional + 60, "optional header")
    directory_count = reader.u32(optional + 108, "optional header")
    reader.expect(section_alignment >= file_alignment,
                  "PE32+ section alignment is below file alignment")
    reader.expect(directory_count > IMAGE_DIRECTORY_ENTRY_IMPORT,
                  "PE32+ container declares no import directory")
    table = optional + optional_size
    reader.slice(table, 40 * section_count, "section table")
    sections = []
    top_of_image = 0
    for index in range(section_count):
        base = table + 40 * index
        raw_name = reader.slice(base, 8, "section header")
        name = raw_name.split(b"\x00", 1)[0].decode("ascii", "replace")
        virtual_span = reader.u32(base + 8, name)
        virtual_address = reader.u32(base + 12, name)
        raw_size = reader.u32(base + 16, name)
        raw_offset = reader.u32(base + 20, name)
        flags = reader.u32(base + 36, name)
        sections.append(Section(name, virtual_address, virtual_span,
                                raw_offset, raw_size, flags))
        reader.expect(virtual_address % section_alignment == 0,
                      f"PE32+ section {name} is misaligned")
        reader.expect(raw_offset + raw_size <= reader.size,
                        f"PE32+ section {name} raw data exceeds the file")
        top_of_image = max(top_of_image,
                           virtual_address + max(virtual_span, raw_size))
    reader.expect(top_of_image <= size_of_image,
                  "PE32+ sections exceed the declared image size")
    reader.expect(size_of_headers <= reader.size,
                  "PE32+ headers exceed the file")
    executable = [section for section in sections
                  if section.flags & IMAGE_SCN_MEM_EXECUTE]
    reader.expect(any(section.address <= entry_rva < section.address + max(
                          section.span, section.raw_size)
                      for section in executable),
                  "PE32+ entry point lies outside executable code")

    def rva_to_offset(rva):
        for section in sections:
            # File content exists only across the raw extent; an RVA in the
            # virtual tail of a section (or in a bss-only section) would alias
            # the following section's bytes, so it must refuse rather than
            # read through.
            if section.address <= rva < section.address + section.raw_size:
                return section.raw_offset + (rva - section.address)
        raise ContainerError(f"PE32+ RVA {rva:#x} lands in no section")

    imports = {}
    import_rva = reader.u32(optional + 112 + 8 * IMAGE_DIRECTORY_ENTRY_IMPORT,
                            "import directory")
    if import_rva:
        descriptor = rva_to_offset(import_rva)
        while True:
            fields = reader.slice(descriptor, 20, "import descriptor")
            lookup_rva, _, _, name_rva, thunk_rva = struct.unpack("<5I", fields)
            if lookup_rva == 0 and name_rva == 0:
                break
            dll = reader.cstring(rva_to_offset(name_rva),
                                 "imported DLL name")
            names = set()
            table_rva = lookup_rva or thunk_rva
            cursor = rva_to_offset(table_rva)
            while True:
                entry = reader.u64(cursor, "import lookup entry")
                cursor += 8
                if entry == 0:
                    break
                if entry & (1 << 63):
                    names.add(b"<ordinal>")
                    continue
                hint_name = rva_to_offset(entry & 0x7FFFFFFF)
                names.add(reader.cstring(hint_name + 2, "imported symbol"))
            imports.setdefault(dll, set()).update(names)
            descriptor += 20
    kernel32 = next((names for dll, names in imports.items()
                     if dll.lower() == b"kernel32.dll"), set())
    missing = [name.decode() for marker, name in REQUIRED_KERNEL32_IMPORTS.items()
               if not any(name.lower() == marker for name in kernel32)]
    reader.expect(not missing,
                  "PE32+ container lacks kernel32 imports: "
                  + ", ".join(missing))
    return sections


def parse_macho(reader):
    reader.expect(reader.u32(0, "Mach-O header") == MACHO_MAGIC_64,
                  "Mach-O container lacks the 64-bit magic")
    cputype = reader.u32(4, "Mach-O header")
    reader.expect(cputype == CPU_TYPE_ARM64,
                  f"Mach-O cputype is {cputype:#x}, not arm64")
    reader.expect(reader.u32(12, "Mach-O header") == MH_EXECUTE,
                  "Mach-O container is not an executable")
    command_count = reader.u32(16, "Mach-O header")
    commands_size = reader.u32(20, "Mach-O header")
    reader.expect(32 + commands_size <= reader.size,
                  "Mach-O load commands exceed the file")
    cursor = 32
    commands_end = cursor + commands_size
    sections = []
    entry_offset = None
    signature = None
    dylib_names = []
    for _ in range(command_count):
        reader.expect(cursor + 8 <= commands_end,
                      "Mach-O load commands overran their declared size")
        command = reader.u32(cursor, "load command")
        command_size = reader.u32(cursor + 4, "load command")
        reader.expect(command_size >= 8 and cursor + command_size <= commands_end,
                      f"Mach-O load command {command:#x} has invalid size")
        if command == LC_SEGMENT_64:
            segname = reader.slice(cursor + 8, 16, "segment name") \
                .split(b"\x00", 1)[0].decode("ascii", "replace")
            file_offset = reader.u64(cursor + 40, segname)
            file_size = reader.u64(cursor + 48, segname)
            section_count = reader.u32(cursor + 64, segname)
            reader.expect(file_offset + file_size <= reader.size,
                          f"Mach-O segment {segname} exceeds the file")
            section_base = cursor + 72
            reader.expect(section_base + 80 * section_count <= commands_end,
                          f"Mach-O segment {segname} sections overrun commands")
            for index in range(section_count):
                base = section_base + 80 * index
                raw_name = reader.slice(base, 16, "section name")
                name = raw_name.split(b"\x00", 1)[0].decode("ascii", "replace")
                address = reader.u64(base + 32, name)
                span = reader.u64(base + 40, name)
                raw_offset = reader.u32(base + 48, name)
                flags = reader.u32(base + 64, name)
                raw_size = 0 if flags & 0xFF == SECTION_TYPE_ZEROFILL else span
                sections.append(Section(name, address, span, raw_offset,
                                        raw_size, flags))
        elif command == LC_MAIN:
            entry_offset = reader.u64(cursor + 8, "LC_MAIN")
        elif command == LC_CODE_SIGNATURE:
            signature = (reader.u32(cursor + 8, "LC_CODE_SIGNATURE"),
                         reader.u32(cursor + 12, "LC_CODE_SIGNATURE"))
        elif command in (LC_LOAD_DYLIB, LC_LOAD_WEAK_DYLIB,
                         LC_REEXPORT_DYLIB, LC_LOAD_UPWARD_DYLIB):
            name_offset = reader.u32(cursor + 8, "dylib command")
            dylib_names.append(
                reader.cstring(cursor + name_offset, "dylib name"))
        cursor += command_size
    reader.expect(cursor <= commands_end,
                  "Mach-O load commands overran their declared extent")
    executable = [section for section in sections
                  if section.flags & (S_ATTR_PURE_INSTRUCTIONS
                                      | S_ATTR_SOME_INSTRUCTIONS)]
    reader.expect(executable, "Mach-O container has no instruction sections")
    reader.expect(entry_offset is not None,
                  "Mach-O container lacks LC_MAIN")
    reader.expect(any(section.raw_offset <= entry_offset
                      < section.raw_offset + section.raw_size
                      for section in executable),
                  "Mach-O entry point lies outside executable code")
    reader.expect(signature is not None,
                  "Mach-O container lacks a code signature (arm64 refuses "
                  "to execute unsigned images)")
    signature_offset, signature_size = signature
    reader.expect(signature_offset + signature_size <= reader.size,
                  "Mach-O code signature exceeds the file")
    superblob = struct.unpack_from(
        ">I", reader.slice(signature_offset, 4, "code signature"))[0]
    reader.expect(superblob == CS_SUPERBLOB_MAGIC,
                  "Mach-O code signature lacks the superblob magic")
    reader.expect(any(b"libSystem" in name for name in dylib_names),
                  "Mach-O container does not link libSystem")
    reader.expect(any(section.flags & 0xFF in (SECTION_TYPE_ZEROFILL, 0x0C)
                      for section in sections),
                  "Mach-O container lacks the zerofill register/bss area")
    return sections


def check_hole(reader, sections, hole_off, hole_size, args):
    tape = next((section for section in sections
                 if section.name in (".tape", "__tape")), None)
    reader.expect(tape is not None,
                  "container has no tape section (.tape/__tape)")
    reader.expect(tape.raw_offset == hole_off and tape.raw_size == hole_size,
                  f"recorded hole ({hole_off:#x}+{hole_size:#x}) is not the "
                  f"tape section's raw extent ({tape.raw_offset:#x}"
                  f"+{tape.raw_size:#x}); stamping would write outside it")
    hole = reader.slice(hole_off, hole_size, "stamping hole")
    if not args.stamped:
        reader.expect(not any(hole),
                      "pristine container's hole is not all zero")
        return "pristine hole is all-zero"
    length = struct.unpack_from("<I", hole[:4])[0]
    reader.expect(length <= args.max_tape,
                  f"stamped length {length} exceeds the {args.max_tape}-byte "
                  "raw tape maximum")
    tail = hole[4 + length:]
    reader.expect(not any(tail),
                  "stamped container's hole has bytes beyond [length][tape]")
    if args.tape is not None:
        with open(args.tape, "rb") as stream:
            tape_bytes = stream.read()
        reader.expect(length == len(tape_bytes)
                      and hole[4:4 + length] == tape_bytes,
                      "stamped hole payload differs from the tape")
    if args.pristine is not None:
        with open(args.pristine, "rb") as stream:
            pristine = stream.read()
        reader.expect(len(pristine) == reader.size,
                      "stamped container size differs from the pristine seed")
        reader.expect(pristine[:hole_off] == reader.data[:hole_off]
                      and pristine[hole_off + hole_size:]
                      == reader.data[hole_off + hole_size:],
                      "stamping changed bytes outside the hole")
    return f"stamped hole carries [length {length}][tape], tail zeroed"


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("container")
    parser.add_argument("--format", choices=("pe", "macho"), required=True)
    parser.add_argument("--hole-off", type=int, required=True)
    parser.add_argument("--hole-size", type=int, required=True)
    parser.add_argument("--max-tape", type=int, required=True)
    parser.add_argument("--stamped", action="store_true",
                        help="the hole holds [length][tape], not zeros")
    parser.add_argument("--tape", help="expected stamped tape payload")
    parser.add_argument("--pristine",
                        help="unstamped container to compare non-hole bytes")
    args = parser.parse_args(argv)
    with open(args.container, "rb") as stream:
        data = stream.read()
    reader = Reader(data)
    try:
        if args.format == "pe":
            sections = parse_pe(reader)
        else:
            sections = parse_macho(reader)
        summary = check_hole(reader, sections, args.hole_off,
                             args.hole_size, args)
    except ContainerError as error:
        print(f"container FAIL {args.container}: {error}", file=sys.stderr)
        return 1
    print(f"container ✓ {args.container}: valid {args.format}; {summary}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
