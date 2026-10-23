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
import qualification
from push_budgets import (COLLISION_BACKOFF_BASE, COLLISION_BACKOFF_CAP,
                        COLLISION_LIMIT, PROBE_ATTEMPTS,
                        PROBE_BACKOFF_SECONDS, PUSH_FAILURE_BACKOFF_BASE,
                        PUSH_FAILURE_BACKOFF_CAP, PUSH_FAILURE_LIMIT)

CLAIM_REF = "refs/coordination/omega-landing/main"
MAIN_REF = "refs/heads/main"
PROVENANCE_REF = "refs/notes/provenance"
# Notes commits carry a fixed committer: a lander's own private address would be
# refused by GitHub's email-privacy check and lose the note.
PROVENANCE_COMMITTER = ("-c", "user.name=omega-landing", "-c", "user.email=landing@omega.invalid")
LEASE_SECONDS = 180
COORDINATION_FILE = re.compile(
    r"^(?:TASKS[^/]*\.md|OWNER_QUESTIONS\.md|tools/swarm/waves/.+)$")
SNAPSHOT_ATTEMPTS = 3
MAINTENANCE_LIMIT = 20

LandingError = coordination.CoordinationError
object_id = coordination.object_id
now = coordination.now
utc = coordination.utc
emit = coordination.emit

def pause(attempt, base, cap, deadline=None):
    """Jittered exponential backoff: the ceiling doubles per attempt, the
    sleep lands in its upper half, and `cap` bounds the wait."""
    delay = min(cap, base * 2 ** (attempt - 1))
    duration = random.uniform(delay / 2, delay)
    remaining = coordination.remaining_seconds(deadline)
    time.sleep(min(duration, remaining) if remaining is not None else duration)

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
RUST_FILE = re.compile(r"\.rs$|(?:^|/)Cargo\.(?:toml|lock)$")
PACKAGE_TABLE = re.compile(r"(?ms)^\[package\](.*?)(?=^\[|\Z)")
PACKAGE_NAME = re.compile(r'^\s*name\s*=\s*"([^"]+)"', re.M)
WORKSPACE_TABLE = re.compile(r"(?m)^\s*\[workspace\]\s*(?:#.*)?$")
RECEIPT_SCHEMA = "omega-landing-receipt-v1"
CORPUS_GOLDEN = re.compile(r"^tests/omega/corpus_[^/]*\.txt$")

def golden_rows(text):
    """The fixture keys a corpus golden records: the first token of each
    data line. `#` comments and tab-indented diagnostics are not rows."""
    rows = set()
    for line in text.splitlines():
        if not line or line[0].isspace():
            continue
        rows.add(line.split()[0])
    return rows

def lost_golden_rows(repository, base, candidate):
    """Corpus golden rows `candidate` silently drops relative to `base`.

    For every tests/omega/corpus_*.txt changed by base..candidate, a row key
    present at base but absent at candidate is a loss only while its fixture
    directory still exists there — a key removed together with
    tests/omega/<key> is a retirement, and a rename retires the old key and
    records a new one. Both the losing commit and the blocked publish can
    then point at the surviving fixture. Pure git reads: it runs inside the
    publish lease as well as ahead of it.
    """
    changed = coordination.git(repository, "diff", "--name-only", base,
                               candidate).stdout.splitlines()
    lost = {}
    for path in changed:
        if not CORPUS_GOLDEN.match(path):
            continue
        before = coordination.git(repository, "show", f"{base}:{path}",
                                  allow_failure=True)
        if before.returncode:
            continue
        after = coordination.git(repository, "show", f"{candidate}:{path}",
                                 allow_failure=True)
        after_rows = golden_rows(after.stdout) if not after.returncode else set()
        for key in sorted(golden_rows(before.stdout) - after_rows):
            if (repository / "tests" / "omega" / key).is_dir():
                lost.setdefault(path, []).append(key)
    return lost

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

def package_workspace(repository, manifest):
    """The enclosing explicit Cargo workspace; '.' selects the repository root.

    Nested workspaces own separate package namespaces, even when a package has
    the same name as one in the root workspace. Route checks through that actual
    manifest instead of interpreting its package name in the root namespace.
    """
    directory = manifest.parent
    while directory != repository:
        workspace = directory / "Cargo.toml"
        if workspace.is_file() and WORKSPACE_TABLE.search(
                workspace.read_text(encoding="utf-8", errors="replace")):
            return workspace.relative_to(repository).as_posix()
        directory = directory.parent
    return "."

def affected_workspaces(repository, base, candidate):
    """Rust packages the base..candidate diff must re-check, by workspace.

    Each changed .rs/Cargo.toml/build.rs walks up to the first manifest
    with a [package] table (deleted files still walk their parents). Any
    non-empty answer adds `omega`: building the CLI package checks the
    whole product dependency closure, which catches dependents an API
    change broke. Root workspace files select it without dropping independent
    workspace changes elsewhere in the same candidate. Nested lockfiles select
    their owning package just like their manifests and Rust sources.
    """
    changed = coordination.git(repository, "diff", "--name-only", base,
                               candidate).stdout.splitlines()
    workspaces = {}
    for path in changed:
        if path in WORKSPACE_SENTINELS:
            workspaces.setdefault(".", set()).add("omega")
            continue
        if not RUST_FILE.search(path):
            continue
        directory = (repository / path).parent
        while directory != repository.parent:
            manifest = directory / "Cargo.toml"
            name = package_name(manifest) if manifest.is_file() else None
            if name:
                workspace = package_workspace(repository, manifest)
                workspaces.setdefault(workspace, set()).add(name)
                break
            if directory == repository:
                break
            directory = directory.parent
    if workspaces:
        workspaces.setdefault(".", set()).add("omega")
    return workspaces

