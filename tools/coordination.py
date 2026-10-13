#!/usr/bin/env python3
"""Shared plumbing for refs/coordination/* compare-and-swap protocols.

landing.py and claims.py each store a JSON document as the message of an
empty-tree commit on a coordination ref. Every mutation pushes with
--force-with-lease=<ref>:<expected>, so a writer holding a stale snapshot is
fenced and concurrent arrivals retry onto the newest document. Chaining each
mutation as a child commit keeps removed identities permanently invalid.
Nothing here knows a document's shape; each protocol owns its record.
"""

from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time
import uuid


class CoordinationError(Exception):
    pass


class CoordinationTimeout(CoordinationError):
    """The command's wall-clock budget expired; a push may have landed."""


def remaining_seconds(deadline):
    if deadline is None:
        return None
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise CoordinationTimeout("Coordination deadline exceeded; inspect status before retrying a mutation.")
    return remaining


def object_id(value, name):
    if not value or not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", value):
        raise CoordinationError(f"{name} requires a full lowercase Git object ID.")


def now():
    return datetime.now(timezone.utc).isoformat()


def utc(value):
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError("UTC timestamp requires an offset")
    return parsed.astimezone(timezone.utc)


def emit(record):
    print(json.dumps(record, ensure_ascii=True, separators=(",", ":")))


def git(repository, *arguments, input_text="", allow_failure=False, deadline=None):
    try:
        # Git launches transport children. Pipes can keep communicate() waiting
        # after Git is killed on Windows because those children inherit them.
        # File-backed streams let the deadline return without waiting for EOF.
        with tempfile.TemporaryFile() as standard_input, tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as error_output:
            standard_input.write(input_text.encode("utf-8"))
            standard_input.seek(0)
            environment = {**os.environ, "GIT_TERMINAL_PROMPT": "0"}
            if arguments[:2] == ("cat-file", "-e"):
                # An existence probe in a partial clone must remain local.
                # Content reads still acquire promised objects normally: the
                # landing golden-loss gate must observe their actual contents.
                environment["GIT_NO_LAZY_FETCH"] = "1"
            budget = remaining_seconds(deadline)
            # A timed-out fetch can leave index-pack consuming CPU long after
            # coordination returns. Own the transport tree as well as Git, so
            # the deadline also settles its descendants before a retry/build.
            process = subprocess.Popen(
                ["git", "-C", str(repository), *arguments],
                stdin=standard_input, stdout=output, stderr=error_output,
                env=environment,
                start_new_session=os.name != "nt")
            try:
                return_code = process.wait(timeout=budget)
            except subprocess.TimeoutExpired as timeout:
                try:
                    if os.name == "nt":
                        try:
                            cleanup = subprocess.run(
                                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL, timeout=5, check=False)
                            if cleanup.returncode:
                                timeout.cleanup_note = (
                                    f" Transport descendant cleanup is unconfirmed: taskkill exited {cleanup.returncode}.")
                        except (OSError, subprocess.TimeoutExpired):
                            timeout.cleanup_note = " Transport descendant cleanup is unconfirmed: taskkill failed or timed out."
                    else:
                        try:
                            os.killpg(process.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            # The complete process group settled at the deadline.
                            pass
                finally:
                    # Reap the leader even when the platform tree-kill command
                    # fails. Report unconfirmed descendant settlement rather
                    # than treating that fallback as a successful cleanup.
                    try:
                        process.kill()
                    except ProcessLookupError:
                        pass
                    process.wait()
                raise
            result = subprocess.CompletedProcess(process.args, return_code)
            output.seek(0)
            error_output.seek(0)
            result.stdout = output.read().decode("utf-8", errors="replace")
            result.stderr = error_output.read().decode("utf-8", errors="replace")
    except subprocess.TimeoutExpired as error:
        raise CoordinationTimeout(
            "Coordination deadline exceeded during Git; a remote mutation may have landed. "
            "Inspect status before retrying." + getattr(error, "cleanup_note", "")) from error
    result.stdout = result.stdout.rstrip()
    result.stderr = result.stderr.rstrip()
    if result.returncode and not allow_failure:
        raise CoordinationError(f"Git failed ({result.returncode}): {result.stderr} {result.stdout}")
    return result


def push_url(repository, remote, *, deadline=None):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", remote):
        raise CoordinationError("Invalid remote name.")
    repository = Path(repository).resolve()
    git(repository, "rev-parse", "--show-toplevel", deadline=deadline)
    urls = git(repository, "remote", "get-url", "--push", "--all", remote, deadline=deadline).stdout.splitlines()
    if len(urls) != 1 or not urls[0]:
        raise CoordinationError("Exactly one push URL is required.")
    return urls[0]


def remote_refs(repository, url, *references, deadline=None):
    result = git(repository, "ls-remote", "--refs", url, *references, deadline=deadline)
    found = {}
    for line in result.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) != 2:
            raise CoordinationError("Unexpected remote reference response.")
        object_id(parts[0], "Remote reference")
        found[parts[1]] = parts[0]
    return found


