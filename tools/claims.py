#!/usr/bin/env python3
"""Cross-machine work-claim registry on refs/coordination/omega-claims/main.

A claim records one owner's in-progress assignment: a board item or freeform
name, the repository paths it expects to edit, and a renewable lease. Claims
make live work visible across machines, waves, and local sessions so a new
agent can pick unclaimed work instead of duplicating an active assignment.

Claims are advisory fences, not locks: claiming an item another live claim
holds, or a path overlapping another claim's paths, is rejected so the
coordinator can partition explicitly; --allow-overlap records an informed
exception. Expired claims are reaped by the next writer and never block
reuse. Every mutation compares the prior object like tools/landing.py; see
claims.md.
"""

import argparse
from datetime import datetime, timedelta, timezone
import json
import os
from pathlib import Path, PurePosixPath, PureWindowsPath
import random
import re
import sys
import time
import uuid

# Sibling-script import; both files live in tools/.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import coordination

CLAIMS_REF = "refs/coordination/omega-claims/main"
PROTOCOL = "omega-claims-v1"
BOARDS = ("TASKS.md", "TASKS_BOOTSTRAP.md", "TASKS_OPTIMIZER.md")
DEFAULT_LEASE_MINUTES = 480
MIN_LEASE_MINUTES = 15
MAX_LEASE_MINUTES = 1440
NOTE_TEXT_LIMIT = 2000
SWEPT_NOTE_RETENTION = 100
# Patience budgets. Collisions are lost compare-and-swap races against
# concurrent writers; each costs one snapshot + push round. Push failures are
# pushes rejected while the remote ref provably stayed put (network blips,
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
# Transport stall bounds. ls-remote, fetch, and push are the only calls that
# leave the local machine, and a dead connection there must fail fast into the
# retry loops instead of parking the whole command — the write path was
# observed hung >=120s under swarm load while local operations stayed healthy.
# Git reads these bounds from the environment, which coordination.git forwards:
# the libcurl low-speed pair aborts an HTTPS transfer silent for
# TRANSPORT_STALL_SECONDS (including a stalled connect), and GIT_SSH_COMMAND
# gives ssh remotes a non-interactive session with keepalive expiry. Values an
# operator already set win; the git:// transport has no knob and stays
# unbounded.
TRANSPORT_LOW_SPEED_LIMIT = "100"  # bytes/sec; a live transfer exceeds it fast
TRANSPORT_STALL_SECONDS = "20"
SSH_TRANSPORT_BOUND = ("ssh -o BatchMode=yes -o ConnectTimeout=10 "
                       "-o ServerAliveInterval=5 -o ServerAliveCountMax=2")
ITEM_MARKER = re.compile(r"\*\*([A-Za-z0-9][A-Za-z0-9_-]*)\.\*\*")

ClaimsError = coordination.CoordinationError
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
        raise ClaimsError("Supply the 32-character ticket returned by claim.")


def normalize_path(value):
    text = value.strip().replace("\\", "/")
    candidate = PurePosixPath(text)
    parts = candidate.parts
    if (candidate.is_absolute() or PureWindowsPath(text).drive or not parts
            or ".." in parts or text.startswith("/")):
        raise ClaimsError(f"Claim path must be repository-relative: {value!r}")
    normalized = candidate.as_posix()
    if normalized == ".":
        raise ClaimsError(f"Claim path must name a file or directory: {value!r}")
    # A comma means the value was a joined list that escaped split_path_values;
    # storing it literally would void the path fence below.
    if "," in normalized:
        raise ClaimsError(f"Claim path must not contain a comma: {value!r}")
    return normalized


def split_path_values(values):
    """Flatten repeated or comma-joined --path values into individual paths.

    Intake is strict about empty segments: a doubled or trailing comma usually
    means a mistyped list, and silently dropping it would under-fence the
    claim."""
    pieces = [piece for value in values for piece in value.split(",")]
    if any(not piece.strip() for piece in pieces):
        raise ClaimsError("--path entries must not be empty; check for a "
                          "leading, trailing, or doubled comma.")
    return pieces