def affected_packages(repository, base, candidate):
    """Flat package coverage retained for existing receipt readers."""
    return {package for packages in affected_workspaces(repository, base, candidate).values()
            for package in packages}

# A commit that states its own validation or review is still outstanding is
# not ready for main. On 2026-10-10 two such landings each broke programs that
# ran before them: 20d5a856716 ("held for parent-arranged independent review;
# not published") and 17a82816ca3 ("final exact-commit validation and
# independent review remain pending").
PENDING_DECLARATION = re.compile(
    r"(?i)\b(?:not published|review (?:remains? |is )?pending|"
    r"validation[^.]{0,60}pending|held for [^.]{0,60}review)\b")

def declared_pending(repository, base, candidate):
    """(commit, phrase) for each commit in base..candidate whose message says
    its validation or review is still pending."""
    log = coordination.git(repository, "log", "--format=%H%x00%B%x01",
                           f"{base}..{candidate}").stdout
    pending = []
    for entry in log.split("\x01"):
        commit, _, body = entry.strip().partition("\x00")
        match = PENDING_DECLARATION.search(body)
        if commit and match:
            pending.append((commit[:12], match.group(0)))
    return pending

def windows_safe_target_kwargs(repository):
    """Keep Cargo target paths short on Windows inside deep worktree paths so
    build scripts and crates stay below MSVC link.exe's 260-char MAX_PATH."""
    configured = Path(os.environ.get("CARGO_TARGET_DIR") or (Path(repository) / "target"))
    if os.name != "nt" or len(str(configured)) <= 100:
        return {}
    import hashlib
    import tempfile
    digest = hashlib.sha256(str(configured.resolve()).encode("utf-8")).hexdigest()[:10]
    base = Path(os.environ.get("SYSTEMDRIVE", "C:") + "\\tmp")
    try:
        base.mkdir(parents=True, exist_ok=True)
    except OSError:
        base = Path(tempfile.gettempdir())
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(base / f"omega-tgt-{digest}")
    return {"env": env}

def intermediate_checks(repository, base, head, tool):
    """Check every commit before `head` in base..head on its own.

    Main is bisected one commit at a time, so a commit that compiles only
    with its successor breaks every later bisect over it (3469d8ce0ff and
    17a82816ca3 did). Each intermediate commit is checked for the packages
    changed since `base` up to it, in this clean worktree, which is returned
    to `head` afterwards."""
    commits = coordination.git(repository, "rev-list", "--reverse",
                               f"{base}..{head}").stdout.split()
    results = []
    if len(commits) < 2:
        return results
    branch = coordination.git(repository, "symbolic-ref", "-q", "--short", "HEAD",
                              allow_failure=True).stdout.strip()
    target_kwargs = windows_safe_target_kwargs(repository)
    try:
        for commit in commits[:-1]:
            workspaces = affected_workspaces(repository, base, commit)
            if not workspaces:
                continue
            coordination.git(repository, "checkout", "-q", "--detach", commit)
            for workspace, names in sorted(workspaces.items()):
                command = [tool, "check"]
                if workspace != ".":
                    command += ["--manifest-path", str(repository / workspace)]
                for package in sorted(names):
                    command += ["-p", package]
                exit_code = subprocess.run(command, cwd=repository, **target_kwargs).returncode
                results.append({"commit": commit, "workspace": workspace,
                                "packages": sorted(names), "exit_code": exit_code})
    finally:
        if branch:
            coordination.git(repository, "checkout", "-q", branch)
        else:
            coordination.git(repository, "checkout", "-q", "--detach", head)
    return results

