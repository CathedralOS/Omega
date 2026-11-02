"""Observe an explicitly selected normal-exit fixture through the product CLI.

The corpus remains the owner of private harness/stream/inspection oracles. This
consumer covers ordinary package review and an authored numeric exit on both
engines, with the CLI's explicit host environment and unchanged input bytes.
"""

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

from sample_native_gate import DECISION, fixture_oracles, oracle


def expected_exit(root, fixture, target):
    project = root / "tests" / "omega" / fixture
    if not (project / "build.omg").is_file():
        raise ValueError(f"ordinary CLI fixture has no build.omg: {fixture}")
    oracles = fixture_oracles(project, target)
    extra = [path.name for path in project.glob("expected_*")
             if not (path.name == "expected_native_exit.txt"
                     or (path.name.startswith("expected_native_exit.") and path.suffix == ".txt"))]
    if extra or any(project.glob("entry_ram*")) or oracle(project)[1] is not None:
        raise ValueError("ordinary CLI exit fixtures cannot omit authored stream/inspection oracles")
    if not oracles["exit"]:
        raise ValueError(f"ordinary CLI fixture has no {target} numeric exit oracle: {fixture}")
    text = Path(oracles["exit"]).read_text(encoding="utf-8").strip()
    if not re.fullmatch(r"-?\d+", text):
        raise ValueError("ordinary CLI fixture requires an authored numeric exit oracle")
    return int(text)


def validate_exits(code, stderr, expected):
    native = re.findall(r"^native exit (-?\d+)$", stderr, re.MULTILINE)
    terminal = [line for line in stderr.splitlines() if line.startswith("interp")]
    if (code != expected or native != [str(expected)]
            or terminal != [f"interp exit: {expected}"]):
        raise ValueError(
            f"ordinary CLI fixture lacks explicit native/Terminal exit agreement: expected {expected}, "
            f"wrapper {code}, native {native}, Terminal {[line[:1500] for line in terminal[:3]]}")


def observe(root, producer, fixture, output):
    expected = expected_exit(root, fixture, producer.target)
    output.mkdir(parents=True, exist_ok=False)
    workspace = output / "workspace"
    relative_project = Path("tests/omega") / fixture
    paths = subprocess.check_output(
        ["git", "ls-files", "source/library", relative_project.as_posix()],
        cwd=root, text=True).splitlines()
    inputs = {}
    for relative in paths:
        source, copied = root / relative, workspace / relative
        if source.is_symlink():
            raise ValueError("ordinary CLI fixture input escapes its tracked file custody")
        copied.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, copied)
        inputs[relative] = hashlib.sha256(source.read_bytes()).hexdigest()
    record = dict(fixture=fixture, target=producer.target, producer=producer.record(),
                  expected_exit=expected, passed=False, inputs=inputs, stages=[])
    env = dict(os.environ, OMEGA_LIBRARY_ROOT=str(workspace / "source/library"))
    project = workspace / relative_project

    def verify():
        producer.verify()
        for relative, digest in inputs.items():
            for directory in (root, workspace):
                if hashlib.sha256((directory / relative).read_bytes()).hexdigest() != digest:
                    raise ValueError(f"ordinary CLI fixture input changed: {relative}")

    def command(stage, arguments, cwd, timeout):
        verify()
        event = dict(stage=stage, arguments=[str(arg) for arg in arguments], timeout=timeout)
        record["stages"].append(event)
        started = time.monotonic()
        input_path = project / "input.txt"
        input_file = input_path.open("rb") if stage == "both" and input_path.is_file() else None
        stdin = input_file if input_file is not None else subprocess.DEVNULL
        try:
            with (output / f"{stage}.stdout").open("wb") as out, (output / f"{stage}.stderr").open("wb") as err:
                result = subprocess.run(arguments, cwd=cwd, env=env, stdin=stdin,
                                        stdout=out, stderr=err, timeout=timeout)
            event["exit_code"] = result.returncode
            return result.returncode
        except subprocess.TimeoutExpired as error:
            raise ValueError(f"ordinary CLI {stage} exceeded its {timeout}s allowance") from error
        finally:
            if input_file is not None:
                input_file.close()
            event["seconds"] = round(time.monotonic() - started, 3)
            verify()

    try:
        code = command("update", [producer.cli, "update", "--project", project,
                                  "--target", producer.target], workspace, 600)
        review = project / "build/package-manager" / f"review-{producer.target}.txt"
        if review.is_file():
            original = review.read_bytes()
            (output / "review.original.txt").write_bytes(original)
            accepted, count = DECISION.subn(rb"\1 accept", original)
            if count:
                if code != 3:
                    raise ValueError("pending ordinary review decisions lack ReviewRequired status")
                review.write_bytes(accepted)
                (output / "review.accepted.txt").write_bytes(accepted)
                record["accepted_pending_rows"] = count
                code = command("resume", [producer.cli, "update", "--project", project,
                                          "--resume"], workspace, 600)
        if code != 0 or not (project / "omega.lock").is_file():
            raise ValueError(f"ordinary CLI package admission refused ({code})")
        (output / "accepted.omega.lock").write_bytes((project / "omega.lock").read_bytes())
        runtime = output / "runtime"
        runtime.mkdir()
        code = command("both", [producer.cli, "run", "--both", "--keep", project / "main.omg"],
                       runtime, 240)
        validate_exits(code, (output / "both.stderr").read_text(encoding="utf-8", errors="replace"), expected)
        record.update(passed=True, native_exit=expected, terminal_exit=expected)
        return record
    except (OSError, ValueError) as error:
        record["error"] = str(error)
        return record
    finally:
        (output / "observation.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
