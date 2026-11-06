"""Observe an explicitly selected normal-exit fixture through the product CLI.

The corpus owns private harness, stream and inspection-control oracles. This
consumer covers ordinary package review, plain named inspection/reload, and an
authored numeric exit on both engines with unchanged input bytes.
"""

import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import time

from sample_native_gate import DECISION, fixture_oracles, oracle
from sample_projects import project_at


def relative_project(root, selector):
    path = PurePosixPath(selector)
    if (path.as_posix() != selector or "\\" in selector
            or any(part in (".", "..") for part in path.parts)):
        raise ValueError(f"ordinary CLI project is not a normalized path: {selector}")
    if len(path.parts) >= 3 and path.parts[0] == "pass":
        relative = Path("tests/omega") / selector
    elif len(path.parts) >= 3 and path.parts[:2] == ("samples", "cli"):
        relative = Path(selector)
    else:
        raise ValueError("ordinary CLI projects must be exact pass/... or samples/cli/... paths")
    project = root.resolve() / relative
    if project.resolve() != project or not (project / "build.omg").is_file():
        raise ValueError(f"ordinary CLI project lacks its own tracked directory/build: {selector}")
    return relative


def expected_exit(root, fixture, target):
    relative = relative_project(root, fixture)
    project = root.resolve() / relative
    if relative.parts[:2] == ("samples", "cli"):
        declared = project_at(project, root)
        if declared.role not in ("program", "example", "library-consumer") or declared.entry != project / "main.omg":
            raise ValueError("ordinary CLI samples require a normal program with main.omg entry")
    if not (project / "build.omg").is_file():
        raise ValueError(f"ordinary CLI fixture has no build.omg: {fixture}")
    oracles = fixture_oracles(project, target)
    extra = [path.name for path in project.glob("expected_*")
             if not (path.name in ("expected_native_exit.txt", "expected_interpreter_exit.txt",
                                  "expected_terminal_inspections.txt")
                     or (path.name.startswith("expected_native_exit.") and path.suffix == ".txt"))]
    if extra or any(project.glob("entry_ram*")) or oracle(project)[1] is not None:
        raise ValueError("ordinary CLI exit fixtures cannot omit authored stream/inspection oracles")
    inspection_machines(project)
    if not oracles["exit"] and relative.parts[:2] == ("samples", "cli"):
        declared_exit, _ = oracle(project)
        if declared_exit is not None:
            return require_interpreter_exit(project, declared_exit)
    if not oracles["exit"]:
        raise ValueError(f"ordinary CLI fixture has no {target} numeric exit oracle: {fixture}")
    text = Path(oracles["exit"]).read_text(encoding="utf-8").strip()
    if not re.fullmatch(r"-?\d+", text):
        raise ValueError("ordinary CLI fixture requires an authored numeric exit oracle")
    return require_interpreter_exit(project, int(text))


def require_interpreter_exit(project, expected):
    path = project / "expected_interpreter_exit.txt"
    if path.is_file():
        text = path.read_text(encoding="utf-8").strip()
        if not re.fullmatch(r"-?\d+", text) or int(text) != expected:
            raise ValueError("ordinary CLI requires the authored native and interpreter exits to agree")
    return expected


