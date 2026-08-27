#!/usr/bin/env python3
"""OMGRFN23 reference and actual native/self same-frame orchestration."""

from __future__ import annotations

import argparse
import importlib.util
import os
import platform
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from omgrfn23_bundle import pack
from omgrfn23_elf import reconstruct
from omgrfn23_profiles import CKIR_FIXTURE, SOURCE_FIXTURE, components
from omgrfn23_source import check_witness_relation

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
GATES = ROOT / "bootstrap/omega-bootstrap/gates"
COMPILER = ROOT / "bootstrap/omega-bootstrap/compiler"
OWNERS = tuple(HERE / f"omgrfn23-{name}.py" for name in
               ("r1", "r2", "r3", "r4-lowering", "r4-source-result",
                "r5-structure", "r5-result", "r5-elf"))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module; spec.loader.exec_module(module); return module


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def reference_gate() -> None:
    started = time.monotonic()
    env = dict(os.environ, PYTHONPATH=f"{HERE}:{GATES}")
    subprocess.run([sys.executable, "-B", str(HERE / "omgrfn23_owner_test.py")],
                   check=True, env=env)
    python_elapsed = time.monotonic() - started
    beta_started = time.monotonic()
    subprocess.run([str(HERE / "omgrfn23-beta-join.sh")], check=True)
    print(f"OMGRFN23 reference integration: modular Python owners "
          f"{python_elapsed:.2f}s; split persisted-Beta projections "
          f"{time.monotonic() - beta_started:.2f}s")


def run_status(executable: Path, contents: bytes, expected: int, label: str) -> bytes:
    try:
        result = subprocess.run([str(executable)], input=contents,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                timeout=90)
    except subprocess.TimeoutExpired as error:
        raise ValueError(f"{label} exceeded 90 seconds") from error
    require(result.returncode == expected,
            f"{label} status {result.returncode}, expected {expected}: "
            f"{result.stderr[-1000:]!r}")
    if expected:
        require(not result.stdout, f"{label} published rejected bytes")
    return result.stdout


def normalized_source(path: Path) -> bytes:
    raw = re.sub(rb"//[^\n]*", b"", path.read_bytes())
    raw = re.sub(rb"\s+", b" ", raw)
    return re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)


