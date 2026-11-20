"""Check package-relative namespaces using an executable Epsilon-written Omega constructor."""
import argparse
from pathlib import Path
import runpy
import struct
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compiled import ROOT, execute, stamp


def cases():
    encode = runpy.run_path(str(ROOT / "source/application/construction/construct.py"))["encode"]
    select = b"use first\\score; machine main() -> u8 { score() }"
    sibling = b"machine main() -> u8 { score() }"
    constant = b"machine main() -> u8 { 42 }"
    score = b"public machine score() -> u8 { 42 }"
    other = b"public machine score() -> u8 { 7 }"
    cases_source = (ROOT / "tests/bootstrap/conformance/omega/payload_bindings.omg").read_bytes()

    def packet(sources, paths=None):
        data, _, _, rows = encode([("app", 0)], [], [(0, src) for src in sources],
                                  0, b"main", paths=paths)
        return data, rows

    for name, sources, paths in [
        ("directory-import", [select, score, other], ["main.omg", "first/x.omg", "second/x.omg"]),
        ("renamed-and-reordered", [select, other, score], ["new.omg", "second/a.omg", "first/b.omg"]),
        ("same-directory", [sibling, other, score], ["first/m.omg", "second/s.omg", "first/s.omg"]),
        ("qualified-call", [b"machine main() -> u8 { first\\score() }", score],
         ["main.omg", "first/x.omg"]),
        ("nested-directory", [select.replace(b"first", b"outer\\first"), score],
         ["main.omg", "outer/first/x.omg"]),
        ("static-case", [cases_source], ["example/cases.omg"]),
        ("legacy-root", [sibling, score], None),
    ]:
        yield name, packet(sources, paths)[0], 42

    data, _, _, _ = encode([("app", 0), ("dep", 0)], [(0, 1, "library")],
                           [(0, select.replace(b"first", b"library\\first")), (1, score), (1, other)],
                           0, b"main", paths=["main.omg", "first/x.omg", "second/x.omg"])
    yield "dependency-directory", data, 42

    data, rows = packet([select, score], ["main.omg", "moved/x.omg"])
    yield "moved-declaration", data, struct.pack("<BII", 0, 1, rows[0][1])
    # The bootstrap compiler implements imports of free declarations. A static
    # member selector must not select a same-spelled free machine in a directory.
    data, rows = packet([select.replace(b"first\\score", b"first::score"), score],
                        ["main.omg", "first/x.omg"])
    yield "static-import-outside-subset", data, struct.pack("<BII", 0, 2, rows[0][1] + 4)
    data, rows = packet([b"module first; " + constant], ["m.omg"])
    yield "retired-header", data, struct.pack("<BII", 0, 1, rows[0][1])
    for name, path, offset in [
        ("bad-directory", "ok/2wrong/x.omg", 3),
        ("reserved-directory", "u3072/x.omg", 0),
        ("empty-component", "ok//x.omg", 3),
        ("missing-basename", "ok/", 3),
        ("parent-basename", "ok/..", 3),
        ("nul-in-name", "ok/a\0b", 4),
        ("backslash-in-path", "ok\\x.omg", 2),
    ]:
        data, rows = packet([constant], [path])
        yield name, data, struct.pack("<BII", 0, 1, rows[0][3] + offset)
    data, _ = packet([constant], ["a" * 256 + "/x.omg"])
    yield "last-directory-byte", data, 42
    data, _ = packet([constant], ["a" * 257 + "/x.omg"])
    yield "directory-too-long", data, struct.pack("<BI", 0, 3)
    data, _ = packet([constant], ["a" * 300 + ".omg"])
    yield "long-basename", data, 42
    yield "truncated-frame", data[:-1], struct.pack("<BI", 0, 3)
    yield "unknown-version", bytes(4) + struct.pack("<I", 2), struct.pack("<BI", 0, 3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("constructor", type=Path)
    parser.add_argument("--case", action="append", choices=[name for name, _, _ in cases()])
    parser.add_argument("--seconds", type=int, default=900)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as directory:
        work = Path(directory)
        for name, request, expected in cases():
            if args.case and name not in args.case:
                continue
            status, output = execute(args.constructor.resolve(), request, args.seconds)
            if status:
                raise SystemExit(f"{name}: constructor exited {status}")
            if isinstance(expected, int):
                if output[:5] != bytes(5) or len(output) <= 5:
                    raise SystemExit(f"{name}: expected Alpha tape, got {output[:40].hex()}")
                tape, program = work / (name + ".tape"), work / name
                tape.write_bytes(output[5:])
                stamp(tape, program)
                if execute(program, timeout=args.seconds) != (expected, b""):
                    raise SystemExit(f"{name}: Alpha program did not exit {expected} with empty output")
            elif output != expected:
                raise SystemExit(f"{name}: got {output.hex()}, expected {expected.hex()}")
            print(f"{name} passed", flush=True)


if __name__ == "__main__":
    main()
