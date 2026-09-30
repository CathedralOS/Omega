#!/usr/bin/env python3
"""Cross-platform FIFO landing coordination using only Python 3 and Git.

Ready worktrees enqueue, claim the head, integrate, check, and publish. Every
queue mutation compares the prior object; publication updates main and the
queue atomically. Only the coordination ref uses force-with-lease. Waiting is
local and bounded. Each promoted head gets at most three minutes. See landing.md.

After a publication lands, the queue owner label and any `--provenance` text
(or `OMEGA_PROVENANCE`) are recorded as a git note on the published commit
under refs/notes/provenance, so which agent and model produced a change can be
evaluated without putting tool attribution in commit messages.
"""

import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import random
import re
import shutil
import subprocess
import sys
import time
import uuid

# Sibling-script import; both files live in tools/.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import coordination
import trusted_surface_digests

CLAIM_REF = "refs/coordination/omega-landing/main"
MAIN_REF = "refs/heads/main"
PROVENANCE_REF = "refs/notes/provenance"
# Notes commits carry a fixed committer: a lander's own private address would be
# refused by GitHub's email-privacy check and lose the note.
PROVENANCE_COMMITTER = ("-c", "user.name=omega-landing", "-c", "user.email=landing@omega.invalid")
LEASE_SECONDS = 180
TRUSTED_SITES = trusted_surface_digests.SITES_RS.relative_to(
    trusted_surface_digests.REPO_ROOT).as_posix()
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


# Changes to these root files rebuild the world; they name the CLI package
# directly instead of a crate-local manifest.
WORKSPACE_SENTINELS = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
RUST_FILE = re.compile(r"\.rs$|(?:^|/)Cargo\.toml$|(?:^|/)build\.rs$")
PACKAGE_TABLE = re.compile(r"(?ms)^\[package\](.*?)(?=^\[|\Z)")
PACKAGE_NAME = re.compile(r'^\s*name\s*=\s*"([^"]+)"', re.M)
RECEIPT_SCHEMA = "omega-landing-receipt-v1"


def package_name(manifest: Path):
    """The `name = "..."` of the manifest's [package] table, or None for a
    workspace-only manifest. Python 3.9 has no tomllib; the regex reads the
    one field landing needs."""
    try:
        text = manifest.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    table = PACKAGE_TABLE.search(text)
    if not table:
        return None
    name = PACKAGE_NAME.search(table.group(1))
    return name.group(1) if name else None


def affected_packages(repository, base, candidate):
    """Rust packages the base..candidate diff must re-check.

    Each changed .rs/Cargo.toml/build.rs walks up to the first manifest
    with a [package] table (deleted files still walk their parents). Any
    non-empty answer adds `omega`: building the CLI package checks the
    whole product dependency closure, which catches dependents an API
    change broke. Root workspace files short-circuit to it directly.
    """
    changed = coordination.git(repository, "diff", "--name-only", base,
                               candidate).stdout.splitlines()
    packages = set()
    for path in changed:
        if path in WORKSPACE_SENTINELS:
            return {"omega"}
        if not RUST_FILE.search(path):
            continue
        directory = (repository / path).parent
        while directory != repository.parent:
            manifest = directory / "Cargo.toml"
            name = package_name(manifest) if manifest.is_file() else None
            if name:
                packages.add(name)
                break
            if directory == repository:
                break
            directory = directory.parent
    if packages:
        packages.add("omega")
    return packages


def receipt_path(repository, candidate):
    return (Path(repository) / "build" / "landing_receipts"
            / f"{candidate}.json")


def write_receipt(repository, receipt):
    path = receipt_path(repository, receipt["candidate"])
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(receipt, ensure_ascii=True, indent=2) + "\n",
                    encoding="utf-8")
    return path


