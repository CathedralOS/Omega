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
import json
import os
from pathlib import Path
import re
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
            result = subprocess.run(
                ["git", "-C", str(repository), *arguments],
                stdin=standard_input, stdout=output, stderr=error_output,
                env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
                timeout=remaining_seconds(deadline))
            output.seek(0)
            error_output.seek(0)
            result.stdout = output.read().decode("utf-8", errors="replace")
            result.stderr = error_output.read().decode("utf-8", errors="replace")
    except subprocess.TimeoutExpired as error:
        raise CoordinationTimeout(
            "Coordination deadline exceeded during Git; a remote mutation may have landed. "
            "Inspect status before retrying.") from error
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


def fetch_object(repository, url, identity, *, deadline=None):
    git(repository, "fetch", "--no-tags", "--no-write-fetch-head", url, identity, deadline=deadline)


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
