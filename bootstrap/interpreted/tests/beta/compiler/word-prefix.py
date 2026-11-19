"""Literal prefix controls for the shared Beta hexadecimal-word parser."""

import subprocess
import sys


def main():
    compiler = sys.argv[1]
    observations = 0
    # Every printable initial token byte except separators/comment introducer.
    # These are authored input families, not a second lexer or assembly relation.
    for initial in range(33, 127):
        if initial in (ord(","), ord(";")):
            continue
        prefix = bytes([initial])
        valid = prefix == b"0"
        cases = (
            ("assertion", prefix + b"x0:", b"", b""),
            ("late-assertion", b"halt r0\n" + prefix + b"x2:", b"\0\0", b"\0\0"),
            ("control-word", b"jmp " + prefix + b"x0", b"\x0c" + bytes(8), b"\x0c"),
            ("data-word", b"dw " + prefix + b"x0", bytes(8), b""),
        )
        for name, source, success, rejected_prefix in cases:
            expected = (0, success, b"") if valid else (7, rejected_prefix, b"")
            for ending in (b"", b"\n"):
                result = subprocess.run(
                    [compiler], input=source + ending, capture_output=True, timeout=30,
                )
                actual = result.returncode, result.stdout, result.stderr
                if actual != expected:
                    raise SystemExit(
                        f"Beta word prefix {name} {source + ending!r}: "
                        f"expected {expected!r}, got {actual!r}"
                    )
                observations += 1
    print(f"Beta word prefix: {observations} exact status/stdout/stderr controls passed")


if __name__ == "__main__":
    main()
