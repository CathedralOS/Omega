#!/usr/bin/env python3
"""Strict, path-independent Delta source-closure snapshot verifier."""

from __future__ import annotations

import copy
import hashlib
import json
import re
import sys
import tempfile
from pathlib import Path, PurePosixPath


SCHEMA = "omega.delta-source-closure-snapshot.v1"
LOCATOR_SCHEMA = "omega.delta-source-closure-locations.v1"
CONTENT_DOMAIN = b"omega.delta-source-content-set.v1\0"
CLOSURE_DOMAIN = b"omega.delta-source-closure-snapshot.v1\0"
MAX_DOCUMENT = 65_536
MAX_SOURCE = 524_288
MAX_AGGREGATE = 2_097_152
LIMITS = {
    "profiles": 32,
    "sources": 128,
    "source_edges": 512,
    "generated_inputs": 64,
    "build_units": 32,
    "tool_artifacts": 32,
    "artifacts": 128,
    "artifact_edges": 512,
}
TOP_KEYS = {
    "schema", "snapshot_id", "status", "claim", "profiles", "sources",
    "source_edges", "generated_inputs", "build_units", "tool_artifacts",
    "artifacts", "artifact_edges", "content_set_sha256", "closure_sha256",
}
ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
SHA_RE = re.compile(r"^[0-9a-f]{64}$")


class SnapshotError(Exception):
    status = 251


class SnapshotResourceError(SnapshotError):
    status = 252


def fail(message: str) -> None:
    raise SnapshotError(message)


def resource(message: str) -> None:
    raise SnapshotResourceError(message)


def strict(value: object, keys: set[str], context: str) -> dict:
    if not isinstance(value, dict) or set(value) != keys:
        fail(f"{context} fields")
    return value


def rows(value: object, context: str, limit: int) -> list:
    if not isinstance(value, list):
        fail(f"{context} rows")
    if len(value) > limit:
        resource(f"{context} row ceiling")
    return value


def identity(value: object, context: str) -> str:
    if not isinstance(value, str) or len(value.encode()) > 128 or not ID_RE.fullmatch(value):
        fail(f"{context} identity")
    return value


def sha(value: object, context: str) -> str:
    if not isinstance(value, str) or not SHA_RE.fullmatch(value):
        fail(f"{context} SHA-256")
    return value


def uint(value: object, context: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0 or value > 0xFFFFFFFF:
        fail(f"{context} integer")
    return value


def sorted_unique(values: object, context: str) -> list[str]:
    if not isinstance(values, list):
        fail(f"{context} list")
    checked = [identity(value, context) for value in values]
    if checked != sorted(set(checked)):
        fail(f"{context} order/uniqueness")
    return checked


def canonical_json(value: object, *, pretty: bool) -> bytes:
    options = {"ensure_ascii": False, "sort_keys": True}
    if pretty:
        return (json.dumps(value, indent=2, **options) + "\n").encode()
    return json.dumps(value, separators=(",", ":"), **options).encode()


def load_document(path: Path, context: str) -> tuple[dict, bytes]:
    raw = path.read_bytes()
    if len(raw) > MAX_DOCUMENT:
        resource(f"{context} byte ceiling")
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"{context} JSON: {error}")
    if not isinstance(value, dict):
        fail(f"{context} object")
    # Git may materialize tracked text with CRLF on Windows. Canonical identity
    # is the parsed document's LF encoding, not the checkout convention.
    normalized = raw.replace(b"\r\n", b"\n")
    if b"\r" in normalized or normalized != canonical_json(value, pretty=True):
        fail(f"{context} noncanonical JSON")
    return value, raw


def path_free(value: object, context: str = "snapshot") -> None:
    if isinstance(value, dict):
        for key, item in value.items():
            path_free(key, context)
            path_free(item, context)
    elif isinstance(value, list):
        for item in value:
            path_free(item, context)
    elif isinstance(value, str):
        lowered = value.lower()
        if ("/" in value or "\\" in value
                or lowered.endswith((".alp", ".delta", ".json"))):
            fail(f"{context} contains path spelling")


def parse_roles(arguments: list[str]) -> dict[str, Path]:
    result: dict[str, Path] = {}
    for argument in arguments:
        if "=" not in argument:
            fail("role argument")
        name, spelling = argument.split("=", 1)
        identity(name, "repository role")
        root = Path(spelling)
        if name in result or not root.is_dir():
            fail("repository role root")
        result[name] = root.resolve()
    return result