def inspection_machines(project):
    path = project / "expected_terminal_inspections.txt"
    if not path.is_file():
        return ()
    rows = [line.split() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if not rows or any(len(row) != 1 for row in rows):
        raise ValueError("ordinary CLI inspection requires plain machine names; debug/runtime/control oracles remain with the corpus")
    return tuple(row[0] for row in rows)


def validate_inspection(machine, source, reloaded):
    selected = re.findall(r"^terminal selected_machine=(\S+) entry=", source, re.MULTILINE)
    identity = re.findall(r"^terminal program_fingerprint=([0-9a-f]{64})$", source, re.MULTILINE)
    restored = re.findall(r"^terminal program_fingerprint=([0-9a-f]{64})$", reloaded, re.MULTILINE)
    restored_selection = re.findall(r"^terminal selected_machine=(\S+) entry=", reloaded, re.MULTILINE)
    if selected != [machine] or restored_selection != ["artifact"] or len(identity) != 1 or restored != identity:
        raise ValueError("named inspection lost its selected machine or canonical artifact identity")
    return identity[0]


def validate_exits(code, stderr, expected):
    native = re.findall(r"^native exit (-?\d+)$", stderr, re.MULTILINE)
    terminal = [line for line in stderr.splitlines() if line.startswith("interp")]
    if (code != expected or native != [str(expected)]
            or terminal != [f"interp exit: {expected}"]):
        raise ValueError(
            f"ordinary CLI fixture lacks explicit native/Terminal exit agreement: expected {expected}, "
            f"wrapper {code}, native {native}, Terminal {[line[:1500] for line in terminal[:3]]}")


def observe(root, producer, fixture, output, *, dependency_projects=()):
    # The child runs inside the copied workspace, so project and library paths must be absolute.
    output = output.absolute()
    expected = expected_exit(root, fixture, producer.target)
    output.mkdir(parents=True, exist_ok=False)
    workspace = output / "workspace"
    project_relative = relative_project(root, fixture)
    if isinstance(dependency_projects, str):
        raise ValueError("dependency_projects requires a sequence of exact project selectors")
    projects = list(dict.fromkeys([
        project_relative, *(relative_project(root, selector) for selector in dependency_projects),
    ]))
    # Preserve authored local dependency paths in the copied workspace. Every
    # dependency file receives the same pre/post digest checks as the entry.
    paths = subprocess.check_output(
        ["git", "ls-files", "source/library", *(project.as_posix() for project in projects)],
        cwd=root, text=True).splitlines()
    for project in projects:
        if not {(project / "main.omg").as_posix(), (project / "build.omg").as_posix()}.issubset(paths):
            raise ValueError("ordinary CLI project entry and build must retain tracked source custody")
    inputs = {}
    for relative in paths:
        source, copied = root / relative, workspace / relative
        if source.is_symlink():
            raise ValueError("ordinary CLI fixture input escapes its tracked file custody")
        copied.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, copied)
        inputs[relative] = hashlib.sha256(source.read_bytes()).hexdigest()
    record = dict(fixture=fixture, target=producer.target, producer=producer.record(),
                  dependency_projects=[project.as_posix() for project in projects[1:]],
                  expected_exit=expected, passed=False, inputs=inputs, stages=[])
    env = dict(os.environ, OMEGA_LIBRARY_ROOT=str(workspace / "source/library"))
    project = workspace / project_relative

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
        print(f"ordinary CLI: {fixture}: {stage} (limit {timeout}s)", flush=True)
        input_path = project / "input.txt"
        input_file = input_path.open("rb") if stage == "both" and input_path.is_file() else None
        stdin = input_file if input_file is not None else subprocess.DEVNULL
        try:
            with (output / f"{stage}.stdout").open("wb") as out, (output / f"{stage}.stderr").open("wb") as err:
                command_env = dict(env, OMEGA_REVIEW_TIMINGS="1") if stage in ("update", "resume") else env
                result = subprocess.run(arguments, cwd=cwd, env=command_env, stdin=stdin,
                                        stdout=out, stderr=err, timeout=timeout)
            event["exit_code"] = result.returncode
            return result.returncode
        except subprocess.TimeoutExpired as error:
            raise ValueError(f"ordinary CLI {stage} exceeded its {timeout}s allowance") from error
        finally:
            if input_file is not None:
                input_file.close()
            event["seconds"] = round(time.monotonic() - started, 3)
            print(f"ordinary CLI: {fixture}: {stage} finished in {event['seconds']}s", flush=True)
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
        record["inspection_machines"] = inspection_machines(project)
        record["inspections"] = []
        for index, machine in enumerate(record["inspection_machines"]):
            artifact = output / "inspections" / f"{index}.oterm"
            artifact.parent.mkdir(exist_ok=True)
            stage = f"inspection-{index}-source"
            code = command(stage, [producer.cli, "inspect-terminal", "--machine", machine,
                                   "--target", producer.target, "--output", artifact,
                                   project / "main.omg"], workspace, 600)
            if code != 0 or not artifact.is_file():
                raise ValueError(f"authored named inspection refused: {machine} ({code})")
            reloaded = f"inspection-{index}-reload"
            code = command(reloaded, [producer.cli, "inspect-terminal", "--artifact", artifact], workspace, 240)
            if code != 0:
                raise ValueError(f"named inspection canonical reload refused: {machine} ({code})")
            identity = validate_inspection(machine,
                (output / f"{stage}.stdout").read_text(encoding="utf-8"),
                (output / f"{reloaded}.stdout").read_text(encoding="utf-8"))
            record["inspections"].append(dict(machine=machine, passed=True, program_fingerprint=identity,
                                             artifact_sha256=hashlib.sha256(artifact.read_bytes()).hexdigest()))
        runtime = output / "runtime"
        runtime.mkdir()
        code = command("both", [producer.cli, "run", "--both", "--keep", project / "main.omg"],
                       runtime, 240)
        text = (output / "both.stderr").read_text(encoding="utf-8", errors="replace")
        kept = re.search(r"^build dir kept: (.+)$", text, re.MULTILINE)
        if kept:
            shutil.copytree(Path(kept.group(1).strip()), output / "products")
            record["products"] = {
                path.relative_to(output / "products").as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                for path in sorted((output / "products").rglob("*")) if path.is_file()
            }
        validate_exits(code, text, expected)
        if not kept or not (output / "products/program.oterm").is_file():
            raise ValueError("ordinary CLI fixture lost its retained Terminal product")
        record.update(native_exit=expected, terminal_exit=expected)
        artifact = output / "products/program.oterm"
        artifact_digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
        code = command("runtime-artifact-reload",
                       [producer.cli, "inspect-terminal", "--artifact", artifact], workspace, 240)
        if code != 0 or hashlib.sha256(artifact.read_bytes()).hexdigest() != artifact_digest:
            raise ValueError(f"the exact kept runtime artifact refused canonical reload ({code})")
        record["runtime_artifact_reload"] = dict(
            path=str(artifact), sha256=artifact_digest, exit_code=code,
            same_as_kept_product=record["products"]["program.oterm"] == artifact_digest)
        if not record["runtime_artifact_reload"]["same_as_kept_product"]:
            raise ValueError("the kept runtime artifact changed before canonical reload")
        record["passed"] = True
        return record
    except (OSError, ValueError) as error:
        record["error"] = str(error)
        return record
    finally:
        (output / "observation.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
