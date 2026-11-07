"""Observe one contained-loan handback check with the original retained compiler.

This is a baseline observation, not a repaired compiler or a full-suite run.
The no-later-read probe and its expected refusal are frozen before execution.
"""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import signal
import stat
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
OUT = ROOT / "build/supplied-output-loan/ci-lender-handback-baseline"
CONFIG_SHA = "9385fc854d6c4ab4bc5bd6f58fe41735c62225356cc94bd8159da2a14b7d53fc"
SOURCE = "88acbdf1f4eb3b7eecb72b7e566d2699624f6686"
FLOOR = 800_000_000


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")


def git(*arguments):
    result = subprocess.run(["git", *arguments], cwd=ROOT, capture_output=True,
                            text=True, timeout=30, check=True)
    return result.stdout.strip()


def inputs(config):
    require(platform.system() == "Linux" and platform.machine().lower() == "x86_64",
            "the captured compiler requires Linux x86_64")
    revision = git("rev-parse", "HEAD")
    require(os.environ.get("GITHUB_SHA") == revision and not git("status", "--porcelain"),
            "the controller checkout is not its clean actual CI head")
    require(sha(HERE / "probe.json") == CONFIG_SHA, "the frozen probe setup changed")
    require(config["schema"] == "omega-contained-lender-handback-baseline-v1"
            and config["compiled_source"] == SOURCE, "the frozen source binding changed")
    expected = config["all_ten_compiler_input_objects"]
    require(set(expected) == {"omega-rust", "source/library", "source/psi", "source/omega",
            "bootstrap", "tools/benchmark", "Cargo.toml", "Cargo.lock",
            "rust-toolchain.toml", ".cargo"}, "the compiler input roster is incomplete")
    for name, identity in expected.items():
        row = subprocess.run(["git", "rev-parse", "--verify", "HEAD:" + name],
                             cwd=ROOT, capture_output=True, text=True, timeout=30)
        actual = row.stdout.strip() if row.returncode == 0 else None
        require(actual == identity and (identity is not None or name == ".cargo"),
                "the retained compiler inputs differ: " + name)
    library = {name: sha(ROOT / name) for name in
               git("ls-files", "-z", "--", "source/library").split("\0") if name}
    require(library == config["library_input_sha256"], "the captured library differs")
    for name in library:
        path = ROOT / name
        require(path.is_file() and not path.is_symlink()
                and path.resolve().is_relative_to(ROOT / "source/library"),
                "a captured library input is not an ordinary owned file")
    actual_modules = {p.relative_to(ROOT).as_posix()
                      for p in (ROOT / "source/library").rglob("*.omg") if p.is_file()}
    require(actual_modules == {p for p in library if p.endswith(".omg")},
            "there are extra or missing library modules")
    require(config["oracle"]["expected_check_exit"] == 1
            and config["oracle"]["later_read_is_absent"] is True,
            "the frozen handback oracle changed")
    return dict(revision=revision, tree=git("rev-parse", "HEAD^{tree}"),
                compiler_source=SOURCE, library_input_sha256=library)


def process_members(group):
    result = subprocess.run(["ps", "-axo", "pid=,pgid=,stat=,comm="],
                            capture_output=True, text=True, timeout=10, check=True)
    return [line.strip() for line in result.stdout.splitlines()
            if len(fields := line.split(None, 3)) == 4
            and int(fields[1]) == group and not fields[2].startswith("Z")]


def command(label, arguments, env, validate, timeout, allowed=(0,)):
    validate()
    require(shutil.disk_usage(OUT).free >= FLOOR, "the command capture floor is unavailable")
    stdout, stderr = OUT / (label + ".stdout"), OUT / (label + ".stderr")
    row = dict(arguments=list(map(str, arguments)), timeout_seconds=timeout, timed_out=False)
    process = None
    began = time.monotonic()
    try:
        with stdout.open("xb") as out, stderr.open("xb") as err:
            process = subprocess.Popen(row["arguments"], cwd=ROOT, env=env,
                                       stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                       start_new_session=True)
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                row["timed_out"] = True
    except BaseException as error:
        row["error"] = type(error).__name__ + ": " + str(error)
        raise
    finally:
        try:
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=10)
                row["exit"] = process.returncode
                deadline = time.monotonic() + 10
                remaining = process_members(process.pid)
                while remaining and time.monotonic() < deadline:
                    time.sleep(0.05)  # Drain this command's children; never poll remote jobs.
                    remaining = process_members(process.pid)
                row["remaining_owned_processes"] = remaining
                require(not remaining, "the command process group did not close")
        finally:
            row.update(seconds=round(time.monotonic() - began, 3),
                       stdout=str(stdout), stderr=str(stderr))
            for key, path in (("stdout", stdout), ("stderr", stderr)):
                if path.exists():
                    row[key + "_sha256"] = sha(path)
            write(OUT / (label + "-command.json"), row)
            validate()
    require(not row["timed_out"] and row.get("exit") in allowed,
            "the command did not produce a supported observation: " + label)
    return row


