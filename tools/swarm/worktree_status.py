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


def worktree_row(repository, entry, base):
    path = Path(entry["worktree"]).resolve()
    branch_ref = entry.get("branch") or ""
    branch = (branch_ref[len("refs/heads/"):]
              if branch_ref.startswith("refs/heads/") else None)
    status_lines = coordination.git(
        path, "status", "--porcelain",
        "--untracked-files=normal").stdout.splitlines()
    behind_ahead = coordination.git(
        path, "rev-list", "--left-right", "--count",
        f"{base}...HEAD").stdout.split()
    landed = not coordination.git(
        path, "merge-base", "--is-ancestor", "HEAD", base,
        allow_failure=True).returncode
    subject = coordination.git(path, "log", "-1", "--format=%s",
                               allow_failure=True).stdout
    return {"name": path.name, "path": str(path),
            "primary": path == repository, "branch": branch,
            "detached": branch is None, "head_subject": subject,
            "dirty": len(status_lines),
            "untracked": sum(1 for line in status_lines
                             if line.startswith("??")),
            "behind": int(behind_ahead[0]), "ahead": int(behind_ahead[1]),
            "landed": landed, "locked": entry.get("locked"),
            "claims": []}


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
        owner = normalize(claim["owner"])
        brief = claim_brief(repository, claim)
        matched = [row for row in rows
                   if normalize(row["name"]) in owner
                   or task_token(row["name"]) in owner]
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


def markdown(record):
    lines = [f"# Local worktree status — {record['generated_utc']}", "",
             f"- base: `{record['base']}`"]
    landing_info = record["landing"]
    if isinstance(landing_info, dict) and "state" in landing_info:
        detail = f"queue {landing_info['queue_depth']}"
        if landing_info["active_owner"]:
            detail += f", active {landing_info['active_owner']}"
        lines.append(f"- landing: {landing_info['state']} ({detail})")
    elif isinstance(landing_info, dict):
        lines.append(f"- landing: unavailable ({landing_info['unavailable']})")
    else:
        lines.append(f"- landing: {landing_info}")
    claims_info = record["claims"]
    unmatched = []
    if isinstance(claims_info, dict) and "live" in claims_info:
        unmatched = claims_info["without_worktree"]
        lines.append(f"- live claims: {claims_info['live']} "
                     f"({len(unmatched)} without a local worktree)")
    elif isinstance(claims_info, dict):
        lines.append(f"- claims: unavailable ({claims_info['unavailable']})")
    else:
        lines.append(f"- claims: {claims_info}")
    lines += ["", "| worktree | branch | dirty | ahead | behind | landed "
              "| claim item | item on board | lease ends |",
              "|---|---|---|---|---|---|---|---|---|---|"]
    for row in record["worktrees"]:
        if row["claims"]:
            for brief in row["claims"]:
                lines.append(
                    f"| {row['name']} | {row['branch'] or '(detached)'} "
                    f"| {row['dirty']} | {row['ahead']} | {row['behind']} "
                    f"| {row['landed']} | {brief['item']} "
                    f"| {brief['item_on_board']} "
                    f"| {brief['expires_utc']} |")
        else:
            lines.append(
                f"| {row['name']} | {row['branch'] or '(detached)'} "
                f"| {row['dirty']} | {row['ahead']} | {row['behind']} "
                f"| {row['landed']} | — | — | — |")
    if unmatched:
        lines += ["", "Claims without a local worktree:"]
        for brief in unmatched:
            board = f"{brief['item_on_board']}" if brief["board"] else "n/a"
            lines.append(f"- `{brief['owner']}` — {brief['item']} "
                         f"(on board: {board}, lease ends "
                         f"{brief['expires_utc']})")
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
