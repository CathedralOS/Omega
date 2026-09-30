#!/usr/bin/env python3
"""Native regression watch — the scheduled full native corpus run.

Landing proves a change compiles (`landing.py verify`), and routine validation
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

The gate stops before writing its records when an authored native exit
fails (exit 3). The watch records those fixtures as oracle failures, drops
their oracle files in the scratch checkout, and runs again for the rest.

    python3 tools/native_watch.py                # snapshot origin/main, diff, bisect
    python3 tools/native_watch.py --no-bisect    # report only
    python3 tools/native_watch.py --ref <sha>    # snapshot a specific revision

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


def oracle_failures(output):
    return sorted({line.split("corpus_gate: ", 1)[1].split(":")[0]
                   for line in output.splitlines() if ": expected native" in line})


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


def snapshot(checkout, revision):
    """Return ({fixture: status}, {fixture: first diagnostic}) for one checkout.

    status is 'built', 'oracle-fail', or the gate's own status ('rejected', ...)."""
    moves = checkout / "build" / "native_watch_moves.json"
    result = gate(checkout, ["--write-moves", str(moves)])
    output = result.stdout + result.stderr
    failed = oracle_failures(output)
    if result.returncode == 3 and failed:
        for fixture in failed:
            (checkout / "tests" / "omega" / fixture / "expected_native_exit.txt").unlink(missing_ok=True)
        result = gate(checkout, ["--write-moves", str(moves)])
        output += result.stdout + result.stderr
    if result.returncode >= 4 or not moves.exists():
        raise RuntimeError(f"native gate failed at {revision} (exit {result.returncode}):\n"
                           + "\n".join(output.splitlines()[-20:]))
    records = json.loads(moves.read_text())
    status = {move["fixture"]: move["after"]["status"] for move in records}
    diagnostic = {move["fixture"]: (move["after"].get("diagnostics") or [""])[0] for move in records}
    for fixture in failed:
        status[fixture] = "oracle-fail"
        diagnostic[fixture] = next(line.split(": ", 1)[1] for line in output.splitlines()
                                   if line.startswith(f"corpus_gate: {fixture}: expected native"))
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
    parser.add_argument("--bisect-step", metavar="FIXTURE", help=argparse.SUPPRESS)
    options = parser.parse_args()
    if options.bisect_step:
        return fixture_green(Path.cwd(), options.bisect_step)

    if options.ref.startswith("origin/"):
        run(["git", "-C", str(ROOT), "fetch", "-q", "origin"], check=False)
    revision = run(["git", "-C", str(ROOT), "rev-parse", options.ref]).stdout.strip()
    STATE.mkdir(parents=True, exist_ok=True)
    record_path = STATE / f"{revision[:12]}.json"
    checkout = None
    try:
        if record_path.exists():
            current = json.loads(record_path.read_text())
        else:
            checkout = checkout_of(revision)
            status, diagnostic = snapshot(checkout, revision)
            current = {"revision": revision, "status": status, "diagnostic": diagnostic}
            record_path.write_text(json.dumps(current, indent=1, sort_keys=True) + "\n")
        return report(options, revision, current, checkout)
    finally:
        if checkout:
            forget_checkout(checkout)


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

    culprits = {}
    if violations and not options.no_bisect:
        for fixture in violations[:BISECT_LIMIT]:
            culprits[fixture] = first_bad_commit(fixture, previous_revision, revision)

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
    path = STATE / f"report-{revision[:12]}.md"
    path.write_text("\n".join(lines))
    latest.write_text(revision + "\n")
    print("\n".join(lines[:3] + [f"{len(violations)} regressed, {len(sanctioned)} sanctioned, {len(flaky)} flaky, "
                                 f"{len(arrived_red)} arrived red, {len(turned_green)} turned green, "
                                 f"{len(added_green)} added green -> {path}"]))
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