def landed_worktree_cleanup(repository, candidate):
    """Remove the landed lane's own worktree and candidate branch.

    AGENTS.md makes this a hard rule on the publishing session: once a publish
    completes, the worktree and its candidate branch are dead bookkeeping, not
    a record. Nothing enforced it, so landed lanes accumulated while each
    session relied on remembering. Called here, after main is confirmed to
    hold `candidate`, so the rule is self-enforcing.

    Deliberately conservative: this runs only in the publishing worktree, and
    only when that worktree is provably removable. It refuses, returning a
    reason, whenever anything could still be lost:

    - the publish did not come from a linked worktree (a checkout on main has
      no candidate branch to retire, and removing it would be catastrophic);
    - the worktree has uncommitted or untracked changes, which are not
      recorded anywhere and would be destroyed;
    - HEAD is not `candidate`, or it carries commits main does not hold;
    - the branch is the repository's main line.

    The main checkout is never a candidate: `git worktree list` marks it with
    no branch-checkout entry for main, and its path is the common directory's
    parent. Returns a dict describing what happened; never raises, because a
    cleanup that fails must not imply the publication failed.
    """
    def git(*arguments, cwd=None):
        return subprocess.run(["git", *arguments], cwd=cwd or repository,
                              capture_output=True, text=True, timeout=30)

    try:
        common = git("rev-parse", "--path-format=absolute", "--git-common-dir")
        if common.returncode:
            return {"removed": False, "reason": "cannot resolve the common git directory"}
        main_checkout = Path(common.stdout.strip()).parent.resolve()
        current = Path(repository).resolve()
        if current == main_checkout:
            return {"removed": False, "reason": "publishing from the main checkout, not a lane"}

        branch = git("symbolic-ref", "-q", "--short", "HEAD")
        if branch.returncode or not branch.stdout.strip():
            return {"removed": False, "reason": "detached HEAD has no candidate branch to retire"}
        branch = branch.stdout.strip()
        if branch in ("main", "master"):
            return {"removed": False, "reason": f"HEAD is {branch}, the main line"}

        head = git("rev-parse", "HEAD")
        # `candidate=None` is the prune path: it retires a lane that was never
        # built and holds no work, so any HEAD is acceptable as long as the
        # checks below still prove nothing is being lost.
        if candidate is not None and (head.returncode or head.stdout.strip() != candidate):
            return {"removed": False,
                    "reason": "worktree HEAD is not the published candidate"}

        status = git("status", "--porcelain")
        if status.returncode or status.stdout.strip():
            return {"removed": False,
                    "reason": "worktree has uncommitted or untracked changes"}

        # Anything main lacks would be unpublished work, whatever the indices say.
        # The prune path compares against main instead of a candidate.
        reference = candidate if candidate is not None else "main"
        ahead = git("rev-list", "--count", f"{reference}..HEAD")
        if ahead.returncode or ahead.stdout.strip() != "0":
            return {"removed": False, "reason": "worktree holds commits beyond the candidate"}

        # `git worktree remove` refuses a dirty tree, which the checks above
        # already excluded. It also refuses a directory that still holds
        # ignored files, which is the normal state for a lane that has ever
        # built: on macOS a stray .DS_Store is enough to leave the directory
        # non-empty and fail the removal with "Directory not empty". Because
        # `git status --porcelain` reports only tracked and untracked files,
        # that refusal is invisible to the safety checks above and would
        # otherwise make automatic cleanup useless on this host, which is the
        # host that records the macOS golden.
        #
        # Retrying with --force is safe only here, after every condition that
        # could destroy unrecorded work has already been rejected: the tree is
        # clean, HEAD is exactly the published candidate, and the branch holds
        # nothing main lacks. --force discards exactly the ignored build output
        # that made the clean removal fail.
        # The publish command runs with its worktree as the current directory, so the
        # removal can unregister the worktree and then fail because its own cwd
        # disappeared mid-command. That leaves the tree already retired: the
        # real question is whether the directory is gone and the registration is
        # clear, not what this one invocation reported. Re-check both instead of
        # returning on the return code, or a lane is left unregistered with its
        # candidate branch still present, which is the bookkeeping the rule
        # exists to remove.
        removal = git("worktree", "remove", str(current))
        forced = False
        if removal.returncode:
            removal = git("worktree", "remove", "--force", str(current))
            forced = not removal.returncode

        # A worktree is retired once Git no longer registers it. Read that from
        # the registry rather than from either command's exit status.
        listing = git("worktree", "list", "--porcelain", cwd=str(main_checkout))
        registered = listing.returncode == 0 and str(current) in listing.stdout
        if removal.returncode and registered:
            return {"removed": False,
                    "reason": f"worktree removal refused: {removal.stderr.strip()}"}
        if removal.returncode:
            # Already unregistered by the attempt above; the directory is the
            # only thing left to clear.
            forced = True

        # Clear anything the removal left behind, including ignored build
        # output. Safe because the directory was proven to be a clean lane
        # above, so nothing unrecorded can be inside it.
        leftover = None
        if current.exists():
            shutil.rmtree(current, ignore_errors=True)
            if current.exists():
                leftover = sorted(path.name for path in current.iterdir())[:5]

        # The worktree directory is gone now, so every later command must run
        # from the main checkout; `repository` no longer exists as a cwd.
        deletion = git("branch", "-D", branch, cwd=str(main_checkout))
        if deletion.returncode:
            # The worktree is gone; the branch is now only bookkeeping, but say
            # so rather than reporting a clean sweep.
            return {"removed": True, "branch": branch, "forced": forced,
                    "reason": f"worktree removed; branch deletion failed: {deletion.stderr.strip()}"}
        result = {"removed": True, "branch": branch, "worktree": str(current)}
        if forced:
            result["forced"] = True
        if leftover:
            # The worktree is unregistered and the branch is gone, so the lane
            # is retired; only ignored junk resisted deletion.
            result["reason"] = ("worktree removed and branch deleted; "
                                f"leftover files resisted deletion: {', '.join(leftover)}")
        return result
    except (OSError, subprocess.SubprocessError) as error:
        return {"removed": False, "reason": f"cleanup inspection failed: {type(error).__name__}"}

