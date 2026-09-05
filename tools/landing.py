#!/usr/bin/env python3
"""Cross-platform FIFO landing coordination using only Python 3 and Git.

Ready worktrees enqueue, claim the head, integrate, check, and publish. Every
queue mutation compares the prior object; publication updates main and the
queue atomically. Only the coordination ref uses force-with-lease. Waiting is
local and bounded; tickets never expire automatically. See landing.md.
"""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import random
import re
import subprocess
import sys
import time
import uuid

CLAIM_REF = "refs/coordination/omega-landing/main"
MAIN_REF = "refs/heads/main"


class LandingError(Exception):
    pass


def object_id(value, name):
    if not value or not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", value):
        raise LandingError(f"{name} requires a full lowercase Git object ID.")


def ticket_id(value):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{32}", value):
        raise LandingError("Supply the 32-character ticket returned by enqueue; enqueue before claiming.")


def now():
    return datetime.now(timezone.utc).isoformat()


def emit(record):
    print(json.dumps(record, ensure_ascii=True, separators=(",", ":")))


class Landing:
    def __init__(self, repository, remote):
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", remote):
            raise LandingError("Invalid remote name.")
        self.repository = Path(repository).resolve()
        self.git("rev-parse", "--show-toplevel")
        urls = self.git("remote", "get-url", "--push", "--all", remote).stdout.splitlines()
        if len(urls) != 1 or not urls[0]:
            raise LandingError("Exactly one push URL is required.")
        self.push_url = urls[0]

    def git(self, *arguments, input_text="", allow_failure=False):
        result = subprocess.run(
            ["git", "-C", str(self.repository), *arguments],
            input=input_text, capture_output=True, text=True, encoding="utf-8",
            errors="replace", env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
        )
        result.stdout = result.stdout.rstrip()
        result.stderr = result.stderr.rstrip()
        if result.returncode and not allow_failure:
            raise LandingError(f"Git failed ({result.returncode}): {result.stderr} {result.stdout}")
        return result

    def references(self):
        result = self.git("ls-remote", "--refs", self.push_url, MAIN_REF, CLAIM_REF)
        references = {}
        for line in result.stdout.splitlines():
            parts = line.split("\t")
            if len(parts) != 2:
                raise LandingError("Unexpected remote reference response.")
            object_id(parts[0], "Remote reference")
            references[parts[1]] = parts[0]
        if MAIN_REF not in references:
            raise LandingError("Remote main does not exist.")
        return references

    def fetch(self, identity):
        self.git("fetch", "--no-tags", "--no-write-fetch-head", self.push_url, identity)

    def snapshot(self):
        references = self.references()
        version = references.get(CLAIM_REF, "")
        record = {"protocol": "omega-landing-v2", "entries": [], "active": None}
        if version:
            self.fetch(version)
            try:
                record = json.loads(self.git("show", "-s", "--format=%B", version).stdout)
                if record["protocol"] not in ("omega-landing-v1", "omega-landing-v2"):
                    raise ValueError("unknown protocol")
                if record["protocol"] == "omega-landing-v1":
                    if not isinstance(record["owner"], str):
                        raise ValueError("invalid legacy owner")
                else:
                    self.validate_queue(record)
            except (ValueError, KeyError, TypeError) as error:
                raise LandingError("Unknown or invalid coordination format; do not replace it.") from error
        return {"version": version, "main": references[MAIN_REF], "record": record}

    @staticmethod
    def validate_queue(record):
        entries, active = record["entries"], record["active"]
        if not isinstance(entries, list):
            raise ValueError("invalid entries")
        seen = set()
        for entry in entries:
            ticket_id(entry["ticket"])
            if entry["ticket"] in seen or not isinstance(entry["owner"], str) or not entry["owner"].strip():
                raise ValueError("invalid queue entry")
            if not isinstance(entry["created_utc"], str):
                raise ValueError("invalid creation time")
            seen.add(entry["ticket"])
        if active is not None:
            if not entries or entries[0]["ticket"] != active["ticket"]:
                raise ValueError("active ticket must be first")
            object_id(active["base"], "Reserved base")

    @staticmethod
    def status(snapshot):
        record = snapshot["record"]
        if record["protocol"] == "omega-landing-v1":
            return {"state": "legacy_reserved", "claim": snapshot["version"],
                    "owner": record["owner"], "main": snapshot["main"]}
        queue = [dict(entry, position=index + 1) for index, entry in enumerate(record["entries"])]
        state = "reserved" if record["active"] else "queued" if queue else "available"
        return {"state": state, "main": snapshot["main"], "active": record["active"],
                "queue": queue, "version": snapshot["version"]}

    def clean_head(self, expected=None):
        head = self.git("rev-parse", "HEAD").stdout
        if expected and head != expected:
            raise LandingError("HEAD differs from the verified candidate.")
        if self.git("status", "--porcelain", "--untracked-files=normal").stdout:
            raise LandingError("Checkpoint tracked and untracked changes before enqueueing, reserving, or publishing.")

    def queue_object(self, record, parent):
        record.update(updated_utc=now(), nonce=uuid.uuid4().hex)
        tree = self.git("mktree").stdout
        arguments = ["commit-tree", tree, "-F", "-"]
        if parent:
            arguments.extend(("-p", parent))
        return self.git(*arguments, input_text=json.dumps(record, ensure_ascii=True)).stdout

    def legacy_release(self, options, snapshot):
        if options.command not in ("release", "recover") or options.claim != snapshot["version"]:
            raise LandingError("A legacy reservation is active. Finish with the v1 client or "
                               "release its exact claim before using FIFO.")
        result = self.git("-c", "push.followTags=false", "push", "--atomic", "--porcelain",
                          f"--force-with-lease={CLAIM_REF}:{options.claim}", self.push_url,
                          f":{CLAIM_REF}", allow_failure=True)
        if result.returncode:
            raise LandingError(f"Legacy release uncertain; inspect status. {result.stderr}")
        return {"state": "released", "claim": options.claim, "action": options.command, "reason": options.reason}

    def publication_check(self, options, snapshot):
        object_id(options.base, "Base")
        object_id(options.candidate, "Candidate")
        self.clean_head(options.candidate)
        if options.base != snapshot["record"]["active"]["base"]:
            raise LandingError("Base differs from the reserved integration base.")
        if snapshot["main"] != options.base:
            raise LandingError("Remote main advanced outside the reservation. Release and reconcile.")
        if options.candidate == options.base:
            raise LandingError("There is nothing to publish. Release the reservation.")
        self.git("merge-base", "--is-ancestor", options.base, options.candidate)
        if self.git("rev-list", "--merges", f"{options.base}..{options.candidate}").stdout:
            raise LandingError("The candidate contains merge commits; main must remain linear.")

    def coordinate(self, options):
        deadline = time.monotonic() + options.wait_seconds
        collisions = 0
        while True:
            snapshot = self.snapshot()
            record = snapshot["record"]
            if options.command == "status":
                emit(self.status(snapshot))
                return 0
            if record["protocol"] == "omega-landing-v1":
                emit(self.legacy_release(options, snapshot))
                return 0
            entries, active = record["entries"], record["active"]
            if options.command == "enqueue":
                self.clean_head()
                for index, entry in enumerate(entries):
                    if entry["ticket"] == options.ticket:
                        if entry["owner"] != options.owner:
                            raise LandingError("This ticket belongs to a different owner.")
                        emit({"state": "queued", "ticket": options.ticket, "position": index + 1})
                        return 0
                if snapshot["version"] and self.git("log", "-1", "--format=%H", "--fixed-strings",
                                                    f"--grep={options.ticket}", snapshot["version"]).stdout:
                    raise LandingError("This ticket was already removed. Enqueue with a new ticket.")
                entries.append({"ticket": options.ticket, "owner": options.owner, "created_utc": now()})
                result = {"state": "queued", "ticket": options.ticket, "position": len(entries)}
            elif options.command == "claim":
                self.clean_head()
                if not any(entry["ticket"] == options.ticket for entry in entries):
                    raise LandingError("Ticket is no longer queued. Do not reuse a removed ticket.")
                if entries[0]["ticket"] != options.ticket:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        emit(self.status(snapshot))
                        return 2
                    time.sleep(min(options.poll_seconds, remaining))
                    continue
                if active:
                    self.fetch(active["base"])
                    emit({"state": "reserved", "claim": options.ticket, "base": active["base"], "owner": entries[0]["owner"]})
                    return 0
                record["active"] = {"ticket": options.ticket, "base": snapshot["main"]}
                self.fetch(snapshot["main"])
                result = {"state": "reserved", "claim": options.ticket, "base": snapshot["main"], "owner": entries[0]["owner"]}
            elif options.command == "cancel":
                if active and active["ticket"] == options.ticket:
                    raise LandingError("Ticket is active. Release its claim instead.")
                if not any(entry["ticket"] == options.ticket for entry in entries):
                    raise LandingError("Ticket is no longer queued.")
                record["entries"] = [entry for entry in entries if entry["ticket"] != options.ticket]
                result = {"state": "cancelled", "ticket": options.ticket, "reason": options.reason}
            else:
                ticket_id(options.claim)
                if not active or active["ticket"] != options.claim:
                    raise LandingError("The supplied claim is no longer active. Nothing was changed.")
                if options.command == "publish":
                    self.publication_check(options, snapshot)
                    result = {"state": "published", "candidate": options.candidate, "released_claim": options.claim}
                else:
                    result = {"state": "released", "claim": options.claim, "action": options.command, "reason": options.reason}
                record["entries"] = [entry for entry in entries if entry["ticket"] != options.claim]
                record["active"] = None

            updated = self.queue_object(record, snapshot["version"])
            arguments = ["-c", "push.followTags=false", "push", "--atomic", "--porcelain",
                         f"--force-with-lease={CLAIM_REF}:{snapshot['version']}", self.push_url,
                         f"{updated}:{CLAIM_REF}"]
            if options.command == "publish":
                arguments.append(f"{options.candidate}:{MAIN_REF}")
            mutation = self.git(*arguments, allow_failure=True)
            if not mutation.returncode:
                emit(result)
                return 0
            after = self.references()
            version = after.get(CLAIM_REF, "")
            if version == updated and (options.command != "publish" or after[MAIN_REF] == options.candidate):
                emit(result)
                return 0
            # Retry only metadata contention. Revalidate the active ticket and
            # reapply to the latest queue, preserving concurrent arrivals.
            collisions += 1
            if version == snapshot["version"] or collisions >= 20:
                raise LandingError("Update failed or outcome uncertain. Inspect status and main before retrying. "
                                   f"{mutation.stderr} {mutation.stdout}")
            time.sleep(random.uniform(0.05, 0.2))


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise LandingError(message)


