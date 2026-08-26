#!/usr/bin/env python3
"""Verify exact product-compiler source checkpoint closures and provenance."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import subprocess
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[4]
CHECKPOINT_DIR = Path(__file__).resolve().parent
SCHEMA = "omega.product-compiler-source-checkpoint.v2"
SNAPSHOT_SCHEMA = "omega.source-closure-snapshot.v3"
DOMAIN = b"omega.product-compiler-source-checkpoint.v2\0"
BUILD_PRELUDE_OWNER = (
    "source/compiler/rust/omega/orchestration/omega-compiler/"
    "src/pipeline/stages.rs"
)
BUILD_PRELUDE_SNAPSHOT = "source/compiler/omega/source-checkpoints/inputs/build-prelude.omg"
GENERATOR_INPUT_ROLES = {
    "generator_dependency_lock",
    "generator_package_manifest",
    "generator_source_input",
    "generator_workspace_manifest",
}
PROVENANCE_ROLES = GENERATOR_INPUT_ROLES | {
    "generated_source_provider",
    "virtual_source_provider",
}
TOP_LEVEL_KEYS = {
    "schema",
    "checkpoint",
    "status",
    "claim",
    "entry_source",
    "build_source",
    "source_file_count",
    "content_set_sha256",
    "closure_sha256",
    "configurations",
    "compiled_sources",
    "virtual_sources",
    "dependency_aliases",
    "dependency_edges",
    "provenance_inputs",
    "generated_sources",
    "external_inputs",
}


class CheckpointError(Exception):
    pass


def fail(message: str) -> None:
    raise CheckpointError(message)


def strict_keys(value: object, expected: set[str], context: str) -> dict:
    if not isinstance(value, dict):
        fail(f"{context} must be an object")
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        unknown = sorted(actual - expected)
        fail(f"{context} keys differ; missing={missing}, unknown={unknown}")
    return value


def validate_relative_path(spelling: str, *, directory: bool = False) -> Path:
    if not isinstance(spelling, str):
        fail("repository path must be a string")
    path = PurePosixPath(spelling)
    if path.is_absolute() or ".." in path.parts or str(path) != spelling:
        fail(f"non-canonical repository path {spelling!r}")
    resolved = ROOT / spelling
    expected = resolved.is_dir() if directory else resolved.is_file()
    if not expected:
        kind = "directory" if directory else "file"
        fail(f"missing repository {kind} {spelling}")
    return resolved


def sha256(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def content_set_digest(manifest: dict) -> str:
    digest = hashlib.sha256()
    digest.update(b"omega.product-compiler-content-set.v2\0")
    for group in ("compiled_sources", "virtual_sources"):
        for row in manifest[group]:
            identity = row.get("path") if group == "compiled_sources" else row.get("identity")
            path = ROOT / row["path"]
            content = path.read_bytes()
            digest.update(group.encode("utf-8") + b"\0")
            digest.update(identity.encode("utf-8") + b"\0")
            digest.update(len(content).to_bytes(8, "little"))
            digest.update(content)
    return digest.hexdigest()


def closure_digest(manifest: dict) -> str:
    projection = {key: value for key, value in manifest.items() if key != "closure_sha256"}
    canonical = json.dumps(
        projection, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(DOMAIN + len(canonical).to_bytes(8, "little") + canonical).hexdigest()


def refresh_digests(manifest: dict) -> None:
    manifest["content_set_sha256"] = content_set_digest(manifest)
    manifest["closure_sha256"] = closure_digest(manifest)


def verify_build_prelude_snapshot() -> None:
    owner = (ROOT / BUILD_PRELUDE_OWNER).read_text(encoding="utf-8")
    marker = 'const BUILD_PRELUDE: &str = r#"'
    start = owner.find(marker)
    if start < 0:
        fail("cannot locate BUILD_PRELUDE in its pinned owner")
    start += len(marker)
    end = owner.find('"#;', start)
    if end < 0:
        fail("cannot locate the end of BUILD_PRELUDE in its pinned owner")
    if (ROOT / BUILD_PRELUDE_SNAPSHOT).read_bytes() != owner[start:end].encode("utf-8"):
        fail("build-prelude snapshot differs from the compiler-injected source")


def verify_source_rows(manifest: dict) -> tuple[dict[str, dict], dict[str, dict]]:
    compiled: dict[str, dict] = {}
    for index, row in enumerate(manifest["compiled_sources"]):
        row = strict_keys(
            row, {"byte_length", "origin", "path", "sha256"}, f"compiled_sources[{index}]"
        )
        path = row["path"]
        if row["origin"] not in ("repository", "toolchain"):
            fail(f"invalid compiled source origin for {path}")
        source = validate_relative_path(path).read_bytes()
        if row["byte_length"] != len(source) or row["sha256"] != sha256(source):
            fail(f"compiled source content mismatch for {path}")
        if path in compiled:
            fail(f"duplicate compiled source {path}")
        compiled[path] = row
    if list(compiled) != sorted(compiled):
        fail("compiled_sources must be sorted by path")

    virtual: dict[str, dict] = {}
    for index, row in enumerate(manifest["virtual_sources"]):
        row = strict_keys(
            row,
            {
                "byte_length",
                "identity",
                "origin",
                "path",
                "provider",
                "provider_symbol",
                "sha256",
            },
            f"virtual_sources[{index}]",
        )
        if row["origin"] != "virtual" or not (
            row["identity"].startswith("<") and row["identity"].endswith(">")
        ):
            fail(f"invalid virtual source identity {row['identity']!r}")
        source = validate_relative_path(row["path"]).read_bytes()
        validate_relative_path(row["provider"])
        if row["byte_length"] != len(source) or row["sha256"] != sha256(source):
            fail(f"virtual source content mismatch for {row['identity']}")
        if row["identity"] in virtual:
            fail(f"duplicate virtual source {row['identity']}")
        virtual[row["identity"]] = row
    if list(virtual) != sorted(virtual):
        fail("virtual_sources must be sorted by identity")
    if set(compiled) & {row["path"] for row in virtual.values()}:
        fail("compiled and virtual snapshot paths overlap")
    if manifest["source_file_count"] != len(compiled) + len(virtual):
        fail("source_file_count does not match resolved source inventory")
    return compiled, virtual


def cargo_lock_packages(path: Path) -> dict[str, dict[str, str]]:
    locked: dict[str, dict[str, str]] = {}
    for section in path.read_text(encoding="utf-8").split("[[package]]")[1:]:
        fields = {}
        for key in ("name", "version", "checksum"):
            match = re.search(rf'^\s*{key}\s*=\s*"([^"]+)"\s*$', section, re.MULTILINE)
            if match is not None:
                fields[key] = match.group(1)
        if set(fields) == {"name", "version", "checksum"}:
            locked[f"{fields['name']}@{fields['version']}"] = fields
    return locked


def verify_provenance(manifest: dict, compiled: dict[str, dict], virtual: dict[str, dict]) -> None:
    provenance: dict[str, dict] = {}
    for index, row in enumerate(manifest["provenance_inputs"]):
        row = strict_keys(row, {"path", "role", "sha256"}, f"provenance_inputs[{index}]")
        if row["role"] not in PROVENANCE_ROLES:
            fail(f"unknown provenance role {row['role']!r} for {row['path']}")
        content = validate_relative_path(row["path"]).read_bytes()
        if row["sha256"] != sha256(content):
            fail(f"provenance digest mismatch for {row['path']}")
        if row["path"] in provenance:
            fail(f"duplicate provenance input {row['path']}")
        provenance[row["path"]] = row
    if list(provenance) != sorted(provenance):
        fail("provenance_inputs must be sorted by path")

    externals: dict[str, dict] = {}
    for index, row in enumerate(manifest["external_inputs"]):
        row = strict_keys(
            row,
            {"name", "registry_sha256", "unicode_version", "version"},
            f"external_inputs[{index}]",
        )
        identity = f"{row['name']}@{row['version']}"
        if identity in externals:
            fail(f"duplicate external input {identity}")
        externals[identity] = row
    if list(externals) != sorted(externals):
        fail("external_inputs must be sorted by name and version")

    generated_paths = []
    generator_references: set[str] = set()
    input_references: set[str] = set()
    external_references: set[str] = set()
    for index, row in enumerate(manifest["generated_sources"]):
        row = strict_keys(
            row,
            {"external_inputs", "generator", "input_paths", "path"},
            f"generated_sources[{index}]",
        )
        if row["path"] not in compiled:
            fail(f"generated source {row['path']} is not compiled")
        generator = provenance.get(row["generator"])
        if generator is None:
            fail(f"generated source {row['path']} has an unpinned generator")
        if generator["role"] != "generated_source_provider":
            fail(f"generated source {row['path']} has a non-generator provider")
        generator_references.add(row["generator"])
        if row["input_paths"] != sorted(set(row["input_paths"])):
            fail(f"generated source {row['path']} inputs must be sorted and unique")
        dependency_locks = []
        for path in row["input_paths"]:
            source = provenance.get(path)
            if source is None:
                fail(f"generated source {row['path']} has an unpinned input")
            if source["role"] not in GENERATOR_INPUT_ROLES:
                fail(f"generated source {row['path']} has invalid input role {source['role']}")
            if source["role"] == "generator_dependency_lock":
                dependency_locks.append(path)
            input_references.add(path)
        if row["external_inputs"] != sorted(set(row["external_inputs"])):
            fail(f"generated source {row['path']} external inputs must be sorted and unique")
        if row["external_inputs"] and len(dependency_locks) != 1:
            fail(
                f"generated source {row['path']} with external inputs must reference "
                "exactly one generator dependency lock"
            )
        locked = (
            cargo_lock_packages(validate_relative_path(dependency_locks[0]))
            if dependency_locks
            else {}
        )
        # Provenance is generic: bind external package identity/checksum through
        # the declared lock and bind the generated file through compiled-source
        # custody. Payload-specific comments or headers are not a verifier rule.
        for identity in row["external_inputs"]:
            external = externals.get(identity)
            if external is None:
                fail(f"generated source {row['path']} has an unknown external input")
            package = locked.get(identity)
            if package is None or package["checksum"] != external["registry_sha256"]:
                fail(f"external input {identity} differs from the referenced dependency lock")
            external_references.add(identity)
        generated_paths.append(row["path"])
    if generated_paths != sorted(set(generated_paths)):
        fail("generated_sources must be sorted and unique")

    for path, row in provenance.items():
        if row["role"] == "generated_source_provider" and path not in generator_references:
            fail(f"orphan generated-source provider {path}")
        if row["role"] in GENERATOR_INPUT_ROLES and path not in input_references:
            fail(f"orphan generated-source input {path}")
    orphan_externals = set(externals) - external_references
    if orphan_externals:
        fail(f"orphan external inputs: {sorted(orphan_externals)}")

    virtual_provider_references = set()
    for identity, row in virtual.items():
        provider = provenance.get(row["provider"])
        if provider is None or provider["role"] != "virtual_source_provider":
            fail(f"virtual source {identity} has an unpinned provider")
        virtual_provider_references.add(row["provider"])
    orphan_virtual_providers = {
        path
        for path, row in provenance.items()
        if row["role"] == "virtual_source_provider" and path not in virtual_provider_references
    }
    if orphan_virtual_providers:
        fail(f"orphan virtual-source providers: {sorted(orphan_virtual_providers)}")


def bytes_text(expression: dict, context: str) -> str:
    if expression.get("kind") != "string" or not isinstance(expression.get("bytes"), list):
        fail(f"{context} is not a string literal")
    try:
        return bytes(expression["bytes"]).decode("utf-8")
    except (ValueError, UnicodeDecodeError) as error:
        fail(f"{context} is not UTF-8: {error}")
    raise AssertionError("unreachable")


def snapshot_source_map(snapshot: dict) -> dict[int, str]:
    result = {}
    for row in snapshot["sources"]:
        strict_keys(row, {"source_id", "identity", "origin", "byte_length", "sha256"}, "snapshot source")
        if row["source_id"] in result:
            fail(f"snapshot duplicates source id {row['source_id']}")
        result[row["source_id"]] = row["identity"]
    return result


def derive_dependency_aliases(snapshot: dict) -> list[dict]:
    source_ids = snapshot_source_map(snapshot)
    aliases = []
    for item in snapshot["syntax"]["root_items"]:
        if item.get("kind") != "machine" or item.get("name", {}).get("text") != "build":
            continue
        declared_in = source_ids.get(item["name"]["source_id"])
        if declared_in is None or declared_in.startswith("<"):
            fail("build machine has no physical declaring source")
        for state in item["states"]:
            for statement in state["statements"]:
                if statement.get("kind") != "call" or statement.get("target", {}).get("text") != "depend_as":
                    continue
                arguments = statement.get("arguments", [])
                if len(arguments) != 2:
                    fail("depend_as must have exactly two arguments")
                alias = bytes_text(arguments[0], "depend_as alias")
                source = arguments[1]
                if source.get("kind") != "struct_literal" or source.get("type_name", {}).get("text") != "Source":
                    fail("depend_as source is not a Source literal")
                fields = source.get("fields", [])
                if len(fields) != 1 or fields[0].get("name", {}).get("text") != "location":
                    fail("depend_as Source literal must contain only location")
                location = bytes_text(fields[0]["value"], "depend_as location")
                declared_path = ROOT / declared_in
                target = (declared_path.parent / location).resolve()
                try:
                    root = target.relative_to(ROOT.resolve()).as_posix()
                except ValueError:
                    fail(f"dependency alias {alias} escapes the repository")
                validate_relative_path(root, directory=True)
                aliases.append(
                    {"alias": alias, "declared_in": declared_in, "location": location, "root": root}
                )
    return sorted(aliases, key=lambda row: (row["alias"], row["declared_in"]))


def resolve_use(requester: str, request: str, aliases: dict[str, str]) -> str:
    segments = request.split("::")
    if not segments or any(not segment for segment in segments):
        fail(f"invalid use request {request!r}")
    if segments[0] in aliases:
        base = ROOT / aliases[segments[0]]
        segments = segments[1:]
    elif segments[0] == "omega":
        base = ROOT / "omega"
        segments = segments[1:]
    else:
        base = (ROOT / requester).parent
    unresolved = base.joinpath(*segments)
    candidates = [
        unresolved.with_suffix(".omg"),
        unresolved / "mod.omg",
        unresolved.with_suffix(".omega"),
        unresolved / "mod.omega",
    ]
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve().relative_to(ROOT.resolve()).as_posix()
    fail(f"use request {request!r} from {requester} does not resolve")
    raise AssertionError("unreachable")


def derive_dependency_edges(snapshot: dict, aliases: list[dict]) -> list[dict]:
    source_ids = snapshot_source_map(snapshot)
    alias_roots = {row["alias"]: row["root"] for row in aliases}
    edges = []
    for item in snapshot["syntax"]["root_items"]:
        if item.get("kind") != "use":
            continue
        path = item.get("path", [])
        if not path:
            fail("empty use path in syntax snapshot")
        requester = source_ids.get(path[0]["source_id"])
        if requester is None or requester.startswith("<"):
            fail("use item has no physical requester")
        request = "::".join(member["text"] for member in path)
        edges.append(
            {
                "from": requester,
                "kind": "use",
                "request": request,
                "to": resolve_use(requester, request, alias_roots),
            }
        )
    return sorted(edges, key=lambda row: (row["from"], row["kind"], row["request"], row["to"]))


def verify_resolver_snapshot(
    manifest: dict, snapshot: dict, compiled: dict[str, dict], virtual: dict[str, dict], target: str
) -> None:
    strict_keys(
        snapshot,
        {"schema", "entry_source", "selected_target", "native_provider_substitution", "sources", "syntax"},
        f"resolver snapshot {target}",
    )
    if snapshot["schema"] != SNAPSHOT_SCHEMA:
        fail(f"resolver snapshot {target} has unsupported schema")
    if snapshot["entry_source"] != manifest["entry_source"] or snapshot["selected_target"] != target:
        fail(f"resolver snapshot {target} has wrong entry or target")
    if snapshot["native_provider_substitution"] is not True:
        fail(f"resolver snapshot {target} did not use native source resolution")

    expected = {
        **{
            path: {
                "origin": row["origin"],
                "byte_length": row["byte_length"],
                "sha256": row["sha256"],
            }
            for path, row in compiled.items()
        },
        **{
            identity: {
                "origin": row["origin"],
                "byte_length": row["byte_length"],
                "sha256": row["sha256"],
            }
            for identity, row in virtual.items()
        },
    }
    actual = {
        row["identity"]: {
            "origin": row["origin"],
            "byte_length": row["byte_length"],
            "sha256": row["sha256"],
        }
        for row in snapshot["sources"]
    }
    if actual != expected:
        fail(f"resolver snapshot {target} source closure differs from manifest")

    aliases = derive_dependency_aliases(snapshot)
    if aliases != manifest["dependency_aliases"]:
        fail(f"resolver snapshot {target} dependency aliases differ from manifest")
    edges = derive_dependency_edges(snapshot, aliases)
    if edges != manifest["dependency_edges"]:
        fail(f"resolver snapshot {target} dependency edges differ from manifest")
    target_names = {
        item["name"]["text"]
        for item in snapshot["syntax"]["root_items"]
        if item.get("kind") == "target"
    }
    if target not in target_names:
        fail(f"resolver snapshot {target} does not declare its selected target")


def load_resolver_snapshot(manifest: dict, target: str) -> dict:
    command = [
        "cargo",
        "run",
        "-q",
        "--locked",
        "--offline",
        "-p",
        "omega-compiler",
        "--bin",
        "omega-source-snapshot",
        "--",
        "--repository-root",
        str(ROOT),
        "--target",
        target,
        manifest["entry_source"],
    ]
    completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
    if completed.returncode != 0:
        fail(f"resolver snapshot {target} failed: {completed.stderr.strip()}")
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        fail(f"resolver snapshot {target} emitted invalid JSON: {error}")
    raise AssertionError("unreachable")


def verify_data(manifest: dict, snapshots: dict[str, dict] | None) -> None:
    strict_keys(manifest, TOP_LEVEL_KEYS, "manifest")
    if manifest["schema"] != SCHEMA or manifest["checkpoint"] != 1:
        fail("unsupported checkpoint schema or number")
    if manifest["status"] != "provisional" or not isinstance(manifest["claim"], str):
        fail("checkpoint status/claim is malformed")

    configurations = manifest["configurations"]
    targets = []
    for index, row in enumerate(configurations):
        row = strict_keys(
            row,
            {"native_provider_substitution", "target"},
            f"configurations[{index}]",
        )
        if row["native_provider_substitution"] is not True:
            fail("checkpoint configurations must trace native source resolution")
        targets.append(row["target"])
    if targets != sorted(set(targets)) or not targets:
        fail("configurations must have sorted unique targets")

    compiled, virtual = verify_source_rows(manifest)
    if manifest["entry_source"] not in compiled or manifest["build_source"] not in compiled:
        fail("entry_source and build_source must be compiled roots")
    if Path(manifest["entry_source"]).name != "main.omg" or Path(manifest["build_source"]).name != "build.omg":
        fail("checkpoint roots must name main.omg and build.omg")
    if Path(manifest["entry_source"]).parent != Path(manifest["build_source"]).parent:
        fail("build_source is not the entry companion")

    aliases = manifest["dependency_aliases"]
    alias_keys = []
    for index, row in enumerate(aliases):
        row = strict_keys(
            row, {"alias", "declared_in", "location", "root"}, f"dependency_aliases[{index}]"
        )
        if row["declared_in"] not in compiled:
            fail(f"dependency alias {row['alias']} has unknown declaring source")
        validate_relative_path(row["root"], directory=True)
        alias_keys.append((row["alias"], row["declared_in"]))
    if alias_keys != sorted(set(alias_keys)):
        fail("dependency_aliases must be sorted and unique")

    edge_keys = []
    for index, row in enumerate(manifest["dependency_edges"]):
        row = strict_keys(row, {"from", "kind", "request", "to"}, f"dependency_edges[{index}]")
        if row["kind"] != "use" or row["from"] not in compiled or row["to"] not in compiled:
            fail("dependency edge kind or endpoint is invalid")
        edge_keys.append((row["from"], row["kind"], row["request"], row["to"]))
    if edge_keys != sorted(set(edge_keys)):
        fail("dependency_edges must be sorted and unique")
    incoming = {row["to"] for row in manifest["dependency_edges"]}
    unreachable = set(compiled) - {manifest["entry_source"], manifest["build_source"]} - incoming
    if unreachable:
        fail(f"compiled sources have no incoming dependency edge: {sorted(unreachable)}")

    verify_provenance(manifest, compiled, virtual)
    verify_build_prelude_snapshot()
    if manifest["content_set_sha256"] != content_set_digest(manifest):
        fail("content-set digest mismatch")
    if manifest["closure_sha256"] != closure_digest(manifest):
        fail("closure digest mismatch")

    if snapshots is not None:
        if set(snapshots) != set(targets):
            fail("resolver snapshot target set differs from configurations")
        for target in targets:
            verify_resolver_snapshot(manifest, snapshots[target], compiled, virtual, target)


def mutation_teeth(manifest: dict, snapshots: dict[str, dict]) -> None:
    mutations = []

    def add(name: str, mutate) -> None:
        candidate = copy.deepcopy(manifest)
        mutate(candidate)
        refresh_digests(candidate)
        mutations.append((name, candidate))

    add("unknown field", lambda value: value.__setitem__("unknown", 1))
    add("bogus entry", lambda value: value.__setitem__("entry_source", "missing/main.omg"))
    add(
        "rewired edge",
        lambda value: value["dependency_edges"][0].__setitem__(
            "to", "source/compiler/omega/psi/lex/lexer.omg"
        ),
    )
    add(
        "bogus alias root",
        lambda value: value["dependency_aliases"][0].__setitem__("root", "source/compiler/omega/omega"),
    )
    add(
        "external checksum",
        lambda value: value["external_inputs"][0].__setitem__("registry_sha256", "0" * 64),
    )

    def wrong_generator_role(value: dict) -> None:
        generator_path = value["generated_sources"][0]["generator"]
        generator = next(
            row for row in value["provenance_inputs"] if row["path"] == generator_path
        )
        generator["role"] = "generator_source_input"

    add("wrong generated-source provider role", wrong_generator_role)

    def omit_generated_input(value: dict) -> None:
        value["generated_sources"][0]["input_paths"].pop()

    add("omitted generated-source input", omit_generated_input)

    def duplicate_generated_input(value: dict) -> None:
        inputs = value["generated_sources"][0]["input_paths"]
        inputs.append(inputs[0])
        inputs.sort()

    add("duplicate generated-source input", duplicate_generated_input)

    def orphan_generated_input(value: dict) -> None:
        path = "source/compiler/omega/README.md"
        value["provenance_inputs"].append(
            {
                "path": path,
                "role": "generator_source_input",
                "sha256": sha256((ROOT / path).read_bytes()),
            }
        )
        value["provenance_inputs"].sort(key=lambda row: row["path"])

    add("orphan generated-source input", orphan_generated_input)

    def orphan_external_input(value: dict) -> None:
        existing = {
            f"{row['name']}@{row['version']}" for row in value["external_inputs"]
        }
        identity, package = next(
            (identity, package)
            for identity, package in sorted(cargo_lock_packages(ROOT / "Cargo.lock").items())
            if identity not in existing
        )
        value["external_inputs"].append(
            {
                "name": package["name"],
                "registry_sha256": package["checksum"],
                "unicode_version": "0.0.0",
                "version": package["version"],
            }
        )
        value["external_inputs"].sort(key=lambda row: (row["name"], row["version"]))

    add("orphan external input", orphan_external_input)

    def omit_source(value: dict) -> None:
        value["compiled_sources"].pop(2)
        value["source_file_count"] -= 1

    add("omitted source", omit_source)

    def pad_source(value: dict) -> None:
        path = "source/compiler/omega/README.md"
        content = (ROOT / path).read_bytes()
        value["compiled_sources"].append(
            {
                "byte_length": len(content),
                "origin": "repository",
                "path": path,
                "sha256": sha256(content),
            }
        )
        value["compiled_sources"].sort(key=lambda row: row["path"])
        value["source_file_count"] += 1

    add("padded source", pad_source)

    for name, candidate in mutations:
        try:
            verify_data(candidate, snapshots)
        except CheckpointError:
            continue
        fail(f"mutation tooth did not reject {name}")


def verify(manifest_path: Path, *, content_only: bool) -> None:
    text = manifest_path.read_text(encoding="utf-8")
    manifest = json.loads(text)
    canonical = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    if text != canonical:
        fail(f"{manifest_path.name} is not canonical pretty JSON")
    snapshots = None
    if not content_only:
        snapshots = {
            row["target"]: load_resolver_snapshot(manifest, row["target"])
            for row in manifest.get("configurations", [])
        }
    verify_data(manifest, snapshots)
    if snapshots is not None:
        mutation_teeth(manifest, snapshots)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--content-only",
        action="store_true",
        help="skip resolver replay; never use this mode as checkpoint acceptance",
    )
    arguments = parser.parse_args()
    manifests = sorted(CHECKPOINT_DIR.glob("checkpoint-*.json"))
    if not manifests:
        fail("no checkpoint manifests")
    for manifest in manifests:
        verify(manifest, content_only=arguments.content_only)
    mode = "content only" if arguments.content_only else "resolver-exact with mutation teeth"
    print(f"verified {len(manifests)} product source checkpoint manifest(s): {mode}")


if __name__ == "__main__":
    try:
        main()
    except (CheckpointError, json.JSONDecodeError) as error:
        raise SystemExit(f"product source checkpoint: {error}") from error