def compile_tools(temp: Path) -> dict[str, Path]:
    manifest = ROOT / "bootstrap/delta/rust/Cargo.toml"
    delta = ROOT / "bootstrap/delta/rust/target/debug/delta"
    sources = {
        "resolver": COMPILER / "omega-bootstrap-token-stream-resolve.alp",
        "lowerer": COMPILER / "omega-bootstrap-token-stream-to-ckir.alp",
        "backend": COMPILER / "omega-bootstrap-checked-ir-v20-to-elf.alp",
    }
    lowermachine_source = ROOT / "bootstrap/delta/samples/lowermachine.alp"
    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(manifest)],
                   check=True)
    env = dict(os.environ, DELTA_ARCH="aarch64"); tools: dict[str, Path] = {}
    for name, source in (*sources.items(), ("lowermachine", lowermachine_source)):
        destination = temp / f"{name}.native"
        subprocess.run([str(delta), str(source), str(destination)], check=True,
                       env=env, stdout=subprocess.DEVNULL)
        destination.chmod(0o755); tools[f"{name}.native"] = destination
    lowermachine = tools["lowermachine.native"]
    for name, source in sources.items():
        assembly = temp / f"{name}.self.s"
        with assembly.open("wb") as stream:
            result = subprocess.run([str(lowermachine)], input=normalized_source(source),
                                    stdout=stream, timeout=90)
        require(result.returncode == 0, f"self build rejected {name}")
        destination = temp / f"{name}.self"
        subprocess.run(["clang", "-arch", "arm64", "-o", str(destination),
                        str(assembly)], check=True)
        subprocess.run(["codesign", "-f", "-s", "-", str(destination)], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        tools[f"{name}.self"] = destination
    return tools


def observe(owner: Path, frame: bytes, expected: int, label: str) -> None:
    result = subprocess.run([sys.executable, "-B", str(owner)], input=frame,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=10)
    require(result.returncode == expected,
            f"{label}/{owner.name} status {result.returncode}, expected {expected}: "
            f"{result.stderr[-1000:]!r}")
    require(not result.stdout, f"{label}/{owner.name} published bytes")


def producer_gate() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("OMGRFN23 same-frame composite: skipped (requires Darwin arm64)")
        return
    lowering = load("omgrfn23_lowering_fixture",
                    GATES / "delta-token-stream-to-ckir20-fixture.py")
    expected_omg, expected_witness, expected_ckir, expected_elf = components()
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="omgrfn23-") as raw:
        temp = Path(raw); source_dir = temp / "source"
        SOURCE_FIXTURE.matrix(source_dir); tools = compile_tools(temp)
        positives = [(line.split("\t")[0], Path(line.split("\t")[1]).read_bytes())
                     for line in (source_dir / "positives.tsv").read_text(
                         encoding="ascii").splitlines()]
        witnesses: dict[tuple[str, str], bytes] = {}
        outputs: dict[tuple[str, str, str], bytes] = {}
        for name, omgcomp in positives:
            for resolver_mode in ("native", "self"):
                witness = run_status(tools[f"resolver.{resolver_mode}"], omgcomp, 0,
                                     f"{name}/{resolver_mode} resolver")
                witnesses[(name, resolver_mode)] = witness
                check_witness_relation(omgcomp, witness)
                for lowerer_mode in ("native", "self"):
                    ckir = run_status(tools[f"lowerer.{lowerer_mode}"],
                                      lowering.pack(omgcomp, witness), 0,
                                      f"{name}/{resolver_mode}/{lowerer_mode} lowerer")
                    require(ckir == expected_ckir,
                            f"{name}/{resolver_mode}/{lowerer_mode} exact CKIR20")
                    outputs[(name, resolver_mode, lowerer_mode)] = ckir
            require(witnesses[(name, "native")] == witnesses[(name, "self")],
                    f"{name} native/self OMGRSWC12 identity")

        canonical_omg = dict(positives)["canonical"]
        canonical_witness = witnesses[("canonical", "native")]
        canonical_ckir = outputs[("canonical", "native", "native")]
        require((canonical_omg, canonical_witness, canonical_ckir) ==
                (expected_omg, expected_witness, expected_ckir),
                "actual canonical OMGCOMP1/OMGRSWC12/CKIR20 identity")
        artifacts = {mode: run_status(tools[f"backend.{mode}"], canonical_ckir, 0,
                                      f"canonical {mode} backend")
                     for mode in ("native", "self")}
        require(artifacts["native"] == artifacts["self"] == expected_elf,
                "actual native/self exact ELF identity")
        frame = pack(canonical_omg, canonical_witness, canonical_ckir,
                     artifacts["native"])
        for owner in OWNERS:
            observe(owner, frame, 0, "actual canonical same frame")

        renamed_omg = dict(positives)["renamed"]
        renamed_witness = witnesses[("renamed", "native")]
        for mode in ("native", "self"):
            run_status(tools[f"lowerer.{mode}"],
                       lowering.pack(canonical_omg, renamed_witness), 251,
                       f"actual source/witness cross-pair {mode}")
        cross_frame = pack(canonical_omg, renamed_witness, canonical_ckir,
                           artifacts["native"])
        observe(HERE / "omgrfn23-r2.py", cross_frame, 251,
                "actual alternate-witness source cross-pair")
        observe(HERE / "omgrfn23-r4-lowering.py", cross_frame, 251,
                "actual alternate-witness lowering cross-pair")
        high_ckir = CKIR_FIXTURE.encode(CKIR_FIXTURE.tables(
            extra="high-half-transport"))
        high_elf = reconstruct(high_ckir)
        observe(HERE / "omgrfn23-r4-lowering.py",
                pack(canonical_omg, canonical_witness, high_ckir, high_elf), 251,
                "actual CKIR cross-pair")
        observe(HERE / "omgrfn23-r5-elf.py",
                pack(canonical_omg, canonical_witness, canonical_ckir, high_elf), 251,
                "actual ELF cross-pair")

        negatives = (source_dir / "negatives.tsv").read_text(
            encoding="ascii").splitlines()
        for line in negatives:
            name, expected, path = line.split("\t")
            contents = Path(path).read_bytes()
            for mode in ("native", "self"):
                run_status(tools[f"resolver.{mode}"], contents, int(expected),
                           f"{name} resolver {mode}")
        exhausted_ckir = CKIR_FIXTURE.malformed()["operation-exhaustion"][0]
        for mode in ("native", "self"):
            run_status(tools[f"lowerer.{mode}"], bytes(274_834), 252,
                       f"input-exhausted lowerer {mode}")
            run_status(tools[f"backend.{mode}"], exhausted_ckir, 252,
                       f"operation-exhausted backend {mode}")
        changed_elf = artifacts["native"][:-1] + bytes([artifacts["native"][-1] ^ 1])
        observe(HERE / "omgrfn23-r5-elf.py",
                pack(canonical_omg, canonical_witness, canonical_ckir, changed_elf),
                251, "actual artifact byte drift")
    print("OMGRFN23 same-frame composite: actual native/self OMGRSWC12 + lowerer "
          "+ CKIR20 backend, accepted variants, cross-pairs, resources, artifact "
          f"tooth, and all owners passed in {time.monotonic() - started:.2f}s")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("reference", "producer"), nargs="?",
                        default="reference")
    reference_gate() if parser.parse_args().command == "reference" else producer_gate()


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"OMGRFN23 gate: {error}", file=sys.stderr)
        raise SystemExit(1)
