"""Replay selected consumers with an authenticated compiler from the CI cache.

The plan is an authored input, not a passing result. Missing bytes or changed
compiler inputs refuse; this path never repairs the cache by building again.
"""

from dataclasses import dataclass
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tempfile

from qualification import RecoveryError, file_digest, source_inputs


SCHEMA = "omega-compiler-pilot-plan-v1"
COMPILER_INPUTS = (
    "omega-rust", "source/library", "source/psi", "source/omega", "bootstrap",
    "tools/benchmark",
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo",
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def input_objects(root, revision):
    commit = subprocess.check_output(
        ["git", "rev-parse", "--verify", revision + "^{commit}"], cwd=root, text=True).strip()
    try:
        return source_inputs(root, commit, COMPILER_INPUTS)
    except RecoveryError as error:
        raise ValueError(str(error)) from error


def check_origin(root, producer):
    source = producer["revision"]
    present = subprocess.run(["git", "cat-file", "-e", source + "^{commit}"], cwd=root,
                             capture_output=True)
    if present.returncode:
        fetched = subprocess.run(["git", "fetch", "--no-tags", "--depth=1", "origin", source],
                                 cwd=root, capture_output=True)
        require(fetched.returncode == 0, "retained producer source is unavailable; no rebuild was attempted")
    tree = subprocess.check_output(["git", "rev-parse", source + "^{tree}"], cwd=root, text=True).strip()
    require(tree == producer["tree"], "retained source tree differs from its original receipt")
    require(input_objects(root, source) == producer["input_objects"],
            "retained compiler inputs do not match the original source")


def check_inputs(root, revision, expected):
    require(isinstance(expected, dict) and set(expected) == set(COMPILER_INPUTS),
            "retained producer must bind the complete compiler input roster")
    require(input_objects(root, revision) == expected,
            "compiler, SDK, bootstrap, manifests or toolchain changed since the retained producer")


@dataclass(frozen=True)
class ReplayPlan:
    path: str
    producer: dict
    fixtures: tuple
    source_only: bool
    samples: tuple
    cli_fixtures: tuple


def load(root, request):
    require(isinstance(request, str) and request.startswith("@"), "replay plan needs @path")
    name = request[1:]
    relative = PurePosixPath(name)
    require(name and relative.as_posix() == name and not relative.is_absolute()
            and "\\" not in name and all(part not in (".", "..") for part in relative.parts)
            and relative.suffix == ".json", "replay plan must be a normalized repository JSON path")
    path = root.joinpath(*relative.parts)
    require(path.resolve().is_relative_to(root.resolve()) and path.is_file() and not path.is_symlink(),
            "replay plan must remain inside its checkout")
    tracked = subprocess.run(["git", "ls-files", "--error-unmatch", "--", name], cwd=root,
                             capture_output=True, text=True)
    require(tracked.returncode == 0, "replay plan must be tracked")
    data = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(data, dict) and set(data) == {
        "schema", "producer", "fixtures", "source_only", "samples", "cli_fixtures"},
        "replay plan has unknown or missing fields")
    require(data["schema"] == SCHEMA and isinstance(data["source_only"], bool),
            "replay plan schema or source disposition is invalid")
    for key in ("fixtures", "samples", "cli_fixtures"):
        values = data[key]
        require(isinstance(values, list) and all(isinstance(value, str) and value
                and value == value.strip() and "," not in value and not value.startswith("@")
                for value in values)
                and len(set(values)) == len(values), "replay selectors must be distinct strings")
    require(data["fixtures"] or data["samples"] or data["cli_fixtures"],
            "replay plan has no consumers")
    producer = data["producer"]
    require(isinstance(producer, dict) and set(producer) == {
        "revision", "tree", "target", "toolchain", "input_objects", "executables",
        "origin_run", "cache_key"}, "retained producer has unknown or missing fields")
    for field in ("revision", "tree"):
        require(isinstance(producer[field], str) and re.fullmatch(r"[0-9a-f]{40}", producer[field]),
                "retained source needs complete Git identities")
    require(producer["target"] in ("linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64")
            and isinstance(producer["toolchain"], str) and producer["toolchain"],
            "retained target or toolchain is invalid")
    require(type(producer["origin_run"]) is int and producer["origin_run"] > 0
            and isinstance(producer["cache_key"], str) and producer["cache_key"],
            "retained producer needs its original run and cache reference")
    executables = producer["executables"]
    require(isinstance(executables, dict) and set(executables) == {"cli", "runner"},
            "retained executable roster is incomplete")
    for kind, expected in executables.items():
        if kind == "runner" and expected is None:
            require(not data["fixtures"], "source fixtures require a retained runner")
            continue
        require(isinstance(expected, dict) and set(expected) == {"sha256", "profile"}
                and isinstance(expected["sha256"], str)
                and re.fullmatch(r"[0-9a-f]{64}", expected["sha256"]),
                "retained executable needs its immutable digest and profile")
        require(isinstance(expected["profile"], dict)
                and expected["profile"].get("schema") == "omega-compiler-build-profile-v1"
                and expected["profile"].get("opt_level") in ("1", "2", "3", "s", "z"),
                "retained executable needs an optimized build profile")
    check_origin(root, producer)
    check_inputs(root, "HEAD", producer["input_objects"])
    return ReplayPlan(name, producer, tuple(data["fixtures"]), data["source_only"],
                      tuple(data["samples"]), tuple(data["cli_fixtures"]))


