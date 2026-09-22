#!/usr/bin/env python3
"""Constant-fill actuator for a local swarm wave.

worktree_status.py reports what every worktree looks like; this tool decides
what each manifest slot needs next and performs the mechanical recovery the
coordinator would otherwise do by hand. It reads three inputs and joins them
per slot:

- the wave manifest (sessions, items, owning paths),
- worktree state (dirty/ahead/landed) and the claims registry
  (via worktree_status.collect), and
- the handle ledger at build/swarm/<wave>/slots.json — a coordinator-
  maintained map of session name to the in-session subagent handle and its
  last-known liveness. The coordinator writes to it with `note` after every
  spawn and every completion/death notification; the ledger is what lets a
  plan say "resume <agent_id>" instead of "some handle was here".

Commands:

  plan     (default) emit one JSON object: per-slot state, the action the
           coordinator should take, and the ordered action list with pacing
           guidance. Read-only — it never touches git, claims, or the ledger.
  note     --slot S --agent-id ID --state STATE [--note TEXT] records a
           spawn/notification outcome in the ledger. States: running, dead,
           done, parked. `--state dead` is what authorizes recover to act on
           the slot's claim and worktree.
  recover  executes recovery for every slot the ledger marks dead: WIP-commit
           its dirty worktree onto the branch, release its live claim
           tickets, then mark the slot `recovered` so plan emits `resume`.
           Requires --execute; without it, prints the would-be actions.
  sweep    removes clean, landed worktrees that are not in the manifest and
           not marked keep in the ledger — the stale-worktree storage cost.
           Requires --execute; git refuses dirty worktrees, which is the
           safety check.

A slot's recommended action is one of:

  none            live claim plus ledger state running — leave it alone
  check-handle    ledger says running but no live claim and a quiet worktree
                  — verify the handle before trusting it
  resume          ledger has a resumable agent_id for a dead or done slot —
                  re-issue the handle with a continue prompt
  spawn           no resumable handle — spawn a fresh subagent from the
                  rendered prompt at build/swarm/<wave>/prompts/<name>.md
  pick-item       slot is finished and its item is exhausted — coordinator
                  must choose a new board item before spawning
  land            branch carries unpublished commits with no live claim —
                  resume the handle to land them (or land by hand)
  wait            a named external blocker (expired ledger, queue freeze) —
                  report it, do not spawn

The pacing field exists because burst-refilling re-trips the shared model
rate limiter: execute the ordered actions serially — one resume or spawn per
block, ~30-60 s apart while a limiter is hot — rather than in one parallel
burst. See the local-swarm skill's Monitor section.
"""

import argparse
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import coordination
import worktree_status

StatusError = coordination.CoordinationError

LEDGER_STATES = ("running", "dead", "done", "parked", "recovered")


def emit(record):
    print(json.dumps(record, ensure_ascii=True, indent=2))


def load_manifest(path, repository):
    manifest_path = Path(path)
    if not manifest_path.is_absolute():
        manifest_path = repository / manifest_path
    try:
        manifest = json.loads(manifest_path.read_text())
    except (OSError, ValueError) as error:
        raise StatusError(f"Cannot read manifest {manifest_path}: {error}")
    for key in ("wave", "owner_label_prefix", "sessions"):
        if key not in manifest:
            raise StatusError(f"Manifest {manifest_path} lacks '{key}'.")
    return manifest


def ledger_path(repository, wave):
    return (repository / "build" / "swarm" / wave / "slots.json")


def load_ledger(repository, wave):
    path = ledger_path(repository, wave)
    if not path.is_file():
        return {"wave": wave, "slots": {}}
    try:
        ledger = json.loads(path.read_text())
    except (OSError, ValueError) as error:
        raise StatusError(f"Cannot read ledger {path}: {error}")
    ledger.setdefault("slots", {})
    return ledger


def save_ledger(repository, wave, ledger):
    path = ledger_path(repository, wave)
    path.parent.mkdir(parents=True, exist_ok=True)
    ledger["updated_utc"] = coordination.now()
    path.write_text(json.dumps(ledger, indent=2, sort_keys=True) + "\n")
    return path


def slot_owner_tail(manifest, session_name):
    """The claim-owner tail a slot's agent uses: '<prefix>-<name>'."""
    return f"{manifest['owner_label_prefix']}-{session_name}"


