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

    # The installed core is identified by its role, independent of its package
    # display name. Exercise the product declaration without a dependency edge.
    ordering = (ROOT / "source/library/core/fence_ordering.omg").read_bytes()
    order_reader = (
        b"use omega\\language\\core\\FenceOrdering; machine main() -> u8 { "
        b"let order: FenceOrdering = FenceOrdering::ReceivePublish; "
        b"transition order { FenceOrdering::ReceivePublish -> pass(), _ -> fail() } "
        b"machine pass() -> u8 { 42 } machine fail() -> u8 { 7 } }"
    )
    data, _, _, _ = encode([("app", 0), ("installed", 1)], [],
                           [(0, order_reader), (1, ordering)], 0, b"main",
                           paths=["main.omg", "fence_ordering.omg"])
    yield "core-product-declaration", data, 42

    core_import = b"use omega\\language\\core\\sub\\score; machine main() -> u8 { score() }"
    core_call = b"machine main() -> u8 { omega\\language\\core\\sub\\score() }"
    packages = [("app", 0), ("installed", 1), ("impostor", 0)]
    paths = ["main.omg", "sub/x.omg", "language/core/sub/x.omg"]
    for name, caller in [("core-import", core_import), ("core-qualified-call", core_call)]:
        data, _, _, _ = encode(packages, [(0, 2, "omega")], [(0, caller), (1, score), (2, other)],
                               0, b"main", paths=paths)
        yield name, data, 42

    local_alias = b"use alternate\\omega; " + core_call
    data, _, _, _ = encode([("app", 0), ("installed", 1)], [],
                           [(0, local_alias), (1, score), (0, other), (0, b"machine anchor() {}")],
                           0, b"main", paths=["main.omg", "sub/x.omg",
                                              "alternate/omega/language/core/sub/x.omg",
                                              "alternate/omega/marker.omg"])
    yield "core-before-local-namespace", data, 42

    # A normal dependency alias starting with "omega" still selects ordinary std.
    ordinary = b"use omega\\sub\\score; machine main() -> u8 { score() }"
    data, _, _, _ = encode([("app", 0), ("library", 2)], [(0, 1, "omega")],
                           [(0, ordinary), (1, score)], 0, b"main",
                           paths=["main.omg", "sub/x.omg"])
    yield "ordinary-omega-alias", data, 42

    for name, role, path, caller, declaration, owner, edges in [
        ("private-core-member", 1, "sub/x.omg", core_import, b"machine score() -> u8 { 42 }", 1, []),
        ("core-name-without-role", 0, "sub/x.omg", core_import, score, 1, []),
        ("local-core-impostor", 0, "omega/language/core/sub/x.omg", core_import, score, 0, []),
        ("aliased-core-impostor", 0, "language/core/sub/x.omg", core_import, score, 1, [(0, 1, "omega")]),
        ("ordinary-std-unmounted", 2, "sub/x.omg", core_import.replace(b"core", b"std"), score, 1, []),
    ]:
        data, _, _, rows = encode([("app", 0), ("omega_language_core", role)], edges,
                                  [(0, caller), (owner, declaration)], 0, b"main",
                                  paths=["main.omg", path])
        yield name, data, struct.pack("<BII", 0, 1, rows[0][1])

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
