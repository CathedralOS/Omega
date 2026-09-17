#!/usr/bin/env python3
"""Status for local git worktrees alongside the swarm's shared registries.

launch.py's status and report subcommands cover Devin Cloud sessions through
their receipts under build/swarm/; local waves run in plain git worktrees
(for example .codex/worktrees/<wave>-<task>) and produce no receipts, so the
cloud path cannot see them. This script gives local worktrees the same
visibility: branch and uncommitted churn, ahead/behind against the
integration base, whether the worktree head already landed on it, the live
claims-registry assignment matched by owner name, and the landing queue's
state.

The tool is read-only: it never writes to the repository, the boards, or the
coordination refs. Run it from the repository root (or pass --repository).
--offline skips the two network reads (claims and landing) and reports local
git state only. Output is one JSON object; --markdown renders the same
information as a report table.
"""

import argparse
import json
from pathlib import Path
import re
import sys

# Sibling-tool imports; claims.py, landing.py, coordination.py live in tools/.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import claims
import coordination
import landing

StatusError = coordination.CoordinationError
BASE_REF = "origin/main"
LEASE_SOON_MINUTES = 60


def emit(record):
    print(json.dumps(record, ensure_ascii=True, indent=2))


def normalize(value):
    return re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")


def task_token(name):
    """Drop a worktree name's wave prefix: macw3-frame-layout → frame-layout.

    Matching on the de-prefixed name covers owners written as
    'Jarod / macos-w3 frame-layout' as well as 'Jarod / macw3-normalized-abi'.
    A single-segment remainder would over-match, so it falls back to the full
    name.
    """
    normalized = normalize(name)
    remainder = normalized.split("-", 1)[-1]
    return remainder if "-" in remainder else normalized


def owner_task_tail(owner):
    """The task portion of a 'Person / task' owner label.

    A label without '/' is treated as entirely task.
    """
    _, separator, tail = owner.partition("/")
    return normalize(tail if separator else owner)


def task_match(tail, name):
    """True when a worktree name identifies a claim owner's task tail.

    The normalized name or its task token must equal the tail or end it at
    a '-' boundary: 'w1-codec-lineage' matches 'Jarod / swarm-w1-codec-lineage'
    but 'macw3-normalized' is only a prefix of 'macw3-normalized-abi' and bare
    'normalized' is a fragment inside it; neither identifies the task.
    """
    return any(candidate == tail or tail.endswith("-" + candidate)
               for candidate in {normalize(name), task_token(name)}
               if candidate)


def list_worktrees(repository):
    output = coordination.git(repository, "worktree", "list",
                              "--porcelain").stdout
    entries, current = [], {}
    for line in output.splitlines():
        if line.strip():
            key, _, value = line.partition(" ")
            current[key] = value if value else True
        elif current:
            entries.append(current)
            current = {}
    if current:
        entries.append(current)
    return entries


def degraded_reason(path, error):
    """Describe why a present worktree's git probes failed.

    An unborn HEAD (orphan checkout, freshly added worktree branch) fails
    rev-list, merge-base, and log while status still works; naming that
    state keeps the row readable instead of quoting raw stderr.
    """
    message = " ".join(str(error).split())
    if "not a git repository" in message.lower():
        return "not a git repository — gitdir removed or recreated"
    head = coordination.git(path, "rev-parse", "--verify", "--quiet", "HEAD",
                            allow_failure=True).stdout
    if not head:
        return "unborn branch (no commits)"
    return f"git probes failed: {message[:160]}"


