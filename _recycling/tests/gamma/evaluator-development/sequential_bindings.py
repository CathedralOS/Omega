"""Authored grouped-let controls and equivalent nested-let observations."""

import os
import signal
import struct
import subprocess
import sys


def fixtures():
    cases = []

    def equivalent(name, bindings, body, expected, declarations=b""):
        nested = body
        for binder, initializer in reversed(bindings):
            nested = b"(let " + binder + b" Int " + initializer + b" " + nested + b")"
        grouped = b"(let (" + b" ".join(
            b"(" + binder + b" Int " + initializer + b")"
            for binder, initializer in bindings
        ) + b") " + body + b")"
        prefix = declarations + b"(def main () Int "
        cases.append((name, prefix + grouped + b")", prefix + nested + b")", expected))

    equivalent("empty group", [], b"7", (0, b"\x07"))
    equivalent("sequential visibility", [(b"first", b"7"), (b"second", b"(+ first 2)")],
               b"second", (0, b"\x09"))
    equivalent("ordered effects", [(b"first", b"(write 65)"), (b"second", b"(write (+ first 1))")],
               b"0", (0, b"AB\x00"))
    equivalent("pair provenance", [(b"value", b"(pair 9 7)"), (b"other", b"value")],
               b"(first other)", (0, b"\x09"))
    equivalent("pair result", [(b"value", b"(pair 0 1)")], b"value", (0, b""),
               b"(def $application () Int 1)\n")
    equivalent("initializer scope restored", [
        (b"first", b"(let ((private Int 8)) private)"),
        (b"private", b"(+ first 1)")], b"private", (0, b"\x09"))
    equivalent("trap discards prefix", [(b"prefix", b"(write 65)"), (b"failure", b"(/ 1 0)")],
               b"0", (2, b""))
    equivalent("application trap", [(b"prefix", b"(write 65)"), (b"failure", b"(/ 1 0)")],
               b"(pair 0 1)", (249, b""), b"(def $application () Int 1)\n")
    equivalent("pair still forbidden in arithmetic", [(b"value", b"(pair 0 1)")],
               b"(+ value 0)", (2, b""))
    equivalent("duplicate group binding", [(b"value", b"7"), (b"value", b"8")],
               b"value", (1, b""))
    equivalent("self reference", [(b"value", b"value")], b"value", (1, b""))
    equivalent("forward reference", [(b"first", b"second"), (b"second", b"7")],
               b"first", (1, b""))
    cases.extend([
        ("non-tail group restores environment",
         b"(def main () Int (+ (let ((value Int 7)) value) (let ((value Int 2)) value)))",
         b"(def main () Int (+ (let value Int 7 value) (let value Int 2 value)))", (0, b"\x09")),
        ("non-tail pair result keeps provenance",
         b"(def main () Int (first (let ((value Int (pair 9 7))) value)))",
         b"(def main () Int (first (let value Int (pair 9 7) value)))", (0, b"\x09")),
        ("unselected branch", b"(def main () Int (if 0 (let ((value Int (/ 1 0))) value) 9))",
         b"(def main () Int (if 0 (let value Int (/ 1 0) value) 9))", (0, b"\x09")),
        ("tail recursion",
         b"(def loop ((count Int)) Int (if count (let ((next Int (- count 1))) (loop next)) 7))"
         b"(def main () Int (loop 100000))",
         b"(def loop ((count Int)) Int (if count (let next Int (- count 1) (loop next)) 7))"
         b"(def main () Int (loop 100000))", (0, b"\x07")),
    ])
    for name, body in (
        ("missing result", b"(let ((value Int 7)))"),
        ("extra result", b"(let ((value Int 7)) value 8)"),
        ("missing initializer", b"(let ((value Int)) value)"),
        ("extra initializer", b"(let ((value Int 7 8)) value)"),
        ("wrong annotation", b"(let ((value Other 7)) value)"),
        ("malformed binding", b"(let (value) 0)"),
        ("escaped local", b"(+ (let ((value Int 7)) value) value)"),
        ("active shadowing", b"(let value Int 7 (let ((value Int 8)) value))"),
    ):
        cases.append((name, b"(def main () Int " + body + b")", None, (1, b"")))
    cases.append(("unreachable invalid group",
                  b"(def bad () Int (let ((value Int missing)) value))(def main () Int 7)",
                  None, (1, b"")))
    for depth, expected in ((255, (0, b"\x07")), (256, (3, b""))):
        body = b"(+ 0 " * (depth - 3) + b"7" + b")" * (depth - 3)
        cases.append((f"grouped initializer syntax depth {depth}",
                      b"(def main () Int (let ((value Int " + body + b")) value))", None, expected))
    from environment_rows import fixtures as environment_fixtures
    for name, source, expected in environment_fixtures():
        if b"(let extra Int 0 " in source:
            source = source.replace(b"(let extra Int 0 ", b"(let ((extra Int 0)) ")
            cases.append(("grouped " + name, source, None, expected))
    return cases


def observe(evaluator, source):
    process = subprocess.Popen([evaluator], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        output, error = process.communicate(struct.pack("<I", len(source)) + source, timeout=20)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        raise SystemExit("grouped let: evaluator exceeded 20 seconds; process group killed")
    if error:
        raise SystemExit(f"grouped let: unexpected stderr {error!r}")
    return process.returncode, output


def main():
    observations = 0
    for name, source, nested, expected in fixtures():
        actual = observe(sys.argv[1], source)
        if actual != expected:
            raise SystemExit(f"{name}: expected {expected!r}, observed {actual!r}")
        observations += 1
        if nested is not None:
            actual = observe(sys.argv[1], nested)
            if actual != expected:
                raise SystemExit(f"{name}, nested form: expected {expected!r}, observed {actual!r}")
            observations += 1
    print(f"Grouped let: {len(fixtures())} controls, {observations} exact observations")


if __name__ == "__main__":
    main()