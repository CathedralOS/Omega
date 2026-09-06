#!/usr/bin/env python3
"""Run affected library tests without changing workspace feature unification.

Supply a previously verified commit as --base. Git compares its tree with the
current working files (including staged and untracked files). Only Rust files
under a known crate's src/ have a narrow interpretation. Everything else runs
all library tests. Architecture tests always run because they read source trees
without Cargo dependency edges. This is change-impact selection, not a proof
that the compiler or an untested base is correct. See wiki/testing.md.
"""

import argparse
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys


# terminal-codec embeds these crates' source. Record those edges explicitly,
# even though its Cargo manifest currently declares the same dependencies.
# See its build.rs and src/trust_graph.rs. Changes to either are full-suite
# inputs, requiring review of this map before the next narrow run.
SOURCE_READERS = {
    "terminal-codec": {
        "terminal-verifier", "terminal-psi", "proof-admission",
        "semantic-vocabulary", "terminal-semantics",
    },
}


def output(root, arguments):
    return subprocess.check_output(arguments, cwd=root).decode("utf-8")


def changed_paths(root, base):
    # --no-renames exposes BOTH sides of moves, including moves between crates.
    tracked = output(root, ["git", "diff", "--no-ext-diff", "--no-renames",
                            "--name-only", "-z", base, "--"])
    untracked = output(root, ["git", "ls-files", "--others",
                              "--exclude-standard", "-z"])
    return sorted(set((tracked + untracked).split("\0")) - {""})


def selection(root, metadata, paths):
    members = set(metadata["workspace_members"])
    packages = [package for package in metadata["packages"]
                if package["id"] in members]
    owners = []
    for package in packages:
        name = package["name"]
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
            raise ValueError(f"Unsupported package name: {name!r}")
        directory = Path(package["manifest_path"]).parent.relative_to(root)
        owners.append((PurePosixPath(directory.as_posix()), name))
    owners.sort(key=lambda owner: len(owner[0].parts), reverse=True)

    affected = set()
    reasons = []
    for filename in paths:
        path = PurePosixPath(filename)
        if filename in {
            "omega-rust/psi/semantics/terminal-codec/src/trust_graph.rs",
        }:
            reasons.append(f"Source-reader implementation: {filename}")
            continue
        for directory, name in owners:
            if path.is_relative_to(directory):
                relative = path.relative_to(directory)
                if relative.parts[0] == "src" and relative.suffix == ".rs":
                    affected.add(name)
                else:
                    reasons.append(f"Shared or unclassified input: {filename}")
                break
        else:
            reasons.append(f"Outside workspace crate sources: {filename}")

    if reasons:
        return "all()", sorted(affected), reasons
    # A reader can itself be a transitive dependent of another reader. Iterate
    # both source edges and declared dependencies to a fixed point, including
    # dev/build/optional/target dependencies for conservative selection.
    while True:
        expanded = affected | {
            package["name"] for package in packages
            if any(dependency["name"] in affected
                   for dependency in package["dependencies"])
        } | {
            reader for reader, inputs in SOURCE_READERS.items()
            if inputs & affected
        }
        if expanded == affected:
            break
        affected = expanded
    known = {package["name"] for package in packages}
    if not affected <= known:
        return "all()", sorted(affected), ["Source-reader map needs updating"]
    expression = " | ".join(f"rdeps(={name})" for name in sorted(affected))
    return expression or "none()", sorted(affected), []


def make_plan(root, runner, base, full=False):
    paths = [] if full else changed_paths(root, base)
    if full:
        expression, packages, reasons = "all()", [], ["Explicit full run"]
    else:
        metadata = json.loads(output(root, [runner, "metadata", "--locked",
                                            "--format-version", "1", "--no-deps"]))
        if Path(metadata["workspace_root"]).resolve() != root:
            raise ValueError("Cargo workspace root differs from Git root")
        expression, packages, reasons = selection(root, metadata, paths)
    commands = [[runner, "nextest", "run", "--locked", "-p",
                 "omega-architecture-test", "--all-targets", "--no-fail-fast"]]
    if expression != "none()":
        # Keep --workspace even for a narrow filter. Splitting -p builds can
        # change feature unification and no longer match the full baseline.
        commands.append([runner, "nextest", "run", "--locked", "--workspace",
                         "--lib", "--no-fail-fast", "-E", expression])
        if expression != "all()":
            # A valid selection can contain only bin crates (e.g. omega).
            # The separate integration gate covers those; this phase is --lib.
            commands[-1].extend(["--no-tests", "pass"])
    return {"base": base, "changed_paths": paths, "affected_packages": packages,
            "filter": expression, "full_suite_reasons": reasons,
            "commands": commands}


def run_commands(root, commands):
    failed = 0
    for command in commands:
        result = subprocess.run(command, cwd=root, check=False)
        if result.returncode:
            failed = result.returncode
    return failed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--base", help="Previously verified commit; compared to working files")
    mode.add_argument("--full", action="store_true", help="Run the complete portable baseline")
    parser.add_argument("--plan", action="store_true", help="Print JSON without building/running tests")
    args = parser.parse_args()
    try:
        root = Path(output(Path.cwd(), ["git", "rev-parse", "--show-toplevel"]).strip()).resolve()
        base = None
        if args.base:
            base = output(root, ["git", "rev-parse", "--verify", "--end-of-options",
                                 args.base + "^{commit}"]).strip()
        runner = shutil.which("mbx") or shutil.which("cargo")
        if not runner:
            raise ValueError("Install mbx (preferred) or Cargo, and cargo-nextest")
        plan = make_plan(root, runner, base, args.full)
        print(json.dumps(plan, indent=2), flush=True)
        if args.plan:
            return 0
        return run_commands(root, plan["commands"])
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"test_affected: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