def worktree_row(repository, entry, base):
    path = Path(entry["worktree"]).resolve()
    branch_ref = entry.get("branch") or ""
    branch = (branch_ref[len("refs/heads/"):]
              if branch_ref.startswith("refs/heads/") else None)
    row = {"name": path.name, "path": str(path),
           "primary": path == repository, "branch": branch,
           "detached": branch is None, "head_subject": None,
           "dirty": None, "untracked": None, "behind": None,
           "ahead": None, "landed": None,
           "locked": entry.get("locked"), "claims": []}
    if not path.is_dir():
        # A deleted-but-unpruned worktree fails every git -C call; degrade to
        # a marked row so one stale registration cannot abort the report.
        # The porcelain entry can still name the ref it pointed at, but the
        # directory is gone, so branch/detached stay unknown rather than
        # reporting a live-looking branch for a vanished checkout.
        prunable = entry.get("prunable")
        detail = f": {prunable}" if isinstance(prunable, str) else ""
        row["missing"] = True
        row["degraded"] = f"missing{detail}"
        row["branch"] = None
        row["detached"] = None
        return row
    try:
        status_lines = coordination.git(
            path, "status", "--porcelain",
            "--untracked-files=normal").stdout.splitlines()
        row["dirty"] = sum(1 for line in status_lines
                           if not line.startswith("??"))
        row["untracked"] = sum(1 for line in status_lines
                               if line.startswith("??"))
        behind_ahead = coordination.git(
            path, "rev-list", "--left-right", "--count",
            f"{base}...HEAD").stdout.split()
        row["behind"] = int(behind_ahead[0])
        row["ahead"] = int(behind_ahead[1])
        ancestor = coordination.git(
            path, "merge-base", "--is-ancestor", "HEAD", base,
            allow_failure=True)
        if ancestor.returncode in (0, 1):
            row["landed"] = ancestor.returncode == 0
        else:
            raise StatusError(
                f"Git failed ({ancestor.returncode}): "
                f"{ancestor.stderr} {ancestor.stdout}")
        row["head_subject"] = coordination.git(
            path, "log", "-1", "--format=%s", allow_failure=True).stdout
    except StatusError as error:
        row["degraded"] = degraded_reason(path, error)
    return row


def claim_brief(repository, claim):
    brief = {"owner": claim["owner"], "item": claim["item"],
             "board": claim["board"], "ticket": claim["ticket"],
             "expires_utc": claim["expires_utc"], "item_on_board": None}
    if claim["board"]:
        try:
            brief["item_on_board"] = claim["item"] in claims.board_items(
                repository, claim["board"])
        except StatusError:
            pass
    return brief


def attach_claims(repository, rows, live):
    unmatched = []
    for claim in live:
        tail = owner_task_tail(claim["owner"])
        brief = claim_brief(repository, claim)
        matched = [row for row in rows if task_match(tail, row["name"])]
        if matched:
            for row in matched:
                row["claims"].append(brief)
        else:
            unmatched.append(brief)
    return unmatched


def landing_state(repository, remote):
    snapshot = landing.Landing(repository, remote).snapshot()
    status = landing.Landing.status(snapshot)
    active = status.get("active")
    return {"state": status["state"],
            "queue_depth": len(status.get("queue") or []),
            "active_owner": active.get("owner") if active else None,
            "head_expires_utc": status.get("head_expires_utc")}


def collect(options, repository):
    if not coordination.git(repository, "rev-parse", "--verify", "--quiet",
                            f"{options.base}^{{commit}}",
                            allow_failure=True).stdout:
        raise StatusError(f"Base ref {options.base} not found; "
                          "fetch first or pass --base.")
    rows = [worktree_row(repository, entry, options.base)
            for entry in list_worktrees(repository)]
    record = {"generated_utc": coordination.now(), "base": options.base,
              "worktrees": rows}
    if options.offline:
        record["claims"] = "skipped (--offline)"
        record["landing"] = "skipped (--offline)"
        return record
    try:
        snapshot = claims.Claims(repository, options.remote).snapshot()
        live = claims.live_claims(snapshot["record"])
        unmatched = attach_claims(repository, rows, live)
        record["claims"] = {"live": len(live),
                            "without_worktree": unmatched}
    except StatusError as error:
        record["claims"] = {"unavailable": str(error)}
    try:
        record["landing"] = landing_state(repository, options.remote)
    except StatusError as error:
        record["landing"] = {"unavailable": str(error)}
    return record


def lease_minutes(generated, timestamp):
    if generated is None or not timestamp:
        return None
    try:
        return (coordination.utc(timestamp) - generated).total_seconds() / 60
    except (TypeError, ValueError):
        return None


def lease_phrase(minutes):
    if minutes is None:
        return ""
    if minutes < 0:
        return "expired"
    if minutes < 90:
        return f"in {minutes:.0f} min"
    return f"in {minutes / 60:.1f} h"