def path_atoms(value):
    """Explode one stored path field into the individual paths it carries.

    A field holding comma-joined paths is the malformed form recorded before
    intake learned to split them; reading each segment independently keeps
    those existing claims fencing until they are renewed or expire. Empty
    segments are skipped here rather than rejected — reading a record must
    tolerate the malformed entries it repairs."""
    return [PurePosixPath(piece.strip().replace("\\", "/"))
            for piece in value.split(",") if piece.strip()]


def paths_overlap(first, second):
    return any(a == b or a in b.parents or b in a.parents
               for a in path_atoms(first) for b in path_atoms(second))


def board_items(repository, board):
    path = Path(repository) / board
    if not path.is_file():
        raise ClaimsError(f"Board file {board} not found under {repository}.")
    return ITEM_MARKER.findall(path.read_text(encoding="utf-8", errors="replace"))


def expired_claims(record):
    instant = datetime.now(timezone.utc)
    return [claim for claim in record["claims"] if instant >= utc(claim["expires_utc"])]


def live_claims(record):
    instant = datetime.now(timezone.utc)
    return [claim for claim in record["claims"] if instant < utc(claim["expires_utc"])]


def validate_claim(claim):
    ticket_id(claim["ticket"])
    if not isinstance(claim["item"], str) or not claim["item"].strip():
        raise ValueError("invalid claim item")
    if claim["board"] is not None and claim["board"] not in BOARDS:
        raise ValueError("invalid claim board")
    if not isinstance(claim["owner"], str) or not claim["owner"].strip():
        raise ValueError("invalid claim owner")
    if not isinstance(claim["paths"], list) or not all(
            isinstance(path, str) for path in claim["paths"]):
        raise ValueError("invalid claim paths")
    if not isinstance(claim["lease_minutes"], int):
        raise ValueError("invalid claim lease")
    utc(claim["claimed_utc"])
    utc(claim["expires_utc"])


def validate_note(note):
    ticket_id(note["ticket"])
    if not isinstance(note["item"], str) or not note["item"].strip():
        raise ValueError("invalid note item")
    if not isinstance(note["owner"], str) or not note["owner"].strip():
        raise ValueError("invalid note owner")
    if (not isinstance(note["text"], str) or not note["text"].strip()
            or len(note["text"]) > NOTE_TEXT_LIMIT):
        raise ValueError("invalid note text")
    utc(note["created_utc"])
    if note.get("swept_utc") is not None:
        utc(note["swept_utc"])


def trim_notes(record):
    """Bound the ledger: pending notes always stay; only swept history trims."""
    swept_seen = 0
    kept = []
    for note in reversed(record["notes"]):
        if note.get("swept_utc") is not None:
            swept_seen += 1
            if swept_seen > SWEPT_NOTE_RETENTION:
                continue
        kept.append(note)
    record["notes"] = list(reversed(kept))


def pending_notes(record):
    return [note for note in record.get("notes", [])
            if note.get("swept_utc") is None]