def slot_claims(row, session_name, manifest):
    """Live claims attached to this slot's worktree or owner label.

    worktree_status.attach_claims already matched claims to worktree rows by
    owner tail; extension claims (freeform item names under the same owner)
    land in the same row.
    """
    return list(row.get("claims") or [])


def classify(session_name, row, ledger_entry, manifest):
    """One slot's state and recommended coordinator action.

    The ledger's liveness word is authoritative over raw worktree shape: a
    live claim on a quiet worktree can be a long-running build, but the same
    shape with a ledger-dead handle is a corpse to recover.
    """
    state_record = {"slot": session_name, "claims": [
        {"ticket": c["ticket"], "item": c["item"],
         "expires_utc": c["expires_utc"]}
        for c in slot_claims(row, session_name, manifest)] if row else []}
    agent_id = (ledger_entry or {}).get("agent_id")
    liveness = (ledger_entry or {}).get("state")
    state_record["agent_id"] = agent_id
    state_record["liveness"] = liveness
    if row is None:
        state_record.update(
            {"worktree": None, "state": "missing",
             "action": "spawn",
             "detail": "no worktree — render+spawn fresh (launch.py local "
                      "--sessions <name> --create-worktrees) or confirm the "
                      "session left rotation"})
        return state_record
    dirty = (row.get("dirty") or 0) + (row.get("untracked") or 0)
    ahead = row.get("ahead") or 0
    landed = row.get("landed")
    live_claims = state_record["claims"]
    state_record.update(
        {"worktree": row["path"], "branch": row.get("branch"),
         "dirty": dirty, "ahead": ahead, "landed": landed,
         "head_subject": row.get("head_subject")})
    if liveness == "dead":
        if dirty:
            action = "recover"
            detail = ("dead handle with unpreserved work — run `fill.py "
                      "recover` to WIP-commit, then resume")
        elif live_claims:
            action = "recover"
            detail = ("dead handle holding live claim(s) — run `fill.py "
                      "recover` to release, then resume")
        else:
            action = "resume" if agent_id else "spawn"
            detail = ("dead handle, worktree preserved — resume the ledger "
                      "agent_id with a continue prompt" if agent_id
                      else "dead slot, no resumable handle — spawn fresh "
                      "from the rendered prompt")
        state_record.update({"state": "dead", "action": action,
                             "detail": detail})
        return state_record
    if liveness == "parked":
        state_record.update({"state": "parked", "action": "none",
                             "detail": "deliberately out of rotation"})
        return state_record
    if live_claims:
        if liveness == "running":
            state_record.update(
                {"state": "running", "action": "none",
                 "detail": "live claim and live handle"})
        else:
            state_record.update(
                {"state": "claimed-untracked", "action": "check-handle",
                 "detail": "live claim but ledger has no running handle — "
                          "verify the handle before reporting it as running"})
        return state_record
    if ahead:
        action = "resume" if agent_id else "land"
        state_record.update(
            {"state": "unlanded", "action": action,
             "detail": f"{ahead} unpublished commit(s), no live claim — "
                      + ("resume the handle to land them" if agent_id
                         else "no resumable handle — inspect and land by "
                         "hand or spawn a landing slot")})
        return state_record
    if dirty:
        state_record.update(
            {"state": "unpreserved", "action": "check-handle",
             "detail": "dirty worktree with no live claim — either a running "
                      "agent that has not claimed yet or unpreserved dead "
                      "work; check the handle, then recover if dead"})
        return state_record
    if landed:
        state_record.update(
            {"state": "done", "action": "spawn",
             "detail": "landed and clean — refill with a continuation resume "
                      "or a fresh item"})
        if agent_id and liveness in ("done", None):
            state_record["action"] = "resume"
            state_record["detail"] = ("landed and clean — resume the ledger "
                                      "agent_id for the next leg, or swap "
                                      "the manifest row for a fresh item")
        return state_record
    state_record.update(
        {"state": "idle", "action": "spawn",
         "detail": "clean worktree, no claim, no commits — never started or "
                  "fully drained"})
    return state_record


