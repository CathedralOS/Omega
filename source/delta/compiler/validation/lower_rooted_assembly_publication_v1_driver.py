#!/usr/bin/env python3
"""Prepare, inspect, and finalize one Delta assembly publication attempt.

This is replaceable orchestration.  ``prepare`` reconstructs the short-lived
lower-rooted tools and writes explicit runner scripts; it does not elaborate or
execute the compiler.  Those scripts state the translator, transport, and
interpreter commands literally.  Public ``stage-start``/``stage-finish``
commands only keep custody markers around those commands, while ``stage-watch``
enforces the declared wall-time ceiling.  Stage finish/replay applies each
output's declared byte ceiling before hashing it into custody.  ``status`` is
read-only.  ``finalize`` performs only the independent decoding/reconstruction
already owned by the V1 verifier.
"""

from __future__ import annotations

import hashlib
import json
import os
import platform
import secrets
import signal
import shlex
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import lower_rooted_assembly_publication_v1 as publication
import publication_support as support


HERE = Path(__file__).resolve().parent
DELTA = HERE.parents[1]
REPOSITORY = HERE.parents[3]
MANIFEST = HERE / "source-closures/canonical-compiler-v1.json"
LOCATIONS = HERE / "source-closures/canonical-compiler-v1.locations.json"
ALPHA = REPOSITORY / "source/alpha"
ASSEMBLER = ALPHA / "assembler/beta_arm64_macos"
ALPHA_SEED = ALPHA / "alpha_arm64_macos"
BC_TAPE = REPOSITORY / "source/beta/compiler/artifacts/beta_compiler_bytecode.tape"
TRANSLATOR_SOURCE = REPOSITORY / "source/delta/meaning/delta2gamma.beta"
INTERPRETER_SOURCE = REPOSITORY / "source/gamma/interp.beta"
PACKER = REPOSITORY / "source/delta/meaning/encode-gamma-input.py"

PLAN_SCHEMA = "omega.delta-assembly-publication-attempt.v1"
MARKER_SCHEMA = "omega.delta-assembly-publication-stage.v1"
STATUS_SCHEMA = "omega.delta-assembly-publication-attempt-status.v1"
STAGES = ("elaboration", "packing", "execution-0", "execution-1")
MAX_PLAN_BYTES = 65_536
MAX_BUILD_SECONDS = 300
STAGE_CEILINGS = {
    "elaboration": 900,
    "packing": 300,
    "execution-0": 43_200,
    "execution-1": 43_200,
}
STAGE_OUTPUT_CEILINGS = {
    "elaboration": (publication.MAX_TEMPLATE, publication.MAX_DOCUMENT),
    "packing": (publication.MAX_CLOSED_GAMMA, publication.MAX_DOCUMENT),
    "execution-0": (publication.MAX_GAMMA_OBSERVATION, publication.MAX_DOCUMENT),
    "execution-1": (publication.MAX_GAMMA_OBSERVATION, publication.MAX_DOCUMENT),
}
STAGE_INPUT_CEILINGS = {
    "elaboration": (
        publication.MAX_ALPHA_HOST_ARTIFACT,
        publication.MAX_SOURCE_IMAGE,
    ),
    "packing": (
        publication.MAX_TEMPLATE,
        publication.MAX_SOURCE_IMAGE,
        None,
    ),
    "execution-0": (
        publication.MAX_ALPHA_HOST_ARTIFACT,
        publication.MAX_CLOSED_GAMMA,
    ),
    "execution-1": (
        publication.MAX_ALPHA_HOST_ARTIFACT,
        publication.MAX_CLOSED_GAMMA,
    ),
}

# None is deliberate: these source files have exact identity custody but no
# independently declared byte ceiling. Do not manufacture one in orchestration.
PLAN_INPUT_CEILINGS = {
    "alpha_vm_semantics": None,
    "alpha_vm_source": None,
    "alpha_vm_seed": publication.MAX_ALPHA_HOST_ARTIFACT,
    "alpha_stamping_source": None,
    "assembler_source": None,
    "assembler_artifact": publication.MAX_ALPHA_HOST_ARTIFACT,
    "assembler_tape": publication.MAX_TAPE,
    "beta_compiler_source": None,
    "beta_compiler_tape": publication.MAX_TAPE,
    "decoder_source": None,
    "driver_source": None,
    "interpreter_source": None,
    "translator_tape": publication.MAX_TAPE,
    "interpreter_tape": publication.MAX_TAPE,
    "manifest": publication.MAX_DOCUMENT,
    "locations": publication.MAX_DOCUMENT,
    "packer_source": None,
    "publication_support_source": None,
    "receipt_verifier_source": None,
    "source_closure_verifier_source": None,
    "translator_source": None,
    "translator_executable": publication.MAX_ALPHA_HOST_ARTIFACT,
    "interpreter_executable": publication.MAX_ALPHA_HOST_ARTIFACT,
    "source_image": publication.MAX_SOURCE_IMAGE,
}


