#!/usr/bin/env python3
"""Native regression watch — the scheduled full native corpus run.

The merge queue checks integration compiles, and routine validation
runs the native corpus leg only under `--filter` for the behavior an author
changed (testing.md). Nothing ran the whole native leg, so a program the flow
had turned green could go red again unnoticed: on 2026-09-29 a snapshot found
`calls/runtime_explicit_discard_executes_exit`, green since 46f28365fd,
rejected natively on main a day later. This tool is the scheduled workload
testing.md describes: run every few hours, it records which programs run and
reports the ones that stopped.

Each run snapshots one revision of main (default `origin/main`) in a scratch
checkout, runs `corpus_gate.py --native` against an empty golden so every
fixture reports its own record, and stores the result under
`build/native_watch/`. A fixture is *green* when it builds and meets its
native oracle, if it has one. The run then diffs against the previous
snapshot:

- regressed: green before, present and not green now. testing.md sanctions
  one kind - a fixture a structural move left rejecting with an
  `unimplemented:` diagnostic - so those are listed as debt. Every other
  regression (another diagnostic, a crash, a program that builds and exits
  wrongly) is a violation: bisected to its first bad commit unless
  `--no-bisect`, and the run exits 1. Each regressed fixture is first re-run
  alone in the same checkout; one that passes is listed as flaky instead,
  since a loaded host times fixtures out (the 2026-09-29 test run reported
  `collections/runtime_vec_u32_growth_exit` as a timeout beside six busy
  agent sessions).
- arrived red: a fixture new since the last snapshot whose native oracle
  already fails — added as a target, or added broken. Reported, not failed:
  only green-to-red counts as a regression, so an intentionally red target
  never alarms.
- turned green / added green: reported as the flow's progress.

The watch never edits boards or goldens; the report names the commit range
and first bad commit so the coordinator can file the item.

The gate stops before writing moves when an authored oracle fails (exit 3),
but its `--json` verdict document carries every observation and names each
oracle failure on every outcome. The watch reads that document from a single
run, so an oracle failure, or a fixture that times out under load, is one
red row rather than a second full run that can trip on a different fixture.

    python3 tools/native_watch.py                # snapshot origin/main, diff, bisect
    python3 tools/native_watch.py --no-bisect    # report only
    python3 tools/native_watch.py --ref <sha>    # snapshot a specific revision
    python3 tools/native_watch.py --no-bisect --shards 12 --minimum-fixtures 10298

`--shards N` runs the leg as N hash shards (`corpus_gate.py --shard k/N`), persisting
each shard under `build/native_watch/shard-<revision>-<k>of<N>.json` as it finishes and
merging them into the same snapshot JSON. Re-running with the same `--shards` reads the
finished shards back instead of measuring them again. Use it with
`--minimum-fixtures <corpus count>` so a partial shard set cannot be written as a
composite snapshot.

Sharding is not a convenience. On 2026-10-20 the single-invocation leg ran 119 minutes
on a 10-core, 16 GB host, its runner's RSS grew 1.25 -> 2.80 -> 3.83 -> 4.05 -> 6.83 GB,
and it was killed at a 4 GB bound; because the gate writes its verdict document once,
at the end, every fixture outcome died with it and the three-week-stale baseline stayed
stale. The corpus is now over ten thousand fixtures, so that leg cannot complete as one
invocation on this class of host. Shard it.

Scheduling is per machine; testing.md has launchd and Task Scheduler
entries.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STATE = ROOT / "build" / "native_watch"
BISECT_LIMIT = 5


def run(arguments, cwd=None, check=True, **keywords):
    result = subprocess.run(arguments, cwd=cwd, capture_output=True, text=True, **keywords)
    if check and result.returncode != 0:
        raise RuntimeError(f"{' '.join(map(str, arguments))} -> {result.returncode}: {result.stderr[-2000:]}")
    return result


def gate(checkout, arguments):
    """Run the checkout's own corpus gate natively against an empty golden."""
    golden = checkout / "build" / "native_watch_empty_golden.txt"
    golden.parent.mkdir(parents=True, exist_ok=True)
    golden.write_text("")
    return run([sys.executable, "tools/corpus_gate.py", "--native", "--golden", str(golden), *arguments],
               cwd=checkout, check=False)


