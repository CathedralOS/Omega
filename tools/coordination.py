#!/usr/bin/env python3
"""Shared plumbing for refs/coordination/* compare-and-swap protocols.

claims.py stores its JSON document as the message of an
empty-tree commit on a coordination ref. Every mutation pushes with
--force-with-lease=<ref>:<expected>, so a writer holding a stale snapshot is
fenced and concurrent arrivals retry onto the newest document. Chaining each
mutation as a child commit keeps removed identities permanently invalid.
Nothing here knows a document's shape; each protocol owns its record.
"""

from contextlib import contextmanager
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


def _stop_history_process(process):
    """Settle the owned Git tree, including a transport holding a pipe open."""
    note = ""
    try:
        if os.name == "nt":
            result = subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                                    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                    stderr=subprocess.DEVNULL, timeout=5, check=False)
            if result.returncode:
                note = "Git history descendant cleanup is unconfirmed."
        else:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    except (OSError, subprocess.TimeoutExpired):
        note = "Git history descendant cleanup is unconfirmed."
    finally:
        try:
            process.kill()
        except ProcessLookupError:
            pass
        process.wait()
    return note


@contextmanager
def _history_process(repository, arguments, standard_input, output, deadline):
    # Keep stdout file-backed: on Windows an exited leader's descendant may
    # retain a pipe even after taskkill reports unconfirmed tree cleanup.
    # Regular-file polling below still returns at the shared deadline.
    remaining_seconds(deadline)
    with tempfile.TemporaryFile() as errors:
        process = subprocess.Popen(
            ["git", "-C", str(repository), "-c", "core.deltaBaseCacheLimit=16m",
             *arguments], stdin=standard_input, stdout=output, stderr=errors,
            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
            start_new_session=os.name != "nt", bufsize=0)
        completed = False
        control = {"matched": False}
        try:
            yield process, control
            remaining_seconds(deadline)
            if control["matched"]:
                # Git is still waiting for its next request. Settle its owned
                # tree before closing stdin, while Windows can identify it.
                note = _stop_history_process(process)
                if note:
                    raise CoordinationError(note)
            else:
                code = process.wait(timeout=remaining_seconds(deadline))
                if code:
                    errors.seek(0)
                    raise CoordinationError(f"Git history failed ({code}): "
                                            + errors.read(8192).decode("utf-8", errors="replace"))
            completed = True
        except subprocess.TimeoutExpired as error:
            raise CoordinationTimeout("Coordination deadline exceeded during Git history.") from error
        finally:
            if not completed:
                note = _stop_history_process(process)
                if note:
                    raise CoordinationError(note)
            if process.stdin is not None:
                process.stdin.close()


class _HistoryOutput:
    """Read a growing regular file without blocking on a retained pipe."""
    def __init__(self, stream, process, deadline):
        self.stream, self.process, self.deadline = stream, process, deadline

    def read(self, size):
        while True:
            remaining = remaining_seconds(self.deadline)
            chunk = self.stream.read(size)
            if chunk:
                return chunk
            if self.process.poll() is not None:
                # The producer may have written and exited between read/poll.
                return self.stream.read(size)
            time.sleep(min(0.0005, remaining) if remaining is not None else 0.0005)

    def readline(self, limit):
        line = bytearray()
        while len(line) < limit:
            chunk = self.read(1)
            if not chunk:
                break
            line.extend(chunk)
            if chunk == b"\n":
                break
        return bytes(line)


def _message_matches(stream, size, needle, deadline):
    # Registry snapshots can be megabytes. Retain only a chunk and the overlap
    # needed to recognize a header separator or fixed string across chunks.
    in_message, matched, tail = False, False, b""
    while size:
        remaining_seconds(deadline)
        chunk = stream.read(min(size, 65536))
        if not chunk:
            raise CoordinationError("Truncated Git history object.")
        size -= len(chunk)
        chunk = tail + chunk
        if not in_message:
            separator = chunk.find(b"\n\n")
            if separator < 0:
                tail = chunk[-1:]
                continue
            in_message = True
            chunk = chunk[separator + 2:]
        matched = matched or needle in chunk
        tail = chunk[-(len(needle) - 1):] if len(needle) > 1 else b""
    if not in_message:
        raise CoordinationError("Malformed Git history commit header.")
    return matched