def materialize(root, output, plan, profile_query):
    """Copy only verified cache outputs, keeping the original producer identity."""
    expected = plan.producer
    check_inputs(root, "HEAD", expected["input_objects"])
    cache = Path(os.environ.get("CARGO_TARGET_DIR", str(root / "target")))
    if not cache.is_absolute():
        cache = root / cache
    suffix = ".exe" if os.name == "nt" else ""
    retained = Path(tempfile.mkdtemp(prefix="producer-replay-", dir=output))
    profiles, paths, sources = {}, {}, {}
    for kind, filename in (("cli", "omega"), ("runner", "corpus_runner")):
        identity = expected["executables"][kind]
        if identity is None:
            paths[kind], profiles[kind] = None, None
            continue
        candidates = [cache / "debug" / (filename + suffix)]
        if kind == "runner":
            # Cargo test uses either a hashed deps name or a unit output home.
            # The receipt's digest, not the newest filename, selects its bytes.
            candidates.extend(sorted(path for path in (cache / "debug/deps").glob("corpus_runner-*")
                                     if re.fullmatch(r"corpus_runner-[0-9a-f]+" + re.escape(suffix), path.name)))
            candidates.extend(sorted((cache / "debug/build").glob("*/*/out/" + filename + suffix)))
            # Cargo's bound receipt is written beside its actual test output.
            # It can identify a unit basename without guessing that basename;
            # the executable still has to match the original digest below.
            for receipt in sorted((cache / "debug").rglob("*.compiler-profile.json")):
                if receipt.is_symlink():
                    continue
                try:
                    data = json.loads(receipt.read_text(encoding="utf-8"))
                except (OSError, UnicodeError, ValueError):
                    continue
                if isinstance(data, dict) and data.get("binary_sha256") == identity["sha256"]:
                    candidates.append(receipt.with_name(receipt.name.removesuffix(".compiler-profile.json")))
        candidates = list(dict.fromkeys(candidates))
        candidates = [path for path in candidates if path.is_file() and not path.is_symlink()]
        if not candidates:
            found = [dict(path=str(path), symlink=path.is_symlink(), file=path.is_file())
                     for path in sorted((cache / "debug").rglob("*" + filename + "*"))]
            (retained / "cache-discovery.json").write_text(
                json.dumps(dict(kind=kind, related_paths=found), indent=2) + "\n", encoding="utf-8")
        require(candidates,
                f"retained {kind} cache output is missing; no rebuild was attempted")
        matching = [path for path in candidates if file_digest(path) == identity["sha256"]]
        require(matching,
                "cached executable differs from the original retained producer; no rebuild was attempted")
        source = matching[0]
        observed = profile_query(source)
        require({key: observed.get(key) for key in ("sha256", "profile")} == identity,
                "cached executable differs from the original retained producer; no rebuild was attempted")
        destination = retained / source.name
        shutil.copy2(source, destination)
        sidecar = Path(str(source) + ".compiler-profile.json")
        if sidecar.is_file():
            shutil.copy2(sidecar, str(destination) + ".compiler-profile.json")
        copied = profile_query(destination)
        require({key: copied.get(key) for key in ("sha256", "profile")} == identity
                and profile_query(source) == observed,
                "retained executable changed during cache capture")
        paths[kind], profiles[kind] = destination.resolve(), copied
        sources[kind] = dict(path=str(source), sha256=identity["sha256"])
    (retained / "cache-inputs.json").write_text(json.dumps(sources, indent=2) + "\n", encoding="utf-8")
    return paths, profiles