def prune_worktrees(repository, idle_hours=0.0, dry_run=False):
    """Retire worktrees that waste a build slot without holding warm work.

    Keeping lanes is the reuse path: a worktree owns a managed target that a
    cold rebuild pays minutes to recreate, while reusing it is seconds. So a
    lane that has ever been built is worth keeping even after it lands. What
    is not worth keeping is a lane that can never pay that back:

    - an unregistered or missing directory, which is nothing but stale
      bookkeeping in `git worktree list`;
    - a registered lane whose managed target is empty and which has been idle
      past `idle_hours`, since it has never been built and holds no work.

    A lane holding uncommitted changes, or commits main lacks, is never a
    candidate: that is unpublished work, not a spare slot. Removal reuses the
    staged checks in `landed_worktree_cleanup`, so the same refusals apply.
    Returns one record per worktree inspected; `--dry-run` reports without
    removing.
    """
    listing = subprocess.run(["git", "worktree", "list", "--porcelain"],
                             cwd=repository, capture_output=True, text=True, timeout=30)
    if listing.returncode:
        raise LandingError(f"cannot list worktrees: {listing.stderr.strip()}")
    common = subprocess.run(["git", "rev-parse", "--path-format=absolute",
                             "--git-common-dir"], cwd=repository,
                            capture_output=True, text=True, timeout=30)
    main_checkout = Path(common.stdout.strip()).parent.resolve() if not common.returncode else None

    entries, current = [], {}
    for line in listing.stdout.splitlines() + [""]:
        if line.startswith("worktree "):
            if current:
                entries.append(current)
            current = {"path": line.split(" ", 1)[1].strip()}
        elif line.startswith("branch ") and current:
            current["branch"] = line.split(" ", 1)[1].strip().removeprefix("refs/heads/")
        elif line.startswith("detached") and current:
            current["detached"] = True
    if current:
        entries.append(current)

    results = []
    for entry in entries:
        path = Path(entry["path"]).resolve()
        if main_checkout is not None and path == main_checkout:
            continue
        record = {"worktree": str(path), "removed": False}
        if not path.exists():
            record["reason"] = "missing directory; prune the registration"
            if not dry_run:
                subprocess.run(["git", "worktree", "prune"], cwd=repository,
                               capture_output=True, text=True, timeout=30)
                record["removed"] = True
            results.append(record)
            continue
        status = subprocess.run(["git", "status", "--porcelain"], cwd=path,
                                capture_output=True, text=True, timeout=30)
        if status.returncode or status.stdout.strip():
            record["reason"] = "holds uncommitted or untracked changes"
            results.append(record); continue
        if entry.get("branch"):
            ahead = subprocess.run(["git", "rev-list", "--count",
                                    f"main..{entry['branch']}"], cwd=repository,
                                   capture_output=True, text=True, timeout=30)
            if ahead.stdout.strip() not in ("", "0"):
                record["reason"] = "holds commits main lacks"
                results.append(record); continue
        # A managed target is where a warm build lives; report its presence so
        # the operator can see which lanes are expensive to recreate.
        warm = None
        for candidate in ("target", "build/gate-cli"):
            directory = path / candidate
            if directory.is_dir() and any(directory.iterdir()):
                warm = candidate; break
        record["warm_target"] = warm
        if warm:
            record["reason"] = "warm target retained for reuse"
            results.append(record); continue
        if idle_hours > 0:
            touched = max((p.stat().st_mtime for p in path.iterdir()), default=0)
            idle = (time.time() - touched) / 3600 if touched else 0.0
            record["idle_hours"] = round(idle, 2)
            if idle < idle_hours:
                record["reason"] = f"idle {idle:.2f}h, below the {idle_hours}h threshold"
                results.append(record); continue
        record["reason"] = "never built and not needed"
        if not dry_run:
            outcome = landed_worktree_cleanup(str(path), None)
            record.update(outcome)
        results.append(record)
    return results

def receipt_path(repository, candidate):
    return (Path(repository) / "build" / "landing_receipts"
            / f"{candidate}.json")

def write_receipt(repository, receipt):
    path = receipt_path(repository, receipt["candidate"])
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(receipt, ensure_ascii=True, indent=2) + "\n",
                    encoding="utf-8")
    return path

def reusable_build_receipt(repository, base, head, workspaces, packages):
    """Reuse a prior passing build receipt in this worktree when rebasing
    onto a new base leaves all Rust/Cargo/toolchain inputs unchanged."""
    directory = Path(repository) / "build" / "landing_receipts"
    if not directory.is_dir():
        return None
    commits = coordination.git(repository, "rev-list", "--reverse",
                               f"{base}..{head}").stdout.split()
    candidates = sorted(directory.glob("*.json"),
                        key=lambda path: path.stat().st_mtime, reverse=True)
    for path in candidates:
        try:
            prior = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if (not isinstance(prior, dict)
                or prior.get("schema") != RECEIPT_SCHEMA
                or prior.get("exit_code") != 0):
            continue
        prior_candidate = prior.get("candidate")
        prior_base = prior.get("base")
        if (not isinstance(prior_candidate, str)
                or not isinstance(prior_base, str)
                or prior_candidate == head):
            continue
        if (coordination.git(repository, "cat-file", "-e",
                             f"{prior_candidate}^{{commit}}",
                             allow_failure=True).returncode
                or coordination.git(repository, "cat-file", "-e",
                                    f"{prior_base}^{{commit}}",
                                    allow_failure=True).returncode):
            continue
        # The old build belongs to immutable Rust inputs, not the mutable
        # qualification state that has since advanced to this candidate.
        # verify validates current qualification before and after this reuse;
        # publication still uses the full receipt validator below.
        if build_receipt_failure(path, prior, prior_candidate, set(packages),
                                 workspaces=workspaces, base=prior_base) is not None:
            continue
        if (affected_workspaces(repository, prior_base, base)
                or affected_workspaces(repository, prior_candidate, head)):
            continue
        prior_commits = coordination.git(
            repository, "rev-list", "--reverse",
            f"{prior_base}..{prior_candidate}", allow_failure=True
        ).stdout.split()
        if len(prior_commits) != len(commits):
            continue
        if any(affected_workspaces(repository, old_commit, new_commit)
               for old_commit, new_commit in zip(prior_commits, commits)):
            continue
        return prior
    return None