def worktree_summary(record):
    """Census line: how many worktrees exist and in what condition."""
    rows = record["worktrees"]
    primaries = sum(1 for row in rows if row["primary"])
    degraded = sum(1 for row in rows
                   if row.get("missing") or row.get("degraded"))
    dirty = [row for row in rows if row["dirty"]]
    files = sum(row["dirty"] for row in dirty)
    untracked = sum(row["untracked"] for row in dirty)
    parts = [f"{len(rows) - len(dirty) - degraded} clean"]
    if dirty:
        parts.append(f"{len(dirty)} dirty "
                     f"({files} files, {untracked} untracked)")
    for key in ("detached", "locked"):
        count = sum(1 for row in rows if row[key])
        if count:
            parts.append(f"{count} {key}")
    return (f"- {len(rows)} worktrees ({primaries} main, "
            f"{len(rows) - primaries} linked): {degraded} missing/degraded, "
            f"{', '.join(parts)}")


def progress_summary(record, claims_known):
    """Progress line: how much of the wave is landed, in flight, or stale."""
    rows = record["worktrees"]
    landed = sum(1 for row in rows if row["landed"])
    ahead = [row for row in rows if row["ahead"]]
    commits = sum(row["ahead"] for row in ahead)
    behind = sum(1 for row in rows if row["behind"])
    text = (f"- vs `{record['base']}`: {landed} landed, {len(ahead)} ahead "
            f"({commits} unpublished commits), {behind} behind")
    if claims_known:
        unclaimed = sum(1 for row in rows if not row["claims"])
        text += f", {unclaimed} without a live claim"
    return text


def attention_items(record, generated, claims_known):
    """One bullet per worktree state that calls for a coordinator action.

    Mirrors the recovery runbook in tools/swarm/README.md: a landed head
    is a cleanup candidate, uncommitted files are unpreserved work, a
    detached or locked worktree blocks ordinary handling, unpublished
    commits with no live claim mark a finished or dead slot, a claim
    whose item left its board is stale, and a lease inside
    LEASE_SOON_MINUTES needs renewal or release. A missing or degraded
    worktree needs pruning or repair before anything else about it can
    be trusted. head_subject was already collected for JSON; showing it
    here identifies what a slot was doing.
    """
    items = []
    for row in record["worktrees"]:
        reasons = []
        if row.get("missing"):
            reasons.append(f"missing worktree directory — prune or recreate "
                           f"({row.get('degraded') or 'no detail'})")
        elif row.get("degraded"):
            reasons.append(f"degraded — {row['degraded']}")
        if row["detached"]:
            reasons.append("detached HEAD — work is not on a branch")
        if row["locked"]:
            note = (row["locked"] if isinstance(row["locked"], str)
                    else "no reason recorded")
            reasons.append(f"locked ({note})")
        if row["landed"]:
            if row["dirty"]:
                reasons.append(f"head already on {record['base']} but "
                               f"{row['dirty']} uncommitted file(s) "
                               f"({row['untracked']} untracked) unpreserved")
            else:
                reasons.append(f"head already on {record['base']} — "
                               "cleanup candidate")
        elif row["dirty"]:
            reasons.append(f"{row['dirty']} uncommitted file(s) "
                           f"({row['untracked']} untracked) — unpreserved")
        if claims_known:
            if row["landed"] and row["claims"]:
                reasons.append("work landed but claim still live — release it")
            if row["ahead"] and not row["claims"]:
                reasons.append(f"{row['ahead']} unpublished commit(s) and no "
                               "live claim — finished or dead slot")
            for brief in row["claims"]:
                if brief["item_on_board"] is False:
                    reasons.append(f"claim item {brief['item']} is not on "
                                   f"{brief['board']} — closed or misnamed")
                left = lease_minutes(generated, brief["expires_utc"])
                if left is not None and left <= LEASE_SOON_MINUTES:
                    reasons.append(f"claim lease {lease_phrase(left)} — "
                                   "renew it or treat the slot as dead")
        if reasons:
            tag = " (main checkout)" if row["primary"] else ""
            detail = "; ".join(reasons)
            if row["head_subject"]:
                detail += f'; last commit "{row["head_subject"]}"'
            items.append(f"- `{row['name']}`{tag}: {detail}")
    return items