def fetch_object(repository, url, identity, *, reference=None, deadline=None):
    # Coordination consumes objects in this repository, never submodule
    # checkouts. Even an already-present empty-tree tip makes Git inspect
    # submodule history by default; in a long-lived registry that exhausted
    # the entire claims deadline. Keep acquisition local to the chosen object
    # store, including when identity is a fallback advertised reference.
    arguments = ["fetch", "--no-tags", "--no-write-fetch-head",
                 "--no-recurse-submodules"]
    if reference:
        cached = git(repository, "rev-parse", "--verify", "--quiet",
                     cache_ref(url, reference), allow_failure=True, deadline=deadline)
        if cached.returncode == 0:
            # Git's default negotiation may offer hundreds of unrelated main
            # commits before this independent history, then download the whole
            # registry. Offer the verified tip of this exact remote/ref instead.
            # Capture its ID so a competing cache writer cannot change the hint
            # between this read and fetch. The remote still supplies identity.
            arguments.append(f"--negotiation-tip={cached.stdout}")
    git(repository, *arguments, url, identity, deadline=deadline)


def cache_ref(url, reference):
    """An opaque local reachability hint, isolated by remote and protocol ref."""
    identity = hashlib.sha256((url + "\0" + reference).encode("utf-8")).hexdigest()
    return f"refs/coordination-cache/{identity}"


def remember_object(repository, url, reference, identity, *, deadline=None):
    """Keep a verified tip reachable for subsequent fetch negotiation.

    Raw-object fetches without FETCH_HEAD leave the independent coordination
    history dangling. Git then walks that whole history on each fresh snapshot.
    This local ref is only a performance hint: competing readers may move it
    backwards or fail to lock it without affecting the remote lease or record.
    The command deadline still bounds this local operation.
    """
    object_id(identity, "Cached coordination tip")
    git(repository, "update-ref", "--no-deref", cache_ref(url, reference), identity,
        allow_failure=True, deadline=deadline)


def commit_record(repository, record, parent, *, deadline=None):
    record.update(updated_utc=now(), nonce=uuid.uuid4().hex)
    tree = git(repository, "mktree", deadline=deadline).stdout
    arguments = ["commit-tree", tree, "-F", "-"]
    if parent:
        arguments.extend(("-p", parent))
    return git(repository, *arguments,
               input_text=json.dumps(record, ensure_ascii=True), deadline=deadline).stdout


def push_atomic(repository, url, refspecs, lease=None, *, deadline=None):
    arguments = ["-c", "push.followTags=false", "push", "--atomic", "--porcelain"]
    if lease:
        arguments.append(f"--force-with-lease={lease[0]}:{lease[1]}")
    return git(repository, *arguments, url, *refspecs, allow_failure=True, deadline=deadline)