def verify(repository, base, qualification_path=None):
    """Build-receipt producer: `check` the affected crates at a clean HEAD.

    Publish cannot afford this — the queue head lease is LEASE_SECONDS —
    so the check runs here, ahead of enqueueing, and publish reads the
    receipt. Prints and writes the receipt; exits 0 only when the check
    did."""
    started = time.monotonic()
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
    qualification_receipt = None
    try:
        active_qualification = qualification.active_path(repository)
    except qualification.RecoveryError as error:
        raise LandingError(str(error)) from error
    if qualification_path and active_qualification:
        if Path(qualification_path).resolve() != Path(active_qualification).resolve():
            raise LandingError("verify must use this worktree's active qualification")
    qualification_path = qualification_path or active_qualification
    if qualification_path:
        try:
            qualification_receipt = qualification.validate(repository, qualification_path, head, base)
        except qualification.RecoveryError as error:
            raise LandingError(str(error)) from error
    workspaces = affected_workspaces(repository, base, head)
    packages = sorted({package for names in workspaces.values() for package in names})
    command = None
    checks = []
    intermediate = []
    reused_from = None
    exit_code = 0
    if packages:
        reused = reusable_build_receipt(repository, base, head, workspaces, packages)
        if reused is not None:
            command = reused.get("command")
            checks = reused.get("checks", [])
            intermediate = reused.get("intermediate", [])
            reused_from = reused.get("candidate")
        else:
            import build_slot
            tool = "mbx" if shutil.which("mbx") else "cargo"
            target_kwargs = windows_safe_target_kwargs(repository)
            with build_slot.hold(repository):
                for workspace, names in sorted(workspaces.items()):
                    check_command = [tool, "check"]
                    if workspace != ".":
                        check_command += ["--manifest-path", str(repository / workspace)]
                    for package in sorted(names):
                        check_command += ["-p", package]
                    check_started = time.monotonic()
                    checked_exit = subprocess.run(check_command, cwd=repository, **target_kwargs).returncode
                    checks.append({"workspace": workspace, "packages": sorted(names),
                                   "command": check_command, "exit_code": checked_exit,
                                   "duration_seconds": time.monotonic() - check_started})
                    if command is None:
                        command = check_command
                    if checked_exit and not exit_code:
                        exit_code = checked_exit
                intermediate = intermediate_checks(repository, base, head, tool)
    broken = sorted({check["commit"][:12] for check in intermediate if check["exit_code"]})
    if broken:
        exit_code = exit_code or 1
        print("landing: these commits do not build on their own; squash each into "
              f"the commit that completes it: {', '.join(broken)}", file=sys.stderr)
    pending = declared_pending(repository, base, head)
    if pending:
        exit_code = exit_code or 1
        for commit, phrase in pending:
            print(f"landing: commit {commit} says \"{phrase}\"; finish that "
                  "validation or review before landing", file=sys.stderr)
    # A golden corpus row can only vanish through a record/history accident;
    # an intended retirement deletes the fixture with it, which
    # lost_golden_rows does not flag. Block the publish here, before the
    # rework cycle, rather than rediscovering the loss in a bisect.
    lost = lost_golden_rows(repository, base, head)
    if lost:
        exit_code = 1
        print("landing: candidate loses golden corpus rows whose fixtures "
              "still exist:", file=sys.stderr)
        for path, keys in lost.items():
            for key in keys:
                print(f"landing:   {path}: {key}", file=sys.stderr)
    receipt = {"schema": RECEIPT_SCHEMA, "candidate": head, "base": base,
               "packages": packages, "command": command,
               "checks": checks, "intermediate": intermediate,
               "pending": [list(item) for item in pending],
               "exit_code": exit_code, "utc": now(),
               "duration_seconds": time.monotonic() - started}
    if reused_from:
        receipt["reused_build_from"] = reused_from
    if qualification_receipt:
        # Mandatory build checks still ran for this exact HEAD. Rejoin the
        # qualification again afterwards: a concurrently replaced compiler or
        # library cannot inherit acceptance from the pre-build observation.
        try:
            receipt["qualification"] = qualification.validate(repository, qualification_path, head, base)
        except qualification.RecoveryError as error:
            raise LandingError(str(error)) from error
    if lost:
        receipt["lost_rows"] = lost
    write_receipt(repository, receipt)
    emit(receipt)
    return 0 if exit_code == 0 else 1

def build_receipt_failure(path, receipt, candidate, affected, workspaces=None, base=None):
    """Validate build facts; current qualification is a separate obligation."""
    if receipt.get("schema") != RECEIPT_SCHEMA:
        return f"build receipt {path} is not {RECEIPT_SCHEMA}"
    if receipt.get("candidate") != candidate:
        return f"build receipt {path} covers {receipt.get('candidate')}, not this candidate"
    if base is not None and receipt.get("base") != base:
        return "build receipt integration base differs from the reserved publication base"
    if receipt.get("lost_rows"):
        return (f"build receipt {path} recorded lost corpus rows "
                f"{receipt['lost_rows']}")
    if receipt.get("exit_code") != 0:
        return f"build receipt {path} recorded exit {receipt.get('exit_code')}"
    broken = [check["commit"][:12] for check in receipt.get("intermediate", [])
              if check.get("exit_code")]
    if broken:
        return f"build receipt {path} recorded commits that do not build alone: {broken}"
    missing = affected - set(receipt.get("packages") or [])
    if missing:
        return (f"build receipt {path} covers {receipt.get('packages')}; "
                f"the candidate also affects {sorted(missing)}")
    # Legacy root-only receipts remain valid. A standalone namespace requires
    # its own successful command record; flat names cannot prove that coverage.
    for workspace, packages in (workspaces or {}).items():
        if workspace == ".":
            continue
        if not any(check.get("workspace") == workspace
                   and check.get("exit_code") == 0
                   and packages <= set(check.get("packages") or [])
                   for check in receipt.get("checks", [])):
            return f"build receipt {path} lacks a passing check for workspace {workspace}"
    return None