def ordered_actions(plan):
    """Flatten slot actions into the paced refill order.

    Recovery first (it is local and free), then resumes (transcript replay is
    the expensive message), then fresh spawns. The coordinator executes these
    serially, not in one parallel block — see the module docstring.
    """
    order = {"recover": 0, "land": 1, "resume": 2, "pick-item": 3,
             "spawn": 4, "check-handle": 5}
    actions = []
    for slot in plan["slots"]:
        if slot["action"] == "none":
            continue
        entry = {"slot": slot["slot"], "action": slot["action"],
                 "detail": slot["detail"]}
        if slot.get("agent_id"):
            entry["agent_id"] = slot["agent_id"]
        if slot["action"] == "spawn":
            entry["prompt"] = str(
                Path("build/swarm") / plan["wave"] / "prompts"
                / f"{slot['slot']}.md")
        actions.append(entry)
    actions.sort(key=lambda e: (order.get(e["action"], 9), e["slot"]))
    return actions


def command_plan(repository, manifest, ledger, markdown=False):
    record = worktree_status.collect(_status_options(), repository)
    by_name = {row["name"]: row for row in record["worktrees"]}
    wave = manifest["wave"]
    slots = []
    seen = set()
    for session in manifest["sessions"]:
        name = session["name"]
        seen.add(name)
        row = by_name.get(f"{wave}-{name}")
        slots.append(classify(name, row, ledger["slots"].get(name), manifest))
        slots[-1]["item"] = session["item"]
    for name, entry in sorted(ledger["slots"].items()):
        if name in seen:
            continue
        row = by_name.get(f"{wave}-{name}")
        slot = classify(name, row, entry, manifest)
        slot["item"] = "(off-manifest)"
        slot["off_manifest"] = True
        slots.append(slot)
    plan = {"wave": wave, "generated_utc": record["generated_utc"],
            "ledger": str(ledger_path(repository, wave)), "slots": slots}
    plan["actions"] = ordered_actions(plan)
    running = sum(1 for s in slots if s["state"] == "running")
    plan["tank"] = {"target": len(manifest["sessions"]),
                    "running": running,
                    "filling": len(plan["actions"])}
    plan["pacing"] = ("execute actions in order, one per block — a resume "
                      "replays a transcript and a fresh spawn renders a "
                      "prompt, so a parallel burst re-trips the shared "
                      "model rate limiter; ~30-60 s apart while hot")
    return plan


def _status_options():
    options = argparse.Namespace(base="origin/main", remote="origin",
                                 offline=False)
    return options


def command_note(repository, manifest, ledger, arguments):
    slot = arguments.slot
    entry = ledger["slots"].setdefault(slot, {})
    if arguments.agent_id:
        entry["agent_id"] = arguments.agent_id
    if arguments.state:
        if arguments.state not in LEDGER_STATES:
            raise StatusError(f"--state must be one of {LEDGER_STATES}")
        entry["state"] = arguments.state
    if arguments.note:
        entry["note"] = arguments.note
    entry["updated_utc"] = coordination.now()
    path = save_ledger(repository, manifest["wave"], ledger)
    emit({"ledger": str(path), "slot": slot, "entry": entry})


def wip_commit(worktree, wave, slot, reason):
    git = lambda *args, **kwargs: coordination.git(worktree, *args, **kwargs)
    git("add", "-A")
    message = f"wip: {wave} {slot} — {reason}, coordinator preserve"
    committed = git("commit", "-m", message, allow_failure=True)
    if committed.returncode != 0:
        raise StatusError(f"WIP commit failed in {worktree}: "
                          f"{committed.stderr or committed.stdout}")
    head = git("log", "-1", "--format=%h %s").stdout
    return head


def release_ticket(repository, ticket):
    return subprocess.run(
        [sys.executable,
         str(Path(__file__).resolve().parent.parent / "claims.py"),
         "--repository", str(repository), "release",
         "--ticket", ticket],
        capture_output=True, text=True)


