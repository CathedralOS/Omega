#!/usr/bin/env python3
"""Start Claude Code cloud sessions with the compiler already built.

A cloud session is a fresh 4-CPU container. Building the compiler from
nothing there takes 12 minutes 27 seconds, and every session used to pay that
before it could run one program. CI on main builds the same compiler on an
identical container and publishes the build directory. A session restores it
and rebuilds only the crates that changed since then.

    python3 tools/warm_build.py publish --worktree W   # CI on main
    python3 tools/warm_build.py restore                # unpack into .agents/worktree/warm
    python3 tools/warm_build.py session-start          # SessionStart hook, cloud only
    python3 tools/warm_build.py wait                   # block until the restore settles

Cargo decides that a crate is fresh by comparing source file times with the
build's. A clone gives every file a new time, so a restored build would look
stale and recompile all 123 workspace crates. The restore therefore checks out
the exact published commit, sets every tracked file to one old time (sound
only because the content is the published content: it refuses a dirty tree),
unpacks the build, then checks out the requested revision. Git rewrites only
the files that differ, those get new times, and Cargo rebuilds exactly their
crates. Measured on base 90f3fe11a: a fresh checkout reusing a build
recompiled 123 crates in 12m11s; with times set this way it recompiled none.

A session should keep the published commit as its base rather than move to
the newest main. Main gains about 65 commits a day, and they often touch the
lowest crates (`terminal-psi`, `typed-trees`), which every other crate depends
on: moving a restored build 15 commits forward (80 minutes of main) still took
11m46s, as long as a cold build. CI publishes after every push, so the
published commit is at most one build behind main, and the merge queue
integrates each change with current main anyway.

The build is published from the same absolute path a session restores into,
`/home/user/Omega/.agents/worktree/warm`. The `compiler` crate's build script
registers every source file by absolute path, so a build from another path
reruns it and recompiles `compiler` and the crates above it (85 s measured).
"""

import argparse
import contextlib
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

REPOSITORY = "CathedralOS/Omega"
TAG = "warm-build-linux-x86_64"
# A local directory or another URL stands in for the release in tests.
SOURCE = os.environ.get("OMEGA_WARM_BUILD_SOURCE",
                        f"https://github.com/{REPOSITORY}/releases/download/{TAG}")
MANIFEST = "warm-build.json"
SCHEMA = "omega-warm-build-v1"
# 2000-01-01T00:00:00Z: older than any build, so restored outputs stay fresh.
OLD_TIME = 946684800
# GitHub refuses release assets of 2 GiB or more.
PART_BYTES = 1900 * 1024 * 1024
MARKER = ".warm-build.json"
EXCLUDED = ("target/cargo-timings", "target/tmp")


class WarmBuildError(RuntimeError):
    pass


def run(*arguments, cwd=None, capture=True):
    result = subprocess.run(arguments, cwd=cwd, text=True,
                            capture_output=capture, stdin=subprocess.DEVNULL)
    if result.returncode:
        detail = (result.stderr or result.stdout or "").strip() if capture else ""
        raise WarmBuildError(f"{' '.join(map(str, arguments))} exited {result.returncode}: {detail}")
    return result.stdout.strip() if capture else ""


def main_checkout(start):
    """The repository's main checkout, wherever `start` sits inside it."""
    common = run("git", "rev-parse", "--path-format=absolute", "--git-common-dir", cwd=start)
    return Path(common).parent


def warm_worktree(root):
    return root / ".agents" / "worktree" / "warm"


def toolchain(worktree):
    """The exact Rust toolchain a build in `worktree` uses; artifacts need it to match."""
    return {"rustc": run("rustc", "-vV", cwd=worktree), "cargo": run("cargo", "-V", cwd=worktree)}


def require_clean(worktree):
    # Untracked build output is fine; any change to tracked content is not,
    # because old file times would then claim a stale build is fresh.
    if run("git", "status", "--porcelain", "--untracked-files=no", cwd=worktree):
        raise WarmBuildError(f"{worktree} has uncommitted changes; restore only into a clean tree")


def age_tracked_files(worktree):
    listed = run("git", "ls-files", "-z", cwd=worktree)
    for name in filter(None, listed.split("\0")):
        path = Path(worktree, name)
        with contextlib.suppress(FileNotFoundError):
            os.utime(path, (OLD_TIME, OLD_TIME), follow_symlinks=False)


