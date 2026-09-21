#!/usr/bin/env python3
"""Cross-platform FIFO landing coordination using only Python 3 and Git.

Ready worktrees enqueue, claim the head, integrate, check, and publish. Every
queue mutation compares the prior object; publication updates main and the
queue atomically. Only the coordination ref uses force-with-lease. Waiting is
local and bounded. Each promoted head gets at most three minutes. See landing.md.
"""

import argparse
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import random
import re
import sys
import time
import uuid

# Sibling-script import; both files live in tools/.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import coordination

CLAIM_REF = "refs/coordination/omega-landing/main"
MAIN_REF = "refs/heads/main"
LEASE_SECONDS = 180
COORDINATION_FILE = re.compile(
    r"^(?:TASKS[^/]*\.md|OWNER_QUESTIONS\.md|tools/swarm/waves/.+)$")
# Patience budgets. Collisions are lost compare-and-swap races against
# concurrent writers; each costs one snapshot + push round. Push failures are
# pushes rejected while the remote refs provably stayed put (network blips,
# ref-lock contention, object-DB lag), so re-pushing the same commit is safe.
COLLISION_LIMIT = 40
COLLISION_BACKOFF_BASE = 0.1
COLLISION_BACKOFF_CAP = 1.5
PUSH_FAILURE_LIMIT = 6
PUSH_FAILURE_BACKOFF_BASE = 0.4
PUSH_FAILURE_BACKOFF_CAP = 3.0
PROBE_ATTEMPTS = 3
PROBE_BACKOFF_SECONDS = 0.3
SNAPSHOT_ATTEMPTS = 3
MAINTENANCE_LIMIT = 20

LandingError = coordination.CoordinationError
object_id = coordination.object_id
now = coordination.now
utc = coordination.utc
emit = coordination.emit


def pause(attempt, base, cap):
    """Jittered exponential backoff: the ceiling doubles per attempt, the
    sleep lands in its upper half, and `cap` bounds the wait."""
    delay = min(cap, base * 2 ** (attempt - 1))
    time.sleep(random.uniform(delay / 2, delay))


def ticket_id(value):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{32}", value):
        raise LandingError("Supply the 32-character ticket returned by enqueue; enqueue before claiming.")


def promote_head(record):
    """Only changing the head starts a new lease; claim is not a renewal."""
    if record["entries"]:
        started = datetime.now(timezone.utc)
        record["head_promoted_utc"] = started.isoformat()
        record["head_expires_utc"] = (started + timedelta(seconds=LEASE_SECONDS)).isoformat()
    else:
        record["head_promoted_utc"] = None
        record["head_expires_utc"] = None


def expired(record):
    return bool(record["entries"]) and datetime.now(timezone.utc) >= utc(record["head_expires_utc"])