def locator_path(row: dict, roots: dict[str, Path], context: str) -> Path:
    role = identity(row["repository_role"], f"{context} role")
    if role not in roots or not isinstance(row["relative_path"], str):
        fail(f"{context} repository role/path")
    relative = PurePosixPath(row["relative_path"])
    if relative.is_absolute() or ".." in relative.parts or "." in relative.parts or str(relative) != row["relative_path"]:
        fail(f"{context} noncanonical relative path")
    candidate = roots[role].joinpath(*relative.parts)
    if not candidate.is_file():
        fail(f"{context} missing file")
    return candidate


def load_locations(path: Path, snapshot_id: str, roots: dict[str, Path]) -> tuple[dict[str, bytes], dict[str, tuple[bytes, bytes]], dict[str, bytes]]:
    value, _ = load_document(path, "locations")
    strict(value, {"schema", "snapshot_id", "sources", "tool_artifacts", "artifacts"}, "locations")
    if value["schema"] != LOCATOR_SCHEMA or value["snapshot_id"] != snapshot_id:
        fail("locations identity")

    sources: dict[str, bytes] = {}
    for index, item in enumerate(rows(value["sources"], "source locators", LIMITS["sources"])):
        row = strict(item, {"id", "repository_role", "relative_path"}, f"source locator {index}")
        row_id = identity(row["id"], "source locator")
        if row_id in sources:
            fail("duplicate source locator")
        sources[row_id] = locator_path(row, roots, "source locator").read_bytes()

    tools: dict[str, tuple[bytes, bytes]] = {}
    for index, item in enumerate(rows(value["tool_artifacts"], "tool locators", LIMITS["tool_artifacts"])):
        row = strict(item, {"id", "repository_role", "relative_path", "manifest_relative_path"}, f"tool locator {index}")
        row_id = identity(row["id"], "tool locator")
        if row_id in tools:
            fail("duplicate tool locator")
        artifact = locator_path(row, roots, "tool locator").read_bytes()
        manifest_row = {"repository_role": row["repository_role"], "relative_path": row["manifest_relative_path"]}
        manifest = locator_path(manifest_row, roots, "tool manifest locator").read_bytes()
        tools[row_id] = (artifact, manifest)

    artifacts: dict[str, bytes] = {}
    for index, item in enumerate(rows(value["artifacts"], "artifact locators", LIMITS["artifacts"])):
        row = strict(item, {"id", "repository_role", "relative_path"}, f"artifact locator {index}")
        row_id = identity(row["id"], "artifact locator")
        if row_id in artifacts:
            fail("duplicate artifact locator")
        artifacts[row_id] = locator_path(row, roots, "artifact locator").read_bytes()
    return sources, tools, artifacts


def graph_acyclic(nodes: set[str], edges: list[tuple[str, str]], context: str) -> None:
    outgoing = {node: [] for node in nodes}
    incoming = {node: 0 for node in nodes}
    for left, right in edges:
        if left not in nodes or right not in nodes:
            fail(f"{context} endpoint")
        outgoing[left].append(right)
        incoming[right] += 1
    ready = [node for node in sorted(nodes) if incoming[node] == 0]
    seen = 0
    while ready:
        node = ready.pop(0)
        seen += 1
        for target in outgoing[node]:
            incoming[target] -= 1
            if incoming[target] == 0:
                ready.append(target)
                ready.sort()
    if seen != len(nodes):
        fail(f"{context} cycle")


def closure_digest(manifest: dict) -> str:
    projection = {key: value for key, value in manifest.items() if key != "closure_sha256"}
    compact = canonical_json(projection, pretty=False)
    return hashlib.sha256(CLOSURE_DOMAIN + len(compact).to_bytes(8, "little") + compact).hexdigest()


def content_digest(source_bytes: dict[str, bytes], generated: dict[str, bytes]) -> str:
    digest = hashlib.sha256()
    digest.update(CONTENT_DOMAIN)
    for group, values in ((b"source", source_bytes), (b"generated_input", generated)):
        for row_id in sorted(values):
            content = values[row_id]
            digest.update(group + b"\0" + row_id.encode() + b"\0")
            digest.update(len(content).to_bytes(8, "little"))
            digest.update(content)
    return digest.hexdigest()