def stale_incremental_sessions(worktree):
    """Every incremental session but the newest of each crate: Cargo reads only that one."""
    stale = []
    for crate in Path(worktree, "target/debug/incremental").glob("*"):
        sessions = sorted((path for path in crate.glob("s-*") if path.is_dir()),
                          key=lambda path: path.stat().st_mtime)
        for session in sessions[:-1]:
            stale.append(session)
            lock = session.with_name(session.name + ".lock")
            if lock.exists():
                stale.append(lock)
    return [path.relative_to(worktree).as_posix() for path in stale]


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def pack(worktree, output):
    """Write the compressed build in parts plus its manifest; return the manifest path."""
    worktree = Path(worktree).resolve()
    require_clean(worktree)
    # Incremental compilation is part of each crate's identity. A build made
    # without it (Cargo's default when CI=true) is never reused by a session.
    if not any(Path(worktree, "target/debug/incremental").glob("*/s-*")):
        raise WarmBuildError("the build has no incremental state; build with CARGO_INCREMENTAL=1 "
                             "(Cargo turns it off when CI=true) so sessions can reuse it")
    commit = run("git", "rev-parse", "HEAD", cwd=worktree)
    output.mkdir(parents=True, exist_ok=True)
    prefix = output / f"target-{commit[:12]}.tar.zst."
    with tempfile.NamedTemporaryFile("w", delete=False, suffix=".exclude") as exclusions:
        exclusions.write("\n".join([*EXCLUDED, *stale_incremental_sessions(worktree)]) + "\n")
    try:
        # POSIX (pax) headers keep nanosecond file times; GNU tar's default
        # format rounds them to whole seconds.
        # The tar stream's length is the space the build takes once unpacked;
        # a session checks it has that much room before it starts.
        counted = output / ".tar-bytes"
        command = (f"tar -C {worktree} --format=posix --exclude-from={exclusions.name} -cf - target"
                   f" | tee >(wc -c > {counted}) | zstd -T0 -3 -q"
                   f" | split -b {PART_BYTES} -d -a 2 - {prefix}")
        subprocess.run(["bash", "-o", "pipefail", "-c", command], check=True)
        unpacked_bytes = int(counted.read_text().strip())
        counted.unlink()
    finally:
        os.unlink(exclusions.name)
    parts = sorted(output.glob(prefix.name + "*"))
    manifest = dict(schema=SCHEMA, commit=commit, path=str(worktree), **toolchain(worktree),
                    unpacked_bytes=unpacked_bytes,
                    created=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
                    parts=[dict(name=part.name, bytes=part.stat().st_size, sha256=sha256(part))
                           for part in parts])
    path = output / MANIFEST
    path.write_text(json.dumps(manifest, indent=1) + "\n", encoding="utf-8")
    return path


def upload(manifest_path):
    """Replace the release's build; the manifest goes last so readers never see missing parts."""
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    directory = manifest_path.parent
    try:
        run("gh", "release", "view", TAG, "--repo", REPOSITORY)
    except WarmBuildError:
        run("gh", "release", "create", TAG, "--repo", REPOSITORY, "--prerelease", "--latest=false",
            "--title", "Warm compiler build (linux x86_64)",
            "--notes", "Build cache for cloud sessions, replaced on every main build by "
                       "tools/warm_build.py. Not a product release.")
    names = [part["name"] for part in manifest["parts"]]
    run("gh", "release", "upload", TAG, "--repo", REPOSITORY, "--clobber",
        *[str(directory / name) for name in names], capture=False)
    run("gh", "release", "upload", TAG, "--repo", REPOSITORY, "--clobber", str(manifest_path))
    assets = json.loads(run("gh", "release", "view", TAG, "--repo", REPOSITORY, "--json", "assets"))
    keep = {MANIFEST, *names}
    for asset in assets["assets"]:
        if asset["name"] not in keep:
            run("gh", "release", "delete-asset", TAG, asset["name"], "--repo", REPOSITORY, "--yes")


def fetch(source, name, destination):
    if "://" in source:
        run("curl", "-fsSL", "--retry", "3", "-o", str(destination), f"{source}/{name}")
    else:
        shutil.copyfile(Path(source, name), destination)