def forget_checkout(checkout):
    """Drop a scratch checkout and the build output mbx keeps for it (4-26 GB each)."""
    if shutil.which("mbx"):
        run(["mbx", "cache", "remove", str(checkout.resolve())], check=False)
    shutil.rmtree(checkout, ignore_errors=True)
    if shutil.which("mbx"):
        run(["mbx", "gc"], check=False)


def checkout_of(revision):
    """A scratch checkout of one revision, outside the repository, with no history."""
    checkout = Path(tempfile.mkdtemp(prefix="omega-native-watch-"))
    archive = subprocess.Popen(["git", "-C", str(ROOT), "archive", revision], stdout=subprocess.PIPE)
    run(["tar", "-x", "-C", str(checkout)], stdin=archive.stdout)
    archive.wait()
    run(["git", "init", "-q"], cwd=checkout)
    return checkout


def shard_path(state, revision, index, shards):
    """Where one shard's observations are kept, so a kill cannot lose finished work."""
    return state / f"shard-{revision[:12]}-{index}of{shards}.json"


def snapshot_shard(checkout, revision, index, shards, state):
    """Run one hash shard of the native corpus; persist and return its observations.

    `corpus_gate.py --shard k/N` selects a deterministic, disjoint hash slice of the
    corpus and the slices are stable across fixture additions, so N invocations cover
    everything exactly once without the watch carrying its own fixture plan. A
    finished shard is written before the next one starts, so re-running the watch
    with the same `--shards` reads it back instead of measuring it again.

    Sharding bounds the runner's resident memory, which grows with the work one
    invocation performs: the monolithic leg reached 6.83 GB in 119 minutes, was
    killed at a 4 GB bound, and lost every fixture outcome with it because the gate
    writes its verdict document once, at the end.
    """
    path = shard_path(state, revision, index, shards)
    if path.exists():
        kept = json.loads(path.read_text())
        print(f"native_watch: shard {index}/{shards} reused from {path.name} "
              f"({len(kept['status'])} fixtures)", flush=True)
        return kept["status"], kept["diagnostic"]
    verdict = checkout / "build" / f"native_watch_verdict-{index}of{shards}.json"
    result = gate(checkout, ["--shard", f"{index}/{shards}", "--json", str(verdict)])
    document = json.loads(verdict.read_text()) if verdict.exists() else {}
    if result.returncode >= 4 or not document or document.get("infrastructure_error"):
        output = result.stdout + result.stderr
        raise RuntimeError(
            f"native gate failed at {revision} shard {index}/{shards} "
            f"(exit {result.returncode}); finished shards are kept in {state}:\n"
            + "\n".join(output.splitlines()[-20:]))
    records = document["observations"]
    status = {record["fixture"]: record["status"] for record in records}
    diagnostic = {record["fixture"]: (record.get("diagnostics") or [""])[0]
                  for record in records}
    # A refusal's own diagnostic says why the oracle went unmet; the oracle
    # text ("expected native 70, observed rejected") stands in only when the
    # program built and ran but produced the wrong result.
    for detail in document["oracle_failures"]:
        fixture, _, reason = detail.partition(": ")
        status[fixture] = "oracle-fail"
        diagnostic[fixture] = diagnostic.get(fixture) or reason
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"revision": revision, "shard": index,
                                "shards": shards, "status": status,
                                "diagnostic": diagnostic},
                               indent=1, sort_keys=True) + "\n")
    print(f"native_watch: shard {index}/{shards} measured {len(status)} fixtures",
          flush=True)
    return status, diagnostic


def snapshot(checkout, revision, shards=1, state=None):
    """Return ({fixture: status}, {fixture: first diagnostic}) for one checkout.

    status is 'built', 'oracle-fail', or the gate's own status ('rejected', ...).
    `shards > 1` runs the native leg once per hash shard, persisting each shard as
    it finishes and merging the shards into one snapshot."""
    if shards == 1:
        return snapshot_whole(checkout, revision)
    merged_status, merged_diagnostic = {}, {}
    for index in range(shards):
        status, diagnostic = snapshot_shard(checkout, revision, index, shards, state)
        overlap = set(status) & set(merged_status)
        if overlap:
            # Hash shards are disjoint by construction; an overlap means the shard
            # denominator or the hash changed between runs, which would silently
            # double-count.
            raise RuntimeError(f"native watch shard {index}/{shards} overlaps an "
                               f"earlier shard on {len(overlap)} fixtures")
        merged_status.update(status)
        merged_diagnostic.update(diagnostic)
    return merged_status, merged_diagnostic


