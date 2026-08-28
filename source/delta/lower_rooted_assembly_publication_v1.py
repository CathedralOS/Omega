#!/usr/bin/env python3
"""Construct and verify candidate lower-rooted Delta assembly receipts.

The verifier does not run the lower-rung tools. It checks exact observations
of those runs, independently reconstructs the packed Gamma program and decoded
assembly, and binds the established Alpha/Beta construction antecedents.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
from pathlib import Path

import publication_support as support
import source_closure_snapshot_v1 as closure


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[1]
OMEGA2GAMMA_SOURCE = REPOSITORY / "source/delta/meaning/omega2gamma.beta"
INTERPRETER_SOURCE = REPOSITORY / "source/gamma/interp.beta"
PACKER_SOURCE = REPOSITORY / "source/delta/meaning/encode-gamma-input.py"
DECODER_SOURCE = REPOSITORY / "source/delta/meaning/decode-gamma-output.py"
BETA_COMPILER_SOURCE = REPOSITORY / "source/beta/bc.beta"
BETA_COMPILER_TAPE = REPOSITORY / "source/beta/artifacts/bc.tape"
ALPHA_ASSEMBLER_SOURCE = REPOSITORY / "source/alpha/assembler/assembler.alpha"

ELABORATION_SCHEMA = "omega.delta-gamma-elaboration-observation.v1"
EXECUTION_SCHEMA = "omega.delta-gamma-execution-observation.v1"
RECEIPT_SCHEMA = "omega.delta-lower-rooted-assembly-publication.v1"
RECEIPT_DOMAIN = b"omega.delta-lower-rooted-assembly-publication.v1\0"
PUBLICATION_ID = "delta.compiler.darwin-arm64-assembly.v1"
CLAIM = "candidate_lower_rooted_assembly_only"
VALIDATION_PROFILE = "delta.darwin-arm64-assembly.strict-v1"

MAX_DOCUMENT = 65_536
MAX_TEMPLATE = 1_048_576
MAX_CLOSED_GAMMA = 4 * 1024 * 1024
MAX_TAPE = 262_140
MAX_GAMMA_OBSERVATION = 256 * 1024 * 1024
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()

RESOURCE_PROFILE = {
    "argument_scratch_values": 512,
    "evaluator_fuel": 50_000_000,
    "heap_arena_bytes": 40 * 1024 * 1024,
    "heap_map_bytes": 5 * 1024 * 1024,
    "id": "gamma.interp.canonical-v1",
    "return_stack_reserve_bytes": 1 * 1024 * 1024,
    "source_bytes": MAX_CLOSED_GAMMA,
}


class ReceiptError(Exception):
    status = 251


class ReceiptResourceError(ReceiptError):
    status = 252


def fail(message: str) -> None:
    raise ReceiptError(message)


def resource(message: str) -> None:
    raise ReceiptResourceError(message)


def canonical_json(value: object, *, pretty: bool) -> bytes:
    options = {"ensure_ascii": False, "sort_keys": True}
    if pretty:
        return (json.dumps(value, indent=2, **options) + "\n").encode()
    return json.dumps(value, separators=(",", ":"), **options).encode()


def load_json(path: Path, context: str) -> dict:
    raw = path.read_bytes()
    if len(raw) > MAX_DOCUMENT:
        resource(f"{context} byte ceiling")
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"{context} JSON: {error}")
    if not isinstance(value, dict) or raw != canonical_json(value, pretty=True):
        fail(f"{context} canonical JSON")
    return value


def strict(value: object, keys: set[str], context: str) -> dict:
    if not isinstance(value, dict) or set(value) != keys:
        fail(f"{context} fields")
    return value


def file_identity(path: Path, role: str, limit: int | None = None) -> dict:
    raw = path.read_bytes()
    if limit is not None and len(raw) > limit:
        resource(f"{role} byte ceiling")
    return {"byte_length": len(raw), "role": role, "sha256": hashlib.sha256(raw).hexdigest()}


def source_identity(
    manifest_path: Path, locations_path: Path, roots: dict[str, Path]
) -> tuple[dict, dict, bytes]:
    try:
        manifest, _ = closure.load_document(manifest_path, "snapshot")
        image = support.materialize_canonical_image(manifest_path, locations_path, roots)
    except support.PublicationSupportResourceError as error:
        raise ReceiptResourceError(str(error)) from error
    except (support.PublicationSupportError, closure.SnapshotError) as error:
        fail(str(error))
    return (
        {"closure_sha256": manifest["closure_sha256"], "id": manifest["snapshot_id"]},
        {
            "byte_length": len(image),
            "role": "delta_compiler_canonical_lf_image",
            "sha256": hashlib.sha256(image).hexdigest(),
        },
        image,
    )


def validate_interpreter_profile() -> None:
    source = INTERPRETER_SOURCE.read_bytes()
    required = (
        b"n == 4194304",
        b"ARG_SCRATCH [4 KiB; 512 values)",
        b"eval(expr, 50000000)",
        b"16777216 HEAP [16 MiB,56 MiB)",
        b"58720256 HEAP_MAP [56 MiB,61 MiB)",
        b"[63 MiB,64 MiB) remains the descending Alpha return-stack reserve",
    )
    if any(fragment not in source for fragment in required):
        fail("Gamma interpreter resource profile/source relation")


def toolchain_identity(
    assembler_tape: Path, translator_tape: Path, interpreter_tape: Path
) -> dict:
    validate_interpreter_profile()
    return {
        "alpha_assembler": {
            "authority_role": "canonical_alpha_written_assembler_artifact",
            "source": file_identity(ALPHA_ASSEMBLER_SOURCE, "alpha_assembler_source"),
            "tape": file_identity(assembler_tape, "alpha_assembler_persisted_selfhost_tape", MAX_TAPE),
        },
        "beta_compiler": {
            "authority_role": "persisted_alpha_rooted_beta_compiler_fixed_point",
            "source": file_identity(BETA_COMPILER_SOURCE, "beta_compiler_source"),
            "tape": file_identity(BETA_COMPILER_TAPE, "beta_compiler_persisted_fixed_point_tape", MAX_TAPE),
        },
        "gamma_input_packer": {
            "source": file_identity(PACKER_SOURCE, "gamma_input_packer_source"),
        },
        "gamma_output_decoder": {
            "source": file_identity(DECODER_SOURCE, "gamma_output_decoder_source"),
        },
        "interpreter": {
            "artifact": file_identity(interpreter_tape, "gamma_interpreter_beta_built_tape", MAX_TAPE),
            "source": file_identity(INTERPRETER_SOURCE, "gamma_interpreter_beta_source"),
        },
        "translator": {
            "artifact": file_identity(translator_tape, "delta_to_gamma_beta_built_tape", MAX_TAPE),
            "source": file_identity(OMEGA2GAMMA_SOURCE, "delta_to_gamma_beta_source"),
        },
    }


def _load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        fail(f"cannot load {name}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def reconstruct_closed_gamma(template_path: Path, image: bytes, gamma_path: Path) -> dict:
    template = template_path.read_bytes()
    if len(template) > MAX_TEMPLATE:
        resource("Gamma template byte ceiling")
    try:
        expected = _load_module("delta_publication_gamma_input", PACKER_SOURCE).inject(template, image)
    except Exception as error:
        fail(f"Gamma packing: {error}")
    if len(expected) > MAX_CLOSED_GAMMA:
        resource("closed Gamma byte ceiling")
    if gamma_path.read_bytes() != expected:
        fail("closed Gamma reconstruction")
    return file_identity(gamma_path, "delta_compiler_elaborated_closed_gamma", MAX_CLOSED_GAMMA)


def decode_execution(raw_path: Path, assembly_path: Path) -> tuple[int, dict, dict]:
    raw = raw_path.read_bytes()
    if len(raw) > MAX_GAMMA_OBSERVATION:
        resource("Gamma observation byte ceiling")
    try:
        semantic_status, decoded = _load_module(
            "delta_publication_gamma_output", DECODER_SOURCE
        ).decode(raw.decode("ascii"))
    except Exception as error:
        fail(f"Gamma observation decode: {error}")
    if semantic_status != 0:
        fail("compiled Delta status")
    if decoded != assembly_path.read_bytes():
        fail("assembly decode custody")
    try:
        support.validate_darwin_arm64_assembly(decoded)
    except support.PublicationSupportResourceError as error:
        raise ReceiptResourceError(str(error)) from error
    except support.PublicationSupportError as error:
        fail(str(error))
    return (
        semantic_status,
        file_identity(raw_path, "gamma_interpreter_structured_stdout", MAX_GAMMA_OBSERVATION),
        file_identity(assembly_path, "darwin_arm64_assembly_stdout", support.MAX_ASSEMBLY_BYTES),
    )


def empty_stderr(path: Path, role: str) -> dict:
    identity = file_identity(path, role, MAX_DOCUMENT)
    if identity["byte_length"] != 0 or identity["sha256"] != EMPTY_SHA256:
        fail("execution diagnostics")
    return identity


def validate_status_elapsed(status: int, elapsed_ms: int) -> None:
    if isinstance(status, bool) or not isinstance(status, int):
        fail("execution status type")
    if isinstance(elapsed_ms, bool) or not isinstance(elapsed_ms, int):
        fail("elapsed milliseconds type")
    if status != 0:
        fail("execution status")
    if elapsed_ms < 0 or elapsed_ms > (1 << 63) - 1:
        fail("elapsed milliseconds")


def make_elaboration_observation(
    status: int, elapsed_ms: int, manifest: Path, locations: Path,
    roots: dict[str, Path], assembler_tape: Path, translator_tape: Path,
    interpreter_tape: Path, template: Path, stderr: Path,
) -> dict:
    validate_status_elapsed(status, elapsed_ms)
    snapshot, image_identity, _ = source_identity(manifest, locations, roots)
    return {
        "elapsed_milliseconds": elapsed_ms,
        "schema": ELABORATION_SCHEMA,
        "source_image": image_identity,
        "source_snapshot": snapshot,
        "status": status,
        "stderr": empty_stderr(stderr, "delta_to_gamma_diagnostic_stderr"),
        "template": file_identity(template, "delta_compiler_gamma_template", MAX_TEMPLATE),
        "toolchain": toolchain_identity(assembler_tape, translator_tape, interpreter_tape),
    }


def make_execution_observation(
    ordinal: int, status: int, elapsed_ms: int, manifest: Path, locations: Path,
    roots: dict[str, Path], assembler_tape: Path, translator_tape: Path,
    interpreter_tape: Path, template: Path, gamma: Path, raw_stdout: Path,
    assembly: Path, stderr: Path,
) -> dict:
    if ordinal not in (0, 1):
        fail("observation ordinal")
    validate_status_elapsed(status, elapsed_ms)
    snapshot, image_identity, image = source_identity(manifest, locations, roots)
    semantic_status, raw_identity, assembly_identity = decode_execution(raw_stdout, assembly)
    return {
        "assembly": assembly_identity,
        "elaborated_gamma": reconstruct_closed_gamma(template, image, gamma),
        "elapsed_milliseconds": elapsed_ms,
        "gamma_stdout": raw_identity,
        "ordinal": ordinal,
        "resource_profile": RESOURCE_PROFILE,
        "schema": EXECUTION_SCHEMA,
        "semantic_status": semantic_status,
        "source_image": image_identity,
        "source_snapshot": snapshot,
        "status": status,
        "stderr": empty_stderr(stderr, "gamma_interpreter_diagnostic_stderr"),
        "template": file_identity(template, "delta_compiler_gamma_template", MAX_TEMPLATE),
        "toolchain": toolchain_identity(assembler_tape, translator_tape, interpreter_tape),
    }


ELABORATION_KEYS = {
    "elapsed_milliseconds", "schema", "source_image", "source_snapshot", "status",
    "stderr", "template", "toolchain",
}
EXECUTION_KEYS = {
    "assembly", "elaborated_gamma", "elapsed_milliseconds", "gamma_stdout", "ordinal",
    "resource_profile", "schema", "semantic_status", "source_image", "source_snapshot",
    "status", "stderr", "template", "toolchain",
}


def validate_observation_shapes(elaboration: dict, executions: list[dict]) -> None:
    strict(elaboration, ELABORATION_KEYS, "elaboration observation")
    if elaboration["schema"] != ELABORATION_SCHEMA or elaboration["status"] != 0:
        fail("elaboration observation identity/status")
    for ordinal, execution in enumerate(executions):
        strict(execution, EXECUTION_KEYS, "execution observation")
        if (
            execution["schema"] != EXECUTION_SCHEMA
            or execution["ordinal"] != ordinal
            or execution["status"] != 0
            or execution["semantic_status"] != 0
            or execution["resource_profile"] != RESOURCE_PROFILE
        ):
            fail("execution observation identity/status/resource")


def receipt_digest(receipt: dict) -> str:
    projection = {key: value for key, value in receipt.items() if key != "receipt_sha256"}
    compact = canonical_json(projection, pretty=False)
    return hashlib.sha256(
        RECEIPT_DOMAIN + len(compact).to_bytes(8, "little") + compact
    ).hexdigest()


def make_receipt(
    manifest: Path, locations: Path, roots: dict[str, Path], assembler_tape: Path,
    translator_tape: Path, interpreter_tape: Path, template: Path, gamma: Path,
    elaboration_path: Path, elaboration_stderr: Path,
    execution_paths: tuple[Path, Path], raw_paths: tuple[Path, Path],
    assembly_paths: tuple[Path, Path], stderr_paths: tuple[Path, Path],
) -> dict:
    elaboration = load_json(elaboration_path, "elaboration observation")
    executions = [load_json(path, f"execution observation {index}") for index, path in enumerate(execution_paths)]
    validate_observation_shapes(elaboration, executions)
    expected_elaboration = make_elaboration_observation(
        0, elaboration["elapsed_milliseconds"], manifest, locations, roots,
        assembler_tape, translator_tape, interpreter_tape, template, elaboration_stderr,
    )
    if elaboration != expected_elaboration:
        fail("elaboration observation custody")
    for index, execution in enumerate(executions):
        expected = make_execution_observation(
            index, 0, execution["elapsed_milliseconds"], manifest, locations, roots,
            assembler_tape, translator_tape, interpreter_tape, template, gamma,
            raw_paths[index], assembly_paths[index], stderr_paths[index],
        )
        if execution != expected:
            fail(f"execution observation {index} custody")

    common = (
        "source_snapshot", "source_image", "toolchain", "template",
        "elaborated_gamma", "gamma_stdout", "assembly", "stderr",
    )
    if any(executions[0][key] != executions[1][key] for key in common):
        fail("execution observation agreement")
    if any(elaboration[key] != executions[0][key] for key in ("source_snapshot", "source_image", "toolchain", "template")):
        fail("elaboration/execution cross-pair")
    if assembly_paths[0].read_bytes() != assembly_paths[1].read_bytes():
        fail("assembly byte agreement")

    receipt = {
        "assembly": executions[0]["assembly"],
        "claim": CLAIM,
        "elaborated_gamma": executions[0]["elaborated_gamma"],
        "elaboration": {
            key: elaboration[key]
            for key in ("elapsed_milliseconds", "status", "stderr", "template")
        },
        "executions": [
            {
                key: execution[key]
                for key in (
                    "assembly", "elapsed_milliseconds", "gamma_stdout", "ordinal",
                    "resource_profile", "semantic_status", "status", "stderr",
                )
            }
            for execution in executions
        ],
        "publication_id": PUBLICATION_ID,
        "receipt_sha256": "0" * 64,
        "schema": RECEIPT_SCHEMA,
        "source_image": executions[0]["source_image"],
        "source_snapshot": executions[0]["source_snapshot"],
        "target": {
            "abi": "darwin-arm64-assembly-v1",
            "configuration": "conservative",
            "target": "darwin_arm64",
        },
        "template": executions[0]["template"],
        "toolchain": executions[0]["toolchain"],
        "validation_profile": VALIDATION_PROFILE,
    }
    receipt["receipt_sha256"] = receipt_digest(receipt)
    if len(canonical_json(receipt, pretty=True)) > MAX_DOCUMENT:
        resource("receipt byte ceiling")
    return receipt


def parse_roots(arguments: list[str]) -> dict[str, Path]:
    try:
        return closure.parse_roles(arguments)
    except closure.SnapshotError as error:
        fail(str(error))


def parse_join(arguments: list[str]) -> tuple:
    if len(arguments) < 17:
        fail("join arguments")
    fixed = list(map(Path, arguments[:17]))
    manifest, locations, assembler, translator, interpreter, template, gamma = fixed[:7]
    elaboration, elaboration_stderr = fixed[7:9]
    execution0, raw0, assembly0, stderr0, execution1, raw1, assembly1, stderr1 = fixed[9:17]
    return (
        manifest, locations, parse_roots(arguments[17:]), assembler, translator,
        interpreter, template, gamma, elaboration, elaboration_stderr,
        (execution0, execution1), (raw0, raw1), (assembly0, assembly1),
        (stderr0, stderr1),
    )


def main(arguments: list[str]) -> int:
    if not arguments:
        fail("command")
    command, *rest = arguments
    if command == "observe-elaboration":
        if len(rest) < 9:
            fail("observe-elaboration arguments")
        status, elapsed = int(rest[0]), int(rest[1])
        manifest, locations, assembler, translator, interpreter, template, stderr = map(Path, rest[2:9])
        value = make_elaboration_observation(
            status, elapsed, manifest, locations, parse_roots(rest[9:]), assembler,
            translator, interpreter, template, stderr,
        )
        sys.stdout.buffer.write(canonical_json(value, pretty=True))
        return 0
    if command == "observe-execution":
        if len(rest) < 13:
            fail("observe-execution arguments")
        ordinal, status, elapsed = map(int, rest[:3])
        fixed = list(map(Path, rest[3:13]))
        manifest, locations, assembler, translator, interpreter, template, gamma, raw, assembly, stderr = fixed
        value = make_execution_observation(
            ordinal, status, elapsed, manifest, locations, parse_roots(rest[13:]),
            assembler, translator, interpreter, template, gamma, raw, assembly, stderr,
        )
        sys.stdout.buffer.write(canonical_json(value, pretty=True))
        return 0
    if command == "generate":
        sys.stdout.buffer.write(canonical_json(make_receipt(*parse_join(rest)), pretty=True))
        return 0
    if command == "verify":
        if not rest:
            fail("verify arguments")
        candidate = load_json(Path(rest[0]), "receipt")
        expected = make_receipt(*parse_join(rest[1:]))
        if candidate != expected or candidate.get("receipt_sha256") != receipt_digest(candidate):
            fail("receipt custody")
        return 0
    fail("command")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (ReceiptError, support.PublicationSupportError, closure.SnapshotError, OSError, ValueError) as error:
        status = getattr(error, "status", 251)
        print(f"Delta assembly publication V1: {error}", file=sys.stderr)
        raise SystemExit(status)