def verify(repository, base):
    """Build-receipt producer: `check` the affected crates at a clean HEAD.

    Publish cannot afford this — the queue head lease is LEASE_SECONDS —
    so the check runs here, ahead of enqueueing, and publish reads the
    receipt. Prints and writes the receipt; exits 0 only when the check
    did."""
    repository = Path(repository).resolve()
    head = coordination.git(repository, "rev-parse", "HEAD").stdout
    if coordination.git(repository, "status", "--porcelain").stdout:
        raise LandingError("Checkpoint tracked and untracked changes before verifying.")
    # Publish requires base to be an ancestor of the candidate; a
    # non-ancestor base makes the two-dot diff include other lanes' changes,
    # so verify would check (and receipt) crates this candidate never
    # touched. Refuse here with the same rule instead.
    if coordination.git(repository, "merge-base", "--is-ancestor", base, head,
                        allow_failure=True).returncode:
        raise LandingError(f"Base {base[:12]} is not an ancestor of HEAD; "
                           "rebase onto it before verifying.")
    packages = sorted(affected_packages(repository, base, head))
    command = None
    exit_code = 0
    if packages:
        tool = "mbx" if shutil.which("mbx") else "cargo"
        command = [tool, "check"]
        for package in packages:
            command += ["-p", package]
        exit_code = subprocess.run(command, cwd=repository).returncode
    receipt = {"schema": RECEIPT_SCHEMA, "candidate": head, "base": base,
               "packages": packages, "command": command,
               "exit_code": exit_code, "utc": now()}
    write_receipt(repository, receipt)
    emit(receipt)
    return 0 if exit_code == 0 else 1


def receipt_failure(repository, candidate, affected):
    """None when a valid build receipt covers `affected`; else the reason."""
    path = receipt_path(repository, candidate)
    if not path.is_file():
        return f"no build receipt at {path}"
    try:
        receipt = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        return f"unreadable build receipt {path} ({error})"
    if receipt.get("schema") != RECEIPT_SCHEMA:
        return f"build receipt {path} is not {RECEIPT_SCHEMA}"
    if receipt.get("candidate") != candidate:
        return f"build receipt {path} covers {receipt.get('candidate')}, not this candidate"
    if receipt.get("exit_code") != 0:
        return f"build receipt {path} recorded exit {receipt.get('exit_code')}"
    missing = affected - set(receipt.get("packages") or [])
    if missing:
        return (f"build receipt {path} covers {receipt.get('packages')}; "
                f"the candidate also affects {sorted(missing)}")
    return None