class Claims:
    def __init__(self, repository, remote):
        self.repository = Path(repository).resolve()
        os.environ.setdefault("GIT_HTTP_LOW_SPEED_LIMIT", TRANSPORT_LOW_SPEED_LIMIT)
        os.environ.setdefault("GIT_HTTP_LOW_SPEED_TIME", TRANSPORT_STALL_SECONDS)
        os.environ.setdefault("GIT_SSH_COMMAND", SSH_TRANSPORT_BOUND)
        self.push_url = coordination.push_url(self.repository, remote)

    def git(self, *arguments, input_text="", allow_failure=False):
        return coordination.git(self.repository, *arguments, input_text=input_text,
                                allow_failure=allow_failure)

    def snapshot(self):
        references = coordination.remote_refs(self.repository, self.push_url, CLAIMS_REF)
        version = references.get(CLAIMS_REF, "")
        record = {"protocol": PROTOCOL, "claims": [], "notes": []}
        if version:
            self.fetch_version(version)
            try:
                record = json.loads(self.git("show", "-s", "--format=%B", version).stdout)
                if record["protocol"] != PROTOCOL:
                    raise ValueError("unknown protocol")
                if not isinstance(record["claims"], list):
                    raise ValueError("invalid claims")
                for claim in record["claims"]:
                    validate_claim(claim)
                notes = record.setdefault("notes", [])
                if not isinstance(notes, list):
                    raise ValueError("invalid notes")
                for note in notes:
                    validate_note(note)
            except (ValueError, KeyError, TypeError) as error:
                raise ClaimsError("Unknown or invalid claims format; do not replace it.") from error
        return {"version": version, "record": record}

    def fetch_version(self, version):
        """Fetch the claims tip commit, healing a missing object once.

        A fetch by raw object ID can be refused by the remote or lose to
        object-DB lag; fetching the ref itself is always permitted and pulls
        the same chained history. Verified with cat-file so a silently
        incomplete fetch does not surface as a parse failure.
        """
        try:
            coordination.fetch_object(self.repository, self.push_url, version)
        except ClaimsError:
            pass
        else:
            if not self.git("cat-file", "-e", version,
                            allow_failure=True).returncode:
                return
        self.git("fetch", "--no-tags", "--no-write-fetch-head",
                 self.push_url, CLAIMS_REF)
        if self.git("cat-file", "-e", version, allow_failure=True).returncode:
            raise ClaimsError(f"Claims tip {version} is still missing after "
                              f"fetching {CLAIMS_REF}; inspect the remote "
                              "object store before retrying.")

    def remote_version(self):
        """Current remote value of CLAIMS_REF, retrying short read failures.

        This is the post-push probe: if it cannot answer, the push outcome is
        genuinely unknown and the caller must not guess success.
        """
        attempts = 0
        while True:
            try:
                references = coordination.remote_refs(
                    self.repository, self.push_url, CLAIMS_REF)
                return references.get(CLAIMS_REF, "")
            except ClaimsError as error:
                attempts += 1
                if attempts >= PROBE_ATTEMPTS:
                    raise ClaimsError(
                        "Cannot read the claims ref to confirm a push "
                        "outcome; the result is uncertain. Inspect status "
                        "before retrying.") from error
                time.sleep(PROBE_BACKOFF_SECONDS)

    def publish(self, expected, updated):
        """Push `updated` onto CLAIMS_REF leased on `expected`.

        True once the remote ref holds `updated` — including when a push that
        reported failure actually landed. False when another writer moved the
        ref first, leaving the caller to re-snapshot and rebuild. A rejected
        push with the ref still at `expected` is a non-contention failure, so
        the same commit is re-pushed a bounded number of times; only an
        exhausted or unverifiable outcome raises.
        """
        failures = 0
        while True:
            mutation = coordination.push_atomic(
                self.repository, self.push_url, [f"{updated}:{CLAIMS_REF}"],
                lease=(CLAIMS_REF, expected))
            if not mutation.returncode:
                return True
            after = self.remote_version()
            if after == updated:
                return True
            if after != expected:
                return False
            failures += 1
            if failures >= PUSH_FAILURE_LIMIT:
                raise ClaimsError(
                    "Update failed repeatedly while the remote ref stayed "
                    "unchanged; outcome uncertain. Inspect status before "
                    f"retrying. {mutation.stderr} {mutation.stdout}")
            pause(failures, PUSH_FAILURE_BACKOFF_BASE,
                  PUSH_FAILURE_BACKOFF_CAP)

    def back_off_after_collision(self, collisions):
        """Bounded patience for lost push races, then an honest stop."""
        if collisions >= COLLISION_LIMIT:
            raise ClaimsError(
                "Update kept losing push races to concurrent writers; "
                "inspect status before retrying.")
        pause(collisions, COLLISION_BACKOFF_BASE, COLLISION_BACKOFF_CAP)

    def maintenance(self, snapshot):
        """Reap expired claims separately, before interpreting an action.

        None when nothing had expired, True when the reaping commit landed,
        False when another writer moved the ref first.
        """
        expired = expired_claims(snapshot["record"])
        if not expired:
            return None
        record = snapshot["record"]
        record["claims"] = live_claims(record)
        updated = coordination.commit_record(self.repository, record, snapshot["version"])
        return self.publish(snapshot["version"], updated)

    def conflicts(self, record, item, paths):
        found = []
        for claim in live_claims(record):
            if claim["item"] == item:
                found.append({"ticket": claim["ticket"], "item": claim["item"],
                              "owner": claim["owner"], "reason": "same item",
                              "expires_utc": claim["expires_utc"]})
                continue
            shared = sorted({a for a in paths for b in claim["paths"]
                             if paths_overlap(a, b)})
            if shared:
                found.append({"ticket": claim["ticket"], "item": claim["item"],
                              "owner": claim["owner"], "reason": "overlapping paths",
                              "shared_paths": shared,
                              "expires_utc": claim["expires_utc"]})
        return found

    def coordinate(self, options):
        collisions = 0
        maintenance_attempts = 0
        snapshot_failures = 0
        while True:
            try:
                snapshot = self.snapshot()
            except ClaimsError:
                snapshot_failures += 1
                if snapshot_failures >= SNAPSHOT_ATTEMPTS:
                    raise
                pause(snapshot_failures, PROBE_BACKOFF_SECONDS, 1.0)
                continue
            snapshot_failures = 0
            record = snapshot["record"]
            if options.command in ("status", "available", "notes"):
                return self.observe(options, snapshot)
            reaped = self.maintenance(snapshot)
            if reaped is not None:
                if reaped:
                    maintenance_attempts += 1
                    if maintenance_attempts >= MAINTENANCE_LIMIT:
                        raise ClaimsError("Claims changed repeatedly; inspect status before retrying.")
                else:
                    collisions += 1
                    self.back_off_after_collision(collisions)
                continue
            claims = live_claims(record)
            if options.command == "claim":
                prepared = self.prepare_claim(options, snapshot, claims)
                if not prepared["proceed"]:
                    emit(prepared["record"])
                    return prepared["code"]
                result = prepared["record"]
            elif options.command == "sweep":
                pending = pending_notes(record)
                if options.owner:
                    marked = [note for note in pending
                              if options.owner in (note.get("owner") or "")]
                else:
                    marked = pending
                stamp = now()
                for note in marked:
                    note["swept_utc"] = stamp
                trim_notes(record)
                result = {"state": "swept", "swept": len(marked),
                          "pending_left": len(pending) - len(marked)}
            else:
                ticket_id(options.ticket)
                claim = next((entry for entry in claims
                              if entry["ticket"] == options.ticket), None)
                if options.command == "note":
                    if claim is None:
                        raise ClaimsError("No live claim holds this ticket. "
                                          "Notes attach to a live claim.")
                    record["notes"].append(
                        {"ticket": claim["ticket"], "item": claim["item"],
                         "owner": claim["owner"], "text": options.text.strip(),
                         "created_utc": now(), "swept_utc": None})
                    trim_notes(record)
                    result = {"state": "noted", "ticket": claim["ticket"],
                              "item": claim["item"]}
                elif options.command == "renew":
                    if claim is None:
                        raise ClaimsError("No live claim holds this ticket. Claim again.")
                    lease = options.lease_minutes or claim["lease_minutes"]
                    claim["expires_utc"] = (datetime.now(timezone.utc)
                                            + timedelta(minutes=lease)).isoformat()
                    claim["renewed_utc"] = now()
                    result = {"state": "renewed", "ticket": options.ticket,
                              "item": claim["item"],
                              "expires_utc": claim["expires_utc"]}
                elif options.command == "release":
                    if claim is None:
                        raise ClaimsError("No live claim holds this ticket. Nothing was changed.")
                    result = {"state": "released", "ticket": options.ticket,
                              "item": claim["item"]}
                else:  # recover
                    claim = next((entry for entry in record["claims"]
                                  if entry["ticket"] == options.ticket), None)
                    if claim is None:
                        raise ClaimsError("No claim holds this ticket. Nothing was changed.")
                    result = {"state": "recovered", "ticket": options.ticket,
                              "item": claim["item"], "reason": options.reason}
                if options.command in ("renew", "release", "recover"):
                    record["claims"] = [entry for entry in record["claims"]
                                        if entry["ticket"] != options.ticket]
                if options.command == "renew":
                    record["claims"].append(claim)

            updated = coordination.commit_record(self.repository, record,
                                                 snapshot["version"])
            if self.publish(snapshot["version"], updated):
                emit(result)
                return 0
            collisions += 1
            self.back_off_after_collision(collisions)

    def prepare_claim(self, options, snapshot, claims):
        record = snapshot["record"]
        existing = next((entry for entry in claims
                         if entry["ticket"] == options.ticket), None)
        if existing:
            if existing["owner"] != options.owner:
                raise ClaimsError("This ticket belongs to a different owner.")
            return {"code": 0, "proceed": False,
                    "record": {"state": "claimed", "ticket": options.ticket,
                               "item": existing["item"],
                               "expires_utc": existing["expires_utc"],
                               "idempotent": True}}
        if snapshot["version"] and self.git(
                "log", "-1", "--format=%H", "--fixed-strings",
                f"--grep={options.ticket}", snapshot["version"]).stdout:
            raise ClaimsError("This ticket was already removed. Claim with a new ticket.")
        found = self.conflicts(record, options.item, options.paths)
        if found and not options.allow_overlap:
            return {"code": 2, "proceed": False,
                    "record": {"state": "conflict", "item": options.item,
                               "conflicts": found}}
        lease = options.lease_minutes
        entry = {"ticket": options.ticket, "item": options.item,
                 "board": options.board, "owner": options.owner,
                 "paths": options.paths, "note": options.note,
                 "lease_minutes": lease, "claimed_utc": now(),
                 "expires_utc": (datetime.now(timezone.utc)
                                 + timedelta(minutes=lease)).isoformat()}
        record["claims"].append(entry)
        result = {"state": "claimed", "ticket": options.ticket, "item": options.item,
                  "expires_utc": entry["expires_utc"]}
        if found:
            result["overlaps"] = found
        return {"code": 0, "proceed": True, "record": result}

    def audit(self, options):
        """Read-only coverage check: a worktree's changed paths vs live claims.

        Changed means uncommitted work plus commits ahead of --base. A path is
        a violation when it lands inside a different owner's live claim; paths
        covered by no claim at all are reported uncovered. Claims record intent
        at claim time — this is the check that edits stayed inside the fence.
        """
        live = live_claims(self.snapshot()["record"])
        worktree = Path(options.worktree).resolve()
        changed = set()
        status = coordination.git(self.repository, "-C", str(worktree),
                                  "status", "--porcelain").stdout
        for line in status.splitlines():
            if line.startswith("??"):
                continue
            path = line[3:].split(" -> ", 1)[-1].strip().strip('"')
            if path:
                changed.add(path)
        others = coordination.git(self.repository, "-C", str(worktree),
                                  "ls-files", "--others",
                                  "--exclude-standard").stdout
        changed.update(line for line in others.splitlines() if line)
        diff = coordination.git(self.repository, "-C", str(worktree), "diff",
                                "--name-only", f"{options.base}...HEAD",
                                allow_failure=True).stdout
        changed.update(line for line in diff.splitlines() if line)
        own = [path for claim in live if claim["owner"] == options.owner
               for path in claim["paths"]]
        uncovered, violations = [], []
        for path in sorted(changed):
            if any(paths_overlap(path, owned) for owned in own):
                continue
            uncovered.append(path)
            hit = next((claim for claim in live
                        if claim["owner"] != options.owner and any(
                            paths_overlap(path, other)
                            for other in claim["paths"])), None)
            if hit is not None:
                violations.append({"path": path, "item": hit["item"],
                                   "claimed_by": hit["owner"],
                                   "ticket": hit["ticket"],
                                   "expires_utc": hit["expires_utc"]})
        emit({"state": "audited", "worktree": str(worktree),
              "owner": options.owner, "base": options.base,
              "changed": sorted(changed), "uncovered": uncovered,
              "violations": violations})
        return 2 if violations else 0

    def observe(self, options, snapshot):
        record = snapshot["record"]
        live, expired = live_claims(record), expired_claims(record)
        if options.command == "status":
            emit({"state": "claimed" if live else "available", "claims": live,
                  "expired": expired, "notes_pending": len(pending_notes(record)),
                  "version": snapshot["version"]})
            return 0
        if options.command == "notes":
            pending = pending_notes(record)
            emit({"state": "notes", "notes": pending, "count": len(pending)})
            return 0
        items = board_items(self.repository, options.board)
        claimed_items = {claim["item"] for claim in live}
        emit({"board": options.board,
              "unclaimed": [item for item in items if item not in claimed_items],
              "claimed": sorted(claim["item"] for claim in live
                                if claim["board"] == options.board)})
        return 0


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise ClaimsError(message)


