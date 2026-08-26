#!/usr/bin/env python3
"""Verify and materialize bounded generated ordinary-source custody."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import selectors
import subprocess
import sys
import time
from pathlib import Path, PurePosixPath


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
COMPILER = HERE.parent / "compiler"
sys.path.insert(0, str(COMPILER))
import omega_bootstrap_bundle as source_bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


SCHEMA = "omega.bootstrap.generated-source-recipe.v1"
RESOURCE_PROFILE = "omega.bootstrap.generated-source-custody.v1"
DOMAIN = b"omega.bootstrap.generated-source-recipe.v1\0"
TOP_KEYS = {
    "schema", "recipe_id", "status", "resource_profile", "runner",
    "generator", "repository_inputs", "external_inputs", "output",
    "omgcomp1_join", "closure_sha256",
}
MAX_RECIPE_BYTES = 16_384
MAX_REPOSITORY_INPUTS = 8
MAX_EXTERNAL_INPUTS = 4
MAX_PATH_BYTES = 128
MAX_PATH_COMPONENTS = 16
MAX_ARGUMENTS = 8
MAX_ENVIRONMENT = 8
MAX_SCALAR_BYTES = 64
MAX_GENERATOR_BYTES = 8_192
MAX_REPOSITORY_INPUT_BYTES = 65_536
MAX_TOTAL_REPOSITORY_BYTES = 65_536
MAX_OUTPUT_BYTES = compilation.MAX_SOURCE_BYTES
MAX_STDERR_BYTES = 65_536
MAX_JOIN_SOURCES = 4
RUN_TIMEOUT_SECONDS = 180.0
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")


class CustodyError(Exception):
    def __init__(self, message: str, status: int = 251):
        super().__init__(message)
        self.status = status


def reject(message: str) -> CustodyError:
    return CustodyError(message, 251)


def exhaust(message: str) -> CustodyError:
    return CustodyError(message, 252)


def strict_keys(value: object, expected: set[str], context: str) -> dict:
    if not isinstance(value, dict):
        raise reject(f"{context} must be an object")
    actual = set(value)
    if actual != expected:
        raise reject(
            f"{context} keys differ; missing={sorted(expected-actual)}, "
            f"unknown={sorted(actual-expected)}"
        )
    return value


def bounded_string(value: object, context: str, maximum: int = MAX_SCALAR_BYTES) -> str:
    if not isinstance(value, str) or not value:
        raise reject(f"{context} must be a nonempty string")
    if len(value.encode("utf-8")) > maximum:
        raise exhaust(f"{context} exceeds {maximum} bytes")
    return value


def nonnegative(value: object, context: str) -> int:
    if type(value) is not int or value < 0:
        raise reject(f"{context} must be a nonnegative integer")
    return value


def digest(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def digest_field(value: object, context: str) -> str:
    if not isinstance(value, str) or HEX_64.fullmatch(value) is None:
        raise reject(f"{context} must be lowercase SHA-256")
    return value


def canonical_projection(recipe: dict) -> bytes:
    projection = {key: value for key, value in recipe.items() if key != "closure_sha256"}
    return json.dumps(
        projection, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def recipe_digest(recipe: dict) -> str:
    canonical = canonical_projection(recipe)
    return digest(DOMAIN + len(canonical).to_bytes(8, "little") + canonical)


def refresh_digest(recipe: dict) -> None:
    recipe["closure_sha256"] = recipe_digest(recipe)


def repository_path(value: object, context: str) -> Path:
    if not isinstance(value, str):
        raise reject(f"{context} path must be a string")
    raw = value.encode("utf-8")
    path = PurePosixPath(value)
    if (
        not value or len(raw) > MAX_PATH_BYTES or path.is_absolute()
        or ".." in path.parts or "." in path.parts or str(path) != value
    ):
        if len(raw) > MAX_PATH_BYTES:
            raise exhaust(f"{context} path exceeds {MAX_PATH_BYTES} bytes")
        raise reject(f"{context} path is not canonical repository-relative POSIX")
    if len(path.parts) > MAX_PATH_COMPONENTS:
        raise exhaust(f"{context} path exceeds {MAX_PATH_COMPONENTS} components")
    resolved = ROOT / value
    if not resolved.is_file():
        raise reject(f"{context} file is absent: {value}")
    return resolved


def validate_file_row(row: object, context: str, expected_keys: set[str]) -> tuple[str, str, int]:
    row = strict_keys(row, expected_keys, context)
    role = bounded_string(row["role"], f"{context} role")
    path = repository_path(row["path"], context)
    raw = path.read_bytes()
    length = nonnegative(row["byte_length"], f"{context} byte_length")
    expected_digest = digest_field(row["sha256"], f"{context} sha256")
    if length != len(raw) or expected_digest != digest(raw):
        raise reject(f"{context} exact file custody differs")
    return role, path.relative_to(ROOT).as_posix(), len(raw)


def cargo_lock_rows(path: Path) -> dict[tuple[str, str], dict[str, str]]:
    rows: dict[tuple[str, str], dict[str, str]] = {}
    for section in path.read_text(encoding="utf-8").split("[[package]]")[1:]:
        fields: dict[str, str] = {}
        for key in ("name", "version", "source", "checksum"):
            match = re.search(rf'^\s*{key}\s*=\s*"([^"]+)"\s*$', section, re.MULTILINE)
            if match is not None:
                fields[key] = match.group(1)
        if "name" in fields and "version" in fields:
            identity = (fields["name"], fields["version"])
            if identity in rows:
                raise reject(f"Cargo.lock duplicates {identity[0]}@{identity[1]}")
            rows[identity] = fields
    return rows


def validate_recipe_data(recipe: object, *, check_digest: bool = True) -> dict:
    recipe = strict_keys(recipe, TOP_KEYS, "recipe")
    if recipe["schema"] != SCHEMA:
        raise reject("unsupported generated-source recipe schema")
    bounded_string(recipe["recipe_id"], "recipe_id", 128)
    if recipe["status"] != "bounded_cost_evidence":
        raise reject("recipe status must be bounded_cost_evidence")
    if recipe["resource_profile"] != RESOURCE_PROFILE:
        raise reject("recipe resource profile differs")
    digest_field(recipe["closure_sha256"], "closure_sha256")
    if check_digest and recipe["closure_sha256"] != recipe_digest(recipe):
        raise reject("recipe closure digest differs")

    runner = strict_keys(
        recipe["runner"],
        {"kind", "working_directory", "workspace_manifest", "package", "binary",
         "locked", "offline", "arguments", "environment"},
        "runner",
    )
    if runner["kind"] != "cargo-stdout-v1" or runner["working_directory"] != ".":
        raise reject("runner kind or working directory is unsupported")
    if runner["locked"] is not True or runner["offline"] is not True:
        raise reject("runner must remain locked and offline")
    bounded_string(runner["package"], "runner package")
    bounded_string(runner["binary"], "runner binary")
    workspace = repository_path(runner["workspace_manifest"], "runner workspace manifest")
    arguments = runner["arguments"]
    if not isinstance(arguments, list) or len(arguments) > MAX_ARGUMENTS:
        if isinstance(arguments, list):
            raise exhaust(f"runner arguments exceed {MAX_ARGUMENTS}")
        raise reject("runner arguments must be an array")
    for index, argument in enumerate(arguments):
        bounded_string(argument, f"runner argument[{index}]")
    environment = runner["environment"]
    if not isinstance(environment, list) or len(environment) > MAX_ENVIRONMENT:
        if isinstance(environment, list):
            raise exhaust(f"runner environment exceeds {MAX_ENVIRONMENT}")
        raise reject("runner environment must be an array")
    env_keys: list[str] = []
    for index, row in enumerate(environment):
        row = strict_keys(row, {"name", "value"}, f"environment[{index}]")
        name = bounded_string(row["name"], f"environment[{index}] name")
        bounded_string(row["value"], f"environment[{index}] value")
        if re.fullmatch(r"[A-Z][A-Z0-9_]*", name) is None:
            raise reject(f"environment[{index}] name is not canonical")
        env_keys.append(name)
    if env_keys != sorted(set(env_keys)):
        raise reject("runner environment must be sorted and unique")

    role, generator_path, generator_length = validate_file_row(
        recipe["generator"], "generator", {"role", "path", "byte_length", "sha256"}
    )
    if role != "generated_source_provider":
        raise reject("generator has the wrong role")
    if generator_length > MAX_GENERATOR_BYTES:
        raise exhaust(f"generator exceeds {MAX_GENERATOR_BYTES} bytes")

    inputs = recipe["repository_inputs"]
    if not isinstance(inputs, list):
        raise reject("repository_inputs must be an array")
    if len(inputs) > MAX_REPOSITORY_INPUTS:
        raise exhaust(f"repository inputs exceed {MAX_REPOSITORY_INPUTS}")
    if not inputs:
        raise reject("repository_inputs must not be empty")
    input_paths: list[str] = []
    input_roles: list[str] = []
    input_lengths = generator_length
    input_by_role: dict[str, str] = {}
    for index, row in enumerate(inputs):
        role, path, length = validate_file_row(
            row, f"repository_inputs[{index}]", {"role", "path", "byte_length", "sha256"}
        )
        if length > MAX_REPOSITORY_INPUT_BYTES:
            raise exhaust(f"repository_inputs[{index}] exceeds {MAX_REPOSITORY_INPUT_BYTES} bytes")
        input_paths.append(path)
        input_roles.append(role)
        input_lengths += length
        input_by_role[role] = path
    if input_paths != sorted(set(input_paths)) or len(set(input_roles)) != len(input_roles):
        raise reject("repository inputs must have sorted unique paths and roles")
    required_roles = {
        "generator_dependency_lock", "generator_workspace_manifest", "generator_package_manifest"
    }
    if set(input_roles) != required_roles:
        raise reject("repository input roles do not exactly close the version-1 recipe")
    if input_by_role["generator_workspace_manifest"] != workspace.relative_to(ROOT).as_posix():
        raise reject("runner workspace manifest is detached from repository inputs")
    dependency_lock = ROOT / input_by_role["generator_dependency_lock"]
    if dependency_lock.parent != workspace.parent or dependency_lock.name != "Cargo.lock":
        raise reject("runner dependency lock is detached from its workspace manifest")
    package_manifest = ROOT / input_by_role["generator_package_manifest"]
    expected_generator = package_manifest.parent / "src" / "bin" / f"{runner['binary']}.rs"
    if expected_generator.resolve() != (ROOT / generator_path).resolve():
        raise reject("runner binary is detached from the pinned generator source")
    package_text = package_manifest.read_text(encoding="utf-8")
    package_name = re.search(
        r'^\s*name\s*=\s*"([^"]+)"\s*$', package_text, re.MULTILINE
    )
    if package_name is None or package_name.group(1) != runner["package"]:
        raise reject("runner package is detached from its package manifest")
    if input_lengths > MAX_TOTAL_REPOSITORY_BYTES:
        raise exhaust(f"generator plus repository inputs exceed {MAX_TOTAL_REPOSITORY_BYTES} bytes")

    externals = recipe["external_inputs"]
    if not isinstance(externals, list):
        raise reject("external_inputs must be an array")
    if len(externals) > MAX_EXTERNAL_INPUTS:
        raise exhaust(f"external inputs exceed {MAX_EXTERNAL_INPUTS}")
    if not externals:
        raise reject("external_inputs must not be empty")
    lock = cargo_lock_rows(dependency_lock)
    identities: list[str] = []
    for index, row in enumerate(externals):
        row = strict_keys(
            row, {"identity", "kind", "name", "version", "source", "content_sha256"},
            f"external_inputs[{index}]",
        )
        identity = bounded_string(row["identity"], f"external_inputs[{index}] identity")
        name = bounded_string(row["name"], f"external_inputs[{index}] name")
        version = bounded_string(row["version"], f"external_inputs[{index}] version")
        source = bounded_string(row["source"], f"external_inputs[{index}] source", 128)
        checksum = digest_field(row["content_sha256"], f"external_inputs[{index}] content_sha256")
        if row["kind"] != "cargo_registry_package" or identity != f"{name}@{version}":
            raise reject(f"external_inputs[{index}] identity or kind differs")
        locked = lock.get((name, version))
        if locked is None or locked.get("source") != source or locked.get("checksum") != checksum:
            raise reject(f"external_inputs[{index}] differs from the dependency lock")
        identities.append(identity)
    if identities != sorted(set(identities)):
        raise reject("external inputs must be sorted and unique")

    output = strict_keys(
        recipe["output"], {"path", "byte_length", "sha256", "media_type"}, "output"
    )
    output_path = repository_path(output["path"], "output")
    output_raw = output_path.read_bytes()
    output_length = nonnegative(output["byte_length"], "output byte_length")
    if output["media_type"] != "text/x-omega":
        raise reject("output media type differs")
    if output_length > MAX_OUTPUT_BYTES:
        raise exhaust(f"output exceeds {MAX_OUTPUT_BYTES} bytes")
    if output_length != len(output_raw) or digest_field(output["sha256"], "output sha256") != digest(output_raw):
        raise reject("output exact file custody differs")

    join = strict_keys(
        recipe["omgcomp1_join"],
        {"package_key", "owner", "machine", "root_label", "generated_source_id", "sources"},
        "omgcomp1_join",
    )
    try:
        package_key = bytes.fromhex(join["package_key"])
    except (TypeError, ValueError) as error:
        raise reject("OMGCOMP1 package key is not hexadecimal") from error
    if len(package_key) != 32 or not any(package_key):
        raise reject("OMGCOMP1 package key must be 32 nonzero bytes")
    bounded_string(join["owner"], "OMGCOMP1 owner")
    bounded_string(join["machine"], "OMGCOMP1 machine")
    root_label = bounded_string(join["root_label"], "OMGCOMP1 root label", compilation.MAX_LABEL_BYTES)
    generated_source_id = nonnegative(join["generated_source_id"], "generated_source_id")
    sources = join["sources"]
    if not isinstance(sources, list) or not (1 <= len(sources) <= MAX_JOIN_SOURCES):
        if isinstance(sources, list) and len(sources) > MAX_JOIN_SOURCES:
            raise exhaust(f"OMGCOMP1 join sources exceed {MAX_JOIN_SOURCES}")
        raise reject("OMGCOMP1 join sources must be a nonempty array")
    labels: list[str] = []
    generated_ordinals: list[int] = []
    for index, row in enumerate(sources):
        if not isinstance(row, dict) or row.get("kind") not in ("generated_output", "repository"):
            raise reject(f"OMGCOMP1 join source[{index}] kind differs")
        expected = {"kind", "label", "module"}
        if row["kind"] == "repository":
            expected |= {"path", "byte_length", "sha256"}
        row = strict_keys(row, expected, f"OMGCOMP1 join source[{index}]")
        label = bounded_string(row["label"], f"OMGCOMP1 join source[{index}] label", compilation.MAX_LABEL_BYTES)
        if row["module"] != "":
            raise reject("version-1 focused join sources must share the root module")
        labels.append(label)
        if row["kind"] == "generated_output":
            generated_ordinals.append(index)
        else:
            validate_file_row(
                {"role": "join_repository_source", "path": row["path"],
                 "byte_length": row["byte_length"], "sha256": row["sha256"]},
                f"OMGCOMP1 join source[{index}]",
                {"role", "path", "byte_length", "sha256"},
            )
    if labels != sorted(set(labels)) or root_label not in labels:
        raise reject("OMGCOMP1 join labels must be sorted, unique, and contain the root")
    if generated_ordinals != [generated_source_id]:
        raise reject("generated_source_id does not name the unique generated source")

    return recipe


def canonical_recipe(path: Path) -> dict:
    raw = path.read_bytes()
    if len(raw) > MAX_RECIPE_BYTES:
        raise exhaust(f"recipe exceeds {MAX_RECIPE_BYTES} bytes")
    try:
        text = raw.decode("utf-8")
        recipe = json.loads(text)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise reject(f"recipe is not canonical UTF-8 JSON: {error}") from error
    if text != json.dumps(recipe, ensure_ascii=False, indent=2) + "\n":
        raise reject("recipe is not canonical pretty JSON")
    return validate_recipe_data(recipe)


def bounded_command(command: list[str], environment: dict[str, str], maximum: int) -> bytes:
    process = subprocess.Popen(
        command, cwd=ROOT, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    output = bytearray()
    errors = bytearray()
    started = time.monotonic()
    resource_error: CustodyError | None = None
    while selector.get_map():
        remaining = RUN_TIMEOUT_SECONDS - (time.monotonic() - started)
        if remaining <= 0:
            process.kill()
            process.wait()
            raise exhaust("generator execution exceeded the time ceiling")
        events = selector.select(min(remaining, 1.0))
        if not events and process.poll() is not None:
            events = [(key, 0) for key in list(selector.get_map().values())]
        for key, _ in events:
            chunk = os.read(key.fileobj.fileno(), 8192)
            if not chunk:
                selector.unregister(key.fileobj)
                continue
            target = output if key.data == "stdout" else errors
            target.extend(chunk)
            limit = maximum if key.data == "stdout" else MAX_STDERR_BYTES
            if len(target) > limit and resource_error is None:
                resource_error = exhaust(f"generator {key.data} exceeds {limit} bytes")
                process.kill()
    return_code = process.wait()
    if resource_error is not None:
        raise resource_error
    if return_code != 0:
        detail = bytes(errors[:1000]).decode("utf-8", errors="replace").strip()
        raise reject(f"generator exited {return_code}: {detail}")
    return bytes(output)


def runner_command(recipe: dict) -> tuple[list[str], dict[str, str]]:
    runner = recipe["runner"]
    command = [
        "cargo", "run", "-q", "--locked", "--offline", "--manifest-path",
        runner["workspace_manifest"], "-p", runner["package"], "--bin", runner["binary"],
    ]
    if runner["arguments"]:
        command += ["--", *runner["arguments"]]
    environment = dict(os.environ)
    for row in runner["environment"]:
        environment[row["name"]] = row["value"]
    return command, environment


def require_reproductions(first: bytes, second: bytes, expected: bytes) -> None:
    if first != second:
        raise reject("generator observations are nondeterministic")
    if first != expected:
        raise reject("generator observation differs from the committed output")


def reproduce(recipe: dict) -> bytes:
    command, environment = runner_command(recipe)
    first = bounded_command(command, environment, MAX_OUTPUT_BYTES)
    second = bounded_command(command, environment, MAX_OUTPUT_BYTES)
    expected = (ROOT / recipe["output"]["path"]).read_bytes()
    require_reproductions(first, second, expected)
    return first


def materialize_carrier(recipe: dict, generated: bytes) -> bytes:
    join = recipe["omgcomp1_join"]
    entries: list[source_bundle.Entry] = []
    for row in join["sources"]:
        content = generated if row["kind"] == "generated_output" else (ROOT / row["path"]).read_bytes()
        entries.append(source_bundle.Entry(row["label"], content))
    try:
        packed = source_bundle.encode(entries)
    except source_bundle.BundleError as error:
        raise reject(f"OMGCOMP1 nested source bundle rejects: {error}") from error
    key = join["package_key"]
    manifest = {
        "target": "linux_x86_64",
        "packages": [{
            "key": key,
            "sources": [{"label": row["label"], "module": row["module"]} for row in join["sources"]],
        }],
        "aliases": [],
        "root": {
            "package": key, "source": join["root_label"],
            "owner": join["owner"], "machine": join["machine"],
        },
    }
    try:
        carrier = compilation.encode_manifest(manifest, packed)
    except compilation.CompilationError as error:
        raise CustodyError(f"OMGCOMP1 materialization rejects: {error}", error.status) from error
    verify_carrier(recipe, carrier, generated)
    return carrier


def verify_carrier(recipe: dict, carrier: bytes, generated: bytes | None = None) -> None:
    try:
        decoded = compilation.decode(carrier)
    except compilation.CompilationError as error:
        raise CustodyError(f"OMGCOMP1 verification rejects: {error}", error.status) from error
    if generated is None:
        generated = (ROOT / recipe["output"]["path"]).read_bytes()
    source_id = recipe["omgcomp1_join"]["generated_source_id"]
    if source_id >= len(decoded.sources):
        raise reject("materialized generated source ID is absent")
    entry_id = decoded.sources[source_id].bundle_entry_id
    if entry_id >= len(decoded.bundle_entries):
        raise reject("materialized generated bundle entry is absent")
    content = decoded.bundle_entries[entry_id].content
    if content != generated or len(content) != recipe["output"]["byte_length"]:
        raise reject("materialized OMGCOMP1 generated source extent differs")
    if digest(content) != recipe["output"]["sha256"]:
        raise reject("materialized OMGCOMP1 generated source digest differs")


def expect_rejection(recipe: dict, name: str, mutate, expected_status: int = 251, *, refresh: bool = True) -> None:
    candidate = copy.deepcopy(recipe)
    mutate(candidate)
    if refresh:
        refresh_digest(candidate)
    try:
        validate_recipe_data(candidate)
    except CustodyError as error:
        if error.status != expected_status:
            raise reject(f"mutation {name} returned {error.status}, expected {expected_status}") from error
        return
    raise reject(f"mutation {name} was admitted")


def mutation_teeth(recipe: dict) -> None:
    expect_rejection(recipe, "unknown field", lambda value: value.__setitem__("unknown", 1))
    expect_rejection(recipe, "stale digest", lambda value: value["output"].__setitem__("media_type", "text/plain"), refresh=False)
    expect_rejection(recipe, "generator role", lambda value: value["generator"].__setitem__("role", "ordinary_input"))
    expect_rejection(recipe, "generator digest", lambda value: value["generator"].__setitem__("sha256", "0" * 64))
    expect_rejection(recipe, "input order", lambda value: value["repository_inputs"].reverse())
    expect_rejection(recipe, "input role", lambda value: value["repository_inputs"][0].__setitem__("role", "generator_package_manifest"))
    expect_rejection(recipe, "external checksum", lambda value: value["external_inputs"][0].__setitem__("content_sha256", "0" * 64))
    expect_rejection(recipe, "runner unlocked", lambda value: value["runner"].__setitem__("locked", False))
    expect_rejection(recipe, "runner workspace", lambda value: value["runner"].__setitem__("workspace_manifest", recipe["generator"]["path"]))
    expect_rejection(recipe, "output digest", lambda value: value["output"].__setitem__("sha256", "0" * 64))
    expect_rejection(recipe, "join generated source", lambda value: value["omgcomp1_join"].__setitem__("generated_source_id", 1))
    expect_rejection(recipe, "join source digest", lambda value: value["omgcomp1_join"]["sources"][1].__setitem__("sha256", "0" * 64))

    environment = dict(os.environ)
    exact_command = [sys.executable, "-c", f"import sys;sys.stdout.buffer.write(b'x'*{MAX_OUTPUT_BYTES})"]
    exact = bounded_command(exact_command, environment, MAX_OUTPUT_BYTES)
    if len(exact) != MAX_OUTPUT_BYTES:
        raise reject("exact capture ceiling did not publish its complete private result")
    adjacent_command = [sys.executable, "-c", f"import sys;sys.stdout.buffer.write(b'x'*{MAX_OUTPUT_BYTES + 1})"]
    try:
        bounded_command(adjacent_command, environment, MAX_OUTPUT_BYTES)
    except CustodyError as error:
        if error.status != 252:
            raise reject("adjacent capture did not select status 252") from error
    else:
        raise reject("adjacent capture was admitted")
    prefix_failure = [sys.executable, "-c", "import sys;sys.stdout.write('prefix');sys.exit(7)"]
    try:
        bounded_command(prefix_failure, environment, MAX_OUTPUT_BYTES)
    except CustodyError as error:
        if error.status != 251:
            raise reject("prefix failure did not select status 251") from error
    else:
        raise reject("prefix failure was admitted")
    try:
        require_reproductions(b"left", b"right", b"left")
    except CustodyError as error:
        if error.status != 251:
            raise reject("nondeterminism did not select status 251") from error
    else:
        raise reject("nondeterministic observations were admitted")

    publications: list[bytes] = []
    try:
        failed = bounded_command(prefix_failure, environment, MAX_OUTPUT_BYTES)
        publications.append(materialize_carrier(recipe, failed))
    except CustodyError:
        pass
    if publications:
        raise reject("failed generator published an OMGCOMP1 carrier")


def usage() -> None:
    raise SystemExit(
        "usage: generated_source_custody.py "
        "verify RECIPE | reproduce RECIPE | teeth RECIPE | materialize RECIPE | "
        "verify-carrier RECIPE CARRIER"
    )


def main() -> None:
    if len(sys.argv) < 3:
        usage()
    command = sys.argv[1]
    recipe_path = Path(sys.argv[2])
    recipe = canonical_recipe(recipe_path)
    if command == "verify" and len(sys.argv) == 3:
        return
    if command == "reproduce" and len(sys.argv) == 3:
        reproduce(recipe)
        return
    if command == "teeth" and len(sys.argv) == 3:
        mutation_teeth(recipe)
        return
    if command == "materialize" and len(sys.argv) == 3:
        generated = reproduce(recipe)
        carrier = materialize_carrier(recipe, generated)
        sys.stdout.buffer.write(carrier)
        return
    if command == "verify-carrier" and len(sys.argv) == 4:
        verify_carrier(recipe, Path(sys.argv[3]).read_bytes())
        return
    usage()


if __name__ == "__main__":
    try:
        main()
    except CustodyError as error:
        print(f"generated source custody: {error}", file=sys.stderr)
        raise SystemExit(error.status) from error