class Landing:
    def __init__(self, repository, remote):
        self.repository = Path(repository).resolve()
        self.push_url = coordination.push_url(self.repository, remote)

    def git(self, *arguments, input_text="", allow_failure=False):
        return coordination.git(self.repository, *arguments, input_text=input_text,
                                allow_failure=allow_failure)

    def blob(self, revision, path):
        """Exact bytes of `path` at `revision`, or None when it is absent."""
        result = subprocess.run(
            ["git", "-C", str(self.repository), "cat-file", "blob", f"{revision}:{path}"],
            capture_output=True, env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
        return result.stdout if result.returncode == 0 else None

    def stale_trusted_sites(self, candidate, changed):
        """Pinned trusted-surface files the candidate changes without re-recording.

        `sites.rs` pins a SHA-256 for every bound verifier implementation file,
        and terminal-verifier's `recorded_digests_match_the_working_tree` fails
        when one drifts. A candidate that changes such a file without updating
        its digest landed a verifier change nobody revalidated against the
        ledger entries that cite it, so the queue refuses it at the source.
        Only the candidate's own changes are checked; drift already on main
        does not block unrelated work.
        """
        sites = self.blob(candidate, TRUSTED_SITES)
        if sites is None:
            return []
        recorded = dict(trusted_surface_digests.collect_sites(sites.decode("utf-8")))
        stale = []
        for path in changed:
            digest = recorded.get(path)
            if digest is None:
                continue
            contents = self.blob(candidate, path)
            if contents is None or hashlib.sha256(contents).hexdigest() != digest:
                stale.append(path)
        return stale

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

    def record_provenance(self, candidate, lines):
        """Attach `lines` to `candidate` on PROVENANCE_REF; returns what happened.

        Runs only after main already holds the candidate, as a separate push, so
        it can never block or undo a publication. Concurrent landers race on one
        notes ref: lease on the value read, rebuild the note on the winner, and
        retry a bounded number of times before reporting the note as unrecorded.
        """
        scratch = "refs/notes/landing-provenance-scratch"
        text = "\n".join(lines) + "\n"
        for attempt in range(1, PUSH_FAILURE_LIMIT + 1):
            try:
                remote = coordination.remote_refs(self.repository, self.push_url,
                                                  PROVENANCE_REF).get(PROVENANCE_REF, "")
                if remote:
                    coordination.fetch_object(self.repository, self.push_url, remote)
                    self.git("update-ref", scratch, remote)
                else:
                    self.git("update-ref", "-d", scratch, allow_failure=True)
                self.git(*PROVENANCE_COMMITTER, "notes", "--ref", scratch, "add", "-f",
                         "-F", "-", candidate, input_text=text)
                updated = self.git("rev-parse", scratch).stdout
                pushed = coordination.push_atomic(self.repository, self.push_url,
                                                  [f"{updated}:{PROVENANCE_REF}"],
                                                  lease=(PROVENANCE_REF, remote))
                if not pushed.returncode:
                    return "recorded"
                failure = (pushed.stderr or pushed.stdout).strip()
            except LandingError as error:
                failure = str(error)
            finally:
                self.git("update-ref", "-d", scratch, allow_failure=True)
            pause(attempt, PUSH_FAILURE_BACKOFF_BASE, PUSH_FAILURE_BACKOFF_CAP)
        return f"not recorded: {failure[-300:]}"

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
        stale = self.stale_trusted_sites(options.candidate, changed)
        if stale:
            raise LandingError("The candidate changes trusted-surface implementation sites "
                               "without re-recording their digests: " + ", ".join(stale) +
                               ". Revalidate the ledger entries that cite them, then run "
                               "`python3 tools/trusted_surface_digests.py --write`.")
        if not options.board_update and all(COORDINATION_FILE.match(path)
                                            for path in changed):
            raise LandingError("The candidate touches only coordination files "
                               "(TASKS*.md, OWNER_QUESTIONS.md, tools/swarm/waves/). Worker "
                               "evidence belongs in `claims.py note --ticket <claim>` and the "
                               "session report; a coordinator board sweep passes --board-update.")
        # Publish must not build: the head lease is 180s and a rebuild has
        # already cost a session its reservation. A Rust-touching candidate
        # needs a `verify` receipt for this exact sha instead.
        affected = affected_packages(self.repository, options.base,
                                     options.candidate)
        if affected:
            failure = receipt_failure(self.repository, options.candidate,
                                      affected)
            if failure:
                raise LandingError(
                    f"{failure}; run `python tools/landing.py verify "
                    f"--base {options.base}` at this exact candidate before "
                    "claiming or after rebasing (a rebase makes a new "
                    "candidate), since publish itself never builds inside "
                    "the three-minute lease.")

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
                    owner = next((entry["owner"] for entry in entries if entry["ticket"] == options.claim), "")
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
                if options.command == "publish":
                    provenance = options.provenance or os.environ.get("OMEGA_PROVENANCE", "")
                    result["provenance"] = self.record_provenance(options.candidate, [
                        f"owner: {owner}", *([f"provenance: {provenance}"] if provenance.strip() else []),
                        f"ticket: {options.claim}", f"published_utc: {now()}"])
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
                        choices=("status", "enqueue", "cancel", "claim", "publish", "release", "recover", "verify"))
    parser.add_argument("--repository", default=".")
    parser.add_argument("--remote", default="origin")
    for name in ("owner", "ticket", "claim", "base", "candidate", "reason", "provenance"):
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
        if options.provenance is not None and (options.command != "publish" or len(options.provenance) > 500):
            raise LandingError("--provenance applies only to publish and holds at most 500 characters.")
        if options.command == "enqueue":
            if not options.owner or not options.owner.strip() or len(options.owner) > 200:
                raise LandingError("Supply a short, recognizable --owner.")
            options.ticket = options.ticket or uuid.uuid4().hex
        if options.command in ("enqueue", "claim", "cancel"):
            ticket_id(options.ticket)
        if options.command == "recover" and (not options.reason or not options.reason.strip()):
            raise LandingError("Recovery requires --reason after checking with the owner.")
        if options.command == "verify":
            if not options.base:
                raise LandingError("verify requires --base <full-sha> — the integration base the claim reserved.")
            object_id(options.base, "Base")
            return verify(options.repository, options.base)
        return Landing(options.repository, options.remote).coordinate(options)
    except (LandingError, OSError) as error:
        identity = f" Ticket={options.ticket} Claim={options.claim}" if options else ""
        print(f"landing: {error}{identity}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
