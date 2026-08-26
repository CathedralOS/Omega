#!/usr/bin/env python3
"""Bounded elaboration and persisted-Gamma execution for CKIR4 meaning gates."""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import time
from pathlib import Path


def record(path: Path, label: str, elapsed: float, size: int) -> None:
    with path.open("a", encoding="ascii") as timings:
        timings.write(f"{elapsed:.6f}\t{size}\t{label}\n")


def payload_error(payload: bytes, ceiling: int) -> str | None:
    placeholder_count = payload.count(b"STDIN")
    if placeholder_count != 1:
        return f"Gamma placeholder count {placeholder_count}"
    if not payload or b"E2G-UNSUPPORTED" in payload or len(payload) > ceiling:
        return f"Gamma bytes {len(payload)} outside 1..={ceiling} or unsupported"
    return None


def require_payload(payload: bytes, ceiling: int, label: str) -> None:
    error = payload_error(payload, ceiling)
    if error is not None:
        raise SystemExit(f"{label} FAIL - {error}")


def elaborate(args: list[str]) -> None:
    executable, source_name, output_name, timing_name, label = args[:5]
    timeout, ceiling = float(args[5]), int(args[6])
    started = time.monotonic()
    print(f"{label}: START elaboration (timeout {timeout:.0f}s)", flush=True)
    with open(source_name, "rb") as source, open(output_name, "wb") as output:
        try:
            result = subprocess.run(
                [executable], stdin=source, stdout=output, stderr=subprocess.PIPE,
                timeout=timeout, check=False,
            )
        except subprocess.TimeoutExpired as error:
            raise SystemExit(f"{label} FAIL - elaboration exceeded {timeout:.0f}s") from error
    elapsed = time.monotonic() - started
    payload = Path(output_name).read_bytes()
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace")[-1000:]
        raise SystemExit(f"{label} FAIL - elaboration status {result.returncode}: {detail}")
    require_payload(payload, ceiling, label)
    record(Path(timing_name), "elaboration", elapsed, len(payload))
    print(
        f"{label}: PASS elaboration {len(payload)} bytes in {elapsed:.2f}s "
        f"(ceiling {ceiling})", flush=True,
    )


def gamma_stdin(raw: bytes) -> str:
    value = "Nil"
    for byte in reversed(raw):
        value = f"(Cons {byte} {value})"
    return value


def run(args: list[str]) -> None:
    interpreter, template_name, input_name, output_name, timing_name, label = args[:6]
    timeout = float(args[6])
    template = Path(template_name).read_text(encoding="ascii")
    if template.count("STDIN") != 1:
        raise SystemExit(f"{label} FAIL - Gamma placeholder count")
    program = template.replace("STDIN", gamma_stdin(Path(input_name).read_bytes())).encode("ascii")
    started = time.monotonic()
    print(
        f"{label}: START Gamma ({len(program)} bytes, timeout {timeout:.0f}s)",
        flush=True,
    )
    process = subprocess.Popen(
        [interpreter], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(program, timeout=timeout)
    except subprocess.TimeoutExpired as error:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        raise SystemExit(f"{label} FAIL - Gamma exceeded {timeout:.0f}s") from error
    elapsed = time.monotonic() - started
    if process.returncode != 0:
        detail = stderr.decode("utf-8", errors="replace")[-1000:]
        raise SystemExit(f"{label} FAIL - interpreter status {process.returncode}: {detail}")
    Path(output_name).write_bytes(stdout)
    record(Path(timing_name), label.rsplit(" ", 1)[-1], elapsed, len(program))
    print(f"{label}: PASS Gamma in {elapsed:.2f}s", flush=True)


def capacity_tooth(args: list[str]) -> None:
    ceiling = int(args[0])
    marker = b"STDIN"
    if ceiling < len(marker):
        raise SystemExit("Gamma capacity tooth FAIL - ceiling cannot retain one placeholder")
    exact = marker + b"x" * (ceiling - len(marker))
    require_payload(exact, ceiling, "Gamma capacity tooth exact-limit")
    error = payload_error(exact + b"x", ceiling)
    expected = f"Gamma bytes {ceiling + 1} outside 1..={ceiling} or unsupported"
    if error != expected:
        raise SystemExit(
            f"Gamma capacity tooth FAIL - adjacent over-limit returned {error!r}, "
            f"expected {expected!r}"
        )
    print(
        f"Gamma capacity tooth: exact {ceiling} accepted; {ceiling + 1} rejected",
        flush=True,
    )


def main(args: list[str]) -> None:
    if len(args) == 8 and args[0] == "elaborate":
        elaborate(args[1:])
    elif len(args) == 8 and args[0] == "run":
        run(args[1:])
    elif len(args) == 2 and args[0] == "capacity-tooth":
        capacity_tooth(args[1:])
    else:
        raise SystemExit(
            "usage: elaborate EXE SOURCE OUTPUT TIMINGS LABEL TIMEOUT CEILING | "
            "run INTERP TEMPLATE INPUT OUTPUT TIMINGS LABEL TIMEOUT | "
            "capacity-tooth CEILING"
        )


if __name__ == "__main__":
    main(sys.argv[1:])
