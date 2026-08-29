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

    positive = fol_dir / "trace-refinement.elab"
    negatives = sorted((fol_dir / "negative").glob("*.elab"))
    cases = [(positive, "accept")] + [(path, "reject") for path in negatives]
    failures = 0
    positive_bytes = 0
    positive_ms = 0.0
    for source, expected in cases:
        cert = elaborate(elab, source)
        rooted, rooted_ms = verdict([str(checker)], cert)
        reference, _ = verdict([sys.executable, str(check_ref)], cert)
        gamma = gamma_verdict(
            gamma_translator, gamma_interpreter, gamma_definitions, cert
        )
        if source == positive:
            positive_bytes = len(cert)
            positive_ms = rooted_ms
        if rooted != expected or reference != expected or gamma != expected:
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
        f"{1 if not failures else 0} positive, "
        f"{len(negatives) if not failures else len(negatives) - failures} negative; "
        f"certificate={positive_bytes} bytes, checker={positive_ms:.1f} ms, "
        f"child_peak_rss={peak_kib} KiB"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
