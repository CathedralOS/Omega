"""Build the authored Delta interpreter on one Gamma host and run sealed inputs."""
import argparse
import hashlib
from pathlib import Path
import struct
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from compiled import ROOT, execute, pack, reconstruct, shell
from conformance.vectors import epsilon_observations, epsilon_syntax_controls
from gamma_host_matrix import executable, produce

HERE = Path(__file__).resolve().parent
DELTA = ROOT / "bootstrap/delta"
PROFILE_MEMBERS = {"conformance": "interpreter/conformance.gamma", "construction": "interpreter/construction.gamma"}
DOUBLER = b"(def main ((source Bytes)) Bytes (bytes_concat source source))\n"


def materialize(work, profile="conformance"):
    listing = DELTA / "interpreter/sources.list"
    members = listing.read_text().splitlines()
    selector = members.index(PROFILE_MEMBERS["conformance"])
    members[selector] = PROFILE_MEMBERS[profile]
    paths = [DELTA / member for member in members]
    source = b"\n".join(path.read_bytes() for path in paths)
    (work / "interpreter.gamma").write_bytes(source)
    print(f"Interpreter: {len(source)} Gamma bytes, sha256 {hashlib.sha256(source).hexdigest()}", flush=True)
    return source


def frame(source, guest=b""):
    return struct.pack("<I", len(source)) + source + guest


def failure(status, space, code, offset, limit=0, requested=0):
    return status, b"\xffDCOUT\x01\x00" + struct.pack("<BBHIQQQ", status, space, 0, code, offset, limit, requested)


def observations(profile="conformance"):
    for payload in (b"", b"A\x00B", bytes(range(256))):
        yield f"doubler {len(payload)} input bytes", frame(DOUBLER, payload), (0, payload * 2)
    values = (HERE / "interpreter_values.delta").read_bytes()
    for payload in (b"", b"\x00\xff"):
        yield f"nominal matches and mutual recursion {len(payload)} input bytes", frame(values, payload), (
            0, b"+" + payload[-1:] + b'\n\\"')
    # Sibling scopes may reuse a spelling; equal-length and prefix-related
    # names in one active scope still refer to their distinct bindings.
    scopes = b"""(def main ((scope Bytes)) Bytes
      (let scope_a Bytes "a" (let scope_b Bytes "b" (if (bytes_length scope)
        (let result Bytes scope_b (bytes_concat result scope_a))
        (let result Bytes scope_a (bytes_concat result scope_b))))))"""
    yield "sibling scope left", frame(scopes, b"x"), (0, b"ba")
    yield "sibling scope right", frame(scopes), (0, b"ab")
    strict = b"(def drop ((unused Int)) Bytes \"ok\")\n(def main ((x Bytes)) Bytes (drop (/ 1 0)))"
    yield "unused strict argument traps", frame(strict), (249, b"")
    for expression in (b"(+ 9223372036854775807 1)", b"(- -9223372036854775808 1)",
                       b"(* -1 -9223372036854775808)", b"(* 4611686018427387904 4)",
                       b"(/ -9223372036854775808 -1)", b"(% 1 0)", b"(bytes_get x 1)", b"256"):
        source = b"(def main ((x Bytes)) Bytes (bytes_single " + expression + b"))"
        yield f"trap {expression.decode()}", frame(source), (249, b"")
    # A shallow rope can cross either the publication limit or the signed
    # logical-length limit without materializing its exponentially many bytes.
    extents = b"""(def grow ((n Int) (x Bytes)) Bytes
      (if (eq 0 n) x (grow (- n 1) (bytes_concat x x))))
    (def main ((x Bytes)) Bytes (grow (bytes_get x 0) "a"))"""
    yield "output extent", frame(extents, bytes([23 if profile == "conformance" else 28])), (254, b"")
    yield "logical length overflow", frame(extents, bytes([63])), (249, b"")
    yield "short header", b"\x03", failure(1, 4, 1, 1)
    yield "short source", struct.pack("<I", 9) + b"abc", failure(1, 4, 1, 7)
    source_limit = (4 if profile == "conformance" else 64) * 1024 * 1024
    yield "source extent", struct.pack("<I", source_limit + 1), failure(2, 4, 1, 0, source_limit, source_limit + 1)
    if profile == "construction":
        # This frame is inside the construction source limit but lacks its body.
        yield "larger source admitted", struct.pack("<I", 4194305), failure(1, 4, 1, 4)
        peek = b"(def main ((x Bytes)) Bytes (bytes_single (bytes_get x 4194304)))"
        yield "larger guest input", frame(peek, bytes(4194304) + b"X"), (0, b"X")
        yield "guest input extent", frame(peek, bytes(8388609)), (253, b"")

    mismatch = b"(def bad () Bytes 42)\n" + DOUBLER
    missing = b"(data Bit (Zero) (One))\n(def main ((x Bytes)) Bytes (match One (One x)))"
    entry = b"(def main ((x Int)) Int x)"
    duplicate = b'(def main ((input Bytes)) Bytes (let input Bytes "" input))'
    unbound = b"(def main ((input Bytes)) Bytes (let bound Bytes input missing))"
    for label, source, code, offset in (
        ("duplicate active binding", duplicate, 9, duplicate.index(b"input", duplicate.index(b"(let"))),
        ("unbound local", unbound, 14, unbound.index(b"missing")),
        ("bad source byte", DOUBLER + b"\x00", 3, len(DOUBLER)),
        ("unused function result type", mismatch, 15, mismatch.index(b"42")),
        ("incomplete match", missing, 18, missing.index(b"(match")),
        ("invalid application entry", entry, 20, entry.index(b"main")),
    ):
        yield label, frame(source), failure(1, 1, code, offset)


