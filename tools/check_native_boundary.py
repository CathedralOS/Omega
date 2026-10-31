#!/usr/bin/env python3
"""Check native production and hosted execution's source-free dependencies.

Uses Cargo's declared path dependencies, including optional, build and
target-specific edges. This checks every configuration, not just the current
host's resolved features. Dev dependencies are outside the shipped boundary.
"""

import json
from pathlib import Path
import re
import subprocess
import sys


NATIVE_ROOT = "omega-rust/omega/compilation/native-production"
PROVIDER_SELECTION_ROOT = "omega-rust/omega/compilation/realization/provider-selection"
TRUST_ROOT = "omega-rust/omega/compilation/realization/trust-model"
PERMITTED_OWNERS = (
    NATIVE_ROOT,
    PROVIDER_SELECTION_ROOT,
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
PROVIDER_SELECTION_OWNERS = PERMITTED_OWNERS[1:]
TRUST_OWNERS = (TRUST_ROOT, *PERMITTED_OWNERS[1:])
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



def psi_omega_dependency_violations(metadata):
    """Psi never imports Omega, including tests and inactive configurations."""
    root = Path(metadata["workspace_root"]).resolve()
    psi = root / "omega-rust/psi"
    omega = root / "omega-rust/omega"
    violations = []
    for package in metadata["packages"]:
        source = Path(package["manifest_path"]).resolve().parent
        if not source.is_relative_to(psi):
            continue
        for dependency in package["dependencies"]:
            if not dependency.get("path"):
                continue
            destination = Path(dependency["path"]).resolve()
            if destination.is_relative_to(omega):
                violations.append(
                    f"{package['name']} -> {dependency['name']}: Psi depends on Omega ({destination.relative_to(root)})"
                )
    return violations


def realization_checker_entry_violations(metadata):
    """Realization may consume facts, never enter the source checker API.

    This direct-edge gate is separate from source-free native ownership: the
    remaining source-to-declaration migration has its own tracked acceptance.
    """
    root = Path(metadata["workspace_root"]).resolve()
    realization = root / "omega-rust/omega/compilation/realization"
    checker = root / "omega-rust/psi/compilation/checking"
    violations = []
    for package in metadata["packages"]:
        source = Path(package["manifest_path"]).resolve().parent
        if not source.is_relative_to(realization):
            continue
        for dependency in package["dependencies"]:
            if dependency.get("kind") == "dev" or not dependency.get("path"):
                continue
            if Path(dependency["path"]).resolve() == checker:
                violations.append(
                    f"{package['name']} -> {dependency['name']}: realization enters source checking"
                )
    return violations


def representation_spelling_violations(root):
    """The retired text spelling must not decide a runtime representation."""
    receiver = r"[A-Za-z_][A-Za-z_0-9]*(?:\s*\.\s*[A-Za-z_][A-Za-z_0-9]*)*"
    patterns = (
        re.compile(r'\.as_str\s*\([^)]*\)\s*(?:==|!=)\s*"string"'),
        re.compile(r'"string"\s*(?:==|!=)\s*' + receiver + r'\s*\.\s*as_str\s*\([^)]*\)'),
        re.compile(r'\bname\s*(?:==|!=)\s*"string"'),
        re.compile(r'"string"\s*(?:==|!=)\s*\bname\b'),
    )
    # Keep this macro check bounded to one line: a greedy cross-file match
    # would conflate an unrelated matches! with a later string literal.
    match_pattern = re.compile(r'matches!\s*\(.*\.as_str\s*\([^)]*\).*"string"')
    violations = []
    for owner in ("omega-rust/psi", "omega-rust/omega"):
        for path in (root / owner).rglob("*.rs"):
            text = path.read_text(encoding="utf-8")
            lines = {
                text.count("\n", 0, match.start()) + 1
                for pattern in patterns
                for match in pattern.finditer(text)
            }
            lines.update(
                line_number
                for line_number, line in enumerate(text.splitlines(), 1)
                if match_pattern.search(line)
            )
            for line_number in sorted(lines):
                violations.append(
                    f"{path.relative_to(root)}:{line_number}: runtime representation depends on retired text spelling"
                )
    return violations


def main():
    root = Path(__file__).resolve().parent.parent
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=root, capture_output=True, text=True, encoding="utf-8", check=True,
        )
        metadata = json.loads(result.stdout)
        violations = representation_spelling_violations(root)
        violations += psi_omega_dependency_violations(metadata)
        violations += realization_checker_entry_violations(metadata)
        violations += dependency_violations(metadata)
        violations += dependency_violations(metadata, PROVIDER_SELECTION_ROOT, PROVIDER_SELECTION_OWNERS)
        violations += dependency_violations(metadata, TRUST_ROOT, TRUST_OWNERS)
        violations += dependency_violations(metadata, RUNTIME_ROOT, RUNTIME_OWNERS)
        violations += dependency_violations(metadata, NATIVE_PROVIDER_RUNTIME_ROOT, NATIVE_PROVIDER_RUNTIME_OWNERS)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"native boundary could not be checked: {error}", file=sys.stderr)
        return 2
    if violations:
        print("native/runtime architecture violations:")
        print("\n".join(violations))
        return 1
    print("native/runtime boundaries: source-free dependencies and representation spelling checks pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
