"""Measured worst-shape pair-containment study for the Delta compiler.

Compiles scaled worst-shape Delta sources through the bound compiler closure
under an instrumented Alpha interpreter and records each run's cumulative
immutable-pair allocation. The instrument is the reference interpreter at
tests/alpha/reference/alpha_ref.py with one added counter: every store of the
pair node marker 0x50414952 counts exactly one 40-byte node allocation in the
evaluator's bump sequence. The count is cumulative across evaluator startup,
parsing, checking, lowering, normalization, and emission, so each measurement
is a whole-producer total for that source. The instrumented interpreter is a
diagnostic witness; it is not an admitted evaluator variant.

The selected evaluator provisions 3,422,453,760 immutable pairs
(bootstrap/2_gamma/EVALUATOR_PROFILE.md); its tape sets the allocation cursor
at 0x20400000 with the limit at 0x2000000000 and each node is 40 bytes. This
script is the measured worst-shape study named by the DELTA-COMPILER scope
checkpoint in TASKS_BOOTSTRAP.md: each family is one admitted extent's worst
pair-allocating source shape, measured at scaled sizes, and the observed rate
is evaluated at the full extent.

Usage:
    sh -c '. tools/bootstrap/paths.sh &&
           . tools/bootstrap/delta/compiler_env.sh &&
           materialize_delta_compiler "$DIR/compiler.gamma" &&
           materialize_delta_support "$DIR/support.bin"'   # OMEGA_REPO_ROOT set

    python3 tests/delta/resource-boundary/pair_study.py \
        --compiler "$DIR/compiler.gamma" --support "$DIR/support.bin" \
        --sizes coverage:64,256 --sizes wide_reconstruction:32,64

Each measurement prints one JSON line per family size. `--analyze` reads a
results file, fits each family's marginal allocation rate from its two largest
measured sizes, evaluates the rate at the admitted extent, and prints the
containment table: exit 0 when the projected worst-shape totals stay below the
selected arena, exit 1 otherwise. A raw status-252 outcome is reported
separately: it is a Gamma-owned heap failure and an escalation finding, not a
DCOUT outcome.
"""

import argparse
import hashlib
import json
import os
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

GATE_DIR = Path(__file__).resolve().parent
REPO_ROOT = GATE_DIR.parents[2]
ALPHA_REFERENCE = REPO_ROOT / "tests" / "alpha" / "reference" / "alpha_ref.py"
EVALUATOR_TAPE = REPO_ROOT / "bootstrap" / "2_gamma" / "gamma_evaluator_bytecode.tape"

# Selected-evaluator immutable-pair provision (bootstrap/2_gamma/EVALUATOR_PROFILE.md).
PAIR_ARENA = 3422453760

MAIN = b"(def main ((input Bytes)) Bytes input)\n"


def source_functions(n):
    """N distinct ordinary function declarations plus `main`: full pipeline
    work per definition (census, names, resolution, checking, lowering,
    emission)."""
    return (b"".join(f"(def f{i:05d} () Int 0)\n".encode() for i in range(n))
            + MAIN)


def source_constructors(n):
    """One nominal with N constructors: constructor rows."""
    return (b"(data T\n"
            + b"".join(f"  (C{i:05d} Missing)\n".encode() for i in range(n))
            + b")\n" + MAIN)


def source_types(n):
    """N nominal declarations: type rows."""
    return (b"".join(f"(data T{i:05d} (C{i:05d} Missing))\n".encode()
                     for i in range(n))
            + MAIN)


def source_parameters(n):
    """One function with N parameters: active environment rows."""
    parameters = b" ".join(f"(p{i:05d} Int)".encode() for i in range(n))
    return b"(def wide (" + parameters + b") Int (+ 1 2))\n" + MAIN


