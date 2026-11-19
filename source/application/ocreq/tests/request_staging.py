#!/usr/bin/env python3
"""Check the OCREQ staging boundary of a built Omega request application."""
from pathlib import Path
import argparse
import struct
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tests/bootstrap"))
from conformance.vectors import ocreq_request


def check(program: Path, request: bytes, expected: bytes) -> None:
    result = subprocess.run([program], input=request, capture_output=True)
    assert result.returncode == expected[8], (len(request), result.returncode)
    assert result.stdout == expected, (len(request), result.stdout)


def main() -> None:
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("program", type=Path)
    program = args.parse_args().program.resolve()
    name = b"main.omg"
    capacity = 65536
    size = capacity - len(ocreq_request(b"", name))
    filled = ocreq_request(b"\xff" * size, name)
    assert len(filled) == capacity

    # Both frames are Incomplete. The well-formed request reaches the
    # unimplemented semantics phase (25). One byte more exhausts staging (26).
    header = b"\xffOCOUT\x01\x00"
    check(program, filled, header + struct.pack("<BBHIQQQ", 2, 0, 0, 25, 0, 0, 0))
    check(program, filled + b"\xff",
          header + struct.pack("<BBHIQQQ", 2, 1, 0, 26, capacity, capacity, capacity + 1))


if __name__ == "__main__":
    main()
