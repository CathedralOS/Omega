#!/usr/bin/env python3
"""Run affected library tests without changing workspace feature unification.

Supply a previously verified commit as --base. Git compares its tree with the
current working files (including staged and untracked files). Rust files under
a known crate's src/ select dependents; audited documentation selects its source
checks. Other inputs run all library tests. Architecture tests always run because
they read source trees
without Cargo dependency edges.

Routine-diff selections also exclude the measured slow tail in SLOW_TEST_OWNERS:
a listed test is skipped unless its owning package's own src/ files changed or
--with-slow-tail is passed. --full baselines never exclude them. This is
change-impact selection, not a proof
that the compiler or an untested base is correct. See tools/testing.md.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import urllib.request


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

# Audited Markdown locations, not a blanket extension exclusion: Markdown in
# tests/fixtures is executable test input. Architecture reads the optimizer rule
# inventory; the compiler corpus audit below reads prose across the repository.
DOCUMENTATION_FILES = {
    "AGENTS.md", "CLAUDE.md", "README.md", "OWNER_QUESTIONS.md",
    "TASKS.md", "TASKS_BOOTSTRAP.md", "TASKS_OPTIMIZER.md",
    "tools/claims.md", "tools/landing.md", "tools/release_matrix.md",
    "tools/rust_producer_omission.md", "tools/testing.md",
    "omega-rust/omega/representations/optimization-core/rules.md",
}
DOCUMENTATION_TEST = (
    "surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus"
)

# Measured multi-minute library tests by owning package
# (wiki/drafts/measurements/test_cycle_selection_remeasurement.md), and the routine-diff
# exclusion list decided for them. A reverse-dependency selection that picks up
# native-realization otherwise drags ~800 s of runtime stress cases its diff
# cannot affect; a diff under the owner's own src/ keeps its tail because that
# tail is the affected behavior. --with-slow-tail restores excluded members and
# --full never excludes; keep both this table and the remeasurement note current
# before relying on a narrow run.
SLOW_TEST_OWNERS = {
    "native-realization": [
        ("stack_probe_commit", 324),
        ("runtime_spill_pressure", 356),
    ],
}


def is_documentation(filename):
    path = PurePosixPath(filename)
    return filename in DOCUMENTATION_FILES or (
        path.parts[0] in {"wiki", "tools"} and path.suffix == ".md"
    ) or (
        path.is_relative_to("omega-rust/omega/representations/optimization-core/promotions")
        and path.suffix == ".md"
    )


def output(root, arguments):
    return subprocess.check_output(arguments, cwd=root).decode("utf-8")


def changed_paths(root, base):
    # --no-renames exposes BOTH sides of moves, including moves between crates.
    tracked = output(root, ["git", "diff", "--no-ext-diff", "--no-renames",
                            "--name-only", "-z", base, "--"])
    untracked = output(root, ["git", "ls-files", "--others",
                              "--exclude-standard", "-z"])
    return sorted(set((tracked + untracked).split("\0")) - {""})


def workspace_crates(root, metadata):
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
    return packages, owners


def changed_source_crates(owners, paths):
    """Workspace crates whose own src/*.rs files appear in the diff."""
    touched = set()
    for filename in paths:
        path = PurePosixPath(filename)
        for directory, name in owners:
            if path.is_relative_to(directory):
                relative = path.relative_to(directory)
                if relative.parts[0] == "src" and relative.suffix == ".rs":
                    touched.add(name)
                break
    return touched


def selection(root, metadata, paths):
    packages, owners = workspace_crates(root, metadata)

    affected = set()
    reasons = []
    for filename in paths:
        path = PurePosixPath(filename)
        if is_documentation(filename):
            continue
        if filename in {
            "omega-rust/psi/semantics/terminal-codec/src/sections/trust_graph.rs",
            "omega-rust/psi/semantics/terminal-codec/src/sections/trust_graph/current.rs",
            "omega-rust/psi/semantics/terminal-codec/src/sections/trust_graph/identity.rs",
            "omega-rust/psi/semantics/terminal-codec/src/sections/trust_graph/validation.rs",
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


def slow_tail_filter(excluded):
    by_owner = {}
    for entry in excluded:
        if not re.fullmatch(r"[A-Za-z0-9_:]+", entry["test"]):
            raise ValueError(f"Unsupported test name: {entry['test']!r}")
        by_owner.setdefault(entry["package"], []).append(entry["test"])
    clauses = []
    for owner in sorted(by_owner):
        names = " | ".join(f"test({name})" for name in by_owner[owner])
        clauses.append(f"(package(={owner}) & ({names}))")
    return " | ".join(clauses)


JEV_ENDPOINT = "https://api.typesafe.ai/v1/systemone"
JEV_MODEL = "jev-1.13.0"
JEV_FLAG = 0.5
JEV_TIMEOUT_S = 15
JEV_CATALOG = Path(__file__).with_name("test_select_candidates.json")
JEV_CACHE = "build/test-select-cache"
JEV_MAX_PATHS = 60


def jev_api_key(root):
    """Resolve the TypeSafe key; None means the augment is unconfigured."""
    if os.environ.get("TYPESAFE_API_KEY"):
        return os.environ["TYPESAFE_API_KEY"]
    for candidate in (root / "build/typesafe.env.txt",
                      Path.home() / ".config/typesafe/typesafe.env.txt"):
        if candidate.exists():
            keys = [line.partition("=")[2].strip().strip('"\'')
                    for line in candidate.read_text(encoding="utf-8-sig").splitlines()
                    if line.partition("=")[0].strip() == "TYPESAFE_API_KEY"]
            if len(keys) == 1 and keys[0]:
                return keys[0]
    return None


def jev_payload(paths, affected, doc_audit_runs, candidates):
    baseline = [
        "the full omega-architecture-test suite (always runs on every diff)",
        "library tests of every changed crate and its reverse dependencies: "
        + (", ".join(affected) if affected
           else "all crates — the diff touched shared or unknown inputs so "
                "every library test already runs")]
    if doc_audit_runs:
        baseline.append("the documentation canary (audited docs changed)")
    questions = {
        f"needs_run::{c['id']}": {
            "type": "noul",
            "instructions": (
                "Decide whether running test `tests." + c["id"] + "` could catch "
                "a regression `change` might introduce that `baseline_runs` does "
                "NOT already cover. Flag yes when what the test observes "
                "intersects the change (modified code, moved or renamed paths, "
                "changed contracts or spellings, behavior produced by that "
                "code, consumers of the changed surface) AND the baseline does "
                "not already exercise it. A test asserting a catalog, roster, "
                "golden, or path list breaks when covered files move or are "
                "deleted — that IS an intersection. Answer no when coverage "
                "does not intersect, when the baseline already covers it, or "
                "when the only justification is \"everything might break\"."),
            "criteria": {
                "true": "adds coverage the baseline lacks for something this "
                        "change could break",
                "false": "cannot observe this change, or the baseline already "
                         "covers it"}}
        for c in candidates}
    return {"model": JEV_MODEL,
            "state": {"change": {"changed_paths": paths[:JEV_MAX_PATHS],
                                 "total_changed": len(paths)},
                      "baseline_runs": baseline,
                      "tests": {c["id"]: c["covers"] for c in candidates}},
            "questions": questions}


def jev_post(root, payload, key):
    """POST one selection request, cached by payload hash under build/."""
    body = json.dumps(payload, sort_keys=True).encode()
    cache = root / JEV_CACHE / (hashlib.sha256(body).hexdigest() + ".json")
    if cache.exists():
        return json.loads(cache.read_text(encoding="utf-8"))
    request = urllib.request.Request(
        JEV_ENDPOINT, data=json.dumps(payload).encode(),
        headers={"Authorization": "Bearer " + key,
                 "Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(request, timeout=JEV_TIMEOUT_S) as response:
        result = json.load(response)
    cache.parent.mkdir(parents=True, exist_ok=True)
    cache.write_text(json.dumps(result), encoding="utf-8")
    return result


def fixture_filters(paths):
    """tests/omega/<tier>/<group>/... -> corpus gate filter fragments."""
    filters = []
    for path in paths:
        parts = path.split("/")
        if len(parts) >= 4 and parts[:2] == ["tests", "omega"]:
            filters.append("/".join(parts[2:4]))
    return sorted(set(filters))


def jev_augment(root, runner, paths, affected, commands, disabled, full):
    """Union augment: Jev flags test candidates the deterministic baseline
    lacks. It can only add commands, never remove baseline coverage.

    Availability tiers: unconfigured (no API key) -> silent no-op; configured
    but unreachable or rejected -> one stderr warning, baseline unchanged."""
    summary = {"state": "unconfigured", "flagged": [], "added": [],
               "covered": [], "suggested": []}
    if disabled or os.environ.get("OMEGA_JEV_OFFLINE"):
        summary["state"] = "disabled"
        return summary
    if full:
        summary["state"] = "skipped (--full selects everything)"
        return summary
    key = jev_api_key(root)
    if key is None or not JEV_CATALOG.exists():
        return summary
    candidates = json.loads(
        JEV_CATALOG.read_text(encoding="utf-8"))["candidates"]
    doc_audit_runs = any(DOCUMENTATION_TEST in " ".join(c)
                         for c in commands)
    payload = jev_payload(paths, affected, doc_audit_runs, candidates)
    try:
        result = jev_post(root, payload, key)
    except Exception as error:  # augment must never break selection
        summary["state"] = f"unavailable: {error}"
        print(f"test_affected: Jev augment unavailable ({error}); "
              "deterministic selection only", file=sys.stderr)
        return summary
    answers = result.get("answers", {})
    flagged = {c["id"]: answers.get(f"needs_run::{c['id']}", {}).get("noul", 0.0)
               for c in candidates}
    summary["flagged"] = [cid for cid, p in flagged.items() if p >= JEV_FLAG]
    summary["state"] = "augmented"
    existing = {" ".join(c) for c in commands}
    affected_set = set(affected or [])
    for cand in candidates:
        cid = cand["id"]
        if flagged.get(cid, 0.0) < JEV_FLAG:
            continue
        command = cand.get("command")
        if cand.get("baseline"):
            summary["covered"].append(cid)
            continue
        if command is None or cand.get("auto_run") is False:
            summary["suggested"].append(cid)
            continue
        argv = [part.replace("{runner}", runner)
                      .replace("{python}", sys.executable)
                for part in command]
        if cand.get("auto_run") == "filterable":
            fragments = fixture_filters(paths)
            if not fragments:
                summary["suggested"].append(cid)
                continue
            argv = argv + ["--filter", ",".join(fragments)]
        # Affected-crate coverage applies only to nextest -p commands; an
        # mbx-run binary invocation is not something the baseline executes.
        is_nextest_pkg = "nextest" in argv and "-p" in argv
        package = argv[argv.index("-p") + 1] if is_nextest_pkg else None
        # --lib never runs bin-crate tests; those candidates are not covered
        # merely because their package is in the affected set.
        covered_by_baseline = (package and package in affected_set
                               and not cand.get("bin_package"))
        if covered_by_baseline or " ".join(argv) in existing:
            summary["covered"].append(cid)
            continue
        commands.append(argv)
        summary["added"].append(cid)
    return summary


def make_plan(root, runner, base, full=False, with_slow_tail=False):
    paths = [] if full else changed_paths(root, base)
    documentation_paths = [path for path in paths if is_documentation(path)]
    selected = set()
    direct = set()
    if full:
        expression, packages, reasons = "all()", [], ["Explicit full run"]
    else:
        metadata = json.loads(output(root, [runner, "metadata", "--locked",
                                            "--format-version", "1", "--no-deps"]))
        if Path(metadata["workspace_root"]).resolve() != root:
            raise ValueError("Cargo workspace root differs from Git root")
        expression, packages, reasons = selection(root, metadata, paths)
        member_packages, owners = workspace_crates(root, metadata)
        if expression == "all()":
            # Fallbacks still run every library; only the base packages list is
            # the directly changed subset.
            selected = {package["name"] for package in member_packages}
        else:
            selected = set(packages)
        direct = changed_source_crates(owners, paths)
    commands = [[runner, "nextest", "run", "--locked", "-p",
                 "omega-architecture-test", "--all-targets", "--no-fail-fast"]]
    if full or documentation_paths:
        commands.append([runner, "nextest", "run", "--locked", "-p", "compiler",
                         "--test", "canary_suite", "--no-fail-fast", "--no-tests", "fail",
                         "-E", f"test(={DOCUMENTATION_TEST})"])
    slow_tail = [
        {"package": owner, "test": name, "measured_seconds": seconds}
        for owner, tests in SLOW_TEST_OWNERS.items()
        if owner in selected
        for name, seconds in tests
    ]
    excluded = [] if with_slow_tail else [
        entry for entry in slow_tail if entry["package"] not in direct
    ]
    bounded = expression != "all()"
    if expression != "none()":
        if excluded:
            expression = f"({expression}) & not ({slow_tail_filter(excluded)})"
        # Keep --workspace even for a narrow filter. Splitting -p builds can
        # change feature unification and no longer match the full baseline.
        commands.append([runner, "nextest", "run", "--locked", "--workspace",
                         "--lib", "--no-fail-fast", "-E", expression])
        if bounded:
            # A valid selection can contain only bin crates (e.g. omega).
            # The separate integration gate covers those; this phase is --lib.
            commands[-1].extend(["--no-tests", "pass"])
    plan = {"base": base, "changed_paths": paths, "affected_packages": packages,
            "documentation_paths": documentation_paths,
            "filter": expression, "full_suite_reasons": reasons,
            "commands": commands}
    remaining = [entry for entry in slow_tail if entry not in excluded]
    if remaining:
        plan["slow_tail"] = remaining
    if excluded:
        plan["slow_tail_excluded"] = excluded
        plan["slow_tail_restore"] = "--with-slow-tail"
    return plan


def run_commands(root, commands):
    failed = 0
    for command in commands:
        result = subprocess.run(command, cwd=root, check=False)
        if result.returncode:
            failed = result.returncode
    return failed


def extract_failures(log_text):
    """Failure identifiers from a captured test log — nextest FAIL lines,
    pytest FAILED lines, and compile error: lines — deduplicated, capped."""
    failures, seen = [], set()
    for line in log_text.splitlines():
        stripped = line.strip()
        if stripped.startswith(("test result:", "error: test failed")):
            continue
        token = None
        if stripped.startswith("FAIL ") or " FAILED" in stripped:
            token = stripped[:160]
        elif stripped.startswith("FAILED "):
            token = stripped[:160]
        elif stripped.startswith(("error:", "error[")):
            token = stripped[:160]
        if token and token not in seen:
            seen.add(token)
            failures.append(stripped)
    return failures[:12]


def attribute_failures(root, base, log_path, disabled):
    """Advisory: classify each failure in the log as caused by the
    candidate diff, unrelated baseline, or environmental. Worked example:
    build/experiments/failure-triage/ATTRIBUTE.md — 7/7, including the
    discriminating pair (real regression under its own commit vs an
    innocent one). Solo calls per failure: batch contamination poisons
    sibling labels."""
    log_text = Path(log_path).read_text(encoding="utf-8", errors="replace")
    failures = extract_failures(log_text)
    if not failures:
        print("test_affected attribute: no failures parsed from the log",
              file=sys.stderr)
        return 0
    paths = changed_paths(root, base)
    if disabled or os.environ.get("OMEGA_JEV_OFFLINE", "").strip() == "1":
        key = ""
    else:
        key = jev_api_key(root)
    if not key:
        print("test_affected attribute: advisory unavailable "
              "(no TYPESAFE_API_KEY)", file=sys.stderr)
        return 0
    subject = output(root, ["git", "log", "-1", "--format=%s"]).strip()
    for failure in failures:
        payload = {"model": "jev-1.13.0",
                   "state": {"candidate_subject": subject,
                             "changed_paths": paths[:40],
                             "failure": failure},
                   "questions": {"attribution": {
                       "type": "choice",
                       "instructions": (
                           "This failure appeared while the candidate diff "
                           "(subject + changed_paths) was under test. Could "
                           "the diff plausibly produce it? "
                           "`caused_by_candidate`: the touched files could "
                           "produce this output — investigate the commit. "
                           "`unrelated_baseline`: outside the diff's reach "
                           "— check whether it fails at base. "
                           "`environmental`: flake/host/load/timing."),
                       "criteria": {
                           "caused_by_candidate": "the diff could plausibly "
                                                  "produce this failure",
                           "unrelated_baseline": "outside the diff's reach",
                           "environmental": "flake, host, load, or timing"}}}}
        try:
            answers = jev_post(root, payload, key).get("answers", {})
            verdict = answers.get("attribution", {}).get("choice", "?")
        except Exception:
            verdict = "unavailable"
        mark = {"caused_by_candidate": "YOURS", "unrelated_baseline":
                "baseline", "environmental": "environmental"}.get(
                verdict, "unknown")
        print(f"  = attribution [{mark}] {failure[:120]}", file=sys.stderr)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--base", help="Previously verified commit; compared to working files")
    mode.add_argument("--full", action="store_true", help="Run the complete portable baseline")
    parser.add_argument("--plan", action="store_true", help="Print JSON without building/running tests")
    parser.add_argument("--attribute", metavar="LOG",
                        help="With --base: classify each failure in LOG "
                             "against the candidate diff (advisory; runs no "
                             "tests)")
    parser.add_argument("--with-slow-tail", action="store_true",
                        help="Include measured multi-minute tests excluded from "
                             "routine-diff selections; --full never excludes them")
    parser.add_argument("--no-jev", action="store_true",
                        help="Skip the Jev semantic test-selection augment "
                             "(also suppressed by OMEGA_JEV_OFFLINE=1)")
    args = parser.parse_args()
    try:
        root = Path(output(Path.cwd(), ["git", "rev-parse", "--show-toplevel"]).strip()).resolve()
        base = None
        if args.base:
            base = output(root, ["git", "rev-parse", "--verify", "--end-of-options",
                                 args.base + "^{commit}"]).strip()
        if args.attribute:
            if not args.base:
                raise ValueError("--attribute requires --base")
            return attribute_failures(root, base, args.attribute,
                                      args.no_jev)
        runner = shutil.which("mbx") or shutil.which("cargo")
        if not runner:
            raise ValueError("Install mbx (preferred) or Cargo, and cargo-nextest")
        plan = make_plan(root, runner, base, args.full, args.with_slow_tail)
        plan["jev"] = jev_augment(root, runner, plan["changed_paths"],
                                  plan["affected_packages"], plan["commands"],
                                  args.no_jev, args.full)
        print(json.dumps(plan, indent=2), flush=True)
        if args.plan:
            return 0
        return run_commands(root, plan["commands"])
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"test_affected: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