def command_recover(repository, manifest, ledger, arguments):
    record = worktree_status.collect(_status_options(), repository)
    by_name = {row["name"]: row for row in record["worktrees"]}
    wave = manifest["wave"]
    results = []
    changed = False
    for name, entry in sorted(ledger["slots"].items()):
        if entry.get("state") != "dead":
            continue
        row = by_name.get(f"{wave}-{name}")
        actions = {"slot": name, "wip_commit": None,
                   "released_tickets": [], "errors": []}
        if row and not row.get("missing"):
            dirty = (row.get("dirty") or 0) + (row.get("untracked") or 0)
            if dirty:
                if arguments.execute:
                    try:
                        actions["wip_commit"] = wip_commit(
                            row["path"], wave, name,
                            entry.get("death") or "dead handle")
                    except StatusError as error:
                        actions["errors"].append(str(error))
                else:
                    actions["wip_commit"] = (f"would commit {dirty} "
                                             "uncommitted file(s)")
        for claim in slot_claims(row or {}, name, manifest):
            if arguments.execute:
                completed = release_ticket(repository, claim["ticket"])
                if completed.returncode == 0:
                    actions["released_tickets"].append(claim["ticket"])
                else:
                    actions["errors"].append(
                        f"release {claim['ticket']}: "
                        f"{completed.stderr or completed.stdout}")
            else:
                actions["released_tickets"].append(
                    f"would release {claim['ticket']} ({claim['item']})")
        if arguments.execute and not actions["errors"]:
            entry["state"] = "recovered"
            entry["updated_utc"] = coordination.now()
            changed = True
        results.append(actions)
    if arguments.execute and changed:
        save_ledger(repository, manifest["wave"], ledger)
    emit({"wave": wave, "executed": bool(arguments.execute),
          "recovered": results})


def command_sweep(repository, manifest, ledger, arguments):
    record = worktree_status.collect(_status_options(), repository)
    wave = manifest["wave"]
    active = {f"{wave}-{s['name']}" for s in manifest["sessions"]}
    kept = {f"{wave}-{name}" for name, entry in ledger["slots"].items()
            if entry.get("state") in ("running", "parked")}
    results = []
    for row in record["worktrees"]:
        if row["primary"] or row.get("missing"):
            continue
        name = row["name"]
        in_wave = name.startswith(f"{wave}-")
        removable = (row.get("landed") and not row.get("dirty")
                     and not row.get("untracked") and not row.get("ahead"))
        if not removable:
            continue
        if name in active or name in kept:
            continue
        if in_wave or arguments.all_waves:
            if arguments.execute:
                removed = coordination.git(
                    repository, "worktree", "remove", row["path"],
                    allow_failure=True)
                results.append({"worktree": name,
                                "removed": removed.returncode == 0,
                                "error": (removed.stderr or "").strip()
                                if removed.returncode else None})
            else:
                results.append({"worktree": name, "removed": False,
                                "error": "dry run"})
    emit({"wave": wave, "executed": bool(arguments.execute),
          "swept": results})


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise StatusError(message)


def main(argv=None):
    parser = Parser(description=__doc__,
                    formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repository", default=".",
                        help="repository root (default: current directory)")
    parser.add_argument("--manifest", required=True,
                        help="wave manifest under tools/swarm/waves/")
    subcommands = parser.add_subparsers(dest="command")
    subcommands.add_parser("plan", help="emit the refill plan (default)")
    note = subcommands.add_parser("note", help="record a handle state")
    note.add_argument("--slot", required=True)
    note.add_argument("--agent-id")
    note.add_argument("--state", choices=LEDGER_STATES)
    note.add_argument("--note")
    note.add_argument("--death",
                      help="death reason recorded for the WIP commit "
                           "(e.g. 'rate-limit kill')")
    for command in (subcommands.add_parser(
            "recover", help="WIP-commit dead slots' worktrees and release "
            "their claims"), subcommands.add_parser(
            "sweep", help="remove clean landed worktrees out of rotation")):
        command.add_argument("--execute", action="store_true",
                             help="perform the actions; default is a dry run")
    subcommands.choices["sweep"].add_argument(
        "--all-waves", action="store_true",
        help="also sweep clean landed worktrees from other waves")
    try:
        options = parser.parse_args(argv)
        repository = Path(options.repository).resolve()
        coordination.git(repository, "rev-parse", "--show-toplevel")
        manifest = load_manifest(options.manifest, repository)
        ledger = load_ledger(repository, manifest["wave"])
        command = options.command or "plan"
        if command == "note":
            return command_note(repository, manifest, ledger, options) or 0
        if command == "recover":
            return command_recover(repository, manifest, ledger, options) or 0
        if command == "sweep":
            return command_sweep(repository, manifest, ledger, options) or 0
        emit(command_plan(repository, manifest, ledger))
        return 0
    except (StatusError, OSError) as error:
        print(f"fill: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
