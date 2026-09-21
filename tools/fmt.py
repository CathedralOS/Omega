#!/usr/bin/env python3
"""Format every workspace target without exceeding command-line limits.

`cargo fmt --all` batches all workspace targets into a single rustfmt
invocation. This workspace needs a ~33,800-character command line, over
Windows' 32,767 limit, so `cargo fmt --all -- --check` cannot run on a
Windows host (os error 206). This driver enumerates the same targets
through `cargo metadata`, groups them by edition, and invokes `rustfmt`
in chunks bounded below that limit. Use it wherever a whole-workspace
format is needed:

    python tools/fmt.py --check

`rustfmt` resolves the pinned toolchain and rustfmt.toml exactly as it
does under cargo fmt, so both routes produce identical output. Local
path dependencies outside the workspace are not covered; this workspace
has none.
"""

import argparse
import json
import subprocess
import sys

# Windows CreateProcess rejects command lines over 32,767 characters; other
# hosts accept far more. Chunking below the smallest limit keeps every host
# on one code path, at a cost of a few extra rustfmt invocations.
ARGV_BUDGET = 24000

# The target kinds cargo fmt formats, observed from `cargo fmt --all -v`.
TARGET_KINDS = {"lib", "bin", "test", "bench", "example", "custom-build",
                "proc-macro"}


def workspace_targets(repository):
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=repository, capture_output=True, text=True, check=True).stdout)
    members = set(metadata["workspace_members"])
    groups = {}
    for package in metadata["packages"]:
        if package["id"] not in members:
            continue
        for target in package["targets"]:
            if set(target["kind"]) & TARGET_KINDS:
                groups.setdefault(target["edition"], set()).add(target["src_path"])
    return {edition: sorted(paths) for edition, paths in groups.items()}


def chunked_invocations(prefix, files):
    invocations, current = [], list(prefix)
    size = sum(len(arg) + 1 for arg in prefix)
    for path in files:
        cost = len(path) + 1
        if size + cost > ARGV_BUDGET and len(current) > len(prefix):
            invocations.append(current)
            current, size = list(prefix), sum(len(a) + 1 for a in prefix)
        current.append(path)
        size += cost
    if len(current) > len(prefix):
        invocations.append(current)
    return invocations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="Pass --check to rustfmt: report diffs, write nothing")
    parser.add_argument("--verbose", "-v", action="store_true",
                        help="Print each rustfmt invocation before running it")
    parser.add_argument("--repository", default=".",
                        help="Workspace checkout to format (default: cwd)")
    parser.add_argument("rustfmt_args", nargs="*",
                        help="Extra rustfmt arguments after a literal --")
    args = parser.parse_args()

    passthrough = list(args.rustfmt_args)
    if passthrough and passthrough[0] == "--":
        passthrough = passthrough[1:]

    try:
        groups = workspace_targets(args.repository)
    except (subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"fmt: cargo metadata failed: {error}", file=sys.stderr)
        return 1

    failures = 0
    for edition, files in sorted(groups.items()):
        prefix = ["rustfmt", "--edition", edition]
        if args.check:
            prefix.append("--check")
        prefix.extend(passthrough)
        for invocation in chunked_invocations(prefix, files):
            if args.verbose:
                print(" ".join(invocation), flush=True)
            result = subprocess.run(invocation, cwd=args.repository, check=False)
            if result.returncode != 0:
                failures += 1
    if failures:
        print(f"fmt: {failures} rustfmt invocation(s) failed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