class DriverError(Exception):
    status = 251


class DriverUsageError(DriverError):
    status = 2


class DriverPending(DriverError):
    status = 3


class DriverResourceError(DriverError):
    status = 252


def fail(message: str) -> None:
    raise DriverError(message)


def usage(message: str) -> None:
    raise DriverUsageError(message)


def canonical_json(value: object) -> bytes:
    return publication.canonical_json(value, pretty=True)


def bounded_read(path: Path, role: str, limit: int) -> bytes:
    extent = path.stat().st_size
    if extent > limit:
        raise DriverResourceError(f"{role} byte ceiling")
    with path.open("rb") as stream:
        before = os.fstat(stream.fileno())
        if before.st_size > limit:
            raise DriverResourceError(f"{role} byte ceiling")
        raw = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
    if len(raw) > limit or after.st_size > limit:
        raise DriverResourceError(f"{role} byte ceiling")
    if (
        before.st_dev != after.st_dev
        or before.st_ino != after.st_ino
        or before.st_size != after.st_size
        or len(raw) != after.st_size
    ):
        fail(f"{role} changed while reading")
    current = path.stat()
    if current.st_size > limit:
        raise DriverResourceError(f"{role} byte ceiling")
    if (
        current.st_dev != after.st_dev
        or current.st_ino != after.st_ino
        or current.st_size != after.st_size
    ):
        fail(f"{role} path changed while reading")
    return raw


def identity(path: Path, role: str, limit: int | None = None) -> dict:
    extent = path.stat().st_size
    if limit is None:
        raw = path.read_bytes()
        if len(raw) != extent:
            fail(f"{role} changed while reading")
    else:
        raw = bounded_read(path, role, limit)
    return {
        "byte_length": len(raw),
        "role": role,
        "sha256": hashlib.sha256(raw).hexdigest(),
    }


def atomic_write(path: Path, raw: bytes, mode: int = 0o600) -> None:
    if path.exists():
        fail(f"refuse overwrite: {path.name}")
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    except BaseException:
        try:
            path.unlink()
        except FileNotFoundError:
            pass
        raise


def load_canonical(path: Path, context: str) -> dict:
    raw = bounded_read(path, context, MAX_PLAN_BYTES)
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"{context} JSON: {error}")
    if not isinstance(value, dict) or raw != canonical_json(value):
        fail(f"{context} canonical JSON")
    return value


def strict(value: object, keys: set[str], context: str) -> dict:
    if not isinstance(value, dict) or set(value) != keys:
        fail(f"{context} fields")
    return value


def safe_new_directory(path: Path) -> Path:
    if not path.is_absolute():
        usage("evidence directory must be absolute")
    resolved = path.resolve(strict=False)
    forbidden = {Path("/"), Path.home().resolve(), REPOSITORY.resolve()}
    if resolved in forbidden or resolved.exists():
        usage("evidence directory must be a new, narrow path")
    resolved.mkdir(parents=False, mode=0o700)
    return resolved


