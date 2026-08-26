#!/usr/bin/env python3
"""Focused producer-backed OMGRFN17 same-frame orchestration."""

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

from omgrfn17_bundle import pack

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "bootstrap/omega-bootstrap/gates"
COMPILER = REPO / "bootstrap/omega-bootstrap/compiler"
FIXTURES = GATES / "fixtures/ckir15-recurrent-view"
OWNERS = {
    name: HERE / f"omgrfn17-{name}.py" for name in (
        "r1", "r2", "r3", "r4-lowering", "r4-source-result",
        "r5-structure", "r5-result", "r5-elf",
    )
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def load_fixture():
    path = GATES / "delta-resolved-to-ckir15-fixture.py"
    spec = importlib.util.spec_from_file_location("omgrfn17_producer_fixture", path)
    require(spec is not None and spec.loader is not None, "CKIR15 producer fixture loader")
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


def observe(owner: str, frame: bytes, expected: int = 0) -> None:
    result = subprocess.run([sys.executable, str(OWNERS[owner])], input=frame,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=8)
    require(result.returncode == expected,
            f"{owner} returned {result.returncode}, expected {expected}: {result.stderr[-1000:]!r}")
    require(not result.stdout, f"{owner} published {len(result.stdout)} bytes")


def compile_backend(temp: Path) -> Path:
    delta = REPO / "bootstrap/delta/rust/target/debug/delta"
    source = COMPILER / "omega-bootstrap-checked-ir-v5-to-elf.alp"
    destination = temp / "backend-native"
    subprocess.run([str(delta), str(source), str(destination)], check=True,
                   env=dict(os.environ, DELTA_ARCH="aarch64"),
                   stdout=subprocess.DEVNULL)
    return destination


def producer_gate() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("OMGRFN17 same-frame composite: skipped (requires Darwin arm64)")
        return
    fixture = load_fixture()
    sources = {path.stem: fixture.source_text(path) for path in sorted(FIXTURES.glob("*.omg"))}
    require(set(sources) == {"two-byte", "one-byte", "empty", "runtime-only"},
            "producer fixture profile set")
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="omgrfn17-") as raw:
        temp = Path(raw)
        tools = fixture.compile_tools(temp)
        backend = compile_backend(temp)
        frames: dict[str, bytes] = {}
        for name in ("two-byte", "one-byte", "empty", "runtime-only"):
            comp, witness, ckir = fixture.pipeline(tools, sources[name], name)
            elf = fixture.run_status(backend, ckir, 0, f"{name} backend")
            frames[name] = pack(comp, witness, ckir, elf, 70)

        # The recurrent case owns the complete conjunction. One-byte and empty
        # exercise only the responsibilities whose conclusion changes with the
        # input view; this avoids a ceremonial profile/checker Cartesian grid.
        for owner in OWNERS:
            observe(owner, frames["two-byte"])
        for name in ("one-byte", "empty", "runtime-only"):
            changed_owners = ["r2", "r3", "r4-lowering", "r4-source-result", "r5-result"]
            if name == "runtime-only":
                changed_owners.append("r5-elf")
            for owner in changed_owners:
                observe(owner, frames[name])

        # Responsibility-local mutations: no unrelated owner is rerun merely
        # to prove that it ignores bytes outside its authority.
        observe("r1", b"OMGRFNX\0" + frames["two-byte"][8:], 251)
        observe("r5-elf", frames["two-byte"][:-1] +
                bytes([frames["two-byte"][-1] ^ 1]), 251)
        result_cross = bytearray(frames["two-byte"])
        result_cross[32:36] = (71).to_bytes(4, "little")
        result_cross[36:40] = (71).to_bytes(4, "little")
        observe("r4-source-result", bytes(result_cross), 251)
        observe("r5-result", bytes(result_cross), 251)

    print(f"OMGRFN17 same-frame composite: producer-backed recurrent/one-byte/empty/runtime-only "
          f"OMGRSW4 + CKIR15 + conservative ELF passed in {time.monotonic()-started:.2f}s")


def reference_gate() -> None:
    started = time.monotonic()
    subprocess.run(
        [sys.executable, "-B", str(HERE / "omgrfn17_owner_test.py"), "-q"],
        check=True, env=dict(os.environ, PYTHONPATH=f"{HERE}:{GATES}"),
    )
    python_elapsed = time.monotonic() - started
    beta_started = time.monotonic()
    subprocess.run([str(HERE / "omgrfn17-beta-join.sh")], check=True)
    print(f"OMGRFN17 reference integration: Python owners {python_elapsed:.2f}s; "
          f"persisted-Beta representative join {time.monotonic()-beta_started:.2f}s")


if __name__ == "__main__":
    try:
        parser = argparse.ArgumentParser()
        parser.add_argument("command", choices=("reference", "producer"))
        command = parser.parse_args().command
        reference_gate() if command == "reference" else producer_gate()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"OMGRFN17 same-frame composite: {error}", file=sys.stderr)
        raise SystemExit(1)