def source_coverage(n):
    """One match over N nullary constructors: coverage rows."""
    declarations = b"(data T " + b" ".join(
        f"(C{i:05d})".encode() for i in range(n)) + b")\n"
    arms = b" ".join(f"(C{i:05d} 0)".encode() for i in range(n))
    return declarations + b"(def main () Int (match C00000 " + arms + b"))\n"


def source_atoms(n):
    """N empty top-level lists: syntax-arena parser pairs."""
    return b"() " * n


def source_name(n):
    """One N-byte identifier: name-trie and compact-name bytes."""
    return b"(def " + b"a" * n + b" () Int 0)\n" + MAIN


def source_depth(n):
    """N nested arithmetic expressions: parse depth."""
    return b"(def main () Int " + b"(+ 0 " * n + b"0" + b")" * n + b")\n"


def source_wide_reconstruction(n):
    """One N-field constructor selected and rebuilt by one match arm: the
    normalizer's capture aggregate and emission's payload projection. This is
    the scaled form of reconstructed_wide_fixtures in payload_bytes.py."""
    fields = b" ".join(f"f{i:05d}".encode() for i in range(n))
    return (b"(data Wide (Wide" + b" Int" * n + b"))\n"
            b"(def select ((value Wide)) Wide (match value ((Wide "
            + fields + b") (Wide " + fields + b"))))\n" + MAIN)


def source_nested_lets(n):
    """N nested lets, each initializer referencing the enclosing binder and
    the innermost body returning `input`: drives normalizer helper extraction
    and forwarded captures across ancestor cuts once bodies exceed the
    evaluator's 255 nested-list budget. Signed `Bytes -> Bytes` main keeps the
    whole pipeline reachable."""
    opens = b"".join(
        f"(let a{i:05d} Int ".encode()
        + (b"0" if i == 0 else f"a{i-1:05d}".encode()) + b" "
        for i in range(n))
    return (b"(def main ((input Bytes)) Bytes " + opens
            + b"input" + b")" * n + b")\n")


def source_payload_definitions(n):
    """N match definitions over one 120-field constructor: emitted payload
    size and emission traversal pairs (scaled payload_bytes.py shape)."""
    data = b"(data T (C" + b" Int" * 120 + b"))\n"
    binders = b" ".join(f"p{i:03d}".encode() for i in range(120))
    definitions = b"".join(
        b"(def " + f"f{i:03d}".encode()
        + b" ((x T)) Int (match x ((C " + binders + b") 0)))\n"
        for i in range(n))
    return data + definitions + MAIN


FAMILIES = {
    "functions": source_functions,
    "constructors": source_constructors,
    "types": source_types,
    "parameters": source_parameters,
    "coverage": source_coverage,
    "atoms": source_atoms,
    "name_bytes": source_name,
    "depth": source_depth,
    "wide_reconstruction": source_wide_reconstruction,
    "payload_definitions": source_payload_definitions,
    "nested_lets": source_nested_lets,
}

# Admitted extent each family's parameter drives, in that family's units.
# Sources past these limits refuse inside canonical DCOUT before or during the
# measured phase, so the extent is the largest shape the family can carry.
FAMILY_EXTENT = {
    "functions": 32768,            # function rows
    "constructors": 65536,         # constructor rows
    "types": 65536,                # type rows
    "parameters": 65536,           # active environment rows
    "coverage": 65536,             # coverage rows
    "atoms": 357172,               # syntax arena: 2,857,368 pairs at 8 bytes/node
    "name_bytes": 4000047,         # name_storage.py's admitted long-name extent
    "depth": 1024,                 # parse-depth rows
    "wide_reconstruction": 65535,  # active pattern binders in one arm
    "payload_definitions": 246,    # emitted payload bytes at 16,777,212
    "nested_lets": 500,           # binder depth inside the 1,024 parse-depth rows
}