def epsilon_cases(work):
    shell('''. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
        . "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
        . "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"
        require_epsilon_evaluator_entry_identity''')
    closure = pack("bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources", work / "epsilon.delta")
    entry = (ROOT / "bootstrap/epsilon/interpreter.delta").read_bytes()
    for name, payload, expected in (*epsilon_observations(closure), *epsilon_syntax_controls(closure)):
        yield name, frame(closure + entry, payload), expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--host", choices=("gamma", "alpha", "linux"), required=True)
    parser.add_argument("--runner", type=Path, help="already constructed Gamma compiler for Alpha or Linux")
    parser.add_argument("--seconds", type=int, default=120, help="wall limit for each operation")
    parser.add_argument("--epsilon", action="store_true", help="run the checked Delta-written Epsilon evaluator")
    parser.add_argument("--epsilon-case", help="run one named portable Epsilon observation")
    parser.add_argument("--profile", choices=PROFILE_MEMBERS, default="conformance")
    parser.add_argument("--source", type=Path, help="execute this Delta program and write guest.output")
    parser.add_argument("--stdin", type=Path, help="guest input file for --source (default empty)")
    args = parser.parse_args()
    if args.stdin and not args.source:
        parser.error("--stdin requires --source")
    if args.source and (args.epsilon or args.epsilon_case):
        parser.error("choose --source or the Epsilon observations")
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    if (args.runner is None) != (args.host == "gamma"):
        parser.error("--runner is required only for the Alpha and Linux compiler hosts")
    work = args.directory.resolve()
    work.mkdir(parents=True)
    source = materialize(work, args.profile)
    prefix = b""
    if args.host == "gamma":
        reconstruct(work)
        runner = work / "reference"
        prefix = struct.pack("<I", len(source)) + source
    else:
        target = work / ("interpreter.tape" if args.host == "alpha" else "interpreter.elf")
        produce(args.runner.resolve(), source, target, args.seconds)
        runner = executable(target, args.host)
    if args.source:
        payload = args.stdin.read_bytes() if args.stdin else b""
        request = frame(args.source.read_bytes(), payload)
        status, output = execute(runner, prefix + request, args.seconds)
        (work / "guest.output").write_bytes(output)
        print(f"Wrote {len(output)} guest bytes with interpreter exit {status}", flush=True)
        raise SystemExit(status)
    cases = list(observations(args.profile))
    if args.epsilon or args.epsilon_case:
        epsilon = list(epsilon_cases(work))
        if args.epsilon_case:
            epsilon = [case for case in epsilon if case[0] == args.epsilon_case]
            if not epsilon:
                parser.error("unknown --epsilon-case")
        cases.extend(epsilon)
    for label, request, expected in cases:
        started = time.monotonic()
        try:
            actual = execute(runner, prefix + request, args.seconds)
        except subprocess.TimeoutExpired:
            sys.exit(f"{label}: reached the {args.seconds}s wall limit")
        if actual != expected:
            sys.exit(f"{label}: expected {expected!r}, observed {actual!r}")
        print(f"{label}: passed ({time.monotonic() - started:.3f}s)", flush=True)


if __name__ == "__main__":
    main()