def snapshot_whole(checkout, revision):
    """The single-invocation snapshot, kept for a host that can hold the whole leg."""
    verdict = checkout / "build" / "native_watch_verdict.json"
    result = gate(checkout, ["--json", str(verdict)])
    document = json.loads(verdict.read_text()) if verdict.exists() else {}
    if result.returncode >= 4 or not document or document.get("infrastructure_error"):
        output = result.stdout + result.stderr
        raise RuntimeError(f"native gate failed at {revision} (exit {result.returncode}):\n"
                           + "\n".join(output.splitlines()[-20:]))
    records = document["observations"]
    status = {record["fixture"]: record["status"] for record in records}
    diagnostic = {record["fixture"]: (record.get("diagnostics") or [""])[0] for record in records}
    for detail in document["oracle_failures"]:
        fixture, _, reason = detail.partition(": ")
        status[fixture] = "oracle-fail"
        diagnostic[fixture] = diagnostic.get(fixture) or reason
    return status, diagnostic


def fixture_green(checkout, fixture):
    """0 when the fixture is green in this checkout, 1 when not, 125 on gate failure."""
    result = gate(checkout, ["--filter", fixture])
    output = result.stdout + result.stderr
    if f"{fixture}: expected native" in output:
        return 1
    if result.returncode >= 4:
        return 125
    # Against an empty golden the gate lists the fixture as "+ <fixture>: new fixture (<status>)".
    record = next((line for line in output.splitlines() if line.startswith(f"+ {fixture}:")), "")
    return 0 if record.endswith("(built)") else 1


def first_bad_commit(fixture, good, bad):
    clone = Path(tempfile.mkdtemp(prefix="omega-native-bisect-"))
    try:
        run(["git", "clone", "-q", "--shared", str(ROOT), str(clone)])
        run(["git", "bisect", "start", bad, good], cwd=clone)
        result = run(["git", "bisect", "run", sys.executable, str(Path(__file__).resolve()),
                      "--bisect-step", fixture], cwd=clone, check=False)
        line = next((l for l in result.stdout.splitlines() if l.endswith("is the first bad commit")), "")
        if not line:
            return None
        return run(["git", "log", "-1", "--format=%h %an: %s", line.split()[0]], cwd=clone).stdout.strip()
    finally:
        forget_checkout(clone)


def green(status):
    return {fixture for fixture, value in status.items() if value == "built"}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--ref", default="origin/main", help="revision to snapshot (default origin/main)")
    parser.add_argument("--no-bisect", action="store_true", help="report regressions without bisecting them")
    parser.add_argument("--shards", type=int, default=1, metavar="N",
                        help="run the native corpus as N hash shards and merge them into one "
                             "snapshot, persisting each shard as it finishes so a kill costs one "
                             "shard instead of the whole leg (default 1 = one invocation)")
    parser.add_argument("--minimum-fixtures", type=int, default=1, metavar="COUNT",
                        help="refuse to write a composite snapshot covering fewer than COUNT "
                             "fixtures, so an incomplete shard set cannot pass as full coverage")
    parser.add_argument("--bisect-step", metavar="FIXTURE", help=argparse.SUPPRESS)
    options = parser.parse_args()
    if options.shards < 1:
        parser.error("--shards must be at least 1")
    if options.bisect_step:
        return fixture_green(Path.cwd(), options.bisect_step)

    if options.ref.startswith("origin/"):
        run(["git", "-C", str(ROOT), "fetch", "-q", "origin"], check=False)
    revision = run(["git", "-C", str(ROOT), "rev-parse", options.ref]).stdout.strip()
    STATE.mkdir(parents=True, exist_ok=True)
    temp_dir = STATE / "temp"
    if temp_dir.exists():
        for pattern in ("omega-native-watch-*", "omega-native-bisect-*"):
            for stale in temp_dir.glob(pattern):
                if stale.is_dir():
                    forget_checkout(stale)
    record_path = STATE / f"{revision[:12]}.json"
    checkout = None
    try:
        if record_path.exists():
            current = json.loads(record_path.read_text())
        else:
            checkout = checkout_of(revision)
            status, diagnostic = snapshot(checkout, revision, options.shards, STATE)
            if len(status) < options.minimum_fixtures:
                raise RuntimeError(
                    f"native watch covered {len(status)} fixtures, below the required "
                    f"{options.minimum_fixtures}: the shard set is incomplete, and a "
                    f"composite snapshot from it would silently claim full coverage")
            current = {"revision": revision, "status": status, "diagnostic": diagnostic}
            record_path.write_text(json.dumps(current, indent=1, sort_keys=True) + "\n")
            print(f"native_watch: composite snapshot written to {record_path.name} "
                  f"({len(status)} fixtures from {options.shards} shard(s))", flush=True)
        return report(options, revision, current, checkout)
    finally:
        if checkout:
            forget_checkout(checkout)