def markdown(record):
    """Render the status record for a coordinator watching a wave.

    The header bullets carry the census and progress-vs-base rollup next
    to the existing base/landing/claims lines, so the wave's shape reads
    top-down instead of being counted off table rows. "Needs attention"
    is the triage list between the rollup and the full table; the table
    columns and the trailing orphan-claim list keep their detail. Every
    line derives from the JSON record so the two outputs cannot disagree.
    """
    lines = [f"# Local worktree status — {record['generated_utc']}", "",
             f"- base: `{record['base']}`"]
    try:
        generated = coordination.utc(record["generated_utc"])
    except (TypeError, ValueError):
        generated = None
    landing_info = record["landing"]
    if isinstance(landing_info, dict) and "state" in landing_info:
        detail = f"queue {landing_info['queue_depth']}"
        if landing_info["active_owner"]:
            detail += f", active {landing_info['active_owner']}"
        phrase = lease_phrase(lease_minutes(
            generated, landing_info.get("head_expires_utc")))
        if phrase:
            detail += f", head lease {phrase}"
        lines.append(f"- landing: {landing_info['state']} ({detail})")
    elif isinstance(landing_info, dict):
        lines.append(f"- landing: unavailable ({landing_info['unavailable']})")
    else:
        lines.append(f"- landing: {landing_info}")
    claims_info = record["claims"]
    unmatched = []
    claims_known = isinstance(claims_info, dict) and "live" in claims_info
    if claims_known:
        unmatched = claims_info["without_worktree"]
        lines.append(f"- live claims: {claims_info['live']} "
                     f"({len(unmatched)} without a local worktree)")
    elif isinstance(claims_info, dict):
        lines.append(f"- claims: unavailable ({claims_info['unavailable']})")
    else:
        lines.append(f"- claims: {claims_info}")
    lines += [worktree_summary(record),
              progress_summary(record, claims_known)]
    items = attention_items(record, generated, claims_known)
    if isinstance(landing_info, dict) and landing_info.get("state") == "expired":
        items.insert(0, "- landing queue: head lease expired — the next "
                        "queue action advances it")
    if items:
        lines += ["", "## Needs attention", ""] + items
    lines += ["", "## Worktrees", "",
              "| worktree | branch | dirty | ahead | behind | landed "
              "| claim item | item on board | lease ends |",
              "|---|---|---|---|---|---|---|---|---|---|"]
    for row in record["worktrees"]:
        if row.get("missing"):
            prefix = f"| {row['name']} | (missing) | — | — | — | — "
        elif row.get("degraded"):
            prefix = (f"| {row['name']} | {row['branch'] or '(detached)'} "
                      f"(degraded: {row['degraded']}) | — | — | — | — ")
        else:
            prefix = (f"| {row['name']} | {row['branch'] or '(detached)'} "
                      f"| {row['dirty']} | {row['ahead']} | {row['behind']} "
                      f"| {row['landed']} ")
        if row["claims"]:
            for brief in row["claims"]:
                lines.append(f"{prefix}| {brief['item']} "
                             f"| {brief['item_on_board']} "
                             f"| {brief['expires_utc']} |")
        else:
            lines.append(f"{prefix}| — | — | — |")
    if unmatched:
        lines += ["", "## Claims without a local worktree", ""]
        for brief in unmatched:
            board = f"{brief['item_on_board']}" if brief["board"] else "n/a"
            phrase = lease_phrase(lease_minutes(
                generated, brief["expires_utc"]))
            when = f" ({phrase})" if phrase else ""
            lines.append(f"- `{brief['owner']}` — {brief['item']} "
                         f"(on board: {board}, lease ends "
                         f"{brief['expires_utc']}{when})")
    return "\n".join(lines)


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise StatusError(message)


def main(argv=None):
    parser = Parser(description=__doc__)
    parser.add_argument("--repository", default=".",
                        help="repository root (default: current directory)")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--base", default=BASE_REF,
                        help="integration base ref for ahead/behind and "
                             "landed checks (default: %(default)s)")
    parser.add_argument("--offline", action="store_true",
                        help="skip the claims and landing remote reads")
    parser.add_argument("--markdown", action="store_true",
                        help="render a markdown report instead of JSON")
    try:
        options = parser.parse_args(argv)
        repository = Path(options.repository).resolve()
        coordination.git(repository, "rev-parse", "--show-toplevel")
        record = collect(options, repository)
        if options.markdown:
            print(markdown(record))
        else:
            emit(record)
        return 0
    except (StatusError, OSError) as error:
        print(f"worktree-status: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
