#!/usr/bin/env python3
"""Producer-backed focused gate for the OMGRFN15 candidate reference seam."""

from __future__ import annotations

import os
import platform
import subprocess
import sys
import tempfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "bootstrap/omega-bootstrap/gates"
COMPILER = REPO / "bootstrap/omega-bootstrap/compiler"
sys.path.insert(0, str(GATES))
sys.path.insert(0, str(HERE))
import importlib.util  # noqa: E402
import omgrfn15_reference as reference  # noqa: E402


def load_source_gate():
    path = GATES / "delta-resolved-to-ckir13-fixture.py"
    spec = importlib.util.spec_from_file_location("omgrfn15_source_gate", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def rejected(checker: Path, frame: Path, name: str) -> None:
    result = subprocess.run([sys.executable, "-B", str(checker), str(frame)],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    require(result.returncode == 251 and not result.stdout, f"{name} did not reject")


def main() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("OMGRFN15 candidate reference: skipped (requires Darwin arm64)")
        return
    producer = load_source_gate()
    delta_manifest = REPO / "bootstrap/onramps/delta-rust/Cargo.toml"
    delta = REPO / "bootstrap/onramps/delta-rust/target/debug/delta"
    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(delta_manifest)], check=True)
    with tempfile.TemporaryDirectory(prefix="omgrfn15-reference-") as raw:
        temp = Path(raw)
        env = dict(os.environ, DELTA_ARCH="aarch64")
        paths = {
            "resolver": COMPILER / "omega-bootstrap-resolve.alp",
            "lowerer": COMPILER / "omega-bootstrap-resolved-to-ckir4.alp",
            "backend": COMPILER / "omega-bootstrap-checked-ir-v5-to-elf.alp",
        }
        for name, source in paths.items():
            subprocess.run([str(delta), str(source), str(temp / name)], env=env,
                           check=True, stdout=subprocess.DEVNULL)
        source = producer.SUCCESS.read_text(encoding="ascii")
        omg = producer.encode_source(source)
        witness = subprocess.run([str(temp / "resolver")], input=omg,
                                 stdout=subprocess.PIPE, check=True).stdout
        ckir = subprocess.run([str(temp / "lowerer")],
                              input=producer.pack_lowering(omg, witness),
                              stdout=subprocess.PIPE, check=True).stdout
        elf = subprocess.run([str(temp / "backend")], input=ckir,
                             stdout=subprocess.PIPE, check=True).stdout
        for name, contents in (("program.omgc", omg), ("program.omgrsw5", witness),
                               ("program.ckir13", ckir), ("program.elf", elf)):
            (temp / name).write_bytes(contents)
        frame = temp / "program.omgrfn15"
        with frame.open("wb") as output:
            subprocess.run([
                sys.executable, "-B", str(HERE / "omgrfn15_bundle.py"),
                str(temp / "program.omgc"), str(temp / "program.omgrsw5"),
                str(temp / "program.ckir13"), str(temp / "program.elf"),
                "--result", "70",
            ], check=True, stdout=output)
        reference.check(frame)

        canonical = frame.read_bytes()
        omg_len = int.from_bytes(canonical[16:20], "little")
        witness_len = int.from_bytes(canonical[20:24], "little")
        ckir_len = int.from_bytes(canonical[24:28], "little")
        witness_at = 40 + omg_len
        ckir_at = witness_at + witness_len
        elf_at = ckir_at + ckir_len
        mutations: dict[str, bytearray] = {}
        changed = bytearray(canonical); changed[8] = 14; mutations["outer-version"] = changed
        changed = bytearray(canonical); changed[32] = 71; changed[36] = 71; mutations["claim"] = changed
        changed = bytearray(canonical); changed[witness_at + 6] = ord("4"); mutations["witness"] = changed
        changed = bytearray(canonical); changed[ckir_at + 8] = 12; mutations["ckir"] = changed
        changed = bytearray(canonical); marker = changed.find(b"\x2b\x85", elf_at)
        require(marker >= elf_at, "artifact subtract marker")
        changed[marker] = 0x03; mutations["artifact"] = changed
        mutations["trailing"] = bytearray(canonical + b"\0")
        checker = HERE / "omgrfn15_reference.py"
        for name, contents in mutations.items():
            path = temp / f"{name}.omgrfn15"; path.write_bytes(contents)
            rejected(checker, path, name)

        print("OMGRFN15 candidate reference: producer-backed exact frame result 70; "
              "outer/witness/CKIR/artifact/claim/EOF controls passed")


if __name__ == "__main__":
    main()