def render_report(previous_revision, revision, before, after, why,
                  violations, sanctioned, flaky, arrived_red, turned_green, added_green, culprits):
    lines = [f"# Native regression watch: {previous_revision[:12]}..{revision[:12]}", "",
             f"{len(green(after))} of {len(after)} fixtures green (was {len(green(before))} of {len(before)}).", ""]
    for title, fixtures in (("Regressed", violations), ("Sanctioned debt (unimplemented:)", sanctioned),
                            ("Flaky (green on isolated retry)", flaky), ("Arrived red", arrived_red),
                            ("Turned green", turned_green), ("Added green", added_green)):
        lines.append(f"## {title}: {len(fixtures)}")
        for fixture in fixtures:
            note = ""
            if title == "Regressed":
                note = f" — {after[fixture]}: {why.get(fixture, '')[:160]}" + (
                    f"; first bad commit {culprits[fixture]}" if culprits.get(fixture) else "")
            elif title.startswith(("Sanctioned", "Flaky")):
                note = f" — {after[fixture]}: {why.get(fixture, '')[:160]}"
            lines.append(f"- `{fixture}`{note}")
        lines.append("")
    return lines


def report(options, revision, current, checkout):
    """Diff against the previous snapshot, write the report, and return the exit status."""
    latest = STATE / "latest.txt"
    previous_revision = latest.read_text().strip() if latest.exists() else None
    if not previous_revision or previous_revision == revision:
        latest.write_text(revision + "\n")
        print(f"native_watch: {revision[:12]} {len(green(current['status']))} green of "
              f"{len(current['status'])}; no earlier snapshot to compare")
        return 0
    previous = json.loads((STATE / f"{previous_revision[:12]}.json").read_text())

    before, after = previous["status"], current["status"]
    why = current.get("diagnostic", {})
    regressed = sorted(f for f in green(before) if f in after and after[f] != "built")
    # A loaded machine times out or crashes a fixture now and then; one isolated retry
    # separates that from a real regression before anyone is asked to bisect it.
    flaky = [f for f in regressed if checkout and fixture_green(checkout, f) == 0]
    sanctioned = [f for f in regressed if f not in flaky and after[f] == "rejected"
                  and why.get(f, "").startswith("unimplemented:")]
    violations = [f for f in regressed if f not in flaky and f not in sanctioned]
    turned_green = sorted(f for f in green(after) if f in before and before[f] != "built")
    added_green = sorted(f for f in green(after) if f not in before)
    arrived_red = sorted(f for f, value in after.items() if value == "oracle-fail" and f not in green(before))

    path = STATE / f"report-{revision[:12]}.md"
    culprits = {}
    lines = render_report(previous_revision, revision, before, after, why,
                          violations, sanctioned, flaky, arrived_red, turned_green, added_green, culprits)
    path.write_text("\n".join(lines))
    latest.write_text(revision + "\n")

    if violations and not options.no_bisect:
        for fixture in violations[:BISECT_LIMIT]:
            culprits[fixture] = first_bad_commit(fixture, previous_revision, revision)
            lines = render_report(previous_revision, revision, before, after, why,
                                  violations, sanctioned, flaky, arrived_red, turned_green, added_green, culprits)
            path.write_text("\n".join(lines))

    print("\n".join(lines[:3] + [f"{len(violations)} regressed, {len(sanctioned)} sanctioned, {len(flaky)} flaky, "
                                 f"{len(arrived_red)} arrived red, {len(turned_green)} turned green, "
                                 f"{len(added_green)} added green -> {path}"]))
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
