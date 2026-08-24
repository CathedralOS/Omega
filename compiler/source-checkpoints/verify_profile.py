#!/usr/bin/env python3
"""Verify provisional compositional Ωself source-profile checkpoints."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[2]
CHECKPOINT_DIR = Path(__file__).resolve().parent
SCHEMA = "omega.product-compiler-source-profile.v1"
DOMAIN = b"omega.product-compiler-source-profile.v1\0"
TOP_LEVEL_KEYS = {
    "schema",
    "checkpoint",
    "status",
    "checkpoint_manifest",
    "checkpoint_closure_sha256",
    "checkpoint_content_set_sha256",
    "feature_catalog",
    "census_schema",
    "configuration_policy",
    "configurations",
    "admitted_features",
    "provisionally_forbidden_features",
    "resources",
    "required_canaries",
    "canaries",
    "unresolved_decisions",
    "profile_sha256",
}


class ProfileError(Exception):
    pass


def fail(message: str) -> None:
    raise ProfileError(message)


def strict_keys(value: object, expected: set[str], context: str) -> dict:
    if not isinstance(value, dict):
        fail(f"{context} must be an object")
    actual = set(value)
    if actual != expected:
        fail(
            f"{context} keys differ; "
            f"missing={sorted(expected - actual)}, unknown={sorted(actual - expected)}"
        )
    return value


def strict_nonnegative_integer(value: object, context: str) -> int:
    if type(value) is not int or value < 0:
        fail(f"{context} must be a nonnegative integer")
    return value


def sorted_unique_strings(value: object, context: str, *, allow_empty: bool = True) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        fail(f"{context} must be a string array")
    if value != sorted(set(value)):
        fail(f"{context} must be sorted and unique")
    if not allow_empty and not value:
        fail(f"{context} must not be empty")
    return value


def repository_file(spelling: object) -> Path:
    if not isinstance(spelling, str):
        fail("repository path must be a string")
    path = PurePosixPath(spelling)
    if path.is_absolute() or ".." in path.parts or str(path) != spelling:
        fail(f"non-canonical repository path {spelling!r}")
    resolved = ROOT / spelling
    if not resolved.is_file():
        fail(f"missing repository file {spelling}")
    return resolved


def sha256(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def profile_digest(profile: dict) -> str:
    projection = {key: value for key, value in profile.items() if key != "profile_sha256"}
    canonical = json.dumps(
        projection, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(
        DOMAIN + len(canonical).to_bytes(8, "little") + canonical
    ).hexdigest()


def refresh_profile_digest(profile: dict) -> None:
    profile["profile_sha256"] = profile_digest(profile)


def run(command: list[str], context: str) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        fail(f"{context} failed: {detail}")
    return completed


def load_census(
    profile: dict,
    source: str,
    *,
    target: str | None,
    native_provider_substitution: bool,
) -> dict:
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
    ]
    if target is not None:
        command.extend(("--target", target))
    if not native_provider_substitution:
        command.append("--semantic-only")
    command.extend(("--feature-census", source))
    completed = run(command, f"source census for {source}")
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        fail(f"source census for {source} emitted invalid JSON: {error}")
    raise AssertionError("unreachable")


def validate_census(
    census: dict,
    profile: dict,
    *,
    source: str,
    target: str | None,
    native_provider_substitution: bool,
) -> tuple[list[dict], list[dict]]:
    strict_keys(
        census,
        {
            "schema",
            "feature_catalog",
            "entry_source",
            "selected_target",
            "native_provider_substitution",
            "features",
            "resources",
        },
        f"census {source}",
    )
    if census["schema"] != profile["census_schema"]:
        fail(f"census {source} has the wrong schema")
    if census["feature_catalog"] != profile["feature_catalog"]:
        fail(f"census {source} has the wrong feature catalog")
    if census["entry_source"] != source or census["selected_target"] != target:
        fail(f"census {source} has the wrong entry source or target")
    if census["native_provider_substitution"] is not native_provider_substitution:
        fail(f"census {source} has the wrong provider-substitution mode")

    feature_ids = []
    for index, row in enumerate(census["features"]):
        row = strict_keys(row, {"id", "count"}, f"census feature[{index}]")
        if not isinstance(row["id"], str):
            fail("census feature id must be a string")
        strict_nonnegative_integer(row["count"], f"census feature {row['id']} count")
        feature_ids.append(row["id"])
    if feature_ids != sorted(set(feature_ids)) or not feature_ids:
        fail("census feature ids must be sorted, unique, and nonempty")

    resource_ids = []
    for index, row in enumerate(census["resources"]):
        row = strict_keys(
            row, {"id", "unit", "scope", "observed"}, f"census resource[{index}]"
        )
        if any(not isinstance(row[key], str) for key in ("id", "unit", "scope")):
            fail("census resource identity, unit, and scope must be strings")
        strict_nonnegative_integer(row["observed"], f"census resource {row['id']} observed")
        resource_ids.append(row["id"])
    if resource_ids != sorted(set(resource_ids)) or not resource_ids:
        fail("census resource ids must be sorted, unique, and nonempty")
    return census["features"], census["resources"]


def admission_errors(profile: dict, census: dict) -> tuple[list[str], list[str]]:
    forbidden = set(profile["provisionally_forbidden_features"])
    feature_errors = [
        row["id"] for row in census["features"] if row["count"] > 0 and row["id"] in forbidden
    ]
    resource_limits = {row["id"]: row["limit"] for row in profile["resources"]}
    resource_errors = [
        row["id"]
        for row in census["resources"]
        if row["observed"] > resource_limits[row["id"]]
    ]
    return feature_errors, resource_errors


def verify_profile_data(
    profile: dict,
    manifest: dict,
    censuses: dict[str, dict],
    *,
    check_digest: bool = True,
) -> None:
    strict_keys(profile, TOP_LEVEL_KEYS, "profile")
    if profile["schema"] != SCHEMA or profile["checkpoint"] != manifest.get("checkpoint"):
        fail("profile schema or checkpoint number is unsupported")
    if profile["status"] != "provisional":
        fail("profile status must remain provisional")
    manifest_path = repository_file(profile["checkpoint_manifest"])
    if manifest_path != CHECKPOINT_DIR / f"checkpoint-{profile['checkpoint']:06d}.json":
        fail("profile names the wrong checkpoint manifest")
    if profile["checkpoint_closure_sha256"] != manifest.get("closure_sha256"):
        fail("profile checkpoint closure digest is detached")
    if profile["checkpoint_content_set_sha256"] != manifest.get("content_set_sha256"):
        fail("profile checkpoint content-set digest is detached")
    if check_digest and profile["profile_sha256"] != profile_digest(profile):
        fail("profile digest mismatch")

    policy = strict_keys(
        profile["configuration_policy"], {"features", "resources"}, "configuration policy"
    )
    if policy != {"features": "union", "resources": "maximum"}:
        fail("unsupported profile configuration policy")
    targets = sorted_unique_strings(profile["configurations"], "profile configurations", allow_empty=False)
    manifest_targets = [row["target"] for row in manifest.get("configurations", [])]
    if targets != manifest_targets or set(censuses) != set(targets):
        fail("profile and census configurations differ from the manifest")

    validated = {}
    for target in targets:
        validated[target] = validate_census(
            censuses[target],
            profile,
            source=manifest["entry_source"],
            target=target,
            native_provider_substitution=True,
        )

    first_features, first_resources = validated[targets[0]]
    catalog_features = [row["id"] for row in first_features]
    catalog_resources = [(row["id"], row["unit"], row["scope"]) for row in first_resources]
    for target in targets[1:]:
        features, resources = validated[target]
        if [row["id"] for row in features] != catalog_features:
            fail(f"target {target} feature catalog differs")
        if [(row["id"], row["unit"], row["scope"]) for row in resources] != catalog_resources:
            fail(f"target {target} resource catalog differs")

    admitted = sorted_unique_strings(profile["admitted_features"], "admitted features")
    forbidden = sorted_unique_strings(
        profile["provisionally_forbidden_features"], "provisionally forbidden features"
    )
    if sorted(admitted + forbidden) != catalog_features or set(admitted) & set(forbidden):
        fail("profile feature partition does not exactly cover the census catalog")
    present = {
        row["id"]
        for target in targets
        for row in validated[target][0]
        if row["count"] > 0
    }
    if set(admitted) != present:
        fail("admitted features do not exactly match checkpoint feature presence")

    profile_resources = profile["resources"]
    if not isinstance(profile_resources, list):
        fail("profile resources must be an array")
    resource_ids = []
    for index, row in enumerate(profile_resources):
        row = strict_keys(
            row,
            {"id", "unit", "scope", "comparison", "observed_max", "limit"},
            f"profile resource[{index}]",
        )
        if any(not isinstance(row[key], str) for key in ("id", "unit", "scope", "comparison")):
            fail("profile resource identity fields must be strings")
        if row["comparison"] != "lte":
            fail(f"profile resource {row['id']} has an unsupported comparison")
        observed = strict_nonnegative_integer(row["observed_max"], f"resource {row['id']} observed")
        limit = strict_nonnegative_integer(row["limit"], f"resource {row['id']} limit")
        if limit < observed:
            fail(f"profile resource {row['id']} limit is below observed demand")
        resource_ids.append(row["id"])
    if resource_ids != [row[0] for row in catalog_resources]:
        fail("profile resources do not exactly follow the census resource catalog")
    for row, (_, unit, scope) in zip(profile_resources, catalog_resources):
        if row["unit"] != unit or row["scope"] != scope:
            fail(f"profile resource {row['id']} unit or scope differs from the census")
        observed = max(
            next(resource["observed"] for resource in validated[target][1] if resource["id"] == row["id"])
            for target in targets
        )
        if row["observed_max"] != observed:
            fail(f"profile resource {row['id']} has stale checkpoint evidence")

    for target in targets:
        feature_errors, resource_errors = admission_errors(profile, censuses[target])
        if feature_errors or resource_errors:
            fail(
                f"checkpoint target {target} violates its profile: "
                f"features={feature_errors}, resources={resource_errors}"
            )

    canary_ids = []
    for index, canary in enumerate(profile["canaries"]):
        canary = strict_keys(
            canary,
            {
                "id",
                "path",
                "sha256",
                "omega_expectation",
                "profile_expectation",
                "expected_features",
            },
            f"profile canary[{index}]",
        )
        if not isinstance(canary["id"], str):
            fail("profile canary id must be a string")
        fixture = repository_file(canary["path"])
        if canary["sha256"] != sha256(fixture.read_bytes()):
            fail(f"profile canary {canary['id']} digest differs")
        if canary["omega_expectation"] != "checked_accept":
            fail(f"profile canary {canary['id']} must remain valid ordinary Omega")
        if canary["profile_expectation"] not in ("admit", "reject"):
            fail(f"profile canary {canary['id']} has an invalid profile expectation")
        expected = sorted_unique_strings(
            canary["expected_features"], f"profile canary {canary['id']} expected features", allow_empty=False
        )
        disposition = set(admitted if canary["profile_expectation"] == "admit" else forbidden)
        if not set(expected) <= disposition:
            fail(f"profile canary {canary['id']} expects features with the wrong disposition")
        canary_ids.append(canary["id"])
    if canary_ids != sorted(set(canary_ids)) or not canary_ids:
        fail("profile canaries must be sorted, unique, and nonempty")
    required_canaries = sorted_unique_strings(
        profile["required_canaries"], "required profile canaries", allow_empty=False
    )
    if required_canaries != canary_ids:
        fail("required profile canaries are detached from their definitions")

    decision_ids = []
    for index, decision in enumerate(profile["unresolved_decisions"]):
        decision = strict_keys(
            decision, {"id", "scope", "missing_evidence"}, f"unresolved decision[{index}]"
        )
        if any(not isinstance(decision[key], str) or not decision[key] for key in decision):
            fail("unresolved decision fields must be nonempty strings")
        decision_ids.append(decision["id"])
    if decision_ids != sorted(set(decision_ids)) or not decision_ids:
        fail("unresolved decisions must be sorted, unique, and nonempty")


def verify_canaries(profile: dict) -> None:
    paths = [canary["path"] for canary in profile["canaries"]]
    run(
        [
            "cargo",
            "run",
            "-q",
            "--locked",
            "--offline",
            "-p",
            "omega-compiler",
            "--bin",
            "omega-check-source",
            "--",
            *paths,
        ],
        "ordinary-Omega checked profile canaries",
    )
    for canary in profile["canaries"]:
        census = load_census(
            profile,
            canary["path"],
            target=None,
            native_provider_substitution=False,
        )
        validate_census(
            census,
            profile,
            source=canary["path"],
            target=None,
            native_provider_substitution=False,
        )
        present = {row["id"] for row in census["features"] if row["count"] > 0}
        if not set(canary["expected_features"]) <= present:
            fail(f"profile canary {canary['id']} no longer exercises its expected features")
        feature_errors, resource_errors = admission_errors(profile, census)
        if resource_errors:
            fail(f"feature canary {canary['id']} accidentally exceeds resources {resource_errors}")
        if canary["profile_expectation"] == "admit" and feature_errors:
            fail(f"positive profile canary {canary['id']} rejects on {feature_errors}")
        if canary["profile_expectation"] == "reject" and not feature_errors:
            fail(f"negative profile canary {canary['id']} was admitted")


def mutation_teeth(profile: dict, manifest: dict, censuses: dict[str, dict]) -> None:
    mutations: list[tuple[str, dict, bool]] = []

    def add(name: str, mutate, *, refresh: bool = True) -> None:
        candidate = copy.deepcopy(profile)
        mutate(candidate)
        if refresh:
            refresh_profile_digest(candidate)
        mutations.append((name, candidate, refresh))

    add("unknown top-level field", lambda value: value.__setitem__("unknown", 1))
    add("missing admitted feature", lambda value: value["admitted_features"].pop())
    add(
        "duplicate admitted feature",
        lambda value: value["admitted_features"].append(value["admitted_features"][-1]),
    )
    add(
        "unknown feature",
        lambda value: value["provisionally_forbidden_features"].append("unknown.feature"),
    )

    def forbid_present(value: dict) -> None:
        feature = value["admitted_features"].pop(0)
        value["provisionally_forbidden_features"].append(feature)
        value["provisionally_forbidden_features"].sort()

    add("forbid checkpoint feature", forbid_present)
    add("catalog mismatch", lambda value: value.__setitem__("feature_catalog", "wrong"))
    add("configuration omission", lambda value: value["configurations"].pop())
    add(
        "checkpoint closure detachment",
        lambda value: value.__setitem__("checkpoint_closure_sha256", "0" * 64),
    )
    add("missing resource", lambda value: value["resources"].pop())
    add("resource unit", lambda value: value["resources"][0].__setitem__("unit", "words"))
    add("resource scope", lambda value: value["resources"][0].__setitem__("scope", "global"))
    add(
        "resource comparison",
        lambda value: value["resources"][0].__setitem__("comparison", "lt"),
    )
    add(
        "resource observation",
        lambda value: value["resources"][1].__setitem__(
            "observed_max", value["resources"][1]["observed_max"] + 1
        ),
    )
    add(
        "resource limit below checkpoint",
        lambda value: value["resources"][1].__setitem__(
            "limit", value["resources"][1]["observed_max"] - 1
        ),
    )
    add(
        "canary digest",
        lambda value: value["canaries"][0].__setitem__("sha256", "0" * 64),
    )
    add("detached canary", lambda value: value["canaries"].pop())
    add(
        "canary disposition",
        lambda value: value["canaries"][1]["expected_features"].__setitem__(
            0, value["admitted_features"][0]
        ),
    )
    add(
        "stale profile digest",
        lambda value: value["unresolved_decisions"][0].__setitem__(
            "missing_evidence", "mutated"
        ),
        refresh=False,
    )

    for name, candidate, _ in mutations:
        try:
            verify_profile_data(candidate, manifest, censuses)
        except ProfileError:
            continue
        fail(f"profile mutation tooth did not reject {name}")

    base = copy.deepcopy(next(iter(censuses.values())))
    for row in base["features"]:
        row["count"] = 0
    forbidden = profile["provisionally_forbidden_features"][0]
    next(row for row in base["features"] if row["id"] == forbidden)["count"] = 1
    feature_errors, _ = admission_errors(profile, base)
    if forbidden not in feature_errors:
        fail("denied-feature evaluator mutation was admitted")

    for profile_resource in profile["resources"]:
        boundary = copy.deepcopy(base)
        row = next(
            resource for resource in boundary["resources"] if resource["id"] == profile_resource["id"]
        )
        row["observed"] = profile_resource["limit"]
        _, at_limit_errors = admission_errors(profile, boundary)
        if profile_resource["id"] in at_limit_errors:
            fail(f"resource {profile_resource['id']} rejected its exact limit")
        row["observed"] += 1
        _, over_limit_errors = admission_errors(profile, boundary)
        if profile_resource["id"] not in over_limit_errors:
            fail(f"resource {profile_resource['id']} admitted limit plus one")


def canonical_json(path: Path) -> dict:
    text = path.read_text(encoding="utf-8")
    value = json.loads(text)
    canonical = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    if text != canonical:
        fail(f"{path.name} is not canonical pretty JSON")
    return value


def main() -> None:
    run([sys.executable, str(CHECKPOINT_DIR / "verify_manifest.py")], "checkpoint manifest gate")
    profile_paths = sorted(CHECKPOINT_DIR.glob("profile-*.json"))
    if not profile_paths:
        fail("no machine-readable source profiles")
    for profile_path in profile_paths:
        profile = canonical_json(profile_path)
        manifest = canonical_json(repository_file(profile["checkpoint_manifest"]))
        censuses = {
            target: load_census(
                profile,
                manifest["entry_source"],
                target=target,
                native_provider_substitution=True,
            )
            for target in profile["configurations"]
        }
        verify_profile_data(profile, manifest, censuses)
        verify_canaries(profile)
        mutation_teeth(profile, manifest, censuses)
    print(
        f"verified {len(profile_paths)} provisional Ωself profile(s): "
        "manifest-bound census, canaries, resource limits, and mutation teeth"
    )


if __name__ == "__main__":
    try:
        main()
    except (ProfileError, json.JSONDecodeError) as error:
        raise SystemExit(f"product source profile: {error}") from error