def verify_data(manifest: dict, locations_path: Path, roots: dict[str, Path], *, check_digests: bool = True) -> tuple[str, str]:
    strict(manifest, TOP_KEYS, "snapshot")
    path_free(manifest)
    if manifest["schema"] != SCHEMA:
        fail("snapshot schema")
    snapshot_id = identity(manifest["snapshot_id"], "snapshot")
    if manifest["status"] != "canonical_compiler_root":
        fail("snapshot status")
    if not isinstance(manifest["claim"], str) or not manifest["claim"] or len(manifest["claim"].encode()) > 512:
        fail("snapshot claim")
    sha(manifest["content_set_sha256"], "content set")
    sha(manifest["closure_sha256"], "closure")

    profiles: dict[str, dict] = {}
    for index, item in enumerate(rows(manifest["profiles"], "profiles", LIMITS["profiles"])):
        row = strict(item, {"id", "kind", "target", "configuration", "abi", "resource"}, f"profile {index}")
        row_id = identity(row["id"], "profile")
        if row_id in profiles or row["kind"] not in ("build_host", "final_target"):
            fail("profile identity/kind")
        for key in ("target", "configuration", "abi", "resource"):
            identity(row[key], f"profile {key}")
        profiles[row_id] = row
    if list(profiles) != sorted(profiles):
        fail("profile order")

    located_sources, located_tools, located_artifacts = load_locations(locations_path, snapshot_id, roots)
    source_map: dict[str, dict] = {}
    raw_sources: dict[str, bytes] = {}
    aggregate = 0
    for index, item in enumerate(rows(manifest["sources"], "sources", LIMITS["sources"])):
        row = strict(item, {"id", "roles", "byte_length", "sha256"}, f"source {index}")
        row_id = identity(row["id"], "source")
        sorted_unique(row["roles"], "source roles")
        extent = uint(row["byte_length"], "source extent")
        if extent > MAX_SOURCE:
            resource("source byte ceiling")
        sha(row["sha256"], "source")
        if row_id in source_map or row_id not in located_sources:
            fail("source identity/locator")
        content = located_sources[row_id]
        if len(content) != extent or hashlib.sha256(content).hexdigest() != row["sha256"]:
            fail("source extent/digest")
        source_map[row_id] = row
        raw_sources[row_id] = content
        aggregate += extent
    if list(source_map) != sorted(source_map) or set(located_sources) != set(source_map):
        fail("source order/locator closure")

    source_graph: list[tuple[str, str]] = []
    edge_keys = []
    for index, item in enumerate(rows(manifest["source_edges"], "source edges", LIMITS["source_edges"])):
        row = strict(item, {"from", "to", "relation"}, f"source edge {index}")
        key = (identity(row["from"], "source edge"), identity(row["to"], "source edge"), row["relation"])
        if row["relation"] != "depends_on":
            fail("source edge relation")
        edge_keys.append(key)
        source_graph.append(key[:2])
    if edge_keys != sorted(set(edge_keys)):
        fail("source edge order/uniqueness")
    graph_acyclic(set(source_map), source_graph, "source graph")

    generated_map: dict[str, dict] = {}
    generated_bytes: dict[str, bytes] = {}
    generated_source_members: list[str] = []
    for index, item in enumerate(rows(manifest["generated_inputs"], "generated inputs", LIMITS["generated_inputs"])):
        row = strict(item, {"id", "role", "recipe", "inputs", "byte_length", "sha256"}, f"generated input {index}")
        row_id = identity(row["id"], "generated input")
        identity(row["role"], "generated input role")
        if row["recipe"] != "ordered-source-bytes-plus-lf-v1":
            fail("generated input recipe")
        inputs = rows(row["inputs"], "generated input members", LIMITS["sources"])
        materialized = bytearray()
        for ordinal, member in enumerate(inputs):
            member = strict(member, {"kind", "id", "ordinal"}, "generated input member")
            if member["kind"] != "source" or member["ordinal"] != ordinal:
                fail("generated input member kind/order")
            member_id = identity(member["id"], "generated input member")
            if member_id not in raw_sources:
                fail("generated input source")
            generated_source_members.append(member_id)
            materialized += raw_sources[member_id] + b"\n"
        extent = uint(row["byte_length"], "generated input extent")
        if extent > MAX_SOURCE:
            resource("generated input byte ceiling")
        sha(row["sha256"], "generated input")
        if row_id in generated_map or len(materialized) != extent or hashlib.sha256(materialized).hexdigest() != row["sha256"]:
            fail("generated input identity/extent/digest")
        generated_map[row_id] = row
        generated_bytes[row_id] = bytes(materialized)
        aggregate += extent
    if list(generated_map) != sorted(generated_map):
        fail("generated input order")
    if aggregate > MAX_AGGREGATE:
        resource("aggregate content ceiling")

    tools: dict[str, dict] = {}
    for index, item in enumerate(rows(manifest["tool_artifacts"], "tool artifacts", LIMITS["tool_artifacts"])):
        row = strict(item, {"id", "role", "byte_length", "sha256", "manifest_sha256", "build_host_profile"}, f"tool artifact {index}")
        row_id = identity(row["id"], "tool artifact")
        identity(row["role"], "tool artifact role")
        extent = uint(row["byte_length"], "tool artifact extent")
        sha(row["sha256"], "tool artifact")
        sha(row["manifest_sha256"], "tool manifest")
        if row["build_host_profile"] not in profiles or profiles[row["build_host_profile"]]["kind"] != "build_host":
            fail("tool build-host profile")
        if row_id in tools or row_id not in located_tools:
            fail("tool identity/locator")
        artifact, tool_manifest = located_tools[row_id]
        if len(artifact) != extent or hashlib.sha256(artifact).hexdigest() != row["sha256"] or hashlib.sha256(tool_manifest).hexdigest() != row["manifest_sha256"]:
            fail("tool artifact/manifest custody")
        tools[row_id] = row
    if list(tools) != sorted(tools) or set(tools) != set(located_tools):
        fail("tool order/locator closure")

    artifacts: dict[str, dict] = {}
    for index, item in enumerate(rows(manifest["artifacts"], "artifacts", LIMITS["artifacts"])):
        row = strict(item, {"id", "role", "byte_length", "sha256", "producer"}, f"artifact {index}")
        row_id = identity(row["id"], "artifact")
        identity(row["role"], "artifact role")
        extent = uint(row["byte_length"], "artifact extent")
        sha(row["sha256"], "artifact")
        if row_id in artifacts or row_id not in located_artifacts:
            fail("artifact identity/locator")
        content = located_artifacts[row_id]
        if len(content) != extent or hashlib.sha256(content).hexdigest() != row["sha256"]:
            fail("artifact extent/digest")
        artifacts[row_id] = row
    if list(artifacts) != sorted(artifacts) or set(artifacts) != set(located_artifacts):
        fail("artifact order/locator closure")

    builds: dict[str, dict] = {}
    for index, item in enumerate(rows(manifest["build_units"], "build units", LIMITS["build_units"])):
        row = strict(item, {"id", "roles", "input_kind", "input_id", "compiler_tool", "output_artifacts", "build_host_profile", "final_target_profile"}, f"build unit {index}")
        row_id = identity(row["id"], "build unit")
        roles_ = sorted_unique(row["roles"], "build roles")
        if row["input_kind"] != "generated_input" or row["input_id"] not in generated_map:
            fail("build input")
        if row["build_host_profile"] not in profiles or profiles[row["build_host_profile"]]["kind"] != "build_host":
            fail("build host profile")
        if row["final_target_profile"] not in profiles or profiles[row["final_target_profile"]]["kind"] != "final_target":
            fail("build final-target profile")
        outputs = sorted_unique(row["output_artifacts"], "build outputs")
        if row["compiler_tool"] == "none":
            if roles_ != sorted(["source_image_only", "canonical_compiler_root"]) or outputs:
                fail("source-image-only build")
        elif row["compiler_tool"] not in tools:
            fail("build compiler tool")
        if row_id in builds:
            fail("duplicate build unit")
        builds[row_id] = row
    if list(builds) != sorted(builds):
        fail("build order")

    for row in artifacts.values():
        if row["producer"] != "external" and row["producer"] not in builds:
            fail("artifact producer")
    for build in builds.values():
        for artifact_id in build["output_artifacts"]:
            if artifact_id not in artifacts or artifacts[artifact_id]["producer"] != build["id"]:
                fail("build output custody")

    artifact_keys = []
    artifact_graph: list[tuple[str, str]] = []
    for index, item in enumerate(rows(manifest["artifact_edges"], "artifact edges", LIMITS["artifact_edges"])):
        row = strict(item, {"from", "to", "relation"}, f"artifact edge {index}")
        key = (identity(row["from"], "artifact edge"), identity(row["to"], "artifact edge"), row["relation"])
        if row["relation"] not in ("materializes", "runtime_input", "produces"):
            fail("artifact edge relation")
        artifact_keys.append(key)
        artifact_graph.append(key[:2])
    if artifact_keys != sorted(set(artifact_keys)):
        fail("artifact edge order/uniqueness")
    graph_acyclic(set(artifacts), artifact_graph, "artifact graph")

    if len(source_map) != 1 or len(builds) != 1 or source_graph:
        fail("canonical compiler root profile")
    source = next(iter(source_map.values()))
    if source["roles"] != ["canonical_compiler_root", "delta_source", "entry"]:
        fail("canonical compiler source roles")

    computed_content = content_digest(raw_sources, generated_bytes)
    computed_closure = closure_digest(manifest)
    if check_digests and manifest["content_set_sha256"] != computed_content:
        fail("content-set commitment")
    if check_digests and manifest["closure_sha256"] != computed_closure:
        fail("closure commitment")
    return computed_content, computed_closure