def normalized(name):
    path = PurePosixPath(name)
    require(name and not path.is_absolute() and str(path) == name
            and all(part not in (".", "..") for part in path.parts) and "\\" not in name,
            "an exported archive path is unsafe")
    return path


def restore_cli(config, archive):
    selected = config["cli_member"]
    with zipfile.ZipFile(archive) as data:
        entries = data.infolist()
        require(len(entries) == config["archive"]["member_count"]
                and len({row.filename for row in entries}) == len(entries),
                "the capture archive member roster changed")
        for entry in entries:
            normalized(entry.filename.rstrip("/") if entry.is_dir() else entry.filename)
        entry = data.getinfo(selected["archive_member"])
        require(not entry.is_dir() and not stat.S_ISLNK(entry.external_attr >> 16)
                and stat.S_IFMT(entry.external_attr >> 16) in (0, stat.S_IFREG)
                and entry.file_size == selected["bytes"], "the captured compiler export changed")
        require(shutil.disk_usage(OUT).free >= FLOOR + entry.file_size,
                "the compiler restoration floor is unavailable")
        destination = OUT / "retained-producer" / "omega"
        destination.parent.mkdir()
        digest = hashlib.sha256()
        with data.open(entry) as source, destination.open("xb") as target:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                target.write(block)
                digest.update(block)
        require(digest.hexdigest() == selected["sha256"], "the captured compiler bytes changed")
        os.chmod(destination, selected["mode"])
    return destination