def main(arguments=None):
    parser = Parser(description=__doc__)
    parser.add_argument("command", nargs="?", default="status",
                        choices=("status", "enqueue", "cancel", "claim", "publish", "release", "recover"))
    parser.add_argument("--repository", default=".")
    parser.add_argument("--remote", default="origin")
    for name in ("owner", "ticket", "claim", "base", "candidate", "reason"):
        parser.add_argument("--" + name)
    parser.add_argument("--wait-seconds", type=int, default=0)
    parser.add_argument("--poll-seconds", type=int, default=10)
    options = None
    try:
        options = parser.parse_args(arguments)
        if not 0 <= options.wait_seconds <= 43200 or not 1 <= options.poll_seconds <= 300:
            raise LandingError("Wait must be 0..43200 seconds; poll must be 1..300 seconds.")
        if options.command != "claim" and options.wait_seconds:
            raise LandingError("--wait-seconds applies only to claim.")
        if options.command == "enqueue":
            if not options.owner or not options.owner.strip() or len(options.owner) > 200:
                raise LandingError("Supply a short, recognizable --owner.")
            options.ticket = options.ticket or uuid.uuid4().hex
        if options.command in ("enqueue", "claim", "cancel"):
            ticket_id(options.ticket)
        if options.command == "recover" and (not options.reason or not options.reason.strip()):
            raise LandingError("Recovery requires --reason after checking with the owner.")
        return Landing(options.repository, options.remote).coordinate(options)
    except (LandingError, OSError) as error:
        identity = f" Ticket={options.ticket} Claim={options.claim}" if options else ""
        print(f"landing: {error}{identity}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