def verify_paths(manifest_path: Path, locations_path: Path, roots: dict[str, Path], *, check_digests: bool = True) -> tuple[dict, str, str]:
    manifest, _ = load_document(manifest_path, "snapshot")
    content, closure = verify_data(manifest, locations_path, roots, check_digests=check_digests)
    return manifest, content, closure


def mutation_teeth(manifest_path: Path, locations_path: Path, roots: dict[str, Path]) -> None:
    manifest, _, _ = verify_paths(manifest_path, locations_path, roots)
    tests: list[tuple[str, callable, int]] = []

    def add(name: str, mutate, status: int = 251) -> None:
        tests.append((name, mutate, status))

    add("path in canonical claim", lambda value: value.__setitem__("claim", "source/delta source"))
    add("source role drift", lambda value: value["sources"][0]["roles"].pop())
    add("source digest drift", lambda value: value["sources"][0].__setitem__("sha256", "0" * 64))
    add("invented dependency cycle", lambda value: value["source_edges"].append({"from": value["sources"][0]["id"], "relation": "depends_on", "to": value["sources"][0]["id"]}))
    add("normalizing recipe", lambda value: value["generated_inputs"][0].__setitem__("recipe", "strip-comments-v1"))
    add("generated digest drift", lambda value: value["generated_inputs"][0].__setitem__("sha256", "0" * 64))
    final_profile = next(row["id"] for row in manifest["profiles"] if row["kind"] == "final_target")
    host_profile = next(row["id"] for row in manifest["profiles"] if row["kind"] == "build_host")
    add("host/final profile swap", lambda value: value["build_units"][0].__setitem__("build_host_profile", final_profile))
    if manifest["status"] == "canonical_compiler_root":
        add("source-only output", lambda value: value["build_units"][0]["output_artifacts"].append("artifact.missing"))
        add("orphan tool manifest", lambda value: value["tool_artifacts"].append({"build_host_profile": host_profile, "byte_length": 1, "id": "tool.orphan", "manifest_sha256": "0" * 64, "role": "compiler", "sha256": "0" * 64}))
    else:
        add("missing compiler tool", lambda value: value["build_units"][0].__setitem__("compiler_tool", "none"))
        add("artifact-flow omission", lambda value: value["artifact_edges"].clear())
        add("tool digest drift", lambda value: value["tool_artifacts"][0].__setitem__("sha256", "0" * 64))
    add("source ceiling", lambda value: value["sources"][0].__setitem__("byte_length", MAX_SOURCE + 1), 252)

    for name, mutate, expected in tests:
        candidate = copy.deepcopy(manifest)
        mutate(candidate)
        candidate["closure_sha256"] = closure_digest(candidate)
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "candidate.json"
            path.write_bytes(canonical_json(candidate, pretty=True))
            try:
                verify_paths(path, locations_path, roots)
            except SnapshotError as error:
                if error.status != expected:
                    fail(f"mutation {name} returned {error.status}, expected {expected}")
            else:
                fail(f"mutation {name} accepted")