class Landing:
    def __init__(self, repository, remote):
        self.repository = Path(repository).resolve()
        self.push_url = coordination.push_url(self.repository, remote)

    def git(self, *arguments, input_text="", allow_failure=False):
        return coordination.git(self.repository, *arguments, input_text=input_text,
                                allow_failure=allow_failure)

    def references(self):
        references = coordination.remote_refs(self.repository, self.push_url,
                                              MAIN_REF, CLAIM_REF)
        if MAIN_REF not in references:
            raise LandingError("Remote main does not exist.")
        return references

    def fetch_version(self, version, reference=CLAIM_REF):
        """Fetch a remote tip commit, healing a missing object once.

        A fetch by raw object ID can be refused by the remote or lose to
        object-DB lag; fetching the ref itself is always permitted and pulls
        the same chained history. Verified with cat-file so a silently
        incomplete fetch does not surface as a parse failure.
        """
        try:
            coordination.fetch_object(self.repository, self.push_url, version)
        except LandingError:
            pass
        else:
            if not self.git("cat-file", "-e", version,
                            allow_failure=True).returncode:
                return
        self.git("fetch", "--no-tags", "--no-write-fetch-head",
                 self.push_url, reference)
        if self.git("cat-file", "-e", version, allow_failure=True).returncode:
            raise LandingError(f"Remote tip {version} is still missing after "
                               f"fetching {reference}; inspect the remote "
                               "object store before retrying.")

    def snapshot(self):
        references = self.references()
        version = references.get(CLAIM_REF, "")
        record = {"protocol": "omega-landing-v3", "entries": [], "active": None,
                  "head_promoted_utc": None, "head_expires_utc": None}
        if version:
            self.fetch_version(version)
            try:
                record = json.loads(self.git("show", "-s", "--format=%B", version).stdout)
                if record["protocol"] not in ("omega-landing-v1", "omega-landing-v2", "omega-landing-v3"):
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
        if record["protocol"] == "omega-landing-v3":
            started, deadline = record["head_promoted_utc"], record["head_expires_utc"]
            if entries:
                if not isinstance(started, str) or not isinstance(deadline, str):
                    raise ValueError("a queue head requires a lease")
                if utc(deadline) - utc(started) != timedelta(seconds=LEASE_SECONDS):
                    raise ValueError("a head lease must be exactly three minutes")
            elif started is not None or deadline is not None:
                raise ValueError("an empty queue cannot retain a head lease")

    @staticmethod
    def status(snapshot):
        record = snapshot["record"]
        if record["protocol"] == "omega-landing-v1":
            return {"state": "legacy_reserved", "claim": snapshot["version"],
                    "owner": record["owner"], "main": snapshot["main"]}
        if record["protocol"] == "omega-landing-v2":
            return {"state": "legacy_queue", "main": snapshot["main"],
                    "active": record["active"], "queue": record["entries"], "version": snapshot["version"]}
        queue = [dict(entry, position=index + 1) for index, entry in enumerate(record["entries"])]
        state = "reserved" if record["active"] else "queued" if queue else "available"
        if expired(record):
            state = "expired"
        return {"state": state, "main": snapshot["main"], "active": record["active"],
                "queue": queue, "version": snapshot["version"],
                "head_promoted_utc": record["head_promoted_utc"], "head_expires_utc": record["head_expires_utc"]}

    def clean_head(self, expected=None):
        head = self.git("rev-parse", "HEAD").stdout
        if expected and head != expected:
            raise LandingError("HEAD differs from the verified candidate.")
        if self.git("status", "--porcelain", "--untracked-files=normal").stdout:
            raise LandingError("Checkpoint tracked and untracked changes before enqueueing, reserving, or publishing.")

    def queue_object(self, record, parent):
        return coordination.commit_record(self.repository, record, parent)

    def remote_versions(self):
        """Current remote values of the coordination refs, retrying short
        read failures.

        This is the post-push probe: if it cannot answer, the push outcome is
        genuinely unknown and the caller must not guess success.
        """
        attempts = 0
        while True:
            try:
                return self.references()
            except LandingError as error:
                attempts += 1
                if attempts >= PROBE_ATTEMPTS:
                    raise LandingError(
                        "Cannot read the coordination refs to confirm a push "
                        "outcome; the result is uncertain. Inspect status "
                        "before retrying.") from error
                time.sleep(PROBE_BACKOFF_SECONDS)

    def publish(self, expected, updated, candidate=None, base=None):
        """Push `updated` onto CLAIM_REF leased on `expected`.

        `candidate` joins the same atomic push onto MAIN_REF for the publish
        command; `base` is then the remote main value the reservation saw, so
        a main that moved without this push is reported as contention rather
        than re-pushed forever. An empty `updated` deletes CLAIM_REF (legacy
        release).

        True once the remote holds the mutation — including when a push that
        reported failure actually landed. False when another writer moved the
        queue ref first, or when a publication's base main advanced, leaving
        the caller to re-snapshot and rebuild. A rejected push whose provable
        precondition still holds is a non-contention failure, so the same
        commit is re-pushed a bounded number of times; only an exhausted or
        unverifiable outcome raises.
        """
        refspecs = [f"{updated}:{CLAIM_REF}"]
        if candidate is not None:
            refspecs.append(f"{candidate}:{MAIN_REF}")
        failures = 0
        while True:
            mutation = coordination.push_atomic(
                self.repository, self.push_url, refspecs,
                lease=(CLAIM_REF, expected))
            if not mutation.returncode:
                return True
            after = self.remote_versions()
            version = after.get(CLAIM_REF, "")
            if version == updated and (candidate is None
                                       or after[MAIN_REF] == candidate):
                return True
            if version != expected:
                return False
            if candidate is not None and after[MAIN_REF] != base:
                # Main advanced without this push; the same batch cannot land.
                return False
            failures += 1
            if failures >= PUSH_FAILURE_LIMIT:
                raise LandingError(
                    "Update failed repeatedly while the remote refs stayed "
                    "unchanged; outcome uncertain. Inspect status and main "
                    f"before retrying. {mutation.stderr} {mutation.stdout}")
            pause(failures, PUSH_FAILURE_BACKOFF_BASE,
                  PUSH_FAILURE_BACKOFF_CAP)

    def back_off_after_collision(self, collisions):
        """Bounded patience for lost push races, then an honest stop."""
        if collisions >= COLLISION_LIMIT:
            raise LandingError(
                "Update kept losing push races to concurrent writers; "
                "inspect status and main before retrying.")
        pause(collisions, COLLISION_BACKOFF_BASE, COLLISION_BACKOFF_CAP)

    def legacy_release(self, options, snapshot):
        if options.command not in ("release", "recover") or options.claim != snapshot["version"]:
            raise LandingError("A legacy reservation is active. Finish with the v1 client or "
                               "release its exact claim before using FIFO.")
        if self.publish(options.claim, ""):
            return {"state": "released", "claim": options.claim,
                    "action": options.command, "reason": options.reason}
        return None

    def publication_check(self, options, snapshot):
        if expired(snapshot["record"]):
            raise LandingError("The three-minute head lease expired. Rejoin with a new ticket.")
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
        changed = self.git("diff", "--name-only", options.base,
                           options.candidate).stdout.splitlines()
        if not changed:
            raise LandingError("The candidate changes no files; there is nothing to publish. "
                               "Attach the finding to its claim ticket with `claims.py note` "
                               "and release the reservation instead.")
        if not options.board_update and all(COORDINATION_FILE.match(path)
                                            for path in changed):
            raise LandingError("The candidate touches only coordination files "
                               "(TASKS*.md, OWNER_QUESTIONS.md, tools/swarm/waves/). Worker "
                               "evidence belongs in `claims.py note --ticket <claim>` and the "
                               "session report; a coordinator board sweep passes --board-update.")

    def maintenance(self, snapshot):
        """Persist migration/expiry separately, before interpreting an action.

        Exact-ref comparison fences old owners and preserves concurrent arrivals.
        Advancing once gives the next head its own full lease, even if idle.

        None when nothing needed reaping, True when the advancing commit
        landed, False when another writer moved the ref first.
        """
        record = snapshot["record"]
        if record["protocol"] == "omega-landing-v1":
            # A legacy holder has no queue ticket. Retire that exact reservation
            # into history and retain its owner as a newly timed head.
            record = {"protocol": "omega-landing-v3", "entries": [
                {"ticket": uuid.uuid4().hex, "owner": record["owner"], "created_utc": now(),
                 "legacy_claim": snapshot["version"]}], "active": None}
            promote_head(record)
        elif record["protocol"] == "omega-landing-v2":
            record["protocol"] = "omega-landing-v3"
            promote_head(record)
        elif expired(record):
            record["entries"] = record["entries"][1:]
            record["active"] = None
            promote_head(record)
        else:
            return None
        updated = self.queue_object(record, snapshot["version"])
        return self.publish(snapshot["version"], updated)

    def coordinate(self, options):
        deadline = time.monotonic() + options.wait_seconds
        collisions = 0
        maintenance_attempts = 0
        snapshot_failures = 0
        while True:
            try:
                snapshot = self.snapshot()
            except LandingError:
                snapshot_failures += 1
                if snapshot_failures >= SNAPSHOT_ATTEMPTS:
                    raise
                pause(snapshot_failures, PROBE_BACKOFF_SECONDS, 1.0)
                continue
            snapshot_failures = 0
            record = snapshot["record"]
            if options.command == "status":
                emit(self.status(snapshot))
                return 0
            if record["protocol"] == "omega-landing-v1":
                if options.command in ("release", "recover") and options.claim == snapshot["version"]:
                    released = self.legacy_release(options, snapshot)
                    if released is None:
                        collisions += 1
                        self.back_off_after_collision(collisions)
                        continue
                    emit(released)
                    return 0
            advanced = self.maintenance(snapshot)
            if advanced is not None:
                if advanced:
                    maintenance_attempts += 1
                    if maintenance_attempts >= MAINTENANCE_LIMIT:
                        raise LandingError("Queue changed repeatedly; inspect status before retrying.")
                else:
                    collisions += 1
                    self.back_off_after_collision(collisions)
                continue
            lease_deadline = record["head_expires_utc"]
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
                if len(entries) == 1:
                    promote_head(record)
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
                    until_expiry = max(0, (utc(record["head_expires_utc"]) - datetime.now(timezone.utc)).total_seconds())
                    time.sleep(min(options.poll_seconds, remaining, until_expiry))
                    continue
                if active:
                    self.fetch_version(active["base"], MAIN_REF)
                    emit({"state": "reserved", "claim": options.ticket, "base": active["base"], "owner": entries[0]["owner"],
                          "expires_utc": record["head_expires_utc"]})
                    return 0
                record["active"] = {"ticket": options.ticket, "base": snapshot["main"]}
                self.fetch_version(snapshot["main"], MAIN_REF)
                result = {"state": "reserved", "claim": options.ticket, "base": snapshot["main"], "owner": entries[0]["owner"],
                          "expires_utc": record["head_expires_utc"]}
            elif options.command == "cancel":
                if active and active["ticket"] == options.ticket:
                    raise LandingError("Ticket is active. Release its claim instead.")
                if not any(entry["ticket"] == options.ticket for entry in entries):
                    raise LandingError("Ticket is no longer queued.")
                was_head = entries[0]["ticket"] == options.ticket
                record["entries"] = [entry for entry in entries if entry["ticket"] != options.ticket]
                if was_head:
                    promote_head(record)
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
                promote_head(record)

            updated = self.queue_object(record, snapshot["version"])
            candidate = None
            base = None
            if options.command == "publish":
                candidate = options.candidate
                base = snapshot["main"]
                # The record above now describes the successor, so use the
                # pre-mutation lease retained separately for this last check.
                if datetime.now(timezone.utc) >= utc(lease_deadline):
                    raise LandingError("The three-minute head lease expired before push. Rejoin.")
            if self.publish(snapshot["version"], updated, candidate, base):
                emit(result)
                return 0
            # Retry only metadata contention. Revalidate the active ticket and
            # reapply to the latest queue, preserving concurrent arrivals.
            collisions += 1
            self.back_off_after_collision(collisions)


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
    parser.add_argument("--board-update", action="store_true",
                        help="publish a candidate that touches only coordination files "
                             "(coordinator board sweeps)")
    options = None
    try:
        options = parser.parse_args(arguments)
        if not 0 <= options.wait_seconds <= 43200 or not 1 <= options.poll_seconds <= 300:
            raise LandingError("Wait must be 0..43200 seconds; poll must be 1..300 seconds.")
        if options.command != "claim" and options.wait_seconds:
            raise LandingError("--wait-seconds applies only to claim.")
        if options.board_update and options.command != "publish":
            raise LandingError("--board-update applies only to publish.")
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