def run_short(argv: list[str], stdin: bytes, context: str) -> bytes:
    try:
        result = subprocess.run(
            argv,
            input=stdin,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=MAX_BUILD_SECONDS,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise DriverResourceError(f"{context} time ceiling") from error
    if result.returncode != 0 or result.stderr:
        fail(f"{context} status/diagnostics")
    return result.stdout


def stamp(tape: Path, output: Path) -> None:
    script = (
        'set -eu; . "$1/source/alpha/seed_env.sh"; '
        'stamp_seed "$2" "$3" "$4" >/dev/null'
    )
    result = subprocess.run(
        ["sh", "-c", script, "delta-publication", str(REPOSITORY),
         str(tape), str(ALPHA_SEED), str(output)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=MAX_BUILD_SECONDS,
        check=False,
    )
    if result.returncode != 0 or result.stdout or result.stderr:
        fail("Alpha seed stamping")


def embedded_tape(executable: Path) -> bytes:
    raw = bounded_read(
        executable, "Alpha host artifact", publication.MAX_ALPHA_HOST_ARTIFACT
    )
    offset = 32_768
    if len(raw) < offset + 4:
        fail("Alpha artifact tape extent")
    length = int.from_bytes(raw[offset : offset + 4], "little")
    if length > publication.MAX_TAPE or offset + 4 + length > len(raw):
        fail("Alpha artifact tape length")
    return raw[offset + 4 : offset + 4 + length]


def build_tools(root: Path) -> None:
    artifacts = root / "artifacts"
    artifacts.mkdir(mode=0o700)
    assembler_tape = artifacts / "assembler.tape"
    assembler_tape.write_bytes(embedded_tape(ASSEMBLER))

    bc_executable = artifacts / "bc.exe"
    stamp(BC_TAPE, bc_executable)
    translator_assembly = run_short(
        [str(bc_executable)], TRANSLATOR_SOURCE.read_bytes(), "build delta2gamma"
    )
    interpreter_assembly = run_short(
        [str(bc_executable)], INTERPRETER_SOURCE.read_bytes(), "build Gamma interpreter"
    )
    translator_tape = artifacts / "delta_to_gamma_bytecode.tape"
    interpreter_tape = artifacts / "gamma_interpreter_bytecode.tape"
    translator_tape.write_bytes(
        run_short([str(ASSEMBLER)], translator_assembly, "assemble delta2gamma")
    )
    interpreter_tape.write_bytes(
        run_short([str(ASSEMBLER)], interpreter_assembly, "assemble Gamma interpreter")
    )
    if len(translator_tape.read_bytes()) > publication.MAX_TAPE:
        raise DriverResourceError("translator tape ceiling")
    if len(interpreter_tape.read_bytes()) > publication.MAX_TAPE:
        raise DriverResourceError("interpreter tape ceiling")
    stamp(translator_tape, artifacts / "delta2gamma.exe")
    stamp(interpreter_tape, artifacts / "interp.exe")


def plan_inputs(root: Path) -> dict:
    artifacts = root / "artifacts"
    def retained(name: str, path: Path, role: str) -> dict:
        return identity(path, role, PLAN_INPUT_CEILINGS[name])

    values = {
        "alpha_vm_semantics": retained("alpha_vm_semantics", ALPHA / "SEMANTICS.md", "alpha_vm_written_semantics"),
        "alpha_vm_source": retained("alpha_vm_source", ALPHA / "alpha_arm64_macos.s", "alpha_vm_host_source"),
        "alpha_vm_seed": retained("alpha_vm_seed", ALPHA_SEED, "alpha_vm_committed_host_seed"),
        "alpha_stamping_source": retained("alpha_stamping_source", ALPHA / "seed_env.sh", "alpha_vm_tape_stamping_source"),
        "assembler_source": retained("assembler_source", ALPHA / "assembler/assembler.alpha", "alpha_assembler_source"),
        "assembler_artifact": retained("assembler_artifact", ASSEMBLER, "alpha_assembler_committed_host_artifact"),
        "assembler_tape": retained("assembler_tape", artifacts / "assembler.tape", "alpha_assembler_tape"),
        "beta_compiler_source": retained("beta_compiler_source", REPOSITORY / "source/beta/compiler/bc.beta", "beta_compiler_source"),
        "beta_compiler_tape": retained("beta_compiler_tape", BC_TAPE, "beta_compiler_persisted_fixed_point_tape"),
        "decoder_source": retained("decoder_source", publication.DECODER_SOURCE, "gamma_output_decoder_source"),
        "driver_source": retained("driver_source", Path(__file__).resolve(), "attempt_driver_source"),
        "interpreter_source": retained("interpreter_source", INTERPRETER_SOURCE, "gamma_interpreter_beta_source"),
        "translator_tape": retained("translator_tape", artifacts / "delta_to_gamma_bytecode.tape", "delta_to_gamma_tape"),
        "interpreter_tape": retained("interpreter_tape", artifacts / "gamma_interpreter_bytecode.tape", "gamma_interpreter_tape"),
        "manifest": retained("manifest", MANIFEST, "delta_source_closure_manifest"),
        "locations": retained("locations", LOCATIONS, "delta_source_closure_locations"),
        "packer_source": retained("packer_source", PACKER, "gamma_input_packer_source"),
        "publication_support_source": retained("publication_support_source", Path(support.__file__).resolve(), "assembly_validator_source"),
        "receipt_verifier_source": retained("receipt_verifier_source", Path(publication.__file__).resolve(), "receipt_verifier_source"),
        "source_closure_verifier_source": retained("source_closure_verifier_source", HERE / "source_closure_snapshot_v1.py", "source_closure_verifier_source"),
        "translator_source": retained("translator_source", TRANSLATOR_SOURCE, "delta_to_gamma_beta_source"),
        # Signed executable hashes are attempt-local mutation guards only.  They
        # are deliberately absent from the publication receipt.
        "translator_executable": retained("translator_executable", artifacts / "delta2gamma.exe", "attempt_local_translator"),
        "interpreter_executable": retained("interpreter_executable", artifacts / "interp.exe", "attempt_local_interpreter"),
        "source_image": retained("source_image", root / "canonical-source.lf", "delta_compiler_canonical_lf_image"),
    }
    if set(values) != set(PLAN_INPUT_CEILINGS):
        fail("attempt input ceiling map")
    return values


def write_runners(root: Path, attempt_id: str) -> None:
    python = shlex.quote(sys.executable)
    driver = shlex.quote(str(Path(__file__).resolve()))
    evidence = shlex.quote(str(root))
    token = shlex.quote(attempt_id)
    commands = {
        "elaboration": (
            f"{shlex.quote(str(root / 'artifacts/delta2gamma.exe'))}"
            f" < {shlex.quote(str(root / 'canonical-source.lf'))}"
            f" > {shlex.quote(str(root / 'template.gamma'))}"
            f" 2> {shlex.quote(str(root / 'elaboration.stderr'))}"
        ),
        "packing": (
            f"{python} -B {shlex.quote(str(PACKER))} inject"
            f" {shlex.quote(str(root / 'template.gamma'))}"
            f" {shlex.quote(str(root / 'canonical-source.lf'))}"
            f" {shlex.quote(str(root / 'closed.gamma'))}"
            f" > /dev/null 2> {shlex.quote(str(root / 'packing.stderr'))}"
        ),
        "execution-0": (
            f"{shlex.quote(str(root / 'artifacts/interp.exe'))}"
            f" < {shlex.quote(str(root / 'closed.gamma'))}"
            f" > {shlex.quote(str(root / 'execution-0.raw'))}"
            f" 2> {shlex.quote(str(root / 'execution-0.stderr'))}"
        ),
        "execution-1": (
            f"{shlex.quote(str(root / 'artifacts/interp.exe'))}"
            f" < {shlex.quote(str(root / 'closed.gamma'))}"
            f" > {shlex.quote(str(root / 'execution-1.raw'))}"
            f" 2> {shlex.quote(str(root / 'execution-1.stderr'))}"
        ),
    }
    for stage, command in commands.items():
        name = f"run-{stage}.sh"
        # The compiler/transport command and every redirection remain visible
        # and independently runnable. The driver calls that bracket it own only
        # marker custody and the attempt-local wall-time ceiling.
        script = f"""#!/bin/sh
set -u
PYTHON={python}
DRIVER={driver}
EVIDENCE={evidence}
STAGE={shlex.quote(stage)}
TOKEN={token}

"$PYTHON" -B "$DRIVER" stage-start "$EVIDENCE" "$STAGE" "$TOKEN" || exit $?

set +e
{command} &
stage_pid=$!
"$PYTHON" -B "$DRIVER" stage-watch "$EVIDENCE" "$STAGE" "$TOKEN" "$stage_pid" &
watch_pid=$!
wait "$stage_pid"
stage_status=$?
kill "$watch_pid" 2>/dev/null || :
wait "$watch_pid" 2>/dev/null || :
if [ -f "$EVIDENCE/$STAGE.timeout.json" ]; then
  stage_status=124
fi
"$PYTHON" -B "$DRIVER" stage-finish \
  "$EVIDENCE" "$STAGE" "$TOKEN" "$stage_status"
custody_status=$?
set -e
[ "$custody_status" -eq 0 ] || exit "$custody_status"
exit "$stage_status"
"""
        atomic_write(
            root / name,
            script.encode(),
            mode=0o700,
        )


def prepare(path: Path) -> dict:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        usage("prepare requires Darwin arm64")
    for required in (ASSEMBLER, ALPHA_SEED, BC_TAPE, MANIFEST, LOCATIONS):
        if not required.is_file():
            usage(f"missing lower-rooted input: {required}")
    root = safe_new_directory(path)
    try:
        build_tools(root)
        image = support.materialize_canonical_image(MANIFEST, LOCATIONS, {"delta": DELTA})
        (root / "canonical-source.lf").write_bytes(image)
        now = time.time_ns()
        plan = {
            "attempt_id": secrets.token_hex(32),
            "inputs": plan_inputs(root),
            "prepared_epoch_ns": now,
            "publication_id": publication.PUBLICATION_ID,
            "schema": PLAN_SCHEMA,
            "stage_ceilings_seconds": STAGE_CEILINGS,
        }
        atomic_write(root / "attempt.json", canonical_json(plan))
        write_runners(root, plan["attempt_id"])
        return plan
    except BaseException:
        # A failed prepare cannot be mistaken for an attempt: attempt.json is
        # written last.  The narrow incomplete directory is retained for audit.
        raise


def load_plan(root: Path) -> dict:
    plan = load_canonical(root / "attempt.json", "attempt")
    strict(
        plan,
        {"attempt_id", "inputs", "prepared_epoch_ns", "publication_id", "schema", "stage_ceilings_seconds"},
        "attempt",
    )
    if (
        plan["schema"] != PLAN_SCHEMA
        or plan["publication_id"] != publication.PUBLICATION_ID
        or not isinstance(plan["attempt_id"], str)
        or len(plan["attempt_id"]) != 64
        or any(ch not in "0123456789abcdef" for ch in plan["attempt_id"])
        or isinstance(plan["prepared_epoch_ns"], bool)
        or not isinstance(plan["prepared_epoch_ns"], int)
        or plan["prepared_epoch_ns"] <= 0
        or plan["prepared_epoch_ns"] > time.time_ns() + 1_000_000_000
        or plan["stage_ceilings_seconds"] != STAGE_CEILINGS
    ):
        fail("attempt identity")
    expected = plan_inputs(root)
    if plan["inputs"] != expected:
        fail("attempt input custody")
    artifacts = root / "artifacts"
    if embedded_tape(artifacts / "delta2gamma.exe") != (artifacts / "delta_to_gamma_bytecode.tape").read_bytes():
        fail("translator executable/tape relation")
    if embedded_tape(artifacts / "interp.exe") != (artifacts / "gamma_interpreter_bytecode.tape").read_bytes():
        fail("interpreter executable/tape relation")
    return plan


def stage_paths(root: Path, stage: str) -> tuple[list[Path], list[Path]]:
    if stage == "elaboration":
        return (
            [root / "artifacts/delta2gamma.exe", root / "canonical-source.lf"],
            [root / "template.gamma", root / "elaboration.stderr"],
        )
    if stage == "packing":
        return (
            [root / "template.gamma", root / "canonical-source.lf", PACKER],
            [root / "closed.gamma", root / "packing.stderr"],
        )
    if stage in ("execution-0", "execution-1"):
        ordinal = stage[-1]
        return (
            [root / "artifacts/interp.exe", root / "closed.gamma"],
            [root / f"execution-{ordinal}.raw", root / f"execution-{ordinal}.stderr"],
        )
    fail("stage")


def marker_identity(
    paths: list[Path], prefix: str, limits: tuple[int | None, ...] | None = None
) -> list[dict]:
    selected_limits = limits if limits is not None else (None,) * len(paths)
    if len(selected_limits) != len(paths):
        fail("stage identity ceiling shape")
    return [
        identity(path, f"{prefix}_{index}", selected_limits[index])
        for index, path in enumerate(paths)
    ]


def marker_path(root: Path, stage: str, suffix: str) -> Path:
    return root / f"{stage}.{suffix}.json"


def start_stage(root: Path, stage: str, token: str) -> None:
    plan = load_plan(root)
    if token != plan["attempt_id"] or stage not in STAGES:
        fail("stage attempt identity")
    if stage == "packing":
        require_completed(root, plan, "elaboration")
    if stage.startswith("execution-"):
        require_completed(root, plan, "packing")
    inputs, outputs = stage_paths(root, stage)
    started_path = marker_path(root, stage, "started")
    finished_path = marker_path(root, stage, "finished")
    timeout_path = marker_path(root, stage, "timeout")
    if started_path.exists() or finished_path.exists() or timeout_path.exists():
        fail("stage already attempted")
    for path in outputs:
        if path.exists():
            fail(f"refuse stage output overwrite: {path.name}")
    start_epoch = time.time_ns()
    started = {
        "attempt_id": token,
        "inputs": marker_identity(
            inputs, f"{stage}_input", STAGE_INPUT_CEILINGS[stage]
        ),
        "prepared_epoch_ns": plan["prepared_epoch_ns"],
        "schema": MARKER_SCHEMA,
        "stage": stage,
        "start_epoch_ns": start_epoch,
    }
    atomic_write(started_path, canonical_json(started))


def load_started_stage(
    root: Path,
    plan: dict,
    stage: str,
    token: str,
    allowed_states: tuple[str, ...] = ("running",),
) -> tuple[dict, list[Path]]:
    if token != plan["attempt_id"] or stage not in STAGES:
        fail("stage attempt identity")
    result = validate_stage(root, plan, stage)
    if result.get("state") not in allowed_states:
        fail("stage is not running")
    started = load_canonical(marker_path(root, stage, "started"), f"{stage} start")
    _, outputs = stage_paths(root, stage)
    return started, outputs


def watch_stage(root: Path, stage: str, token: str, process_id: int) -> None:
    plan = load_plan(root)
    started, _ = load_started_stage(root, plan, stage, token)
    if process_id <= 1 or process_id == os.getpid():
        usage("invalid stage process id")
    ceiling = STAGE_CEILINGS[stage]
    deadline = started["start_epoch_ns"] + ceiling * 1_000_000_000
    next_heartbeat = time.monotonic() + 60
    while time.time_ns() < deadline:
        remaining = max(0.0, (deadline - time.time_ns()) / 1_000_000_000)
        time.sleep(min(1.0, remaining))
        now = time.monotonic()
        if now >= next_heartbeat:
            elapsed = max(0, (time.time_ns() - started["start_epoch_ns"]) // 1_000_000_000)
            print(f"Delta publication {stage}: {elapsed}s elapsed", file=sys.stderr, flush=True)
            next_heartbeat += 60
    try:
        os.kill(process_id, 0)
    except ProcessLookupError:
        return
    timeout = {
        "attempt_id": token,
        "prepared_epoch_ns": plan["prepared_epoch_ns"],
        "schema": MARKER_SCHEMA,
        "stage": stage,
        "start_epoch_ns": started["start_epoch_ns"],
        "timeout_epoch_ns": time.time_ns(),
    }
    atomic_write(marker_path(root, stage, "timeout"), canonical_json(timeout))
    try:
        os.kill(process_id, signal.SIGTERM)
    except ProcessLookupError:
        return
    grace_deadline = time.monotonic() + 5
    while time.monotonic() < grace_deadline:
        try:
            os.kill(process_id, 0)
        except ProcessLookupError:
            return
        time.sleep(0.1)
    try:
        os.kill(process_id, signal.SIGKILL)
    except ProcessLookupError:
        pass


def finish_stage(root: Path, stage: str, token: str, status: int) -> None:
    plan = load_plan(root)
    started, outputs = load_started_stage(root, plan, stage, token, ("running", "timed-out"))
    if isinstance(status, bool) or status < 0 or status > 255:
        usage("invalid stage process status")
    timed_out = marker_path(root, stage, "timeout").exists()
    if timed_out and status != 124:
        fail("timed-out stage status")
    if not timed_out and status == 124:
        fail("unsubstantiated stage timeout")
    for output in outputs:
        if not output.exists():
            output.write_bytes(b"")
    finish_epoch = time.time_ns()
    elapsed_ms = max(0, (finish_epoch - started["start_epoch_ns"]) // 1_000_000)
    if timed_out:
        elapsed_ms = min(elapsed_ms, STAGE_CEILINGS[stage] * 1000)
    finished = {
        "attempt_id": token,
        "elapsed_milliseconds": elapsed_ms,
        "finish_epoch_ns": finish_epoch,
        "inputs": started["inputs"],
        "outputs": marker_identity(
            outputs, f"{stage}_output", STAGE_OUTPUT_CEILINGS[stage]
        ),
        "prepared_epoch_ns": plan["prepared_epoch_ns"],
        "schema": MARKER_SCHEMA,
        "stage": stage,
        "start_epoch_ns": started["start_epoch_ns"],
        "status": status,
    }
    atomic_write(marker_path(root, stage, "finished"), canonical_json(finished))


STARTED_KEYS = {"attempt_id", "inputs", "prepared_epoch_ns", "schema", "stage", "start_epoch_ns"}
FINISHED_KEYS = STARTED_KEYS | {"elapsed_milliseconds", "finish_epoch_ns", "outputs", "status"}
TIMEOUT_KEYS = {
    "attempt_id", "prepared_epoch_ns", "schema", "stage", "start_epoch_ns", "timeout_epoch_ns"
}


def validate_timeout(root: Path, plan: dict, stage: str, started: dict) -> bool:
    path = marker_path(root, stage, "timeout")
    if not path.exists():
        return False
    timeout = load_canonical(path, f"{stage} timeout")
    strict(timeout, TIMEOUT_KEYS, f"{stage} timeout")
    if (
        timeout["schema"] != MARKER_SCHEMA
        or timeout["attempt_id"] != plan["attempt_id"]
        or timeout["prepared_epoch_ns"] != plan["prepared_epoch_ns"]
        or timeout["stage"] != stage
        or timeout["start_epoch_ns"] != started["start_epoch_ns"]
        or isinstance(timeout["timeout_epoch_ns"], bool)
        or not isinstance(timeout["timeout_epoch_ns"], int)
        or timeout["timeout_epoch_ns"] < started["start_epoch_ns"] + STAGE_CEILINGS[stage] * 1_000_000_000
        or timeout["timeout_epoch_ns"] > time.time_ns() + 1_000_000_000
    ):
        fail(f"{stage} timeout custody")
    return True


def validate_stage(root: Path, plan: dict, stage: str) -> dict:
    started_path = marker_path(root, stage, "started")
    finished_path = marker_path(root, stage, "finished")
    timeout_path = marker_path(root, stage, "timeout")
    if not started_path.exists() and not finished_path.exists():
        if timeout_path.exists():
            fail(f"{stage} timeout without start")
        return {"state": "pending"}
    if not started_path.exists():
        fail(f"{stage} finish without start")
    started = load_canonical(started_path, f"{stage} start")
    strict(started, STARTED_KEYS, f"{stage} start")
    inputs, outputs = stage_paths(root, stage)
    expected_inputs = marker_identity(
        inputs, f"{stage}_input", STAGE_INPUT_CEILINGS[stage]
    )
    now = time.time_ns()
    if (
        started["schema"] != MARKER_SCHEMA
        or started["attempt_id"] != plan["attempt_id"]
        or started["prepared_epoch_ns"] != plan["prepared_epoch_ns"]
        or started["stage"] != stage
        or not isinstance(started["start_epoch_ns"], int)
        or started["start_epoch_ns"] < plan["prepared_epoch_ns"]
        or started["start_epoch_ns"] > now + 1_000_000_000
        or started["inputs"] != expected_inputs
    ):
        fail(f"{stage} stale/cross-paired start")
    timed_out = validate_timeout(root, plan, stage, started)
    if not finished_path.exists():
        return {
            "start_epoch_ns": started["start_epoch_ns"],
            "state": "timed-out" if timed_out else "running",
        }
    finished = load_canonical(finished_path, f"{stage} finish")
    strict(finished, FINISHED_KEYS, f"{stage} finish")
    expected_outputs = marker_identity(
        outputs, f"{stage}_output", STAGE_OUTPUT_CEILINGS[stage]
    )
    for key in STARTED_KEYS:
        if finished[key] != started[key]:
            fail(f"{stage} start/finish cross-pair")
    if (
        not isinstance(finished["finish_epoch_ns"], int)
        or finished["finish_epoch_ns"] < started["start_epoch_ns"]
        or finished["finish_epoch_ns"] > now + 1_000_000_000
        or isinstance(finished["elapsed_milliseconds"], bool)
        or not isinstance(finished["elapsed_milliseconds"], int)
        or finished["elapsed_milliseconds"] < 0
        or finished["elapsed_milliseconds"] > STAGE_CEILINGS[stage] * 1000
        or isinstance(finished["status"], bool)
        or not isinstance(finished["status"], int)
        or finished["status"] < 0
        or finished["status"] > 255
        or (timed_out and finished["status"] != 124)
        or (not timed_out and finished["status"] == 124)
        or finished["outputs"] != expected_outputs
    ):
        fail(f"{stage} finish custody")
    return {
        "elapsed_milliseconds": finished["elapsed_milliseconds"],
        "state": "complete" if finished["status"] == 0 else "failed",
        "status": finished["status"],
    }


def require_completed(root: Path, plan: dict, stage: str) -> dict:
    result = validate_stage(root, plan, stage)
    if result.get("state") != "complete":
        raise DriverPending(f"{stage} not complete")
    return result


def status(root: Path) -> tuple[dict, bool]:
    plan = load_plan(root)
    stages = {stage: validate_stage(root, plan, stage) for stage in STAGES}
    ready = all(value["state"] == "complete" for value in stages.values())
    return ({
        "attempt_id": plan["attempt_id"],
        "ready_to_finalize": ready,
        "schema": STATUS_SCHEMA,
        "stages": stages,
    }, ready)


def finalize(root: Path) -> dict:
    plan = load_plan(root)
    stages = {stage: require_completed(root, plan, stage) for stage in STAGES}
    final_paths = [
        root / "elaboration.json", root / "execution-0.json", root / "execution-1.json",
        root / "execution-0.s", root / "execution-1.s", root / "receipt.json",
    ]
    if any(path.exists() for path in final_paths):
        fail("refuse finalization overwrite")
    if (root / "packing.stderr").read_bytes() != b"":
        fail("packing diagnostics")
    artifacts = root / "artifacts"
    roots = {"delta": DELTA}
    with tempfile.TemporaryDirectory(prefix="finalize-", dir=root) as spelling:
        temporary = Path(spelling)
        assemblies: list[Path] = []
        decoder = publication._load_module("delta_driver_decoder", publication.DECODER_SOURCE)
        for ordinal in (0, 1):
            raw_path = root / f"execution-{ordinal}.raw"
            if raw_path.stat().st_size > publication.MAX_GAMMA_OBSERVATION:
                raise DriverResourceError("Gamma observation byte ceiling")
            raw = raw_path.read_text(encoding="ascii")
            try:
                semantic_status, assembly = decoder.decode(raw)
            except Exception as error:
                fail(f"execution {ordinal} decode: {error}")
            if semantic_status != 0:
                fail(f"execution {ordinal} semantic status")
            path = temporary / f"execution-{ordinal}.s"
            path.write_bytes(assembly)
            assemblies.append(path)
        elaboration = publication.make_elaboration_observation(
            0, stages["elaboration"]["elapsed_milliseconds"], MANIFEST, LOCATIONS,
            roots, artifacts / "assembler.tape", artifacts / "delta_to_gamma_bytecode.tape",
            artifacts / "gamma_interpreter_bytecode.tape", root / "template.gamma", root / "elaboration.stderr",
        )
        elaboration_path = temporary / "elaboration.json"
        elaboration_path.write_bytes(canonical_json(elaboration))
        observations: list[Path] = []
        for ordinal in (0, 1):
            observation = publication.make_execution_observation(
                ordinal, 0, stages[f"execution-{ordinal}"]["elapsed_milliseconds"],
                MANIFEST, LOCATIONS, roots, artifacts / "assembler.tape",
                artifacts / "delta_to_gamma_bytecode.tape", artifacts / "gamma_interpreter_bytecode.tape",
                root / "template.gamma", root / "closed.gamma",
                root / f"execution-{ordinal}.raw", assemblies[ordinal],
                root / f"execution-{ordinal}.stderr",
            )
            path = temporary / f"execution-{ordinal}.json"
            path.write_bytes(canonical_json(observation))
            observations.append(path)
        receipt = publication.make_receipt(
            MANIFEST, LOCATIONS, roots, artifacts / "assembler.tape",
            artifacts / "delta_to_gamma_bytecode.tape", artifacts / "gamma_interpreter_bytecode.tape",
            root / "template.gamma", root / "closed.gamma", elaboration_path,
            root / "elaboration.stderr", (observations[0], observations[1]),
            (root / "execution-0.raw", root / "execution-1.raw"),
            (assemblies[0], assemblies[1]),
            (root / "execution-0.stderr", root / "execution-1.stderr"),
        )
        publication.support.validate_darwin_arm64_assembly(assemblies[0].read_bytes())
        for source, target in (
            (elaboration_path, root / "elaboration.json"),
            (observations[0], root / "execution-0.json"),
            (observations[1], root / "execution-1.json"),
            (assemblies[0], root / "execution-0.s"),
            (assemblies[1], root / "execution-1.s"),
        ):
            atomic_write(target, source.read_bytes())
        atomic_write(root / "receipt.json", canonical_json(receipt))
    return receipt


def main(arguments: list[str]) -> int:
    if len(arguments) < 2:
        usage("expected prepare|status|finalize|stage-* EVIDENCE_DIR")
    command, spelling, *rest = arguments
    root = Path(spelling)
    if not root.is_absolute():
        usage("evidence directory must be absolute")
    if command == "prepare" and not rest:
        plan = prepare(root)
        sys.stdout.buffer.write(canonical_json(plan))
        return 0
    if command == "status" and not rest:
        value, ready = status(root)
        sys.stdout.buffer.write(canonical_json(value))
        return 0 if ready else 3
    if command == "finalize" and not rest:
        receipt = finalize(root)
        sys.stdout.buffer.write(canonical_json(receipt))
        return 0
    if command == "stage-start" and len(rest) == 2:
        stage, token = rest
        start_stage(root, stage, token)
        return 0
    if command == "stage-watch" and len(rest) == 3:
        stage, token, process_id = rest
        try:
            parsed_process_id = int(process_id, 10)
        except ValueError:
            usage("invalid stage process id")
        watch_stage(root, stage, token, parsed_process_id)
        return 0
    if command == "stage-finish" and len(rest) == 3:
        stage, token, stage_status = rest
        try:
            parsed_status = int(stage_status, 10)
        except ValueError:
            usage("invalid stage process status")
        finish_stage(root, stage, token, parsed_status)
        return 0
    usage("arguments")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (DriverError, publication.ReceiptError, support.PublicationSupportError, OSError, ValueError) as error:
        status_code = getattr(error, "status", 251)
        print(f"Delta publication driver V1: {error}", file=sys.stderr)
        raise SystemExit(status_code)