def main(argv=None):
    parser = Parser(description=__doc__)
    parser.add_argument("--repository", default=".")
    parser.add_argument("--remote", default="origin")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("status")
    claim = subparsers.add_parser("claim")
    claim.add_argument("--item", required=True)
    claim.add_argument("--board", choices=BOARDS)
    claim.add_argument("--owner", required=True)
    claim.add_argument("--path", dest="paths", action="append", default=[])
    claim.add_argument("--lease-minutes", type=int, default=DEFAULT_LEASE_MINUTES)
    claim.add_argument("--note")
    claim.add_argument("--ticket")
    claim.add_argument("--allow-overlap", action="store_true")
    renew = subparsers.add_parser("renew")
    renew.add_argument("--ticket", required=True)
    renew.add_argument("--lease-minutes", type=int)
    release = subparsers.add_parser("release")
    release.add_argument("--ticket", required=True)
    recover = subparsers.add_parser("recover")
    recover.add_argument("--ticket", required=True)
    recover.add_argument("--reason", required=True)
    available = subparsers.add_parser("available")
    available.add_argument("--board", required=True, choices=BOARDS)
    note = subparsers.add_parser("note")
    note.add_argument("--ticket", required=True)
    note.add_argument("--text", required=True)
    subparsers.add_parser("notes")
    sweep = subparsers.add_parser(
        "sweep", help="mark pending worker notes as consumed")
    sweep.add_argument(
        "--owner",
        help="sweep only notes whose claim owner contains this substring "
             "(e.g. the wave name) so sibling waves' evidence survives")
    audit = subparsers.add_parser("audit")
    audit.add_argument("--worktree", required=True)
    audit.add_argument("--owner", required=True)
    audit.add_argument("--base", default="origin/main")
    try:
        options = parser.parse_args(argv)
        if options.command in ("claim", "renew"):
            lease = options.lease_minutes or DEFAULT_LEASE_MINUTES
            if not MIN_LEASE_MINUTES <= lease <= MAX_LEASE_MINUTES:
                raise ClaimsError(f"Lease must be {MIN_LEASE_MINUTES}..{MAX_LEASE_MINUTES} minutes.")
            if options.command == "claim":
                options.lease_minutes = lease
        if options.command == "claim":
            if not options.owner.strip() or len(options.owner) > 200:
                raise ClaimsError("Supply a short, recognizable --owner.")
            options.ticket = options.ticket or uuid.uuid4().hex
            ticket_id(options.ticket)
            options.paths = [normalize_path(piece)
                             for piece in split_path_values(options.paths)]
            if options.board and options.item not in board_items(options.repository,
                                                                 options.board):
                raise ClaimsError(f"Item {options.item} not found in {options.board} "
                                  "(no **<item>.** marker).")
        if options.command == "recover" and not options.reason.strip():
            raise ClaimsError("Recovery requires --reason after checking with the owner.")
        if options.command == "note":
            if not options.text.strip():
                raise ClaimsError("Note requires nonempty --text.")
            if len(options.text) > NOTE_TEXT_LIMIT:
                raise ClaimsError(f"Note text must be at most {NOTE_TEXT_LIMIT} characters.")
        if options.command == "audit":
            return Claims(options.repository, options.remote).audit(options)
        return Claims(options.repository, options.remote).coordinate(options)
    except (ClaimsError, OSError) as error:
        print(f"claims: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