# Consecutive interrupted attempts, without new bytes, before a part fails.
PART_ATTEMPTS = 5


def part_blocks(source, name, block_size=1 << 20):
    """Yield one published part's bytes in order, downloading as they are read.

    A cloud session's proxy can drop a long download partway through (seen at
    641 MB of a part), and a part streams straight into zstd, so curl's own
    retry, which restarts from byte zero, cannot help. An interrupted download
    resumes at the next unread byte with a range request instead; curl refuses
    (exit 33) a server that ignores the range, and the caller's digest check
    rejects anything else that went wrong.
    """
    if "://" not in source:
        with open(Path(source, name), "rb") as stream:
            yield from iter(lambda: stream.read(block_size), b"")
        return
    offset = 0
    failures = 0
    while True:
        command = ["curl", "-fsSL", "--retry", "3", f"{source}/{name}"]
        if offset:
            command[1:1] = ["-C", str(offset)]
        download = subprocess.Popen(command, stdout=subprocess.PIPE)
        received = 0
        try:
            for block in iter(lambda: download.stdout.read(block_size), b""):
                received += len(block)
                yield block
        finally:
            download.stdout.close()
            status = download.wait()
        if status == 0:
            return
        offset += received
        failures = 0 if received else failures + 1
        if failures >= PART_ATTEMPTS:
            raise WarmBuildError(f"downloading {name} failed (curl exit {status}) "
                                 f"after {offset} bytes")


def unpack(source, manifest, worktree):
    """Stream every part through its digest check into zstd and tar.

    The parts are never stored: a cloud session has about 18 GB free after the
    image and clone, and keeping 3-5 GB of parts beside the unpacked build is
    what made its first restore fail with tar exit status 2.
    """
    pipeline = subprocess.Popen(
        ["bash", "-o", "pipefail", "-c", f"zstd -d -q -T0 | tar -C {worktree} -xf -"],
        stdin=subprocess.PIPE)
    try:
        for part in manifest["parts"]:
            digest = hashlib.sha256()
            for block in part_blocks(source, part["name"]):
                digest.update(block)
                pipeline.stdin.write(block)
            if digest.hexdigest() != part["sha256"]:
                raise WarmBuildError(f"{part['name']} does not match its published digest")
        pipeline.stdin.close()
        if pipeline.wait():
            raise WarmBuildError(f"unpacking failed (exit {pipeline.returncode}); "
                                 "check free disk space in the worktree's filesystem")
    finally:
        if pipeline.poll() is None:
            pipeline.kill()
            pipeline.wait()


def require_space(manifest, directory):
    """Refuse up front, with numbers, when the unpacked build cannot fit."""
    needed = manifest.get("unpacked_bytes", 0) + (1 << 30)
    free = shutil.disk_usage(directory).free
    if free < needed:
        raise WarmBuildError(f"the build needs {needed / 1e9:.1f} GB free to unpack and "
                             f"{directory} has {free / 1e9:.1f} GB")


def has_commit(cwd, revision):
    try:
        run("git", "cat-file", "-e", f"{revision}^{{commit}}", cwd=cwd)
        return True
    except WarmBuildError:
        return False


def checkout(worktree, revision):
    """Check out `revision`, fetching a remote branch first and a missing commit on demand."""
    if revision.startswith("origin/"):
        run("git", "fetch", "-q", "origin", revision.split("/", 1)[1], cwd=worktree)
    elif not has_commit(worktree, revision):
        run("git", "fetch", "-q", "origin", revision, cwd=worktree)
    run("git", "checkout", "-q", "--detach", revision, cwd=worktree)