def receipt_failure(repository, candidate, affected, workspaces=None, base=None, qualification_path=None):
    """Validate the exact build receipt and its current qualification binding."""
    path = receipt_path(repository, candidate)
    if not path.is_file():
        return f"no build receipt at {path}"
    try:
        receipt = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        return f"unreadable build receipt {path} ({error})"
    failure = build_receipt_failure(path, receipt, candidate, affected, workspaces, base)
    if failure is not None:
        return failure
    if qualification_path and (not receipt.get("qualification")
            or Path(receipt["qualification"].get("path", "")).resolve() != Path(qualification_path).resolve()):
        return "build receipt qualification differs from the active worktree registration"
    if receipt.get("qualification"):
        qualified = receipt["qualification"]
        try:
            current = qualification.validate(repository, qualified["path"], candidate, receipt["base"])
            if current != qualified:
                return "qualification changed since exact-candidate verification"
        except (qualification.RecoveryError, OSError, KeyError, ValueError) as error:
            return "qualification no longer valid: " + str(error)
    return None

class Landing:
    def __init__(self, repository, remote, deadline=None):
        # Setup belongs to the same bounded operation as acquisition. Once the
        # head is observed, its original absolute expiry further narrows it.
        self.deadline = deadline
        self.repository = Path(repository).resolve()
        self.push_url = coordination.push_url(self.repository, remote, deadline=self.deadline)

    def git(self, *arguments, input_text="", allow_failure=False):
        return coordination.git(self.repository, *arguments, input_text=input_text,
                                allow_failure=allow_failure, deadline=getattr(self, "deadline", None))

    def references(self):
        references = coordination.remote_refs(self.repository, self.push_url,
                                              MAIN_REF, CLAIM_REF, deadline=getattr(self, "deadline", None))
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
        # Queue mutations already created their tip locally. Refetching that
        # immutable object needlessly negotiates every reachable history; a
        # large claims cache can exhaust memory before the landing lease starts.
        if not self.git("cat-file", "-e", version, allow_failure=True).returncode:
            coordination.remember_object(self.repository, self.push_url,
                                         reference, version, deadline=getattr(self, "deadline", None))
            return
        try:
            coordination.fetch_object(self.repository, self.push_url, version,
                                      reference=reference, deadline=getattr(self, "deadline", None))
        except coordination.CoordinationTimeout:
            raise
        except LandingError:
            pass
        else:
            if not self.git("cat-file", "-e", version,
                            allow_failure=True).returncode:
                coordination.remember_object(self.repository, self.push_url,
                                             reference, version, deadline=getattr(self, "deadline", None))
                return
        coordination.fetch_object(self.repository, self.push_url, reference,
                                  reference=reference, deadline=getattr(self, "deadline", None))
        if self.git("cat-file", "-e", version, allow_failure=True).returncode:
            raise LandingError(f"Remote tip {version} is still missing after "
                               f"fetching {reference}; inspect the remote "
                               "object store before retrying.")
        coordination.remember_object(self.repository, self.push_url,
                                     reference, version, deadline=getattr(self, "deadline", None))

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
        return coordination.commit_record(self.repository, record, parent,
                                          deadline=getattr(self, "deadline", None))

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
            except coordination.CoordinationTimeout:
                raise
            except LandingError as error:
                attempts += 1
                if attempts >= PROBE_ATTEMPTS:
                    raise LandingError(
                        "Cannot read the coordination refs to confirm a push "
                        "outcome; the result is uncertain. Inspect status "
                        "before retrying.") from error
                pause(1, PROBE_BACKOFF_SECONDS, PROBE_BACKOFF_SECONDS,
                      getattr(self, "deadline", None))

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
                lease=(CLAIM_REF, expected), deadline=getattr(self, "deadline", None))
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
                  PUSH_FAILURE_BACKOFF_CAP, getattr(self, "deadline", None))

    def record_provenance(self, candidate, lines):
        """Attach `lines` to `candidate` on PROVENANCE_REF; returns what happened.

        Runs only after main already holds the candidate, as a separate push, so
        it can never block or undo a publication. Concurrent landers race on one
        notes ref: lease on the value read, rebuild the note on the winner, and
        retry a bounded number of times before reporting the note as unrecorded.
        """
        # Worktrees share local refs too. A peer can otherwise replace this
        # tip between note creation and publication, or delete it in cleanup.
        # Keep the legacy name available for clients already publishing.
        scratch = f"refs/notes/landing-provenance-scratch-{uuid.uuid4().hex}"
        text = "\n".join(lines) + "\n"
        for attempt in range(1, PUSH_FAILURE_LIMIT + 1):
            try:
                remote = coordination.remote_refs(self.repository, self.push_url,
                                                  PROVENANCE_REF, deadline=getattr(self, "deadline", None)).get(PROVENANCE_REF, "")
                if remote:
                    coordination.fetch_object(self.repository, self.push_url, remote,
                                              deadline=getattr(self, "deadline", None))
                    self.git("update-ref", scratch, remote)
                else:
                    self.git("update-ref", "-d", scratch, allow_failure=True)
                self.git(*PROVENANCE_COMMITTER, "notes", "--ref", scratch, "add", "-f",
                         "-F", "-", candidate, input_text=text)
                updated = self.git("rev-parse", scratch).stdout
                pushed = coordination.push_atomic(self.repository, self.push_url,
                                                  [f"{updated}:{PROVENANCE_REF}"],
                                                  lease=(PROVENANCE_REF, remote), deadline=getattr(self, "deadline", None))
                if not pushed.returncode:
                    return "recorded"
                failure = (pushed.stderr or pushed.stdout).strip()
            except OSError as error:
                # Disk-full/tempfile failures belong to this advisory write,
                # not the already confirmed main push. Retrying cannot free
                # local storage; report the missing note without relanding.
                return f"not recorded: {str(error)[-300:]}"
            except LandingError as error:
                failure = str(error)
            finally:
                try:
                    self.git("update-ref", "-d", scratch, allow_failure=True)
                except (LandingError, OSError):
                    # Main is already published. An expired advisory budget
                    # or filesystem failure may leave a uniquely named local
                    # scratch ref, never undo a confirmed main or notes push.
                    pass
            try:
                pause(attempt, PUSH_FAILURE_BACKOFF_BASE, PUSH_FAILURE_BACKOFF_CAP,
                      getattr(self, "deadline", None))
            except coordination.CoordinationTimeout:
                break
        return f"not recorded: {failure[-300:]}"

    def back_off_after_collision(self, collisions):
        """Bounded patience for lost push races, then an honest stop."""
        if collisions >= COLLISION_LIMIT:
            raise LandingError(
                "Update kept losing push races to concurrent writers; "
                "inspect status and main before retrying.")
        pause(collisions, COLLISION_BACKOFF_BASE, COLLISION_BACKOFF_CAP,
              getattr(self, "deadline", None))

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
        pending = declared_pending(self.repository, options.base, options.candidate)
        if pending:
            commit, phrase = pending[0]
            raise LandingError(
                f"Commit {commit} says \"{phrase}\". Finish that validation or "
                "review, then reword the commit before landing.")
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
        # The lost-row gate re-runs inside the lease: it is a few git
        # reads, so publish blocks a row-losing candidate even when its
        # receipt predates this check.
        lost = lost_golden_rows(self.repository, options.base,
                                options.candidate)
        if lost:
            keys = sum(lost.values(), [])
            raise LandingError(
                "The candidate loses golden corpus rows whose fixtures "
                f"still exist ({len(keys)} across {len(lost)} golden files, "
                f"e.g. {keys[:5]}). Delete the fixtures with the rows, or "
                "restore the rows and re-verify.")
        # Publish must not build: the head lease is 180s and a rebuild has
        # already cost a session its reservation. A Rust-touching candidate
        # needs a `verify` receipt for this exact sha instead.
        workspaces = affected_workspaces(self.repository, options.base,
                                        options.candidate)
        affected = {package for packages in workspaces.values() for package in packages}
        # A non-Rust workflow may also carry qualification. Preserve that
        # binding at publication; no dependency on a Rust package is needed.
        qualified = False
        evidence_path = receipt_path(self.repository, options.candidate)
        if evidence_path.is_file():
            try:
                qualified = bool(json.loads(evidence_path.read_text(encoding="utf-8")).get("qualification"))
            except (OSError, ValueError):
                pass  # Rust receipts retain their mandatory refusal below.
        try:
            active_qualification = qualification.active_path(self.repository)
        except qualification.RecoveryError as error:
            raise LandingError(str(error)) from error
        if active_qualification and not qualified:
            raise LandingError("active qualification needs an exact-candidate verify receipt before publication")
        if affected or qualified:
            failure = receipt_failure(self.repository, options.candidate,
                                      affected, workspaces, options.base, active_qualification)
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

    def bound_to_lease(self, record):
        """A claim fetch/push may consume time, but never renews the head."""
        remaining = (utc(record["head_expires_utc"]) - datetime.now(timezone.utc)).total_seconds()
        if remaining <= 0:
            raise LandingError("The three-minute head lease expired. Rejoin with a new ticket.")
        lease_deadline = time.monotonic() + remaining
        self.deadline = min(getattr(self, "deadline", None) or lease_deadline, lease_deadline)

    def emit_reservation(self, result):
        # Git success proves a mutation happened, not that its reservation is
        # still active after a slow fetch/push. Reobserve persisted ownership;
        # concurrent arrivals may change the queue tip without changing the head.
        observed = self.snapshot()["record"]
        expected = {"ticket": result["claim"], "base": result["base"]}
        if (observed.get("active") != expected
                or not observed["entries"]
                or observed["entries"][0]["ticket"] != result["claim"]
                or observed["head_expires_utc"] != result["expires_utc"]):
            raise LandingError("The reservation changed during acquisition. Inspect status before retrying.")
        if expired(observed):
            raise LandingError("The three-minute head lease expired before response. Rejoin.")
        coordination.remaining_seconds(getattr(self, "deadline", None))
        emit(result)

    def coordinate(self, options):
        deadline = time.monotonic() + options.wait_seconds
        collisions = 0
        maintenance_attempts = 0
        snapshot_failures = 0
        while True:
            try:
                snapshot = self.snapshot()
            except coordination.CoordinationTimeout:
                raise
            except LandingError:
                snapshot_failures += 1
                if snapshot_failures >= SNAPSHOT_ATTEMPTS:
                    raise
                pause(snapshot_failures, PROBE_BACKOFF_SECONDS, 1.0,
                      getattr(self, "deadline", None))
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
                if entries and entries[0]["ticket"] == options.ticket:
                    self.bound_to_lease(record)
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
                    self.emit_reservation({"state": "reserved", "claim": options.ticket, "base": active["base"], "owner": entries[0]["owner"],
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
                    self.bound_to_lease(record)
                    self.publication_check(options, snapshot)
                    owner = next((entry["owner"] for entry in entries if entry["ticket"] == options.claim), "")
                    result = {"state": "published", "candidate": options.candidate, "released_claim": options.claim}
                else:
                    result = {"state": "released", "claim": options.claim, "action": options.command, "reason": options.reason}
                record["entries"] = [entry for entry in entries if entry["ticket"] != options.claim]
                record["active"] = None
                promote_head(record)

            if options.command == "claim" and expired(record):
                raise LandingError("The three-minute head lease expired before reservation. Rejoin.")
            updated = self.queue_object(record, snapshot["version"])
            candidate = None
            base = None
            if options.command == "claim" and expired(record):
                raise LandingError("The three-minute head lease expired before reservation push. Rejoin.")
            if options.command == "publish":
                candidate = options.candidate
                base = snapshot["main"]
                # The record above now describes the successor, so use the
                # pre-mutation lease retained separately for this last check.
                if datetime.now(timezone.utc) >= utc(lease_deadline):
                    raise LandingError("The three-minute head lease expired before push. Rejoin.")
            if self.publish(snapshot["version"], updated, candidate, base):
                if options.command == "publish":
                    # Main is confirmed landed. Notes are a separately bounded
                    # advisory operation, independent of the consumed head lease.
                    self.deadline = time.monotonic() + LEASE_SECONDS
                    provenance = options.provenance or os.environ.get("OMEGA_PROVENANCE", "")
                    if not re.search(r"\bmodel=\S", provenance):
                        # Advisory only: a landing is never refused over missing provenance.
                        result["provenance_warning"] = "no model=<name> in --provenance or OMEGA_PROVENANCE"
                        print("landing: warning: " + result["provenance_warning"]
                              + "; the note records the owner only.", file=sys.stderr)
                    result["provenance"] = self.record_provenance(options.candidate, [
                        f"owner: {owner}", *([f"provenance: {provenance}"] if provenance.strip() else []),
                        f"ticket: {options.claim}", f"published_utc: {now()}"])
                    # Main holds the candidate now. The lane is NOT removed:
                    # a warm worktree owns a managed target that costs a cold
                    # rebuild to recreate, and reusing that lane is far cheaper
                    # than provisioning a new one. Retire a lane deliberately
                    # with `prune`, which removes only worktrees that are
                    # unregistered or have never been built. A shared build
                    # cache makes the sources cheap to restore but not the
                    # target directory, so keeping lanes is the reuse path.
                    result["cleanup"] = {"removed": False,
                                         "reason": "lane retained for reuse; use prune to retire it"}
                if options.command == "claim":
                    self.emit_reservation(result)
                else:
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
                        choices=("status", "enqueue", "cancel", "claim", "publish", "release", "recover", "verify", "prune"))
    parser.add_argument("--repository", default=".")
    parser.add_argument("--remote", default="origin")
    for name in ("owner", "ticket", "claim", "base", "candidate", "reason", "provenance"):
        parser.add_argument("--" + name)
    parser.add_argument("--wait-seconds", type=int, default=0)
    parser.add_argument("--poll-seconds", type=int, default=10)
    parser.add_argument("--board-update", action="store_true",
                        help="publish a candidate that touches only coordination files "
                             "(coordinator board sweeps)")
    parser.add_argument("--keep-worktree", action="store_true",
                        help="accepted and does nothing: a successful publish always "
                             "retains the worktree and its candidate branch (retention "
                             "is the default; retire a lane deliberately with prune, "
                             "never automatically at publish)")
    parser.add_argument("--idle-hours", type=float, default=0.0,
                        help="prune: also retire a never-built lane idle at least this long")
    parser.add_argument("--dry-run", action="store_true",
                        help="prune: report what would be retired without removing it")
    parser.add_argument("--qualification", help="verify a pinned qualification assessment before/after the mandatory build")
    options = None
    try:
        options = parser.parse_args(arguments)
        if not 0 <= options.wait_seconds <= 43200 or not 1 <= options.poll_seconds <= 300:
            raise LandingError("Wait must be 0..43200 seconds; poll must be 1..300 seconds.")
        if options.command != "claim" and options.wait_seconds:
            raise LandingError("--wait-seconds applies only to claim.")
        if options.board_update and options.command != "publish":
            raise LandingError("--board-update applies only to publish.")
        if options.qualification and options.command != "verify":
            raise LandingError("--qualification applies only to verify.")
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
            return verify(options.repository, options.base, options.qualification)
        if options.command == "prune":
            records = prune_worktrees(options.repository, options.idle_hours,
                                      options.dry_run)
            removed = [r for r in records if r.get("removed")]
            emit({"state": "pruned", "dry_run": options.dry_run,
                  "inspected": len(records), "removed": len(removed),
                  "worktrees": records})
            return 0
        operation_deadline = time.monotonic() + LEASE_SECONDS + options.wait_seconds
        return Landing(options.repository, options.remote, operation_deadline).coordinate(options)
    except (LandingError, OSError) as error:
        identity = f" Ticket={options.ticket} Claim={options.claim}" if options else ""
        print(f"landing: {error}{identity}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    sys.exit(main())
