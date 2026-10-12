#!/usr/bin/env python3
"""Compare a retained executable against the pre-execution frozen oracle.

No Omega compile, package acceptance or oracle regeneration happens here.
Labels and integer fields must match exactly; each printed fixed-decimal
field tolerates the frozen +/-1 unit in the last digit (data/oracle.json
`print_tolerance_units`) and the precision ratio tolerates +/-1. The
--both mode runs `omega run --both main.omg` and admits exactly the CLI's
`native exit N` / `interp exit: N` stderr as the engine-agreement marker;
anything else on stderr fails the case.
"""
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FIXED = re.compile(r"[+-]\d{8}\.\d{6}")
NUMBER = re.compile(r"\d+")
FIELD = re.compile(r"(dep|lat|adj_dep|adj_lat|length|e_min|e_max|n_min|n_max|e|n)=([+-]\d{8}\.\d{6})")
RATIO = re.compile(r"precision=1:(\d+)")


def require(condition, message):
    if not condition:
        raise SystemExit("validation integrity failure: " + message)


def compare_line(expected, actual, tol_units, ratio_tol):
    if expected == actual:
        return True
    # Split into alternating literal and numeric spans; literals must be
    # byte-identical, numbers must sit inside the frozen tolerances.
    spans_e = list(FIELD.finditer(expected))
    spans_a = list(FIELD.finditer(actual))
    if [m.group(1) for m in spans_e] != [m.group(1) for m in spans_a]:
        return False
    pos_e = pos_a = 0
    for me, ma in zip(spans_e, spans_a):
        if expected[pos_e:me.start()] != actual[pos_a:ma.start()]:
            return False
        def units(text):
            magnitude = int(text[1:].replace(".", ""))
            return -magnitude if text[0] == "-" else magnitude
        if abs(units(me.group(2)) - units(ma.group(2))) > tol_units:
            return False
        pos_e, pos_a = me.end(), ma.end()
    tail_e, tail_a = expected[pos_e:], actual[pos_a:]
    re_, ra_ = RATIO.search(tail_e), RATIO.search(tail_a)
    if (re_ is None) != (ra_ is None):
        return False
    if re_:
        if tail_e[:re_.start()] != tail_a[:ra_.start()] or tail_e[re_.end():] != tail_a[ra_.end():]:
            return False
        if abs(int(re_.group(1)) - int(ra_.group(1))) > ratio_tol:
            return False
        return True
    return tail_e == tail_a


def main():
    manifest_bytes = (ROOT / "data/oracle.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    require(manifest["print_tolerance_units"] == 1, "unexpected print tolerance")
    require(set(manifest["cases"]) == {p.stem for p in (ROOT / "fixtures").glob("*.input")},
            "fixture inventory differs from frozen cases")
    require(len(sys.argv) in (2, 3) and (len(sys.argv) == 2 or sys.argv[2] == "--both"),
            "usage: validate.py EXECUTABLE [--both]")
    binary = Path(sys.argv[1]).resolve()
    comparison = sys.argv[2:] == ["--both"]
    command = [str(binary), "run", "--both", str(ROOT / "main.omg")] if comparison else [str(binary)]
    results = []
    for name, case in sorted(manifest["cases"].items()):
        data = (ROOT / "fixtures" / f"{name}.input").read_bytes()
        expected = (ROOT / "fixtures" / f"{name}.stdout").read_bytes()
        require(hashlib.sha256(data).hexdigest() == case["input_sha256"], name + " input changed")
        require(hashlib.sha256(expected).hexdigest() == case["stdout_sha256"], name + " expected stdout changed")
        result = subprocess.run(command, input=data, capture_output=True, timeout=120)
        expected_stderr = (f"native exit {case['exit']}\ninterp exit: {case['exit']}\n").encode()
        agrees = result.stderr == expected_stderr if comparison else not result.stderr
        expected_lines = expected.decode("ascii").splitlines()
        actual_lines = result.stdout.decode("ascii", errors="replace").splitlines()
        exact = result.stdout == expected
        close = len(expected_lines) == len(actual_lines) and all(
            compare_line(e, a, manifest["print_tolerance_units"], manifest["precision_num_tolerance"])
            for e, a in zip(expected_lines, actual_lines))
        good = result.returncode == case["exit"] and close and agrees
        results.append({"case": name, "exit": result.returncode, "expected_exit": case["exit"],
                        "exact_stdout": exact, "tolerant_match": close, "agrees": agrees,
                        "stderr": result.stderr.decode(errors="replace"), "pass": good})
        out = ROOT / ("build/terminal-validation" if comparison else "build/validation")
        out.mkdir(parents=True, exist_ok=True)
        (out / f"{name}.stdout").write_bytes(result.stdout)
        (out / f"{name}.stderr").write_bytes(result.stderr)
        print(f"{name}: {'PASS' if good else 'FAIL'} exit={result.returncode} exact={exact}")
    (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    raise SystemExit(0 if all(r["pass"] for r in results) else 1)


if __name__ == "__main__":
    main()
