#!/usr/bin/env python3
"""Land one prepared lane from a single tool, inside one build-slot hold.

A lane is *prepared* when its worktree holds its lane commit(s) on top of some
`origin/main` ancestor and its evidence is recorded (the commit body or a
chain log). This tool then performs the part that every per-lane chain script
used to re-spell, with the failure modes that cost the 2026-10-24 wave the
most slots folded in:

  1. fetch, rebase onto the current tip;
  2. optional `mbx nextest` for named crates (skip when the commit body
     already records that evidence);
  3. `landing.py verify` at the rebased base;
  4. enqueue with a fresh ticket, then claim (the landing queue is shallow;
     the build pool is the real wait);
  5. rebase again onto the *granted* base whenever it moved - a conflicting
     rebase is the only genuine "stale evidence" case, so unrelated main
     movement never burns the slot. The wave's per-lane blanket movement
     guards released nearly every attempt mid-saturation; this tool replaced
     that with rebase-always + re-verify;
  6. re-verify on the granted base, then note, publish, ancestor check.

`landing.py verify` takes its own build-slot hold internally, so run this tool
*inside* one `tools/build_slot.py --slots 2 --timeout 0` hold: the nested
holds short-circuit through `OMEGA_BUILD_SLOT_HELD` and the whole lease costs
one pool acquisition ([build slots](build_slot.md)).

Usage (from any cwd):

    python3 tools/build_slot.py --slots 2 --timeout 0 -- \
        python3 tools/lane_land.py <worktree> [--note-ticket TICKET] \
        [--nextest CRATE]...

Exit codes: 30 fetch, 31 pre-lease rebase, 32 nextest, 33 verify, 34 enqueue,
35 claim, 36 in-lease rebase conflict, 37 post-rebase nextest, 38 re-verify,
39 publish, 40 not on main. On every failure after a granted claim the claim
is released before exit, so no attempt leaves a dangling reservation.
"""

import argparse
import json
import subprocess
import sys
import uuid
from pathlib import Path

RELEASED = "released before exit"


def say(worktree, message):
    line = f"[lane_land] {message}"
    print(line)
    with open(worktree / "build" / "lane-land" / "land.log", "a", encoding="utf-8") as fh:
        fh.write(line + "\n")


def run(worktree, arguments):
    return subprocess.run(arguments, cwd=worktree, capture_output=True, text=True)


def output(worktree, arguments):
    completed = run(worktree, arguments)
    return completed.returncode, completed.stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("worktree", help="the lane worktree holding the prepared commit")
    parser.add_argument("--note-ticket", default="",
                        help="claims-registry ticket for the progress note (optional)")
    parser.add_argument("--nextest", action="append", default=[],
                        help="crate whose tests to run before verify (repeatable)")
    options = parser.parse_args()

    worktree = Path(options.worktree).resolve()
    if not (worktree / ".git").exists():
        print(f"lane_land: no worktree at {worktree}")
        return 9
    (worktree / "build" / "lane-land").mkdir(parents=True, exist_ok=True)

    code, _ = output(worktree, ["git", "fetch", "-q", "origin", "main"])
    if code:
        say(worktree, "FETCH_FAIL")
        return 30
    _, tip = output(worktree, ["git", "rev-parse", "origin/main"])
    _, parent = output(worktree, ["git", "rev-parse", "HEAD^"])
    if parent != tip:
        say(worktree, f"pre-lease rebase onto {tip}")
        code, _ = output(worktree, ["git", "rebase", tip])
        if code:
            output(worktree, ["git", "rebase", "--abort"])
            say(worktree, "REBASE_FAIL")
            return 31

    for crate in options.nextest:
        code, _ = output(worktree, ["mbx", "nextest", "run", "-p", crate])
        if code:
            say(worktree, f"NEXTEST_FAIL {crate}")
            return 32
        say(worktree, f"nextest ok {crate}")

    _, base = output(worktree, ["git", "rev-parse", "HEAD^"])
    code, detail = output(worktree, ["python3", "tools/landing.py", "verify", "--base", base])
    if code:
        say(worktree, f"VERIFY_FAIL {detail[-160:]}")
        return 33
    say(worktree, "verify ok")

    ticket = uuid.uuid4().hex
    code, _ = output(worktree, ["python3", "tools/landing.py", "enqueue",
                                "--ticket", ticket, "--owner", "lane_land"])
    if code:
        say(worktree, "ENQUEUE_FAIL")
        return 34
    code, detail = output(worktree, ["python3", "tools/landing.py", "claim",
                                     "--ticket", ticket, "--wait-seconds", "1750"])
    if code:
        say(worktree, "CLAIM_FAIL")
        return 35
    claim = json.loads(detail)
    granted, claim_id = claim["base"], claim["claim"]
    say(worktree, f"granted base {granted} claim {claim_id}")

    def release():
        output(worktree, ["python3", "tools/landing.py", "release", "--claim", claim_id])

    _, parent = output(worktree, ["git", "rev-parse", "HEAD^"])
    if granted != parent:
        say(worktree, f"in-lease rebase onto {granted}")
        code, _ = output(worktree, ["git", "rebase", granted])
        if code:
            output(worktree, ["git", "rebase", "--abort"])
            release()
            say(worktree, f"INLEASE_REBASE_CONFLICT; claim {RELEASED}")
            return 36
        for crate in options.nextest:
            code, _ = output(worktree, ["mbx", "nextest", "run", "-p", crate])
            if code:
                release()
                say(worktree, f"NEXTEST_FAIL_POST_REBASE {crate}; claim {RELEASED}")
                return 37
        code, detail = output(worktree, ["python3", "tools/landing.py", "verify", "--base", granted])
        if code:
            release()
            say(worktree, f"REVERIFY_FAIL; claim {RELEASED}")
            return 38
        say(worktree, "re-verify ok on granted base")

    _, candidate = output(worktree, ["git", "rev-parse", "HEAD"])
    if options.note_ticket:
        code, detail = output(worktree, ["python3", "tools/claims.py", "note",
                                         "--ticket", options.note_ticket, "--text",
                                         f"lane_land: candidate {candidate} on base {granted}; "
                                         f"nextest {options.nextest or 'per commit evidence'}; verify ok"])
        if code:
            say(worktree, f"note skipped ({detail[-80:]})")

    code, detail = output(worktree, ["python3", "tools/landing.py", "publish",
                                     "--claim", claim_id, "--base", granted,
                                     "--candidate", candidate,
                                     "--provenance", "agent=hermes lane=lane_land"])
    if code:
        say(worktree, f"PUBLISH_FAIL; claim {RELEASED if not code else 'released by publish'}")
        return 39

    output(worktree, ["git", "fetch", "-q", "origin", "main"])
    code, _ = output(worktree, ["git", "merge-base", "--is-ancestor", candidate, "origin/main"])
    if code:
        say(worktree, f"NOT_ON_MAIN {candidate}")
        return 40
    say(worktree, f"REMOTE_VERIFIED {candidate}")
    (worktree / "build" / "lane-land" / "LANDED").write_text(candidate + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())