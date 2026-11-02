#!/usr/bin/env python3
"""`corpus_gate.py --compare <base> --remote`: a base comparison on hosted CI.

A local base-vs-candidate compare runs its selection twice and first builds the
base's runner. On 2026-11-01, 43 PRs cited 14,040 compare executions, about 22
CPU-hours plus 4 to 9 build-slot-hours on a 10-core, 16 GB Mac with two build
slots. Hosted runners cost this public repository nothing and start a job about
0.1 minutes after dispatch, so the compare runs there instead: the
`Compiler queue pilot` workflow is dispatched once at the base and once at the
candidate with the same exact fixture list, and its JSON corpus reports are
diffed by the record comparison a local `--compare` uses, with the same exit
codes (0 nothing lost, 1 lost, 2 usage, 4 not measured).

The candidate is the committed HEAD, pushed to the session's existing work
branch; AGENTS.md already requires that push, so an unpushed line of work is
refused rather than given a branch here. A dispatch names a branch, not a
commit, so the base is pushed to `work/compare-base-<sha12>` and that ref is
deleted once its run exists. Base rows are cached beside the local compare rows
(`build/corpus_baselines/<sha>-remote-<host>-<leg>.txt`), so a later remote
compare against the same base dispatches only the candidate.

The default checked leg dispatches `checking_only` on `linux_x86_64`.
`--native`/`--both` also runs the selected pass fixtures through both engines
on the matching host and compares those rows as well. The pilot takes exact
`pass/` and `fail/` IDs only, so `run/` fixtures and `--shard` stay local.
"""

from datetime import datetime
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time

import corpus_gate
import corpus_records

WORKFLOW = "compiler-pilot.yml"
# GitHub refuses dispatch inputs over 65,535 characters in total.
INPUT_LIMIT = 60_000
POLL_SECONDS = 30.0
REPORTS = {"checking": "checking.json", "both": "both.json"}
# The pilot's `native_host` choices.
HOSTS = ("linux_x86_64", "linux_arm64", "windows_x86_64", "macos_arm64")
BASE_BRANCH = "work/compare-base-"


class RemoteUsage(Exception):
    """The request cannot be dispatched as given (exit 2)."""


class RemoteFailure(Exception):
    """The comparison was not measured (exit 4)."""


