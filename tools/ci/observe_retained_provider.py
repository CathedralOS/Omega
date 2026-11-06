"""Observe the unchanged provider check with its original producer artifact."""

import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from ci import compiler_replay_plan
from compiler_profile import require_optimized
from qualification import file_digest


def main():
    output = ROOT / "build/compiler-probe"
    output.mkdir(parents=True, exist_ok=True)
    plan = compiler_replay_plan.load(
        ROOT, "@tools/ci/qualification_plans/endpoint_provider_stack_probe.json")
    expected = plan.producer
    binding = json.loads((ROOT / "tools/ci/qualification_plans/endpoint_provider_stack_probe_inputs.json").read_text())
    assert binding["source"] == expected["revision"]
    assert binding["origin_run"] == expected["origin_run"]
    artifact = binding["artifact"]
    digest = binding["archive_sha256"]
    endpoint = f"repos/CathedralOS/Omega/actions/artifacts/{artifact}"
    metadata = json.loads(subprocess.check_output(["gh", "api", endpoint], text=True))
    assert not metadata["expired"]
    assert metadata["workflow_run"]["id"] == expected["origin_run"]
    assert metadata["workflow_run"]["head_sha"] == expected["revision"]
    assert metadata["digest"] == "sha256:" + digest
    archive = output / "producer-artifact.zip"
    with archive.open("wb") as stream:
        subprocess.run(["gh", "api", endpoint + "/zip"], stdout=stream, check=True)
    assert archive.stat().st_size == metadata["size_in_bytes"]
    assert file_digest(archive) == digest
    binaries = {}
    members = []
    with zipfile.ZipFile(archive) as bundle:
        receipts = []
        for name in bundle.namelist():
            if PurePosixPath(name).name == "producer.json":
                raw = json.loads(bundle.read(name))
                receipt = raw.get("producer", raw)
                if receipt.get("revision") == expected["revision"]:
                    receipts.append((name, receipt))
        assert receipts, "original producer receipt is absent"
        name, receipt = next((name, record) for name, record in receipts
                             if str(PurePosixPath(name).parent) != ".")
        assert receipt["tree"] == expected["tree"]
        assert receipt["target"] == expected["target"]
        assert receipt["toolchain"] == expected["toolchain"]
        for kind in ("cli", "runner"):
            identity = expected["executables"][kind]
            assert {key: receipt[kind + "_profile"][key]
                    for key in ("sha256", "profile")} == identity
            member = str(PurePosixPath(name).parent / Path(receipt[kind]).name)
            destination = output / "producer" / Path(member).name
            destination.parent.mkdir(exist_ok=True)
            with bundle.open(member) as source, destination.open("wb") as target:
                shutil.copyfileobj(source, target)
            destination.chmod(0o755)
            sidecar = member + ".compiler-profile.json"
            if sidecar in bundle.namelist():
                Path(str(destination) + ".compiler-profile.json").write_bytes(bundle.read(sidecar))
            observed = require_optimized(destination)
            assert {key: observed[key] for key in ("sha256", "profile")} == identity
            binaries[kind] = destination
            members.append(dict(member=member, sha256=file_digest(destination), profile=observed))
    source_files = subprocess.check_output(
        ["git", "ls-tree", "-r", "--name-only", "HEAD", "--",
         "tests/omega/" + plan.fixtures[0]], text=True).splitlines()
    frozen = {}
    assert set(source_files) == set(binding["frozen_input_sha256"])
    for path in source_files:
        current = subprocess.check_output(["git", "show", "HEAD:" + path])
        original = subprocess.check_output(["git", "show", expected["revision"] + ":" + path])
        assert current == original, path
        frozen[path] = hashlib.sha256(current).hexdigest()
        assert frozen[path] == binding["frozen_input_sha256"][path], path
    record = dict(consumer_revision=subprocess.check_output(
        ["git", "rev-parse", "HEAD"], text=True).strip(), producer=expected,
        transport="original GitHub workflow artifact", artifact=metadata,
        archive_sha256=digest, executable_members=members, frozen_inputs=frozen,
        purpose="Live stack observation only; no golden recording or acceptance claim.")
    (output / "producer-observation.json").write_text(json.dumps(record, indent=2) + "\n")
    archive.unlink()
    command = [sys.executable, "tools/corpus_gate.py", "--runner", str(binaries["runner"]),
               "--filter", plan.fixtures[0], "--json", str(output / "checking.json")]
    result = subprocess.run([sys.executable, "tools/ci/sample_compiler_stacks.py", *command],
                            stdin=subprocess.DEVNULL)
    for kind, binary in binaries.items():
        assert file_digest(binary) == expected["executables"][kind]["sha256"]
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
