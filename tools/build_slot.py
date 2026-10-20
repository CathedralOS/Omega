"""Gate expensive compiler builds to a fixed number of concurrent slots.

One `mbx build -p omega` on a 10-core host drives rustc to roughly 5.4 cores.
The default is two. It was briefly three, to keep the compilation pipe fed when
sixteen lanes queued on the pool, but the host then ran out of MEMORY rather than
CPU: with three concurrent builds plus sixteen lanes, swap reached 2.6 GB of 4 GB,
the kernel logged 764 jetsam kills in twenty minutes, and lanes' records were
OOM-killed mid-run. Two is the setting that keeps swap out of the picture; raise it
only with a measured reason and watch free memory, not just load.
Agent count and build count are separate resources. This gate caps only the
second one, letting many agents edit, author fixtures and run programs while a
small number of builds proceed at full speed.

Usage, wrapping the command rather than replacing it:

    python3 tools/build_slot.py -- mbx build -p omega
    python3 tools/build_slot.py --slots 2 --timeout 3600 -- mbx nextest run -p psi

The slot is held for the child's whole lifetime and released on exit,
including when the child fails or is interrupted. Waiting is bounded and
reported on stderr so a caller can tell "queued" from "hung"; `--timeout 0`
waits indefinitely.

Slots are per host and per checkout root. A slot directory under the shared
build area holds one file per held slot, created with `O_EXCL` so two
processes can never claim the same one; a file whose owning process has died
is reclaimed rather than blocking the pool forever.
"""

import argparse
import contextlib
import errno
import os
from pathlib import Path
import subprocess
import sys
import time

DEFAULT_SLOTS = 2
POLL_SECONDS = 2.0
# How long an EMPTY slot file must exist before reclaim() treats it as a killed
# builder rather than one that is mid-write between O_CREAT and its PID write.
EMPTY_SLOT_GRACE_SECONDS = 10.0


def slot_directory(repository):
    """The pool lives with the other ignored build state, keyed by checkout.

    A shared absolute path would let two unrelated checkouts (a worktree and
    the main checkout, say) fight over one pool; keying by the common git
    directory keeps each checkout's pool separate while still shared by every
    worktree of that checkout.
    """
    done = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd=repository, capture_output=True, text=True, timeout=30)
    root = Path(done.stdout.strip()).parent if not done.returncode else Path(repository)
    directory = root / "build" / "build-slots"
    directory.mkdir(parents=True, exist_ok=True)
    return directory


def process_alive(pid):
    if pid <= 0:
        return False
    if os.name == "nt":
        import ctypes
        kernel32 = ctypes.windll.kernel32
        handle = kernel32.OpenProcess(0x1000, False, int(pid))
        if not handle:
            # ERROR_ACCESS_DENIED (5) means the process exists under another token.
            return kernel32.GetLastError() == 5
        try:
            exit_code = ctypes.c_ulong()
            if not kernel32.GetExitCodeProcess(handle, ctypes.byref(exit_code)):
                return False
            return exit_code.value == 259  # STILL_ACTIVE
        finally:
            kernel32.CloseHandle(handle)
    try:
        os.kill(pid, 0)
    except OSError as error:
        return error.errno == errno.EPERM
    return True


def reclaim(directory):
    """Drop slots whose owner is gone or whose file names no owner at all.

    An empty file is what a builder killed between O_CREAT and writing its PID leaves
    behind; `int(read_text())` raised ValueError on it, the loop skipped it, and the
    slot was never reclaimed -- two of those wedge the pool permanently for every lane,
    because builds are how a lane gets a producer. Unparseable content never names a
    live owner either. An EMPTY file is only reclaimed once it is older than the grace
    window: a builder that is writing right now also reads empty for the microseconds
    between create and write, and reclaiming that would let two builders hold one slot.
    Reclaiming only unlinks; acquire still takes a slot with O_CREAT|O_EXCL, so a racing
    builder loses the retry rather than sharing the slot.
    """
    reclaimed = []
    now = time.time()
    for entry in directory.glob("slot-*"):
        try:
            text = entry.read_text().strip()
        except OSError:
            continue
        if text:
            try:
                pid = int(text)
            except ValueError:
                pid = 0
            if pid > 0 and process_alive(pid):
                continue
        else:
            pid = 0
            try:
                if now - entry.stat().st_mtime < EMPTY_SLOT_GRACE_SECONDS:
                    continue
            except OSError:
                continue
        try:
            entry.unlink()
            reclaimed.append(pid)
        except OSError:
            pass
    return reclaimed


def acquire(directory, slots, timeout):
    """Take one slot, returning (path, waited_seconds). Blocks until a slot frees."""
    started = time.monotonic()
    announced = False
    while True:
        for index in range(slots):
            candidate = directory / f"slot-{index}"
            try:
                handle = os.open(candidate, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            except FileExistsError:
                continue
            except OSError as error:
                if error.errno == errno.EEXIST:
                    continue
                raise
            with os.fdopen(handle, "w") as stream:
                stream.write(str(os.getpid()))
            waited = time.monotonic() - started
            if announced:
                print(f"build_slot: acquired slot {index} after {waited:.0f}s",
                      file=sys.stderr)
            return candidate, waited
        if reclaim(directory) and not announced:
            continue
        waited = time.monotonic() - started
        if timeout and waited >= timeout:
            raise TimeoutError(
                f"no build slot within {timeout}s; {slots} slot(s) are held")
        if not announced:
            print(f"build_slot: waiting for one of {slots} slot(s); "
                  f"held by {sorted(p.name for p in directory.glob('slot-*'))}",
                  file=sys.stderr)
            announced = True
        time.sleep(POLL_SECONDS)


@contextlib.contextmanager
def hold(repository=".", slots=DEFAULT_SLOTS, timeout=3600.0):
    """Acquire one build slot for this block unless an ancestor process already holds one."""
    if os.environ.get("OMEGA_BUILD_SLOT_HELD") == "1":
        yield None, 0.0
        return
    directory = slot_directory(repository)
    slot, waited = acquire(directory, slots, timeout)
    previous = os.environ.get("OMEGA_BUILD_SLOT_HELD")
    os.environ["OMEGA_BUILD_SLOT_HELD"] = "1"
    try:
        yield slot, waited
    finally:
        if previous is None:
            os.environ.pop("OMEGA_BUILD_SLOT_HELD", None)
        else:
            os.environ["OMEGA_BUILD_SLOT_HELD"] = previous
        if slot is not None:
            try:
                slot.unlink()
            except OSError:
                pass


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--slots", type=int, default=DEFAULT_SLOTS)
    parser.add_argument("--timeout", type=float, default=3600.0,
                        help="seconds to wait for a slot; 0 waits indefinitely")
    parser.add_argument("--repository", default=".")
    parser.add_argument("command", nargs=argparse.REMAINDER,
                        help="the command to run; use `--` before it")
    options = parser.parse_args(arguments)
    command = options.command
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        parser.error("no command given; use `-- <command>`")
    if options.slots < 1:
        parser.error("--slots must be at least 1")

    try:
        with hold(options.repository, options.slots, options.timeout):
            return subprocess.run(command, cwd=options.repository).returncode
    except TimeoutError as error:
        print(f"build_slot: {error}", file=sys.stderr)
        return 75


if __name__ == "__main__":
    sys.exit(main())