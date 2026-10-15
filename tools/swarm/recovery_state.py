"""Crash-safe local swarm records. OS locks die with their holder, not their PID file.

This is local bookkeeping, never a replacement for claims or landing leases.
Keep one store per executor, shared by its coordinators. A different machine
uses its own store; an absolute worktree path is not a transport identity.
"""

from contextlib import contextmanager
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile


class RecoveryError(Exception):
    pass


@contextmanager
def exclusive(path):
    """Nonblocking, process-scoped lock on Linux/macOS/Windows; never steal."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+b") as stream:
        stream.seek(0, os.SEEK_END)
        if stream.tell() == 0:
            stream.write(b"\0")
            stream.flush()
        stream.seek(0)
        try:
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise RecoveryError("record busy; reread before trying a local update") from error
        try:
            yield
        finally:
            stream.seek(0)
            if os.name == "nt":
                msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(stream.fileno(), fcntl.LOCK_UN)


def atomic_json(path, record, *, indent=2, with_digest=False):
    """Readers see the old or new complete record, including on interruption.

    The temporary is on the same filesystem. A crash may leave a harmless
    temporary; it is never read as committed state. Directory fsync on POSIX
    also persists the rename. Windows supplies atomic replacement, but this
    helper makes no power-loss guarantee for a host's filesystem/controller.
    When requested, return the SHA-256 of those exact persisted bytes only
    after replacement and directory synchronization have succeeded.
    """
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = (json.dumps(record, sort_keys=True, indent=indent, allow_nan=False) + "\n").encode()
    descriptor, temporary = tempfile.mkstemp(prefix="." + path.name + ".", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        if os.name != "nt":
            descriptor = os.open(path.parent, os.O_RDONLY)
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    if with_digest:
        return hashlib.sha256(payload).hexdigest()


def read_json(path):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise RecoveryError("missing or invalid JSON record: " + str(path)) from error


class Store:
    def __init__(self, path):
        self.path = Path(path)

    def load(self):
        record = read_json(self.path)
        if (not isinstance(record, dict) or record.get("schema") != 2
                or type(record.get("revision")) is not int
                or record["revision"] < 0
                or not isinstance(record.get("lanes"), dict)
                or not record.get("executor") or not record.get("generation")):
            raise RecoveryError("unsupported recovery record; preserve it, do not reset")
        return record

    def initialize(self, executor, generation):
        if not executor or not generation:
            raise RecoveryError("executor and generation are required")
        with exclusive(str(self.path) + ".lock"):
            if self.path.exists():
                record = self.load()
                if (record["executor"], record["generation"]) != (executor, generation):
                    raise RecoveryError("store belongs to another executor/generation")
                return record
            record = dict(schema=2, revision=0, executor=executor,
                          generation=generation, lanes={}, build_slot=None,
                          observation=None)
            atomic_json(self.path, record)
            return record

    def update(self, revision, change):
        with exclusive(str(self.path) + ".lock"):
            record = self.load()
            if record["revision"] != revision:
                raise RecoveryError("stale revision; reread and reconcile, do not overwrite")
            return self._commit(record, change)

    def update_guarded(self, guard, change):
        """Merge local evidence only if its exact execution identity still matches."""
        with exclusive(str(self.path) + ".lock"):
            record = self.load()
            if not guard(record):
                raise RecoveryError("execution identity changed; preserve result for reconciliation")
            return self._commit(record, change)

    def _commit(self, record, change):
        updated = copy.deepcopy(record)
        change(updated)
        if updated == record:
            return record
        if updated.get("observation") == record.get("observation"):
            updated["observation"] = None
        updated["revision"] += 1
        atomic_json(self.path, updated)
        return updated
