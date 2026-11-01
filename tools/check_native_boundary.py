#!/usr/bin/env python3
"""Check native production and hosted execution's source-free dependencies.

Uses Cargo's declared path dependencies, including optional, build and
target-specific edges. This checks every configuration, not just the current
host's resolved features. Dev dependencies are outside the shipped boundary.

Two checks share that edge list. The closure checks bless an allow-list of
owners behind one facade. `FORBIDDEN_DEPENDENCIES` names single edges that the
ownership contract in omega-rust/pipeline.md forbids between owners that
otherwise legitimately coexist, so a wrong import stops resolving instead of
relying on a source-text search.
"""

import json
from pathlib import Path
import re
from typing import NamedTuple
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


PSI = "omega-rust/psi"
OMEGA = "omega-rust/omega"
PSI_PIPELINE = f"{PSI}/compilation/pipeline"
OMEGA_PIPELINE = f"{OMEGA}/compilation/pipeline"
PACKAGES = f"{OMEGA}/packages"
PACKAGE_MANAGER = f"{PACKAGES}/package-manager"
PRODUCT_COORDINATOR = f"{OMEGA}/compilation/driver"
PRODUCTS = f"{OMEGA}/compilation/products"
BUILD_EVALUATION = f"{OMEGA}/compilation/build-evaluation"
BUILD_APPLICATION = f"{OMEGA}/application/build"
CLI_APPLICATION = f"{OMEGA}/application/cli"
TRUST_MODEL = f"{OMEGA}/compilation/realization/trust-model"
REVIEW_EVIDENCE = f"{PACKAGES}/review-evidence"
PSI_FACADE = f"{PSI}/compilation/driver"
TERMINAL_PRODUCTION = f"{PSI_FACADE}/terminal-production"
SOURCE_STAGES = tuple(
    f"{PSI_PIPELINE}/{stage}"
    for stage in (
        "02_syntax-trees-to-symbol-resolved-trees",
        "03_symbol-resolved-trees-to-typed-trees",
        "03b_typed-trees-to-typed-trees",
        "04_typed-trees-to-checked-trees",
    )
)
PARSER_STAGES = tuple(
    f"{PSI_PIPELINE}/{stage}" for stage in ("00_source-files-to-tokens", "01_tokens-to-syntax-trees")
)
TERMINAL_STAGES = tuple(
    f"{PSI_PIPELINE}/{stage}"
    for stage in (
        "05_checked-trees-to-lowered-psi",
        "06_lowered-psi-to-lowered-psi",
        "07_lowered-psi-to-terminal-psi",
    )
)

class Rule(NamedTuple):
    """Owners and targets are workspace paths; a path covers its sub-tree."""

    owners: tuple
    forbidden: tuple
    reason: str
    except_owners: tuple = ()


# Only production edges count (see `dev` above).
FORBIDDEN_DEPENDENCIES = tuple(Rule(*rule) for rule in (
    ((PSI,), (OMEGA,), "Psi must not depend on Omega"),
    (
        (CLI_APPLICATION,),
        (PSI_PIPELINE, OMEGA_PIPELINE),
        "application operations call compiler facades, not numbered transforms",
    ),
    (
        (BUILD_APPLICATION,),
        (
            PSI_PIPELINE,
            OMEGA_PIPELINE,
            f"{PSI}/compilation/representations/syntax-trees",
            f"{PSI}/compilation/representations/typed-trees",
            f"{PSI}/compilation/representations/checked-trees",
            f"{PSI}/compilation/representations/lowered-psi",
            f"{OMEGA}/compilation/realization/provider-planning",
        ),
        "Build reaches the language through product facades, not stages or representations",
    ),
    (
        (PRODUCT_COORDINATOR,),
        (OMEGA_PIPELINE, TERMINAL_PRODUCTION),
        "the product coordinator does not run native stages or wrap Terminal production",
    ),
    (
        (PRODUCT_COORDINATOR, PRODUCTS, BUILD_EVALUATION),
        SOURCE_STAGES,
        "product consumers call the Psi facade and do not assemble source stages",
    ),
    (
        (f"{PACKAGES}/compilation-inputs", f"{PACKAGES}/build-declarations", PACKAGE_MANAGER),
        PARSER_STAGES,
        "package consumers receive parsed sources and do not run the lexer or parser",
    ),
    (
        (PACKAGE_MANAGER,),
        (
            f"{OMEGA}/compilation/backend/artifacts/component-description",
            f"{OMEGA}/compilation/realization/provider-planning",
            f"{PSI}/foundation/flow-effects",
            f"{PSI}/foundation/symbols",
        ),
        "package custody and review do no checking, provider planning or product production",
    ),
    (
        (REVIEW_EVIDENCE,),
        (PACKAGE_MANAGER, PRODUCT_COORDINATOR),
        "review evidence capture sits below package policy and the compiler",
    ),
    (
        (TRUST_MODEL,),
        (
            f"{PSI}/compilation/compile-time",
            f"{PSI}/interpreter/checked-interpreter",
            f"{PSI_PIPELINE}/04_typed-trees-to-checked-trees",
            f"{PSI}/compilation/checking/terminal-installation",
            f"{PSI}/compilation/representations/terminal-psi",
            f"{OMEGA}/compilation/backend/target",
            f"{PSI}/foundation/semantic-vocabulary",
        ),
        "trust admission does not reconstruct grants from source or target owners",
    ),
    (
        (PSI_FACADE,),
        TERMINAL_STAGES,
        "stages 05-07 are sequenced only by terminal-production",
        (TERMINAL_PRODUCTION,),
    ),
    (
        (TERMINAL_PRODUCTION,),
        (f"{PSI}/compilation/compile-time",),
        "Terminal production sits below compile-time evaluation, which calls it",
    ),
))


def under(relative, owners):
    return any(relative == owner or relative.startswith(owner + "/") for owner in owners)


def forbidden_dependency_violations(metadata, rules=FORBIDDEN_DEPENDENCIES):
    """Return `owner -> dependency: reason` for every forbidden production edge."""
    root = Path(metadata["workspace_root"]).resolve()
    violations = []
    for package in metadata["packages"]:
        owner = Path(package["manifest_path"]).resolve().parent.relative_to(root).as_posix()
        for dependency in package["dependencies"]:
            if dependency.get("kind") == "dev" or not dependency.get("path"):
                continue
            relative = Path(dependency["path"]).resolve().relative_to(root).as_posix()
            for rule in rules:
                if (
                    under(owner, rule.owners)
                    and not under(owner, rule.except_owners)
                    and under(relative, rule.forbidden)
                ):
                    violations.append(f"{package['name']} -> {Path(relative).name}: {rule.reason}")
    return violations


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
        if not under(relative, owners):
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
        violations += forbidden_dependency_violations(metadata)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"native boundary could not be checked: {error}", file=sys.stderr)
        return 2
    if violations:
        print("native/runtime architecture violations:")
        print("\n".join(violations))
        return 1
    print("native/runtime boundaries: source-free dependencies, ownership and representation checks pass")

    return 0


if __name__ == "__main__":
    sys.exit(main())