DEFAULT_SIZES = {
    "functions": (64, 256),
    "constructors": (64, 256),
    "types": (64, 256),
    "parameters": (64, 256),
    "coverage": (64, 256),
    "atoms": (8192, 32768),
    "name_bytes": (4096, 65536),
    "depth": (64, 256),
    "wide_reconstruction": (32, 64, 128),
    "payload_definitions": (4, 8, 16),
    "nested_lets": (300, 450),
}


def instrumented_reference(workdir):
    """Write an instrumented copy of the reference interpreter and return it.

    The copy adds a cumulative pair counter on the immutable-pair node marker
    and a step counter on the dispatch loop, and writes both to the file named
    by the PAIRLOG environment variable on every exit path.
    """
    source = ALPHA_REFERENCE.read_text(encoding="utf-8")
    store = "            wr8(R[M[pc + 1]], R[M[pc + 2]]); pc += 3"
    counted = ("            if (R[M[pc + 2]] & MASK) == 0x50414952:"
               " PAIRS[0] += 1\n" + store)
    assert source.count(store) == 1, "reference interpreter store site changed"
    source = source.replace(store, counted)
    flush = "sys.stdout.buffer.flush()"
    assert source.count(flush) == 2, "reference interpreter exit paths changed"
    source = source.replace(
        flush,
        'sys.stdout.buffer.flush(); open(os.environ["PAIRLOG"], "w")'
        '.write(f"{PAIRS[0]} {STEPS[0]}")')
    dispatch = "        op = M[pc]"
    assert source.count(dispatch) == 1, "reference interpreter dispatch changed"
    source = "import os\nPAIRS=[0]\nSTEPS=[0]\n" + source.replace(
        dispatch, dispatch + "; STEPS[0] += 1", 1)
    path = Path(workdir) / "alpha_instrumented.py"
    path.write_text(source, encoding="utf-8")
    return path


def measure(interpreter, compiler, support, source, timeout):
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(source))
               + source + support)
    sealed = struct.pack("<I", len(compiler)) + compiler + request
    started = time.monotonic()
    with tempfile.NamedTemporaryFile(prefix="pairlog.", delete=False) as log:
        log_path = log.name
    try:
        process = subprocess.run(
            [sys.executable, str(interpreter), str(EVALUATOR_TAPE)],
            input=sealed, stdout=subprocess.PIPE,
            env={**os.environ, "PAIRLOG": log_path}, timeout=timeout)
        elapsed = time.monotonic() - started
        fields = Path(log_path).read_text().split()
        pairs, steps = (int(fields[0]), int(fields[1])) if fields else (-1, -1)
        return {"exit": process.returncode, "pairs": pairs, "steps": steps,
                "out_bytes": len(process.stdout), "seconds": round(elapsed, 1)}
    finally:
        os.unlink(log_path)


def quadratic_fit(points):
    """Least-squares quadratic through the measured points, solved directly."""
    n = len(points)
    sx = sum(p["n"] for p in points)
    sx2 = sum(p["n"] ** 2 for p in points)
    sx3 = sum(p["n"] ** 3 for p in points)
    sx4 = sum(p["n"] ** 4 for p in points)
    sy = sum(p["pairs"] for p in points)
    sxy = sum(p["n"] * p["pairs"] for p in points)
    sx2y = sum(p["n"] ** 2 * p["pairs"] for p in points)
    # Gaussian elimination on the 3x3 normal equations.
    matrix = [[n, sx, sx2, sy], [sx, sx2, sx3, sxy], [sx2, sx3, sx4, sx2y]]
    for col in range(3):
        pivot = max(range(col, 3), key=lambda row: abs(matrix[row][col]))
        matrix[col], matrix[pivot] = matrix[pivot], matrix[col]
        for row in range(col + 1, 3):
            factor = matrix[row][col] / matrix[col][col]
            for k in range(4):
                matrix[row][k] -= factor * matrix[col][k]
    coeffs = [0.0, 0.0, 0.0]
    for row in range(2, -1, -1):
        coeffs[row] = (matrix[row][3]
                       - sum(matrix[row][k] * coeffs[k] for k in range(row + 1, 3))
                       ) / matrix[row][row]
    return coeffs  # c0 + c1*n + c2*n^2


