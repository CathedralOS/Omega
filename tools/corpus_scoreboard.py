#!/usr/bin/env python3
"""Native corpus scoreboard: how much of the corpus runs, per host.

The mission metric is `omega run <program.omg>` producing the program's
authored result. Commit counts, closed items and landed diffs are only
proxies for it, so swarm wave reports lead with this scoreboard instead.

The numbers come from this host's native golden
(`tests/omega/corpus_native_<target>.txt`), which moves only when a lander
re-records with `corpus_gate.py --native --record` — the gate forces that
re-record when observed behavior moves — so the golden is the committed
record of what runs on this host. It is per host: a wave's delta is
measured against the same file on the same machine.

A fixture "executed" when its record carries an `exit` fact. It is
"authored" when it also satisfies `expected_native_exit.txt` — a numeric
oracle matches `exit` exactly, and `trap` matches any recorded signal exit
(corpus_gate enforces the precise signal before recording). Executed
fixtures without an oracle are "unverified": they ran, but nothing pinned
the result.

"regressed" counts rows carrying a `regressed-from:` token — a success this
host recorded and no longer reaches. Those rows are not a `--base` delta:
they are the standing loss the golden itself remembers, so one file answers
whether a fixture has never built here or built and stopped. Standard
library only.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import corpus_gate
import corpus_records

ROOT = Path(__file__).resolve().parent.parent
ORACLE = "expected_native_exit.txt"


class ScoreboardError(Exception):
    pass


def oracle_matches(oracle, facts):
    if oracle == "trap":
        return facts.get("exit") == "signal"
    return facts.get("exit") == oracle


def native_scoreboard(records, oracle_for=None):
    """Counts over parsed golden records. `oracle_for(fixture)` returns the
    authored oracle text or None; without it every executed fixture is
    unverified."""
    board = {"fixtures": len(records), "built": 0, "checked": 0,
             "rejected": 0, "other": 0, "executed": 0, "authored": 0,
             "unverified": 0, "oracle_mismatch": [],
             "stdout_match": 0, "stdout_differs": 0,
             "regressed": 0, "regressed_rows": [],
             "unimplemented": 0, "unimplemented_canaries": [], "malformed_migration": []}
    for record in corpus_records.lost_successes(records):
        board["regressed"] += 1
        board["regressed_rows"].append({
            "fixture": record["fixture"],
            "status": record["status"],
            "lost": record["regressed_from"]})
    for record in records:
        status = record["status"]
        if status == "unimplemented" or record["facts"].get("interpreter") == "unimplemented":
            board["unimplemented"] += 1
            if (corpus_records.migration_gap_route(record) not in ("compile", "native", "interpreter")
                    or not record["facts"].get("migration-capability")
                    or not record["facts"].get("migration-task")):
                board["malformed_migration"].append(record["fixture"])
            board["unimplemented_canaries"].append({
                "fixture": record["fixture"],
                "capability": record["facts"].get("migration-capability"),
                "task": record["facts"].get("migration-task"),
                "route": corpus_records.migration_gap_route(record)})
        if status != "unimplemented":
            board[status if status in ("built", "checked", "rejected")
                  else "other"] += 1
        facts = record["facts"]
        if status == "unimplemented":
            if "exit" in facts and record["fixture"] not in board["malformed_migration"]:
                board["malformed_migration"].append(record["fixture"])
            # A blocked native/compile route cannot also be an authored run,
            # even if a malformed externally supplied golden has exit facts.
            continue
        if "exit" in facts:
            board["executed"] += 1
            oracle = oracle_for(record["fixture"]) if oracle_for else None
            if oracle is None:
                board["unverified"] += 1
            elif oracle_matches(oracle, facts):
                board["authored"] += 1
            else:
                board["oracle_mismatch"].append(record["fixture"])
        verdict = facts.get("stdout")
        if verdict in ("match", "differs"):
            board["stdout_" + verdict] += 1
    return board


def diff_scoreboards(base_records, head_records, oracle_for=None):
    """Before/after boards; `gained`/`lost` key off authored runs (lost is
    the regression list), `executed_gained`/`executed_lost` off raw
    execution, `exit_changed` off a changed exit fact."""
    base_by = {record["fixture"]: record for record in base_records}
    head_by = {record["fixture"]: record for record in head_records}
    oracles = {fixture: oracle_for(fixture)
               for fixture in set(base_by) | set(head_by)} \
        if oracle_for else {}

    def authored_in(fixture, table):
        record = table.get(fixture)
        return (record is not None and record["status"] != "unimplemented"
                and "exit" in record["facts"]
                and oracles.get(fixture) is not None
                and oracle_matches(oracles[fixture], record["facts"]))

    fixtures = sorted(set(base_by) | set(head_by))
    gained = [fixture for fixture in fixtures
              if authored_in(fixture, head_by)
              and not authored_in(fixture, base_by)]
    lost = [fixture for fixture in fixtures
            if authored_in(fixture, base_by)
            and not authored_in(fixture, head_by)]

    def executed_in(fixture, table):
        record = table.get(fixture)
        return (record is not None and record["status"] != "unimplemented"
                and "exit" in record["facts"])

    exit_changed = [
        fixture for fixture in fixtures
        if fixture in base_by and fixture in head_by
        and "exit" in base_by[fixture]["facts"]
        and "exit" in head_by[fixture]["facts"]
        and base_by[fixture]["facts"]["exit"]
            != head_by[fixture]["facts"]["exit"]]
    base_board = native_scoreboard(
        base_records, oracles.get if oracle_for else None)
    head_board = native_scoreboard(
        head_records, oracles.get if oracle_for else None)
    return {"base": base_board, "head": head_board,
            "authored_delta": head_board["authored"] - base_board["authored"],
            "executed_delta": head_board["executed"] - base_board["executed"],
            "gained": gained, "lost": lost,
            "executed_gained": [fixture for fixture in fixtures
                                if executed_in(fixture, head_by)
                                and not executed_in(fixture, base_by)],
            "executed_lost": [fixture for fixture in fixtures
                              if executed_in(fixture, base_by)
                              and not executed_in(fixture, head_by)],
            "exit_changed": exit_changed}


CHECK_GOLDEN = "corpus_outcomes.txt"


def cross_golden_disagreements(native, checked):
    """Native rows recording a success the check golden says cannot compile.

    A native verdict outliving a check-leg regression is the one false green
    no diagnostic detector can find: an acceptance carries no diagnostic to
    read. Two file reads find it instead. Weigh the check row against every
    non-success status, not `rejected` alone -- one of the eight rows this
    was written for recorded `incomplete`.

    A row that already carries its `regressed-from:` token is not listed: it
    agrees with the check golden about what the fixture does today and only
    remembers what it used to do.
    """
    check = {record["fixture"]: record for record in checked}
    disagreements = []
    for record in native:
        if record["status"] not in corpus_records.SUCCESS_STATUSES:
            continue
        other = check.get(record["fixture"])
        if other is None or other["status"] in corpus_records.SUCCESS_STATUSES:
            continue
        disagreements.append({
            "fixture": record["fixture"],
            "native": record["status"],
            "native_facts": dict(record["facts"]),
            "check": other["status"],
            "check_diagnostic": (other["diagnostics"][0]
                                 if other["diagnostics"] else None)})
    return disagreements


def summary_line(board, target):
    debt = f", {board['unimplemented']} unimplemented" if board.get("unimplemented") else ""
    # Named in the lead line because a lost success is the one number a
    # rejected count cannot distinguish from a fixture that never ran here.
    lost = f", {board['regressed']} regressed" if board.get("regressed") else ""
    return (f"{target} native: {board['authored']} authored / "
            f"{board['executed']} executed / {board['fixtures']} fixtures, "
            f"{board['rejected']} rejected{debt}{lost} "
            f"({board['unverified']} executed without an oracle)")


def git_show(repository, revision, relative):
    result = subprocess.run(
        ["git", "-C", str(repository), "show", f"{revision}:{relative}"],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return result.stdout if result.returncode == 0 else None


def records_at(repository, golden, revision):
    """Parsed golden records at a git revision, or None when the golden does
    not exist there."""
    try:
        relative = golden.resolve().relative_to(repository).as_posix()
    except ValueError:
        raise ScoreboardError(f"{golden} is not under {repository}")
    text = git_show(repository, revision, relative)
    return corpus_records.parse(text) if text is not None else None


def oracle_lookup(repository, golden, revision, target=None):
    """`oracle_for(fixture)` reading the authored exit oracle from the
    working tree, or from `revision` when one is given. A target-specific
    `expected_native_exit.<target>.txt` overrides the portable oracle."""
    corpus = golden.resolve().parent

    def read(path):
        if revision is None:
            if not path.is_file():
                return None
            return path.read_text(encoding="utf-8", errors="replace").strip()
        try:
            relative = path.relative_to(repository).as_posix()
        except ValueError:
            return None
        shown = git_show(repository, revision, relative)
        return None if shown is None else shown.strip()

    def oracle_for(fixture):
        text = None
        if target is not None:
            text = read(corpus / fixture / f"expected_native_exit.{target}.txt")
        if text is None:
            text = read(corpus / fixture / ORACLE)
        if text is None:
            return None
        return text.splitlines()[0].strip() if text else None

    return oracle_for


def default_golden():
    target = corpus_gate.host_target()
    if target is None:
        raise ScoreboardError(
            "this host has no catalogued Omega target for a native golden")
    return ROOT / "tests" / "omega" / f"corpus_native_{target}.txt", target


def golden_target(golden):
    name = golden.name
    if name.startswith("corpus_native_") and name.endswith(".txt"):
        return name[len("corpus_native_"):-len(".txt")]
    return golden.stem


def commits_between(repository):
    """git rev-list --count <rev>..HEAD, or None for unknown revisions."""
    def count(revision):
        result = subprocess.run(
            ["git", "-C", str(repository), "rev-list", "--count",
             f"{revision}..HEAD"],
            capture_output=True, text=True, encoding="utf-8")
        text = result.stdout.strip()
        return int(text) if result.returncode == 0 and text.isdigit() else None
    return count


def collect(repository, golden=None, base_rev=None, head_rev=None,
            base_golden=None):
    """The full scoreboard record for one golden: `head` counts, plus
    `base`/`authored_delta`/`gained`/`lost`/`exit_changed` when a base is
    given — either `base_rev` (the golden at that revision) or
    `base_golden` (any recorded file, e.g. a scratch recording that was
    never committed). Raises ScoreboardError when the golden is missing."""
    if base_rev is not None and base_golden is not None:
        raise ScoreboardError("--base and --base-golden name different bases")
    if golden is None:
        golden, target = default_golden()
    else:
        golden = Path(golden)
        if not golden.is_absolute():
            golden = repository / golden
        target = golden_target(golden)
    try:
        relative = golden.resolve().relative_to(repository).as_posix()
    except ValueError:
        relative = str(golden)
    if head_rev is not None:
        head = records_at(repository, golden, head_rev)
        if head is None:
            raise ScoreboardError(
                f"{golden.name} does not exist at {head_rev}")
    else:
        if not golden.is_file():
            raise ScoreboardError(f"{golden} does not exist")
        head = corpus_records.read(golden)
    head_board = native_scoreboard(
        head, oracle_lookup(repository, golden, head_rev, target))
    board = {"target": target, "golden": relative,
             "head_rev": head_rev, "head": head_board,
             # Rows merged forward by subset re-records keep the stamp of
             # the commit that measured them; this is the staleness readout.
             "provenance": corpus_records.provenance(
                 head, commits_between(repository))}
    if base_rev is not None or base_golden is not None:
        if base_golden is not None:
            base_path = Path(base_golden)
            if not base_path.is_file():
                raise ScoreboardError(f"base golden {base_path} does not exist")
            base = corpus_records.read(base_path)
            base_label = str(base_path)
        else:
            base = records_at(repository, golden, base_rev)
            if base is None:
                raise ScoreboardError(
                    f"{golden.name} does not exist at {base_rev}")
            base_label = base_rev
        # The authored oracle lives with the fixture, not the golden; use
        # the head-revision oracle for both sides so a new oracle does not
        # manufacture a false delta.
        oracles = oracle_lookup(repository, golden, head_rev, target)
        diff = diff_scoreboards(base, head, oracles)
        board.update({"base_rev": base_rev,
                      "base_golden": str(base_golden) if base_golden else None,
                      "base_label": base_label, "base": diff["base"],
                      "authored_delta": diff["authored_delta"],
                      "executed_delta": diff["executed_delta"],
                      "gained": diff["gained"], "lost": diff["lost"],
                      "executed_gained": diff["executed_gained"],
                      "executed_lost": diff["executed_lost"],
                      "exit_changed": diff["exit_changed"]})
    return board


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--golden",
                        help="golden path (default: this host's native "
                             "golden tests/omega/corpus_native_<target>.txt)")
    parser.add_argument("--base", metavar="REV",
                        help="compare against the golden at this revision")
    parser.add_argument("--base-golden", metavar="PATH",
                        help="compare against a recorded golden file "
                             "(e.g. a scratch --record output that was "
                             "never committed); mutually exclusive with "
                             "--base")
    parser.add_argument("--head", metavar="REV",
                        help="read the golden at this revision instead of "
                             "the working tree")
    parser.add_argument("--json", action="store_true",
                        help="print the full record as JSON")
    parser.add_argument("--cross-golden", action="store_true",
                        help="also list native rows recording a success "
                             "that tests/omega/corpus_outcomes.txt says "
                             "cannot compile — a native verdict outliving a "
                             "check-leg regression; exits 1 when any remain")
    options = parser.parse_args(argv)
    try:
        board = collect(ROOT, golden=Path(options.golden)
                        if options.golden else None,
                        base_rev=options.base, head_rev=options.head,
                        base_golden=options.base_golden)
    except ScoreboardError as error:
        print(f"corpus_scoreboard: {error}", file=sys.stderr)
        return 1
    disagreements = None
    if options.cross_golden:
        native_golden = (Path(options.golden) if options.golden
                         else default_golden()[0])
        if not native_golden.is_absolute():
            native_golden = ROOT / native_golden
        check_golden = native_golden.parent / CHECK_GOLDEN
        try:
            if options.head is not None:
                checked = records_at(ROOT, check_golden, options.head)
                native = records_at(ROOT, native_golden, options.head)
            else:
                checked = corpus_records.read(check_golden)
                native = corpus_records.read(native_golden)
        except (OSError, UnicodeError, ValueError) as error:
            print(f"corpus_scoreboard: unreadable golden: {error}",
                  file=sys.stderr)
            return 1
        if checked is None or native is None:
            print(f"corpus_scoreboard: a golden is missing at {options.head}",
                  file=sys.stderr)
            return 1
        disagreements = cross_golden_disagreements(native, checked)
        board["cross_golden"] = disagreements
    if options.json:
        print(json.dumps(board, ensure_ascii=True, indent=2))
        return 1 if disagreements else 0
    print(summary_line(board["head"], board["target"]))
    print(corpus_records.provenance_line(board["provenance"]))
    if board["head"]["regressed_rows"]:
        print(f"rows recording a lost success "
              f"({len(board['head']['regressed_rows'])}):")
        for row in board["head"]["regressed_rows"]:
            lost = corpus_records.format_regression(row["lost"])[
                len(corpus_records.REGRESSION_PREFIX):]
            print(f"  {row['fixture']}: now {row['status']}, lost {lost}")
    if "base" in board:
        print(f"base {board['base_label']}: "
              f"{board['base']['authored']} authored / "
              f"{board['base']['executed']} executed / "
              f"{board['base']['fixtures']} fixtures")
        print(f"authored delta: {board['authored_delta']:+d} "
              f"(executed {board['executed_delta']:+d})")
        for key, label in (("gained", "gained authored runs"),
                           ("lost", "lost authored runs (regressions)"),
                           ("executed_gained", "newly executed"),
                           ("executed_lost", "stopped executing"),
                           ("exit_changed", "exit changed")):
            fixtures = board[key]
            if fixtures:
                print(f"{label} ({len(fixtures)}):")
                print("\n".join(f"  {fixture}" for fixture in fixtures))
    if disagreements is not None:
        if not disagreements:
            print("cross-golden: no native row records a success the check "
                  "golden refuses")
            return 0
        print(f"cross-golden: {len(disagreements)} native row(s) record a "
              f"success the check golden refuses:")
        for entry in disagreements:
            print(f"  {entry['fixture']}: native {entry['native']} "
                  f"{entry['native_facts']} / check {entry['check']}")
            if entry["check_diagnostic"]:
                print(f"    {entry['check_diagnostic'][:160]}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
