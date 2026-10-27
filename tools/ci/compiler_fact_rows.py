"""Public CLI evidence for self-contained fact rows and their pinned controls.

The caller supplies the fact and external provider resources. Preparation copies
only authored source/oracle files, then obtains each replay's own package lock.
Measurements retain those exact inputs and both engine products; a zero CLI exit
alone never establishes execution or an intended rejection.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

import compiler_pilot
import sample_native_gate

PLACES = ("local", "field", "parameter", "call_result", "match_join", "const_name",
          "other_state", "other_machine", "target_body")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def authored_files(project):
    return sorted(path for path in project.iterdir() if path.is_file()
                  and (path.suffix == ".omg" or path.name in {
                      "sample.json", "input.txt", "expected.txt", "expected_native_exit.txt",
                      "expected_interpreter_exit.txt", "expected_stdout.txt", "expected_stderr.txt"}))


def fixtures(root, fact, prefix):
    """Pin every authored place/twin before any compiler build or measurement."""
    positive, negative = [], []
    for place in PLACES:
        for tier, suffix, source in (("pass", "exit", fact / place),
                                     ("fail", "rejected", fact / place / "invalid")):
            name = f"{tier}/facts/{prefix}_through_{place}_{suffix}"
            pinned = root / "tests/omega" / name
            required = ["main.omg", "build.omg"] + (["expected.txt"] if tier == "fail" else [])
            for filename in required:
                if not (source / filename).is_file() or not (pinned / filename).is_file():
                    raise ValueError(f"{name}: required authored place/twin input missing: {filename}")
                if (source / filename).read_bytes() != (pinned / filename).read_bytes():
                    raise ValueError(f"{name}: pinned input differs from authored {filename}")
            if tier == "pass":
                if "Expected exit: 0" not in (source / "main.omg").read_text(encoding="utf-8"):
                    raise ValueError(f"{name}: fact has no authored zero-exit oracle")
                positive.append(name)
            else:
                fragments = (source / "expected.txt").read_text(encoding="utf-8").splitlines()
                if not any(line.strip() and not line.startswith("#") for line in fragments):
                    raise ValueError(f"{name}: empty rejection oracle")
                negative.append(name)
    if not (fact / "main.omg").is_file() or not (fact / "build.omg").is_file():
        raise ValueError("fact root has no authored executable entry")
    for source in [fact] + [fact / place for place in PLACES]:
        if "Expected exit: 0" not in (source / "main.omg").read_text(encoding="utf-8"):
            raise ValueError(f"{source}: fact has no authored zero-exit oracle")
        for name in ("expected_native_exit.txt", "expected_interpreter_exit.txt"):
            path = source / name
            if path.is_file() and path.read_text(encoding="utf-8").strip() != "0":
                raise ValueError(f"{source}: fact exit oracle is not zero")
    return tuple(positive), tuple(negative)


def prepare(output, fact, producer):
    """Retain successful and failed preparation separately from frozen runs."""
    producer.verify()
    records = []
    for place in ("root",) + PLACES:
        authored = fact if place == "root" else fact / place
        project = output / "samples" / place
        project.mkdir(parents=True, exist_ok=False)
        sources = {}
        for source in authored_files(authored):
            shutil.copy2(source, project / source.name)
            sources[str(source)] = digest(source)
        failure, text = sample_native_gate.accept_review(
            str(producer.cli), str(project), producer.target, 180)
        (project / "preparation.log").write_text(text, encoding="utf-8")
        record = dict(place=place, project=str(project), authored_inputs=sources,
                      preparation=failure,
                      inputs={str(path): digest(path) for path in authored_files(project)})
        lock = project / "omega.lock"
        if not failure and not lock.is_file():
            raise ValueError(f"{place}: accepted package has no retained lock")
        if lock.is_file():
            record["inputs"][str(lock)] = digest(lock)
        records.append(record)
    inventory = dict(producer=producer.record(), rows=records)
    (output / "prepared-samples.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
    return inventory


def validate_inputs(row):
    for path, expected in {**row["authored_inputs"], **row["inputs"]}.items():
        if digest(path) != expected:
            raise ValueError(f"frozen authored/replay input changed: {path}")


def execute(arguments, project, stem, supplied=b""):
    """Retain raw streams even on timeout, and drain the complete child tree."""
    with subprocess.Popen(arguments, cwd=compiler_pilot.ROOT, stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          start_new_session=(os.name != "nt")) as process:
        timed_out = False
        try:
            stdout, stderr = process.communicate(input=supplied, timeout=180)
        except subprocess.TimeoutExpired:
            timed_out = True
            sample_native_gate.kill_process_tree(process)
            stdout, stderr = process.communicate()
        (project / (stem + ".stdout")).write_bytes(stdout)
        (project / (stem + ".stderr")).write_bytes(stderr)
        if timed_out:
            raise ValueError(f"{stem}: process tree exceeded 180 seconds")
        return subprocess.CompletedProcess(arguments, process.returncode, stdout, stderr)


def measure(output, inventory, producer):
    producer.verify()
    observations = []
    report = dict(producer=producer.record(), observations=observations, passed=False)
    try:
        for row in inventory["rows"]:
            observation = dict(place=row["place"], passed=False)
            observations.append(observation)
            if row["preparation"]:
                observation["error"] = "ordinary package preparation failed: " + row["preparation"]
                continue
            validate_inputs(row)
            project = Path(row["project"])
            arguments = [str(producer.cli), "run", "--both", "--keep", str(project / "main.omg")]
            started = time.monotonic()
            try:
                supplied = project / "input.txt"
                result = execute(arguments, project, "run", supplied.read_bytes() if supplied.is_file() else b"")
                text = (result.stdout + result.stderr).decode("utf-8", errors="replace").replace("\r\n", "\n")
                observation.update(argv=arguments, exit=result.returncode,
                                   seconds=time.monotonic() - started,
                                   native_zero=re.search(r"^native exit 0$", text, re.M) is not None,
                                   terminal_zero=re.search(r"^interp exit: 0$", text, re.M) is not None)
                kept = re.search(r"^build dir kept: (.+)$", text, re.M)
                if kept:
                    shutil.copytree(Path(kept.group(1).strip()), project / "products")
                if result.returncode or not observation["native_zero"] or not observation["terminal_zero"] or not kept:
                    raise ValueError("missing native/Terminal zero exits or retained products")
                for stream in ("stdout", "stderr"):
                    oracle = project / ("expected_" + stream + ".txt")
                    if oracle.is_file():
                        actual = result.stdout if stream == "stdout" else sample_native_gate.program_stderr(result.stderr.decode("utf-8")).encode("utf-8")
                        terminal = project / "products" / ("terminal." + stream)
                        if actual != oracle.read_bytes() or terminal.read_bytes() != oracle.read_bytes():
                            raise ValueError(f"{stream}: public CLI output capture needs exact native/Terminal oracle agreement")
                canonical_arguments = [str(producer.cli), "inspect-terminal", "--artifact", str(project / "products/program.oterm")]
                canonical = execute(canonical_arguments, project, "canonical")
                observation.update(canonical_argv=canonical_arguments, canonical_exit=canonical.returncode)
                if canonical.returncode:
                    raise ValueError("canonical Terminal artifact inspection failed")
                observation["products"] = {str(path.relative_to(project)): digest(path)
                                           for path in project.rglob("*") if path.is_file()}
                observation["passed"] = True
            except (OSError, ValueError, subprocess.SubprocessError) as error:
                observation["error"] = str(error)
            validate_inputs(row)
        producer.verify()
        report["passed"] = len(observations) == 10 and all(row["passed"] for row in observations)
    finally:
        (output / "sample-observations.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if not report["passed"]:
        raise ValueError("one or more public fact places failed; raw observations retained")
    return report
