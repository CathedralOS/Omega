"""Prepare the exact progress repair and retain its foundation test channels."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path.cwd()
OUT = ROOT / "build/compiler-pilot"
SCOPE = ROOT / ".github/progress_repair_scope.json"
NAMES = (
    "an_arrival_preserves_authority_without_independent_receipt_credit",
    "an_available_independent_proof_survives_input_proofs",
    "distinct_input_generations_and_unknown_provenance_fail_closed",
    "branch_consequences_preserve_input_dependency_and_available_local_proof",
)


def command(name, arguments):
    stdout = OUT / (name + ".stdout")
    stderr = OUT / (name + ".stderr")
    with stdout.open("wb") as out, stderr.open("wb") as err:
        completed = subprocess.run(arguments, cwd=ROOT, stdout=out, stderr=err)
    record = {"command": arguments, "exit": completed.returncode,
              "stdout": str(stdout.relative_to(ROOT)),
              "stderr": str(stderr.relative_to(ROOT))}
    (OUT / (name + ".json")).write_text(json.dumps(record, indent=2) + "\n")
    if completed.returncode:
        raise SystemExit(completed.returncode)
    return stdout.read_text(errors="replace") + "\n" + stderr.read_text(errors="replace")


def main():
    assert sys.argv[1:] in ([], ["--consume"])
    consume = sys.argv[1:] == ["--consume"]
    OUT.mkdir(parents=True, exist_ok=True)
    scope = json.loads(SCOPE.read_text())
    assert len(scope["fixtures"]) == 22 and len(set(scope["fixtures"])) == 22
    assert len(scope["samples"]) == 9 and len(set(scope["samples"])) == 9
    assert scope["source_only"] is False and scope["cli_fixtures"] == []
    os.environ["OMEGA_COMPILER_PILOT_FIXTURES"] = ",".join(scope["fixtures"])
    os.environ["OMEGA_COMPILER_PILOT_SAMPLES"] = ",".join(scope["samples"])
    os.environ["OMEGA_COMPILER_PILOT_CLI_FIXTURES"] = ""
    os.environ["OMEGA_COMPILER_PILOT_CHECKING_ONLY"] = "false"
    sys.path.insert(0, str(ROOT))
    from tools.ci import compiler_pilot
    if consume:
        return compiler_pilot.main(["--phase", "consume"])
    cargo = "mbx" if shutil.which("mbx") else "cargo"
    command("repair-compiler-check", [cargo, "check", "--locked", "-p", "omega"])
    tests = ([cargo, "nextest", "run", "--locked", "-p", "facts",
              "--status-level", "all", "qualification_tests"] if cargo == "mbx" else
             [cargo, "test", "--locked", "--lib", "-p", "facts",
              "qualification_tests", "--", "--nocapture"])
    text = command("repair-evidence-tests", tests)
    observed = re.findall(r"qualification_tests::([A-Za-z_][A-Za-z_0-9]*)", text)
    assert sorted(observed) == sorted(NAMES), observed
    passed = []
    for name in NAMES:
        lines = [line for line in text.splitlines() if "qualification_tests::" + name in line]
        assert len(lines) == 1, (name, lines)
        assert re.search(r"\.\.\. ok\s*$", lines[0]) or "PASS" in lines[0], lines[0]
        passed.append(name)
    (OUT / "repair-evidence-tests-inventory.json").write_text(
        json.dumps({"source": os.environ["GITHUB_SHA"], "passed": passed}, indent=2) + "\n")
    return compiler_pilot.main(["--phase", "prepare"])


if __name__ == "__main__":
    sys.exit(main())