def history_contains_message(repository, tip, text, *, deadline=None):
    """Search all reachable commit messages without retaining snapshot bodies.

    `git log --grep` retains parsed commit buffers during its revision walk;
    full registry snapshots across a long history exhausted a 16 GiB executor.
    Enumerate every reachable parent to disk first, then let a single cat-file
    process stream authenticated commit objects. Only successful full exhaustion
    proves absence. A match can stop early because it forbids ticket reuse.
    """
    object_id(tip, "History tip")
    needle = text.encode("ascii")
    if not needle or b"\n" in needle:
        raise CoordinationError("History search requires one nonempty fixed string.")
    with tempfile.TemporaryDirectory(prefix="coordination-history-") as directory:
        identities = Path(directory) / "commits"
        output_path = Path(directory) / "object"
        with identities.open("wb") as output:
            with _history_process(repository, ["rev-list", tip], subprocess.DEVNULL,
                                  output, deadline) as (process, _):
                process.wait(timeout=remaining_seconds(deadline))
        with identities.open("rb") as expected, output_path.open("w+b", buffering=0) as output, \
                output_path.open("rb", buffering=0) as reader:
            with _history_process(repository, ["cat-file", "--batch"], subprocess.PIPE,
                                  output, deadline) as (process, control):
                stream = _HistoryOutput(reader, process, deadline)
                seen, found = False, False
                for line in expected:
                    identity = line.rstrip(b"\n")
                    object_id(identity.decode("ascii"), "History commit")
                    seen = True
                    # One outstanding request makes a complete frame the writer's
                    # acknowledgment. Rewind only after its delimiter is consumed.
                    output.truncate(0)
                    output.seek(0)
                    reader.seek(0)
                    process.stdin.write(identity + b"\n")
                    process.stdin.flush()
                    header = stream.readline(4097)
                    match = re.fullmatch(rb"([0-9a-f]{40}|[0-9a-f]{64}) commit ([0-9]+)\n", header)
                    if not match or match[1] != identity:
                        raise CoordinationError("Malformed or missing Git history object.")
                    matched = _message_matches(stream, int(match[2]), needle, deadline)
                    if stream.read(1) != b"\n":
                        raise CoordinationError("Truncated Git history object delimiter.")
                    if reader.read(1):
                        raise CoordinationError("Unexpected extra Git history output.")
                    if matched:
                        found = True
                        break
                if found:
                    control["matched"] = True
                else:
                    process.stdin.close()
                    # EOF needs successful Git exit, never an empty spool.
                    process.wait(timeout=remaining_seconds(deadline))
                if not seen or reader.read(1):
                    raise CoordinationError("Incomplete Git history enumeration or extra object output.")
    return found


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
            # Registry commits contain full snapshots. Keep index-pack workers
            # and reused delta bases bounded during first acquisition, rather
            # than multiplying large reconstructed snapshots across workers.
            configuration = (["-c", "pack.threads=2", "-c", "core.deltaBaseCacheLimit=96m"]
                             if arguments and arguments[0] == "fetch" else [])
            process = subprocess.Popen(
                ["git", "-C", str(repository), *configuration, *arguments],
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
        operation = next((argument for argument in arguments
                          if argument in ("fetch", "push", "ls-remote", "log", "cat-file")),
                         arguments[0] if arguments else "command")
        advice = ("A remote mutation may have landed; inspect status before retrying."
                  if operation == "push" else "Inspect status before retrying an uncertain write.")
        raise CoordinationTimeout(
            f"Coordination deadline exceeded during Git {operation}. "
            + advice + getattr(error, "cleanup_note", "")) from error
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