def analyze(records):
    exits_252 = [r for r in records if r.get("exit") == 252]
    print(f"raw heap-exhaustion outcomes (status 252): {len(exits_252)}")
    for record in exits_252:
        print(f"  status 252: {json.dumps(record)}")

    total = 0
    by_family = {}
    for record in records:
        by_family.setdefault(record["family"], []).append(record)
    for family, points in sorted(by_family.items()):
        if family not in FAMILY_EXTENT:
            print(f"{family:24s} skipped: not an extent axis "
                  f"(diagnostic row, {len(points)} run(s))")
            continue
        points.sort(key=lambda r: r["n"])
        extent = FAMILY_EXTENT[family]
        if len(points) >= 3:
            c0, c1, c2 = quadratic_fit(points)
            if c2 <= 0:
                projected = int(c0 + c1 * extent)
                model = f"linear ({c1:.3f}/unit)"
            else:
                projected = int(c0 + c1 * extent + c2 * extent * extent)
                model = f"quadratic ({c2:.6f}*n^2 + {c1:.3f}*n)"
        elif len(points) == 2:
            a, b = points
            rate = (b["pairs"] - a["pairs"]) / (b["n"] - a["n"])
            projected = int(a["pairs"] + rate * extent)
            model = f"linear ({rate:.3f}/unit)"
        else:
            rate = points[-1]["pairs"] / points[-1]["n"]
            projected = int(rate * extent)
            model = f"single-point ({rate:.3f}/unit)"
        total += projected
        print(f"{family:24s} extent={extent:>12,d} {model:34s} "
              f"projected={projected:>15,d} "
              f"(measured {points[-1]['pairs']:,} @ n={points[-1]['n']:,})")
    if total:
        headroom = PAIR_ARENA / total
        print(f"{'TOTAL':24s} {'':>12s} {'':>34s}{total:>15,d} pairs; "
              f"arena {PAIR_ARENA:,} -> {headroom:.1f}x headroom")
        return 0 if total < PAIR_ARENA else 1
    return 1


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--compiler", type=Path,
                        help="materialized delta_compiler.gamma")
    parser.add_argument("--support", type=Path,
                        help="materialized support.bin")
    parser.add_argument("--sizes", action="append", default=[],
                        metavar="FAMILY:N,N,...",
                        help="scaled sizes per family (repeatable)")
    parser.add_argument("--timeout", type=float, default=7200,
                        help="per-measurement host watchdog seconds")
    parser.add_argument("--analyze", type=Path, metavar="RESULTS.jsonl",
                        help="analyze an existing results file and exit")
    options = parser.parse_args()

    if options.analyze:
        records = [json.loads(line) for line in
                   options.analyze.read_text().splitlines() if line.strip()]
        raise SystemExit(analyze(records))

    if not options.compiler or not options.support:
        parser.error("measurement requires --compiler and --support")

    sizes = {family: list(DEFAULT_SIZES[family]) for family in FAMILIES}
    for entry in options.sizes:
        family, _, values = entry.partition(":")
        if family not in FAMILIES or not values:
            parser.error(f"invalid --sizes {entry!r}")
        sizes[family] = [int(value) for value in values.split(",")]

    compiler = options.compiler.read_bytes()
    support = options.support.read_bytes()
    with tempfile.TemporaryDirectory() as workdir:
        interpreter = instrumented_reference(workdir)
        for family, ns in sizes.items():
            for n in ns:
                source = FAMILIES[family](n)
                result = measure(interpreter, compiler, support, source,
                                 options.timeout)
                record = {"family": family, "n": n,
                          "source_bytes": len(source),
                          "sha256": hashlib.sha256(source).hexdigest(),
                          **result}
                print(json.dumps(record), flush=True)


if __name__ == "__main__":
    main()
