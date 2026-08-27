#!/usr/bin/env python3
"""Focused platform-neutral OMGRFN18 reference orchestration."""

from __future__ import annotations

import argparse
import importlib.util
import os
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from omgrfn18_bundle import pack

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[3] / "source/on-ramp/omega-bootstrap/gates"
REPO = HERE.parents[3]
COMPILER = REPO / "source/on-ramp/omega-bootstrap/compiler"
FIXTURE_SOURCE = GATES / "fixtures/ckir16-u64-less/general.omg"
OWNERS = {
    name: HERE / f"omgrfn18-{name}.py" for name in (
        "r1", "r2", "r3", "r4-lowering", "r4-source-result",
        "r5-structure", "r5-result", "r5-elf",
    )
}


def reference_gate() -> None:
    started = time.monotonic()
    subprocess.run(
        [sys.executable, "-B", str(HERE / "omgrfn18_owner_test.py"), "-q"],
        check=True,
        env=dict(os.environ, PYTHONPATH=f"{HERE}:{GATES}"),
    )
    python_elapsed = time.monotonic() - started
    beta_started = time.monotonic()
    subprocess.run([str(HERE / "omgrfn18-beta-join.sh")], check=True)
    print(f"OMGRFN18 reference integration: modular Python owners and exact u64 "
          f"artifact reconstruction {python_elapsed:.2f}s; persisted-Beta split "
          f"join {time.monotonic() - beta_started:.2f}s")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def load_producer_fixture():
    path = GATES / "delta-resolved-to-ckir16-fixture.py"
    spec = importlib.util.spec_from_file_location("omgrfn18_producer_fixture", path)
    require(spec is not None and spec.loader is not None, "CKIR16 producer fixture loader")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def observe(owner: str, frame: bytes, expected: int = 0) -> None:
    result = subprocess.run(
        [sys.executable, str(OWNERS[owner])], input=frame,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=8,
    )
    require(result.returncode == expected,
            f"{owner} returned {result.returncode}, expected {expected}: "
            f"{result.stderr[-1000:]!r}")
    require(not result.stdout, f"{owner} published {len(result.stdout)} bytes")


def compile_backend(temp: Path) -> Path:
    delta = REPO / "source/on-ramp/rust/delta/target/debug/delta"
    source = COMPILER / "omega-bootstrap-checked-ir-v5-to-elf.alp"
    destination = temp / "backend-native"
    subprocess.run(
        [str(delta), str(source), str(destination)], check=True,
        env=dict(os.environ, DELTA_ARCH="aarch64"), stdout=subprocess.DEVNULL,
    )
    destination.chmod(0o755)
    return destination


def producer_gate() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("OMGRFN18 same-frame composite: skipped (requires Darwin arm64)")
        return
    fixture = load_producer_fixture()
    source = FIXTURE_SOURCE.read_text(encoding="ascii")
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="omgrfn18-") as raw:
        temp = Path(raw)
        tools = fixture.compile_tools(temp)
        backend = compile_backend(temp)
        omgcomp, witness, ckir = fixture.positive_pipeline(tools, source)
        elf = fixture.run_status(backend, ckir, 0, "actual CKIR16 backend")
        frame = pack(omgcomp, witness, ckir, elf, 70)
        for owner in OWNERS:
            observe(owner, frame)

        observe("r1", b"OMGRFNX\0" + frame[8:], 251)
        result_drift = bytearray(frame)
        result_drift[32:36] = (71).to_bytes(4, "little")
        result_drift[36:40] = (71).to_bytes(4, "little")
        observe("r4-source-result", bytes(result_drift), 251)
        observe("r5-result", bytes(result_drift), 251)
        observe("r5-elf", frame[:-1] + bytes([frame[-1] ^ 1]), 251)
    print(f"OMGRFN18 same-frame composite: actual native/self OMGRSW8 + CKIR16, "
          f"production backend, and all responsibility owners passed in "
          f"{time.monotonic() - started:.2f}s")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("reference", "producer"), nargs="?",
                        default="reference")
    command = parser.parse_args().command
    reference_gate() if command == "reference" else producer_gate()


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"OMGRFN18 gate: {error}", file=sys.stderr)
        raise SystemExit(1)
