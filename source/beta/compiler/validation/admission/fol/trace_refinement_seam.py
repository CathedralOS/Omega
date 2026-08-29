#!/usr/bin/env python3
"""Exercise the first rooted ordinary-FOL trace-refinement seam.

The elaborator and this driver are untrusted.  Authority comes only from the
persisted below-Beta checker accepting the elaborated certificate.  The Python
checker is a differential oracle.  Resource figures are diagnostic and make
proof-size pressure explicit before any kernel extension is considered.
"""

from __future__ import annotations

import resource
import subprocess
import sys
import time
from pathlib import Path


def split_certificate(source: Path, prefix: Path, proof: Path) -> None:
    """Split one elaborated certificate before its final top-level proof form."""
    raw = source.read_bytes()
    depth = 0
    form_starts: list[int] = []
    for offset, byte in enumerate(raw):
        if byte == ord("("):
            if depth == 0:
                form_starts.append(offset)
            depth += 1
        elif byte == ord(")"):
            depth -= 1
            if depth < 0:
                raise SystemExit("certificate split: unbalanced closing parenthesis")
    if depth != 0 or len(form_starts) < 2:
        raise SystemExit("certificate split: malformed top-level forms")
    proof_start = form_starts[-1]
    prefix.write_bytes(raw[:proof_start])
    proof.write_bytes(raw[proof_start:])


def elaborate(elab: Path, source: Path) -> bytes:
    run = subprocess.run(
        [sys.executable, str(elab)],
        input=source.read_bytes(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if run.returncode != 0:
        raise SystemExit(
            f"trace refinement seam: elaboration failed for {source}: "
            f"{run.stderr.decode(errors='replace').strip()}"
        )
    return run.stdout


def verdict(command: list[str], cert: bytes) -> tuple[str, float]:
    started = time.monotonic()
    run = subprocess.run(
        command,
        input=cert,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=120,
    )
    elapsed_ms = (time.monotonic() - started) * 1000
    return run.stdout.decode(errors="replace").strip(), elapsed_ms


def gamma_verdict(
    translator: Path, interpreter: Path, definitions: bytes, cert: bytes
) -> str:
    translated = subprocess.run(
        [sys.executable, str(translator)],
        input=cert,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if translated.returncode != 0 or not translated.stdout:
        return "untranslatable"
    run = subprocess.run(
        [str(interpreter)],
        input=definitions + b"\n" + translated.stdout,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        check=False,
        timeout=120,
    )
    if run.returncode == 1:
        return "accept"
    if run.returncode == 0:
        return "reject"
    return f"error({run.returncode})"


def main() -> int:
    if len(sys.argv) == 5 and sys.argv[1] == "--split":
        split_certificate(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]))
        return 0
    if len(sys.argv) != 8:
        raise SystemExit(
            "usage: trace_refinement_seam.py CHECKER CHECK_REF ELAB "
            "GAMMA_TRANSLATOR GAMMA_INTERP GAMMA_DEFS FOL_DIR"
        )
    checker = Path(sys.argv[1])
    check_ref = Path(sys.argv[2])
    elab = Path(sys.argv[3])
    gamma_translator = Path(sys.argv[4])
    gamma_interpreter = Path(sys.argv[5])
    gamma_definitions = Path(sys.argv[6]).read_bytes()
    fol_dir = Path(sys.argv[7])

    positives = [
        fol_dir / "trace-refinement.proof",
        fol_dir / "bc-main-resource-refinement.proof",
    ]
    negatives = sorted((fol_dir / "negative").glob("*.proof"))
    cases = [(path, "accept") for path in positives]
    cases += [(path, "reject") for path in negatives]
    failures = 0
    positive_ok = 0
    negative_ok = 0
    measurements: list[str] = []
    for source, expected in cases:
        cert = elaborate(elab, source)
        rooted, rooted_ms = verdict([str(checker)], cert)
        reference, _ = verdict([sys.executable, str(check_ref)], cert)
        gamma = gamma_verdict(
            gamma_translator, gamma_interpreter, gamma_definitions, cert
        )
        if source in positives:
            measurements.append(
                f"{source.stem}={len(cert)} bytes/{rooted_ms:.1f} ms"
            )
        case_ok = rooted == expected and reference == expected and gamma == expected
        if case_ok and expected == "accept":
            positive_ok += 1
        if case_ok and expected == "reject":
            negative_ok += 1
        if not case_ok:
            failures += 1
            print(
                f"  FAIL {source.name}: expected {expected}, "
                f"rooted={rooted!r}, reference={reference!r}, gamma={gamma!r}"
            )

    peak_kib = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    if sys.platform == "darwin":
        peak_kib //= 1024
    print(
        "ordinary-FOL trace refinement seam: "
        f"{positive_ok} positive, {negative_ok} negative; "
        f"certificates={', '.join(measurements)}, "
        f"child_peak_rss={peak_kib} KiB"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
