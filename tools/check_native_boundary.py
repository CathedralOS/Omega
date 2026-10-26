#!/usr/bin/env python3
"""Check native production and hosted execution's source-free dependencies.

Uses Cargo's declared path dependencies, including optional, build and
target-specific edges. This checks every configuration, not just the current
host's resolved features. Dev dependencies are outside the shipped boundary.
"""

import json
from pathlib import Path
import subprocess
import sys


NATIVE_ROOT = "omega-rust/omega/compilation/native-production"
PERMITTED_OWNERS = (
    NATIVE_ROOT,
    "omega-rust/omega/compilation/pipeline",
    "omega-rust/omega/compilation/representations",
    "omega-rust/omega/compilation/backend",
    "omega-rust/omega/foundation",
    "omega-rust/psi/foundation",
    "omega-rust/psi/compilation/representations/terminal-psi",
    "omega-rust/psi/compilation/checking/terminal-installation",
    "omega-rust/psi/compilation/proof/admission",
)
RUNTIME_ROOT = "omega-rust/omega/application/terminal-runtime"
NATIVE_PROVIDER_RUNTIME_ROOT = "omega-rust/omega/application/native-provider-runtime"
NATIVE_PROVIDER_RUNTIME_OWNERS = (NATIVE_PROVIDER_RUNTIME_ROOT, *PERMITTED_OWNERS[1:])
RUNTIME_OWNERS = (
    RUNTIME_ROOT,
    NATIVE_PROVIDER_RUNTIME_ROOT,
    "omega-rust/psi/interpreter/terminal-interpreter",
    *PERMITTED_OWNERS[1:],
)


def dependency_violations(metadata, entrance_path=NATIVE_ROOT, owners=PERMITTED_OWNERS):
    """Return offending paths, with the actual dependency chain to each one."""
    root = Path(metadata["workspace_root"]).resolve()
    packages = {
        Path(package["manifest_path"]).resolve().parent: package
        for package in metadata["packages"]
    }
    entrance = root / entrance_path
    if entrance not in packages:
        raise ValueError(f"{entrance_path} is absent from the workspace")
    pending = [(entrance, ())]
    visited = set()
    violations = []
    while pending:
        directory, parents = pending.pop()
        if directory in visited:
            continue
        visited.add(directory)
        package = packages.get(directory)
        if package is None:
            raise ValueError(f"path dependency is not a workspace member: {directory}")
        chain = (*parents, package["name"])
        relative = directory.relative_to(root).as_posix()
        if not any(
            relative == owner or relative.startswith(owner + "/")
            for owner in owners
        ):
            violations.append(f"{' -> '.join(chain)}: {relative}")
            continue
        for dependency in package["dependencies"]:
            if dependency.get("kind") == "dev" or not dependency.get("path"):
                continue
            pending.append((Path(dependency["path"]).resolve(), chain))
    return violations


def main():
    root = Path(__file__).resolve().parent.parent
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=root, capture_output=True, text=True, encoding="utf-8", check=True,
        )
        metadata = json.loads(result.stdout)
        violations = dependency_violations(metadata)
        violations += dependency_violations(metadata, RUNTIME_ROOT, RUNTIME_OWNERS)
        violations += dependency_violations(metadata, NATIVE_PROVIDER_RUNTIME_ROOT, NATIVE_PROVIDER_RUNTIME_OWNERS)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"native boundary could not be checked: {error}", file=sys.stderr)
        return 2
    if violations:
        print("native compilation or hosted execution crosses its ownership boundary:")
        print("\n".join(violations))
        return 1
    print("native/runtime boundaries: no production dependency reaches a source owner")
    return 0


if __name__ == "__main__":
    sys.exit(main())