def main():
    config = json.loads((HERE / "probe.json").read_text())
    initial = inputs(config)
    require(shutil.disk_usage(ROOT).free >= FLOOR, "the initial capture floor is unavailable")
    OUT.mkdir(parents=True, exist_ok=False)
    report = dict(schema="omega-contained-lender-handback-baseline-observation-v1",
                  observation_complete=False, expected_check_exit=1,
                  expected_exit_observed=False, oracle_satisfied=False,
                  test_ci=initial, capture_ci=config["capture_ci"],
                  capture_run=config["capture_run"], producer_rebuilt=False,
                  full_suite_run=False, semantic_attribution="pending-independent-review",
                  slot_acquired=False, slot_released=False, finally_guards_passed=False)
    cli, probe, before_profile, slot_path = None, None, None, None
    try:
        sys.path.insert(0, str(ROOT / "tools"))
        import build_slot
        import compiler_profile
        env = compiler_profile.optimized_environment()
        for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER",
                     "RUSTC_WORKSPACE_WRAPPER", "RUSTC", "RUSTDOC", "CARGO_BUILD_RUSTC"):
            require(not env.get(name), "an unreviewed environment override is present: " + name)
        for name in ("OMEGA_CALL_WRITE_TRACE", "OMEGA_STRUCT_TRACE", "OMEGA_COMPILER_CHECK_SITES"):
            env.pop(name, None)
        require(not any(name.startswith("OMEGA_") and name != "OMEGA_CORPUS_JOBS"
                        for name in env), "an unreviewed Omega environment override is present")
        env["OMEGA_LIBRARY_ROOT"] = str(ROOT / "source/library")
        def validate():
            require(inputs(config) == initial, "the bound checkout or library changed")
            if probe is not None:
                require(sha(probe / "main.omg") == config["probe"]["main_sha256"]
                        and sha(probe / "build.omg") == config["probe"]["build_sha256"],
                        "the frozen probe changed")
            if cli is not None:
                require(sha(cli) == config["cli_member"]["sha256"], "the retained compiler changed")
        toolchain = command("toolchain", ["rustc", "-Vv"], env, validate, 30)
        require(Path(toolchain["stdout"]).read_text().strip() == config["producer"]["toolchain"],
                "the captured toolchain differs")
        description = config["archive"]
        endpoint = "repos/CathedralOS/Omega/actions/artifacts/" + str(description["id"])
        metadata = command("capture-metadata", ["gh", "api", endpoint], env, validate, 60)
        actual = json.loads(Path(metadata["stdout"]).read_text())
        require(actual["id"] == description["id"] and not actual["expired"]
                and actual["name"] == description["name"]
                and actual["size_in_bytes"] == description["bytes"]
                and actual["digest"] == "sha256:" + description["sha256"]
                and actual["workflow_run"]["id"] == config["capture_run"]["run_id"]
                and actual["workflow_run"]["head_sha"] == config["capture_ci"]["revision"],
                "the retained artifact metadata differs")
        require(shutil.disk_usage(OUT).free >= FLOOR + description["bytes"],
                "the archive download floor is unavailable")
        download = command("capture-archive", ["gh", "api", endpoint + "/zip"],
                           env, validate, 600)
        archive = Path(download["stdout"])
        require(archive.stat().st_size == description["bytes"]
                and sha(archive) == description["sha256"], "the capture container bytes differ")
        cli = restore_cli(config, archive)
        probe = OUT / "lender-handback-without-later-read-probe"
        probe.mkdir()
        (probe / "main.omg").write_text(config["probe"]["main"])
        (probe / "build.omg").write_text(config["probe"]["build"])
        validate()
        report["restored_producer"] = dict(original_locator=config["cli_member"]["locator"],
                                            actual_path=str(cli), sha256=sha(cli))
        with build_slot.hold(ROOT) as (slot, waited):
            require(slot is not None and slot.is_file(), "a real build slot was not acquired")
            slot_path = slot
            report.update(slot_acquired=True, slot_path=str(slot), slot_wait_seconds=waited)
            try:
                before_profile = compiler_profile.require_optimized(cli)
                expected_profile = config["producer"]["cli_profile"]
                require(before_profile == dict(expected_profile, binary=str(cli)),
                        "the restored compiler effective profile differs")
                report["producer_profile_before"] = before_profile
                row = command("probe-check", [str(cli), "--check", str(probe / "main.omg")],
                              env, validate, 300, allowed=(0, 1))
                report["check"] = row
                report["observation_complete"] = True
                report["expected_exit_observed"] = row["exit"] == 1
                report["baseline_result"] = ("refused; inspect the actual handback diagnostic"
                                             if row["exit"] == 1 else
                                             "WRONG ACCEPTANCE; expected declared-lender restoration")
            finally:
                validate()
                after_profile = compiler_profile.require_optimized(cli)
                report["producer_profile_after"] = after_profile
                require(before_profile is not None and after_profile == before_profile,
                        "the compiler profile changed")
    except BaseException as error:
        report["error"] = type(error).__name__ + ": " + str(error)
    finally:
        # The hold context has exited before this outer finally, including failures.
        report["slot_released"] = slot_path is not None and not slot_path.exists()
        try:
            require(not report["slot_acquired"] or report["slot_released"],
                    "the acquired build slot was not released")
            require(inputs(config) == initial, "the final checkout or library changed")
            if probe is not None:
                require(sha(probe / "main.omg") == config["probe"]["main_sha256"]
                        and sha(probe / "build.omg") == config["probe"]["build_sha256"],
                        "the final probe bytes differ")
            if cli is not None:
                require(sha(cli) == config["cli_member"]["sha256"],
                        "the final retained compiler bytes differ")
            report["finally_guards_passed"] = True
        except BaseException as error:
            report["finally_guard_error"] = type(error).__name__ + ": " + str(error)
        write(OUT / "observation.json", report)
        print(json.dumps({key: report.get(key) for key in
              ("observation_complete", "expected_exit_observed", "oracle_satisfied", "baseline_result",
               "error", "finally_guard_error", "finally_guards_passed")}), flush=True)
    return 0 if (report["observation_complete"] and report["expected_exit_observed"]
                 and report["finally_guards_passed"] and "error" not in report) else 1


if __name__ == "__main__":
    raise SystemExit(main())
