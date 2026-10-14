#!/usr/bin/env python3
"""Cross-machine work-claim registry on refs/coordination/omega-claims/main.

A claim records one session's in-progress work: a board item or freeform
name, the repository paths it expects to edit, and a renewable lease. Claims
make live work visible across machines, waves, and local sessions so a new
agent can choose unclaimed work instead of unknowingly duplicating it.

Claims grant and refuse nothing. Another live claim on the same item or on
overlapping paths is reported as an overlap, and the sessions reconcile their
changes at rebase. --allow-overlap remains accepted for older callers and
changes nothing.
Expired claims are reaped by the next writer and never block
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
from push_budgets import (COLLISION_BACKOFF_BASE, COLLISION_BACKOFF_CAP,
                        COLLISION_LIMIT, PROBE_ATTEMPTS,
                        PROBE_BACKOFF_SECONDS, PUSH_FAILURE_BACKOFF_BASE,
                        PUSH_FAILURE_BACKOFF_CAP, PUSH_FAILURE_LIMIT)

CLAIMS_REF = "refs/coordination/omega-claims/main"
PROTOCOL = "omega-claims-v1"
BOARDS = ("TASKS.md", "TASKS_BOOTSTRAP.md", "TASKS_OPTIMIZER.md")
DEFAULT_LEASE_MINUTES = 480
DEFAULT_TIMEOUT_SECONDS = 15
ACQUISITION_TIMEOUT_SECONDS = 120
MIN_LEASE_MINUTES = 15
MAX_LEASE_MINUTES = 1440
NOTE_TEXT_LIMIT = 2000
SWEPT_NOTE_RETENTION = 100
# A note on a released or expired ticket is a continuation record, not a
# backlog: its work has landed or its frontier is in the sample README. Keep
# the newest such note per ticket for this long, then drop it.
FINISHED_NOTE_RETENTION_DAYS = 3
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
# A header may name several items at once ("**A / B.**"); each is a real
# board item and each must be claimable, so the marker admits the slash
# form and `board_items` splits it.
ITEM_MARKER = re.compile(
    r"\*\*([A-Za-z0-9][A-Za-z0-9_-]*(?:\s*/\s*[A-Za-z0-9][A-Za-z0-9_-]*)*)\.\*\*")

ClaimsError = coordination.CoordinationError
object_id = coordination.object_id
now = coordination.now
utc = coordination.utc
emit = coordination.emit

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
    # A comma or whitespace means the value was a joined list that escaped
    # split_path_values; storing it literally would void the path fence below.
    if "," in normalized or any(character.isspace() for character in normalized):
        raise ClaimsError(
            f"Claim path must not contain a comma or whitespace: {value!r}")
    return normalized

def split_path_values(values):
    """Flatten repeated, comma-joined or whitespace-joined --path values into
    individual paths.

    Intake is strict about empty comma segments: a doubled or trailing comma
    usually means a mistyped list, and silently dropping it would under-fence
    the claim. Whitespace separates paths too — repository paths carry none,
    and a quoted space-separated list would otherwise be stored as one name
    that fences nothing."""
    pieces = [piece for value in values for piece in value.split(",")]
    if any(not piece.strip() for piece in pieces):
        raise ClaimsError("--path entries must not be empty; check for a "
                          "leading, trailing, or doubled comma.")
    return [path for piece in pieces for path in piece.split()]

def path_atoms(value):
    """Explode one stored path field into the individual paths it carries.

    A field holding comma- or whitespace-joined paths is the malformed form
    recorded before intake learned to split them; reading each segment
    independently keeps those existing claims fencing until they are renewed
    or expire. Empty segments are skipped here rather than rejected — reading
    a record must tolerate the malformed entries it repairs."""
    return [PurePosixPath(piece.replace("\\", "/"))
            for segment in value.split(",") for piece in segment.split()]

def is_task_board(path):
    return any(PurePosixPath(path).name.casefold() == board.casefold()
               for board in BOARDS)

def paths_overlap(first, second):
    # Legacy board-file claims cannot fence shared task lists. Keep the
    # remaining paths in joined fields protected as before.
    return any(a == b or a in b.parents or b in a.parents
               for a in path_atoms(first) if not is_task_board(a)
               for b in path_atoms(second) if not is_task_board(b))

def is_directory_fence(repository, path):
    """A claimed path that fences a whole directory: an existing directory,
    or — when nothing exists there — a name carrying no extension."""
    target = Path(repository) / path
    # A name the filesystem cannot even probe (longer than the OS limit, as a
    # joined list recorded literally can be) exists nowhere; the name rule
    # below still classifies it instead of crashing every status reader.
    try:
        if target.is_dir():
            return True
        if target.exists():
            return False
    except OSError:
        pass
    return "." not in PurePosixPath(path).name

def board_items(repository, board):
    path = Path(repository) / board
    if not path.is_file():
        raise ClaimsError(f"Board file {board} not found under {repository}.")
    markers = ITEM_MARKER.findall(path.read_text(encoding="utf-8", errors="replace"))
    return [name.strip() for marker in markers for name in marker.split("/")]

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
    """Bound the ledger, which every write rewrites in full.

    Every note on a live claim stays. A ticket with no live claim keeps only
    its newest pending note, for FINISHED_NOTE_RETENTION_DAYS. Swept history
    keeps SWEPT_NOTE_RETENTION entries. Without this bound the registry grew
    to 7487 notes (6.4 MB) because nothing swept them, and every claim, renew
    and note write fetched, rewrote and pushed all of it within the 15-second
    coordination budget.
    """
    live = {claim["ticket"] for claim in live_claims(record)}
    cutoff = datetime.now(timezone.utc) - timedelta(days=FINISHED_NOTE_RETENTION_DAYS)
    swept_seen = 0
    finished_seen = set()
    kept = []
    for note in reversed(record["notes"]):
        if note.get("swept_utc") is not None:
            swept_seen += 1
            if swept_seen > SWEPT_NOTE_RETENTION:
                continue
        elif note["ticket"] not in live:
            if note["ticket"] in finished_seen or utc(note["created_utc"]) < cutoff:
                continue
            finished_seen.add(note["ticket"])
        kept.append(note)
    record["notes"] = list(reversed(kept))

def pending_notes(record):
    return [note for note in record.get("notes", [])
            if note.get("swept_utc") is None]

class Claims:
    def __init__(self, repository, remote, timeout_seconds=None):
        self.repository = Path(repository).resolve()
        self.started = time.monotonic()
        self.automatic_acquisition = timeout_seconds is None
        self.deadline = self.started + (DEFAULT_TIMEOUT_SECONDS if timeout_seconds is None else timeout_seconds)
        self.attempted_claims = []
        os.environ.setdefault("GIT_HTTP_LOW_SPEED_LIMIT", TRANSPORT_LOW_SPEED_LIMIT)
        os.environ.setdefault("GIT_HTTP_LOW_SPEED_TIME", TRANSPORT_STALL_SECONDS)
        os.environ.setdefault("GIT_SSH_COMMAND", SSH_TRANSPORT_BOUND)
        self.push_url = coordination.push_url(self.repository, remote, deadline=self.deadline)

    def git(self, *arguments, input_text="", allow_failure=False):
        return coordination.git(self.repository, *arguments, input_text=input_text,
                                allow_failure=allow_failure, deadline=self.deadline)

    def snapshot(self):
        references = coordination.remote_refs(self.repository, self.push_url, CLAIMS_REF,
                                              deadline=self.deadline)
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
        if not self.git("cat-file", "-e", version, allow_failure=True).returncode:
            self.remember_version(version)
            return
        # A missing tip needs object acquisition, whether this is the first
        # read or a cached reader catching up with concurrent writers. Give that
        # transfer its bounded budget once from command startup. Commands whose
        # exact tip is already local stay short; explicit budgets never grow.
        if getattr(self, "automatic_acquisition", False):
            self.deadline = self.started + ACQUISITION_TIMEOUT_SECONDS
            self.automatic_acquisition = False
            print("claims: acquiring registry history (up to 120 seconds); "
                  "locally present snapshots use the 15-second budget.", file=sys.stderr)
        try:
            coordination.fetch_object(self.repository, self.push_url, version,
                                      reference=CLAIMS_REF, deadline=self.deadline)
        except coordination.CoordinationTimeout:
            raise
        except ClaimsError:
            pass
        else:
            if not self.git("cat-file", "-e", version,
                            allow_failure=True).returncode:
                self.remember_version(version)
                return
        coordination.fetch_object(self.repository, self.push_url, CLAIMS_REF,
                                  reference=CLAIMS_REF, deadline=self.deadline)
        if self.git("cat-file", "-e", version, allow_failure=True).returncode:
            raise ClaimsError(f"Claims tip {version} is still missing after "
                              f"fetching {CLAIMS_REF}; inspect the remote "
                              "object store before retrying.")
        self.remember_version(version)

    def remember_version(self, version):
        coordination.remember_object(self.repository, self.push_url, CLAIMS_REF,
                                     version, deadline=self.deadline)

    def remote_version(self):
        """Current remote value of CLAIMS_REF, retrying short read failures.

        This is the post-push probe: if it cannot answer, the push outcome is
        genuinely unknown and the caller must not guess success.
        """
        attempts = 0
        while True:
            try:
                references = coordination.remote_refs(
                    self.repository, self.push_url, CLAIMS_REF, deadline=self.deadline)
                return references.get(CLAIMS_REF, "")
            except coordination.CoordinationTimeout:
                raise
            except ClaimsError as error:
                attempts += 1
                if attempts >= PROBE_ATTEMPTS:
                    raise ClaimsError(
                        "Cannot read the claims ref to confirm a push "
                        "outcome; the result is uncertain. Inspect status "
                        "before retrying.") from error
                self.pause(1, PROBE_BACKOFF_SECONDS, PROBE_BACKOFF_SECONDS)

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
                lease=(CLAIMS_REF, expected), deadline=self.deadline)
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
            self.pause(failures, PUSH_FAILURE_BACKOFF_BASE,
                  PUSH_FAILURE_BACKOFF_CAP)

    def pause(self, attempt, base, cap):
        remaining = coordination.remaining_seconds(self.deadline)
        ceiling = min(cap, base * 2 ** (attempt - 1))
        time.sleep(min(random.uniform(ceiling / 2, ceiling), remaining))
        coordination.remaining_seconds(self.deadline)

    def back_off_after_collision(self, collisions):
        """Bounded patience for lost push races, then an honest stop."""
        if collisions >= COLLISION_LIMIT:
            raise ClaimsError(
                "Update kept losing push races to concurrent writers; "
                "inspect status before retrying.")
        self.pause(collisions, COLLISION_BACKOFF_BASE, COLLISION_BACKOFF_CAP)

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
        trim_notes(record)
        updated = coordination.commit_record(self.repository, record, snapshot["version"],
                                             deadline=self.deadline)
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
            except coordination.CoordinationTimeout:
                raise
            except ClaimsError:
                snapshot_failures += 1
                if snapshot_failures >= SNAPSHOT_ATTEMPTS:
                    raise
                self.pause(snapshot_failures, PROBE_BACKOFF_SECONDS, 1.0)
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

            trim_notes(record)
            updated = coordination.commit_record(self.repository, record,
                                                 snapshot["version"], deadline=self.deadline)
            if options.command == "claim":
                self.attempted_claims.append(updated)
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
        # A new UUID generated by this invocation has no prior lifecycle to
        # search. If an earlier push had an uncertain outcome, check only our
        # attempted commits: an accepted-then-removed claim must not resurrect.
        for candidate in self.attempted_claims:
            if not snapshot["version"]:
                continue
            ancestry = self.git(
                    "merge-base", "--is-ancestor", candidate, snapshot["version"],
                    allow_failure=True)
            if ancestry.returncode not in (0, 1):
                raise ClaimsError("Cannot establish whether an earlier claim landed; inspect status before retrying.")
            if ancestry.returncode == 0:
                raise ClaimsError("This claim landed and was subsequently removed. Claim with a new ticket.")
        # Preserve existing callers supplying fixed/retry tickets. Only this
        # legacy interface needs the historical replay check, within the same
        # wall-clock budget. Ordinary registration never scans commit messages.
        if not getattr(options, "fresh_ticket", False) and snapshot["version"] and self.git(
                "log", "-1", "--format=%H", "--fixed-strings",
                f"--grep={options.ticket}", snapshot["version"]).stdout:
            raise ClaimsError("This ticket was already removed. Claim with a new ticket.")
        # A claim makes work visible; it never refuses work. Another live
        # claim on the same item is reported like an overlapping path, and
        # the sessions reconcile their changes at rebase (AGENTS.md).
        found = self.conflicts(record, options.item, options.paths)
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
        overlapping another outcome is reported for integration. Paths outside
        the initial estimate are informational, never a permission failure.
        """
        live = live_claims(self.snapshot()["record"])
        worktree = Path(options.worktree).resolve()
        changed = set()
        status = coordination.git(self.repository, "-C", str(worktree),
                                  "status", "--porcelain", deadline=self.deadline).stdout
        for line in status.splitlines():
            if line.startswith("??"):
                continue
            path = line[3:].split(" -> ", 1)[-1].strip().strip('"')
            if path:
                changed.add(path)
        others = coordination.git(self.repository, "-C", str(worktree),
                                  "ls-files", "--others",
                                  "--exclude-standard", deadline=self.deadline).stdout
        changed.update(line for line in others.splitlines() if line)
        diff = coordination.git(self.repository, "-C", str(worktree), "diff",
                                "--name-only", f"{options.base}...HEAD",
                                allow_failure=True, deadline=self.deadline).stdout
        changed.update(line for line in diff.splitlines() if line)
        own = [path for claim in live if claim["owner"] == options.owner
               for path in claim["paths"]]
        uncovered, overlaps = [], []
        for path in sorted(changed):
            if is_task_board(path):
                continue
            if not any(paths_overlap(path, owned) for owned in own):
                uncovered.append(path)
            for hit in live:
                if hit["owner"] == options.owner or not any(
                        paths_overlap(path, other) for other in hit["paths"]):
                    continue
                overlaps.append({"path": path, "item": hit["item"],
                                   "claimed_by": hit["owner"],
                                   "ticket": hit["ticket"],
                                   "expires_utc": hit["expires_utc"]})
        emit({"state": "audited", "worktree": str(worktree),
              "owner": options.owner, "base": options.base,
              "changed": sorted(changed), "uncovered": uncovered,
              "overlaps": overlaps, "violations": []})
        return 0

    def observe(self, options, snapshot):
        record = snapshot["record"]
        live, expired = live_claims(record), expired_claims(record)
        if options.command == "status":
            off_board_fences = []
            for claim in live:
                if claim["board"] is not None:
                    continue
                fences = [atom.as_posix() for field in claim["paths"]
                          for atom in path_atoms(field)
                          if is_directory_fence(self.repository,
                                                atom.as_posix())]
                if fences:
                    off_board_fences.append(
                        {"ticket": claim["ticket"], "item": claim["item"],
                         "owner": claim["owner"], "paths": fences,
                         "expires_utc": claim["expires_utc"]})
            emit({"state": "claimed" if live else "available", "claims": live,
                  "expired": expired, "off_board_fences": [],
                  "off_board_paths": off_board_fences,
                  "notes_pending": len(pending_notes(record)),
                  "version": snapshot["version"]})
            return 0
        if options.command == "notes":
            # Pending is the default because it is the coordinator's drain
            # list; --all adds swept history for audit reads. Every note
            # carries owner/created_utc from the claim that wrote it.
            notes = (record.get("notes", []) if options.all
                     else pending_notes(record))
            if options.ticket:
                # A prefix must name one ticket among every ticket the
                # ledger knows (claims and all notes), not just the notes
                # this view happens to show; silently merging inboxes is
                # the failure this refuses.
                known = ({claim["ticket"] for claim in record["claims"]}
                         | {note["ticket"]
                            for note in record.get("notes", [])})
                matches = sorted(ticket for ticket in known
                                 if ticket.startswith(options.ticket))
                if len(matches) > 1:
                    raise ClaimsError(
                        f"Ticket prefix {options.ticket!r} matches "
                        f"{len(matches)} tickets ({', '.join(m[:12] for m in matches[:5])}"
                        f"{', ...' if len(matches) > 5 else ''}); "
                        "supply more characters.")
                notes = [note for note in notes
                         if note["ticket"].startswith(options.ticket)]
            if options.owner:
                notes = [note for note in notes
                         if options.owner in (note.get("owner") or "")]
            emit({"state": "notes", "notes": notes, "count": len(notes)})
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
    parser.add_argument("--timeout-seconds", type=float, default=None,
                        help="explicit total deadline; default 15 seconds, 120 when registry acquisition is needed")
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
    claim.add_argument("--allow-overlap", action="store_true",
                       help="compatibility flag; paths always overlap freely, items do not")
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
    notes = subparsers.add_parser("notes")
    notes.add_argument(
        "--ticket",
        help="only notes attached to this ticket; a prefix works when it "
             "names exactly one known ticket (e.g. 686326 for "
             "68632641d3634d468d3a6b0e30fffef7), and an ambiguous prefix "
             "is refused")
    notes.add_argument(
        "--owner",
        help="only notes whose claim owner contains this substring, like "
             "sweep --owner (e.g. the wave name)")
    notes.add_argument(
        "--all", action="store_true",
        help="include swept notes, not just pending ones — the audit trail")
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
        if options.timeout_seconds is not None and not 0 < options.timeout_seconds <= 300:
            raise ClaimsError("--timeout-seconds must be greater than zero and at most 300.")
        if options.command in ("claim", "renew"):
            lease = options.lease_minutes or DEFAULT_LEASE_MINUTES
            if not MIN_LEASE_MINUTES <= lease <= MAX_LEASE_MINUTES:
                raise ClaimsError(f"Lease must be {MIN_LEASE_MINUTES}..{MAX_LEASE_MINUTES} minutes.")
            if options.command == "claim":
                options.lease_minutes = lease
        if options.command == "claim":
            if not options.owner.strip() or len(options.owner) > 200:
                raise ClaimsError("Supply a short, recognizable --owner.")
            options.fresh_ticket = options.ticket is None
            options.ticket = options.ticket or uuid.uuid4().hex
            ticket_id(options.ticket)
            options.paths = [normalize_path(piece)
                             for piece in split_path_values(options.paths)]
            if options.board and options.item not in board_items(options.repository,
                                                                 options.board):
                raise ClaimsError(f"Item {options.item} not found in {options.board} "
                                  "(no **<item>.** marker).")
            # Paths are estimates. Boards remain shared, not claimable outcomes.
            for path in options.paths:
                if is_task_board(path):
                    raise ClaimsError(
                        f"Task list {path} is shared and cannot be claimed. "
                        "Claim individual items with --board and --item, "
                        "and implementation paths with --path.")
        if options.command == "recover" and not options.reason.strip():
            raise ClaimsError("Recovery requires --reason after checking with the owner.")
        if options.command == "note":
            if not options.text.strip():
                raise ClaimsError("Note requires nonempty --text.")
            if len(options.text) > NOTE_TEXT_LIMIT:
                raise ClaimsError(f"Note text must be at most {NOTE_TEXT_LIMIT} characters.")
        if options.command == "audit":
            return Claims(options.repository, options.remote, options.timeout_seconds).audit(options)
        return Claims(options.repository, options.remote, options.timeout_seconds).coordinate(options)
    except coordination.CoordinationTimeout as error:
        report = {"state": "unavailable", "error": str(error),
                  "command": options.command, "may_have_published": options.command not in
                  ("status", "available", "notes", "audit")}
        if options.command == "claim":
            report.update(ticket=options.ticket, item=options.item, owner=options.owner,
                          paths=options.paths)
            pending = Path(options.repository).resolve() / "build" / "claims-pending" / (options.ticket + ".json")
            try:
                pending.parent.mkdir(parents=True, exist_ok=True)
                pending.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
                report["pending_file"] = str(pending)
            except OSError as recording_error:
                report["recording_error"] = str(recording_error)
        emit(report)
        print("claims: coordination unavailable; continue isolated work, report the pending claim, "
              "and inspect status before retrying. This is not a successful registration.", file=sys.stderr)
        return 3
    except (ClaimsError, OSError) as error:
        print(f"claims: {error}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    sys.exit(main())
