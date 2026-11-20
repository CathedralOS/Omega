"""Run the portable Epsilon observations through the Beta-written Gamma evaluator."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from compiled import ROOT, delta_request, execute, gamma_request, omega_customer, reconstruct, shell
from conformance.vectors import (
    epsilon_envelope_observations, epsilon_historical_at_observations,
    epsilon_observations, epsilon_syntax_controls,
)

PROFILES = {
    "language": epsilon_observations,
    "envelope": epsilon_envelope_observations,
    "syntax-controls": epsilon_syntax_controls,
    "historical-at": epsilon_historical_at_observations,
}

ENVIRONMENT = """
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"
"""


def check(work, receipt_path, seconds, profile, omega):
    reconstruct(work)
    shell(ENVIRONMENT + """
        materialize_epsilon_evaluator "$1/epsilon.delta"
        require_epsilon_evaluator_entry_identity
    """, work)
    closure = (work / "epsilon.delta").read_bytes()
    evaluator = work / "reference"
    if receipt_path is None:
        print(f"constructing canonical Epsilon receipt (limit {seconds}s)", flush=True)
        shell(ENVIRONMENT + """
            materialize_delta_compiler "$1/compiler.gamma"
            materialize_delta_support "$1/support.bin"
        """, work)
        compiler = (work / "compiler.gamma").read_bytes()
        entry = ROOT / "bootstrap/epsilon/interpreter.delta"
        subject = closure + entry.read_bytes()
        request = delta_request(subject, (work / "support.bin").read_bytes())
        started = time.monotonic()
        compiler_request = gamma_request(compiler, request)
        status, receipt = execute(evaluator, compiler_request, seconds)
        if status:
            sys.exit(f"interpreted Delta compiler returned {status}")
        print(f"constructed Epsilon receipt in {time.monotonic() - started:.3f}s", flush=True)
    else:
        print(f"reusing Epsilon receipt from {receipt_path}", flush=True)
        receipt = receipt_path.read_bytes()
    pending = work / "pending.gamma"
    pending.write_bytes(receipt)
    shell(ENVIRONMENT + 'require_epsilon_evaluator_entry_receipt_identity "$1"', pending)
    pending.rename(work / "evaluator.gamma")
    program = gamma_request(receipt)
    print(f"Epsilon receipt: {len(receipt)} bytes, sha256 {hashlib.sha256(receipt).hexdigest()}", flush=True)
    def observe(payload, timeout):
        return execute(evaluator, program + payload, timeout)

    failed = 0
    passed = 0
    for label, payload, expected in PROFILES[profile](closure):
        started = time.monotonic()
        try:
            status, output = observe(payload, seconds)
        except subprocess.TimeoutExpired:
            print(f"{label}: timed out after {seconds}s", flush=True)
            failed += 1
            continue
        elapsed = time.monotonic() - started
        if (status, output) == expected:
            passed += 1
            print(f"{label}: passed in {elapsed:.3f}s", flush=True)
        else:
            failed += 1
            print(f"{label}: exit {status}, stdout {output!r}, expected {expected!r} ({elapsed:.3f}s)",
                  flush=True)
    print(f"interpreted Epsilon ({profile}): {passed} passed, {failed} failed", flush=True)
    if omega:
        shell(ENVIRONMENT + """
            . "$OMEGA_REPO_ROOT/tools/bootstrap/omega/compiler_env.sh"
            require_omega_compiler_identity
            require_omega_executable_entries_identity
        """)
        try:
            omega_customer(work, seconds, observe=observe)
        except subprocess.TimeoutExpired:
            print(f"interpreted Omega customer: timed out after {seconds}s", flush=True)
            return 1
    return int(failed != 0)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="new directory for the constructed artifacts")
    parser.add_argument("--profile", choices=PROFILES, default="language")
    parser.add_argument("--omega", action="store_true",
                        help="also compile and run the shared Omega program through the Epsilon evaluator")
    parser.add_argument("--receipt", type=Path,
                        help="reuse an existing canonical Gamma receipt after checking its bound identity")
    parser.add_argument("--seconds", type=int, default=900,
                        help="time limit for receipt construction and each observation")
    args = parser.parse_args()
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    directory = args.directory.resolve()
    directory.mkdir(parents=True)
    try:
        return check(directory, args.receipt, args.seconds, args.profile, args.omega)
    except subprocess.TimeoutExpired:
        sys.exit(f"Epsilon receipt construction timed out after {args.seconds}s")


if __name__ == "__main__":
    sys.exit(main())