def restore(root, worktree=None, source=SOURCE, revision=None):
    """Unpack the published build into `worktree`, then move it to `revision`."""
    worktree = Path(worktree or warm_worktree(root)).resolve()
    # Scratch sits beside the worktree, on its filesystem and outside the main checkout.
    worktree.parent.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix=".warm-build-", dir=worktree.parent))
    try:
        fetch(source, MANIFEST, scratch / MANIFEST)
        manifest = json.loads((scratch / MANIFEST).read_text(encoding="utf-8"))
        if manifest.get("schema") != SCHEMA:
            raise WarmBuildError(f"unknown warm build schema {manifest.get('schema')!r}")
        if worktree.exists():
            require_clean(worktree)
        else:
            worktree.parent.mkdir(parents=True, exist_ok=True)
            if not has_commit(root, manifest["commit"]):
                run("git", "fetch", "-q", "origin", manifest["commit"], cwd=root)
            run("git", "worktree", "add", "-q", "--detach", str(worktree), manifest["commit"], cwd=root)
        checkout(worktree, manifest["commit"])
        local = toolchain(worktree)
        if local["rustc"] != manifest["rustc"]:
            raise WarmBuildError("this toolchain differs from the published build's:\n"
                                 f"{local['rustc']}\nagainst\n{manifest['rustc']}")
        age_tracked_files(worktree)
        shutil.rmtree(worktree / "target", ignore_errors=True)
        require_space(manifest, worktree.parent)
        unpack(source, manifest, worktree)
        (worktree / "target" / MARKER).write_text(json.dumps(manifest, indent=1) + "\n",
                                                   encoding="utf-8")
        if revision:
            checkout(worktree, revision)
        return manifest
    except BaseException:
        # A half-unpacked build must not pass for a complete one.
        if (worktree / "target").exists() and not (worktree / "target" / MARKER).exists():
            shutil.rmtree(worktree / "target", ignore_errors=True)
        raise
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


@contextlib.contextmanager
def held(path):
    import fcntl
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a+") as stream:
        fcntl.flock(stream.fileno(), fcntl.LOCK_EX)
        yield


def status_path(root):
    return warm_worktree(root).with_name("warm.status")


def lock_path(root):
    return warm_worktree(root).with_name("warm.lock")


def session_start(root):
    """Restore the warm worktree once per cloud session; never touch one in use."""
    if os.environ.get("CLAUDE_CODE_REMOTE") != "true":
        return 0
    worktree = warm_worktree(root)
    if (worktree / "target").exists():
        return 0
    status = status_path(root)
    if not shutil.which("zstd"):
        # A fresh container has no package lists, so install alone finds nothing.
        subprocess.run(["apt-get", "update", "-qq"], capture_output=True)
        subprocess.run(["apt-get", "install", "-y", "-qq", "zstd"], capture_output=True)
    if not shutil.which("zstd"):
        # Without it every part fails to unpack as a misleading download error.
        status.write_text("failed: zstd is not installed; run tools/agent_setup.sh\n",
                          encoding="utf-8")
        return 0
    with held(lock_path(root)):
        if (worktree / "target").exists():
            return 0
        status.write_text("restoring\n", encoding="utf-8")
        try:
            # Stay on the published commit: moving to a newer main usually
            # rebuilds nearly everything (see the module comment).
            manifest = restore(root, source=SOURCE)
        except (WarmBuildError, OSError, subprocess.CalledProcessError) as error:
            status.write_text(f"failed: {error}\n", encoding="utf-8")
            return 0
        status.write_text(f"ready: build of {manifest['commit']} in {worktree}\n", encoding="utf-8")
    return 0


def wait(root):
    """Return once no restore is running, and say what it left."""
    with held(lock_path(root)):
        status = status_path(root)
        print(status.read_text(encoding="utf-8").strip() if status.exists() else "no restore ran")
    return 0


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=("publish", "restore", "session-start", "wait"))
    parser.add_argument("--worktree", type=Path, help="defaults to .agents/worktree/warm")
    parser.add_argument("--source", default=SOURCE,
                        help="release download URL or a local directory holding a packed build")
    parser.add_argument("--to", dest="revision", help="revision to check out after restoring")
    parser.add_argument("--output", type=Path, help="publish: pack here and do not upload")
    options = parser.parse_args(arguments)
    try:
        root = main_checkout(Path.cwd())
        if options.command == "publish":
            worktree = options.worktree or warm_worktree(root)
            output = options.output or root / "build" / "warm-build"
            manifest = pack(worktree, output)
            if not options.output:
                upload(manifest)
            print(manifest)
        elif options.command == "restore":
            manifest = restore(root, options.worktree, options.source, options.revision)
            print(f"restored the build of {manifest['commit']}")
        elif options.command == "session-start":
            return session_start(root)
        else:
            return wait(root)
        return 0
    except (WarmBuildError, OSError, subprocess.CalledProcessError) as error:
        print(f"warm_build: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