def usage() -> None:
    print("usage: source_closure_snapshot_v1.py verify|refresh|mutations SNAPSHOT LOCATIONS ROLE=ROOT...", file=sys.stderr)
    raise SystemExit(2)


def main() -> None:
    if len(sys.argv) < 5 or sys.argv[1] not in ("verify", "refresh", "mutations"):
        usage()
    command = sys.argv[1]
    manifest_path = Path(sys.argv[2])
    locations_path = Path(sys.argv[3])
    roots = parse_roles(sys.argv[4:])
    if command == "mutations":
        mutation_teeth(manifest_path, locations_path, roots)
        print("Delta source closure V1 mutation teeth PASS")
        return
    if command == "refresh":
        manifest, _ = load_document(manifest_path, "snapshot")
        manifest["content_set_sha256"] = "0" * 64
        manifest["closure_sha256"] = "0" * 64
        content, _ = verify_data(manifest, locations_path, roots, check_digests=False)
        manifest["content_set_sha256"] = content
        manifest["closure_sha256"] = closure_digest(manifest)
        sys.stdout.buffer.write(canonical_json(manifest, pretty=True))
    else:
        manifest, _, closure = verify_paths(manifest_path, locations_path, roots)
        print(f"Delta source closure V1 valid: {manifest['snapshot_id']} {closure}")


if __name__ == "__main__":
    try:
        main()
    except SnapshotError as error:
        print(f"Delta source closure V1: {error}", file=sys.stderr)
        raise SystemExit(error.status)