def run(*arguments, input=None) -> str:
    """One `git` or `gh` call in the measured checkout; the tests replace it."""
    result = subprocess.run(arguments, cwd=corpus_gate.ROOT, input=input,
                            capture_output=True, text=True,
                            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()[-2000:]
        raise RemoteFailure(f"{' '.join(arguments[:3])} failed: {detail}")
    return result.stdout.strip()


def fixtures_at(revision: str) -> set:
    """Every pass/ and fail/ fixture ID with a `main.omg` at `revision`."""
    listing = run("git", "ls-tree", "-r", "--name-only", revision, "--",
                  "tests/omega/pass", "tests/omega/fail")
    return {path[len("tests/omega/"):-len("/main.omg")]
            for path in listing.splitlines()
            if path.endswith("/main.omg") and path.count("/") == 5}


def dispatch_selection(filter_text: str, available: set) -> list:
    """The exact IDs a filter selects among `available`, the way the runner's
    substring filter does. Every superstring of a selected ID is selected
    too, so the pilot's collision refusal never fires."""
    fragments = [part.strip() for part in filter_text.split(",") if part.strip()]
    return sorted(fixture for fixture in available
                  if any(fragment in fixture for fragment in fragments))


def work_branch(head: str, base: str, requested: str) -> str:
    """The session's pushed work branch that HEAD continues."""
    if requested:
        try:
            run("git", "rev-parse", "--verify", f"refs/remotes/origin/{requested}")
        except RemoteFailure:
            raise RemoteUsage(f"corpus_gate: --remote names {requested!r}, which is not "
                              "a pushed work branch; push it first (git push origin "
                              "HEAD:refs/heads/work/<name>, with its draft PR)") from None
        if not requested.startswith("work/"):
            raise RemoteUsage(f"corpus_gate: --remote names {requested!r}; pass work/<name>")
        return requested
    listing = run("git", "for-each-ref", "--merged", head, "--no-merged", base,
                  "--format=%(refname:strip=3)", "refs/remotes/origin/work/")
    candidates = [name for name in listing.splitlines()
                  if name and not name.startswith(BASE_BRANCH)]
    if not candidates:
        raise RemoteUsage(
            "corpus_gate: --remote dispatches the work branch this HEAD continues, "
            "and none is pushed; push it first (git push origin "
            "HEAD:refs/heads/work/<name>, with its draft PR) or name it with "
            "--remote work/<name>")
    distance = {name: int(run("git", "rev-list", "--count", f"origin/{name}..{head}"))
                for name in candidates}
    nearest = min(distance.values())
    closest = sorted(name for name, count in distance.items() if count == nearest)
    if len(closest) > 1:
        raise RemoteUsage("corpus_gate: HEAD continues several pushed work branches ("
                          + ", ".join(closest) + "); name one with --remote work/<name>")
    return closest[0]


def dispatch(ref: str, revision: str, host: str, fixtures: list,
             checking_only: bool) -> str:
    """Start the pilot at `ref` (which must point at `revision`); return its run id."""
    inputs = {"fixtures": ",".join(fixtures), "native_host": host,
              "checking_only": "true" if checking_only else "false"}
    started = time.time()
    output = run("gh", "workflow", "run", WORKFLOW, "--ref", ref, "--json",
                 input=json.dumps(inputs))
    found = re.search(r"/actions/runs/([0-9]+)", output)
    if found:
        return found.group(1)
    # A gh that does not print the created run: find it by ref and commit.
    for _ in range(20):
        runs = json.loads(run("gh", "run", "list", "--workflow", WORKFLOW,
                              "--branch", ref, "--event", "workflow_dispatch",
                              "--limit", "20", "--json",
                              "databaseId,headSha,createdAt"))
        for entry in runs:
            created = datetime.fromisoformat(entry["createdAt"].replace("Z", "+00:00"))
            if entry["headSha"] == revision and created.timestamp() >= started - 5:
                return str(entry["databaseId"])
        time.sleep(3)
    raise RemoteFailure(f"dispatched {WORKFLOW} at {ref} but found no run")


def run_url(run_id: str) -> str:
    return run("gh", "run", "view", run_id, "--json", "url", "--jq", ".url")


def wait(run_id: str) -> str:
    """Block until the run completes; return its conclusion."""
    while True:
        view = json.loads(run("gh", "run", "view", run_id, "--json",
                              "status,conclusion"))
        if view["status"] == "completed":
            return view["conclusion"]
        time.sleep(POLL_SECONDS)


def reports(run_id: str, revision: str, directory: Path) -> dict:
    """The run's corpus verdict documents by leg, from its small records
    artifact, or from the full artifact for a base older than that."""
    names = run("gh", "api", f"repos/{{owner}}/{{repo}}/actions/runs/{run_id}/artifacts",
                "--jq", ".artifacts[].name").splitlines()
    records = sorted(name for name in names if name.startswith("compiler-pilot-records-"))
    full = sorted(name for name in names
                  if name.startswith("compiler-pilot-") and name not in records)
    chosen = (records or full)[-1:]
    if not chosen:
        return {}
    target = directory / run_id
    run("gh", "run", "download", run_id, "-n", chosen[0], "-D", str(target))
    documents = {}
    for leg, name in REPORTS.items():
        path = target / name
        if not path.is_file():
            continue
        document = json.loads(path.read_text(encoding="utf-8"))
        if (document.get("schema") != "omega-corpus-verdict-v1"
                or document.get("revision") != revision):
            raise RemoteFailure(f"run {run_id}'s {name} measured another source")
        documents[leg] = document["observations"]
    return documents


def write_rows(cache: Path, rows: dict) -> None:
    cache.parent.mkdir(parents=True, exist_ok=True)
    staged = cache.with_name(f"{cache.name}.{os.getpid()}.tmp")
    staged.write_text(corpus_records.render([rows[fixture] for fixture in sorted(rows)]),
                      encoding="utf-8")
    os.replace(staged, cache)


def compare(options, base: str, host: str, checking_only: bool,
            filter_text: str, requested_branch: str) -> int:
    """Dispatch, wait, diff; return the local compare's exit code."""
    if host not in HOSTS:
        raise RemoteUsage(f"corpus_gate: the pilot has no {host} runner; it runs "
                          + ", ".join(HOSTS))
    head = run("git", "rev-parse", "HEAD")
    if corpus_gate.dirty_paths():
        raise RemoteUsage("corpus_gate: --remote measures commits; commit or "
                          "drop this tree's edits first")
    local_only = [fixture for fixture in corpus_gate.selected_fixtures(filter_text)
                  if fixture.startswith("run/")]
    if local_only:
        raise RemoteUsage(f"corpus_gate: the pilot takes pass/ and fail/ fixtures only; "
                          f"the filter selects {len(local_only)} run/ fixtures "
                          f"({local_only[0]}, ...). Narrow --filter, or compare them "
                          "with --local.")
    selected = dispatch_selection(filter_text, fixtures_at(head))
    if not selected:
        raise RemoteUsage(f"corpus_gate: --filter {filter_text!r} selects no committed fixture")
    base_selection = dispatch_selection(filter_text, fixtures_at(base))
    if max(len(",".join(selected)), len(",".join(base_selection))) > INPUT_LIMIT:
        raise RemoteUsage("corpus_gate: the fixture list exceeds the dispatch input limit; "
                          "narrow --filter")
    legs = ("checking",) if checking_only else ("checking", "both")
    compared = {"checking": [fixture for fixture in selected if fixture in base_selection]}
    compared["both"] = [fixture for fixture in compared["checking"] if fixture.startswith("pass/")]
    caches = {leg: corpus_gate.baseline_path(base, f"remote-{host}-{leg}") for leg in legs}
    base_rows = {leg: corpus_gate.read_baseline(caches[leg]) for leg in legs}
    base_cached = all(fixture in base_rows[leg] for leg in legs for fixture in compared[leg])

    branch = work_branch(head, base, requested_branch)
    if run("git", "rev-parse", f"origin/{branch}") != head:
        run("git", "push", "origin", f"{head}:refs/heads/{branch}")
    runs = {}
    base_ref = None
    if not base_cached and base_selection:
        base_ref = BASE_BRANCH + base[:12]
        run("git", "push", "origin", f"{base}:refs/heads/{base_ref}")
    try:
        if base_ref:
            runs["base"] = dispatch(base_ref, base, host, base_selection, checking_only)
        runs["head"] = dispatch(branch, head, host, selected, checking_only)
        urls = {side: run_url(run_id) for side, run_id in runs.items()}
        for side, url in urls.items():
            print(f"corpus_gate: {side} run {url}", file=sys.stderr)
        conclusions = {side: wait(run_id) for side, run_id in runs.items()}
    finally:
        if base_ref:
            try:
                run("git", "push", "origin", "--delete", base_ref)
            except RemoteFailure:
                pass  # another session comparing the same base removed it

    with tempfile.TemporaryDirectory(prefix="remote-compare-") as directory:
        measured = {side: reports(run_id, base if side == "base" else head, Path(directory))
                    for side, run_id in runs.items()}
    for side, documents in measured.items():
        if "checking" not in documents:
            raise RemoteFailure(f"corpus_gate: the {side} run ({conclusions[side]}) left "
                                f"no corpus rows: {urls[side]}")
    for leg, rows in measured.get("base", {}).items():
        if leg in caches:
            for row in rows:
                row["recorded_at"] = base[:12]
                base_rows[leg][row["fixture"]] = row
            write_rows(caches[leg], base_rows[leg])

    head_rows = measured["head"]
    losses, gains, results = [], [], {}
    for leg in legs:
        before = [base_rows[leg][fixture] for fixture in compared[leg]
                  if fixture in base_rows[leg]]
        after = head_rows.get(leg, [])
        if not before and not after:
            print(f"corpus_gate: no {leg} rows on either side "
                  "(no selected pass fixture reached native/Terminal execution)")
            continue
        if compared[leg] and not before:
            # Every head row reads as new, so this leg cannot show a loss.
            print(f"corpus_gate: the base left no {leg} rows; that leg shows "
                  "gains only", file=sys.stderr)
        result = corpus_records.compare_records(before, after)
        results[leg] = result
        losses += [f"{leg}: {line}" for line in result["losses"]]
        gains += [f"{leg}: {line}" for line in result["gains"]]
    for line in losses:
        print(f"- {line}")
    for line in gains:
        print(f"+ {line}")
    print(f"corpus_gate: compared {len(selected)} fixtures against {base[:12]} on {host} "
          f"({', '.join(results) or 'no legs'}): {len(losses)} lost, {len(gains)} gained; "
          + ", ".join(f"{side} {conclusions[side]} {urls[side]}" for side in runs))
    corpus_gate.write_verdict(options, {
        "schema": "omega-corpus-compare-v1",
        "base": base,
        "head": head,
        "dirty": False,
        "leg": "both" if not checking_only else "checked",
        "remote": {"host": host, "runs": urls, "conclusions": conclusions,
                   "base_rows_cached": "base" not in runs},
        "fixtures": len(selected),
        "losses": losses,
        "gains": gains,
    })
    return 1 if losses else 0
