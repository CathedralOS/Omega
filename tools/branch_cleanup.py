#!/usr/bin/env python3
"""Find and delete stale branches on the GitHub remote.

The repository holds about 1,800 branches, most of them `work/*` heads of pull
requests that GitHub already squash-merged. This script decides which of them
are safe to delete and, only when asked, deletes them. It runs from
`.github/workflows/branch-cleanup.yml`, because cloud agent sessions cannot
delete refs, and it can be run locally for a read-only report.

A branch is deleted only when every one of these holds:

1. It is not protected (see PROTECTED below), and no file on the default branch
   links to it.
2. No open pull request (draft or ready) uses it as its head.
3. Its newest commit is older than --min-age-hours, unless it is one of the
   EXPLICIT leftovers.
4. One of:
   - a merged or closed pull request used it as its head, and the branch still
     points at that pull request's last head commit (a branch reused after its
     PR merged keeps its newer commits and is kept);
   - its tip is already reachable from the default branch;
   - it is an EXPLICIT leftover;
   - it has no pull request and unmerged commits, and --include-unmerged-no-pr
     was given.

Squash merges leave a branch's commits outside the default branch's history,
so the pull-request check, not ancestry, recognizes those branches.

Deletion is lease-guarded: each ref is deleted only if it still points at the
commit this run examined. Only the Python standard library, `git` and `gh` are
needed.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

# Branches that tools, workflows or GitHub itself depend on. Keep this list in
# step with the workflow's header comment.
PROTECTED = {
    "main",  # the default branch; every workflow and tool targets it
    "work/claims",  # claims registry branch being introduced for `cargo xtask claims`
    "gh-pages",  # conventional Pages branch; pages.yml deploys from Actions today
}
# Name prefixes GitHub manages itself.
PROTECTED_PREFIXES = (
    "gh-readonly-queue/",  # merge-queue temporary branches
)
# Leftover probe branches that point at an old main commit with no new commits.
EXPLICIT = {
    "work/claude-probe-delete-me",
    "claude/eager-ritchie-j2dts3",
}

CATEGORIES = [
    ("merged_pr", "Merged pull request", True),
    ("closed_pr", "Closed (unmerged) pull request", True),
    ("ancestor", "Tip already reachable from the default branch", True),
    ("explicit", "Explicit leftover probe branch", True),
    ("unmerged_no_pr", "Unmerged commits, no pull request", False),
    ("moved_after_pr", "Kept: branch has commits after its closed pull request", False),
    ("open_pr", "Kept: open pull request", False),
    ("too_new", "Kept: newest commit younger than the age limit", False),
    ("protected", "Kept: protected or linked from repository files", False),
]


@dataclass
class Branch:
    name: str
    sha: str
    commit_time: int
    category: str = ""
    note: str = ""
    delete: bool = False
    result: str = ""


@dataclass
class PullSummary:
    open: bool = False
    # (number, merged, head_sha) of every non-open pull request with this head.
    finished: list = field(default_factory=list)


def run(*args: str, check: bool = True, cwd: str | None = None) -> str:
    done = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
    if check and done.returncode != 0:
        sys.exit(f"command failed ({done.returncode}): {' '.join(args)}\n{done.stderr}")
    return done.stdout


def gh_paginated(path: str, per_page: int = 100) -> list:
    """Return every item of a paginated REST list.

    Pages are requested by number rather than by following `Link` headers,
    because those name `repositories/<id>/` paths that the cloud session proxy
    refuses.
    """
    items: list = []
    sep = "&" if "?" in path else "?"
    page = 1
    while True:
        batch = json.loads(run("gh", "api", f"{path}{sep}per_page={per_page}&page={page}"))
        items.extend(batch)
        if len(batch) < per_page:
            return items
        page += 1


def fetch_heads(git_dir: str, remote: str) -> None:
    run("git", "-C", git_dir, "fetch", "--quiet", "--prune", "--filter=blob:none",
        remote, f"+refs/heads/*:refs/remotes/{remote}/*")


def list_heads(git_dir: str, remote: str) -> list[Branch]:
    out = run("git", "-C", git_dir, "for-each-ref",
              "--format=%(refname)%09%(objectname)%09%(committerdate:unix)",
              f"refs/remotes/{remote}/")
    prefix = f"refs/remotes/{remote}/"
    heads = []
    for line in out.splitlines():
        ref, sha, when = line.split("\t")
        name = ref[len(prefix):]
        if name == "HEAD":
            continue
        heads.append(Branch(name, sha, int(when or 0)))
    return heads


def merged_into(git_dir: str, remote: str, default: str) -> set[str]:
    out = run("git", "-C", git_dir, "for-each-ref", "--format=%(refname)",
              "--merged", f"refs/remotes/{remote}/{default}", f"refs/remotes/{remote}/")
    prefix = f"refs/remotes/{remote}/"
    return {ref[len(prefix):] for ref in out.splitlines()}


def linked_branches(checkout: str, repo: str, names: set[str]) -> set[str]:
    """Branches that a tracked file of the default-branch checkout links to."""
    pattern = rf"github\.com/{re.escape(repo)}/(tree|blob|commits|compare)/"
    out = run("git", "-C", checkout, "grep", "-hoiIE", pattern + r"[^)\"' <>#?]+",
              check=False)
    linked = set()
    for line in out.splitlines():
        tail = re.split(pattern, line, flags=re.I)[-1]
        parts = tail.split("/")
        # The branch is the longest leading run of path components that names one.
        for end in range(len(parts), 0, -1):
            candidate = "/".join(parts[:end])
            if candidate in names:
                linked.add(candidate)
                break
    return linked


def pull_summaries(repo: str) -> dict[str, PullSummary]:
    summaries: dict[str, PullSummary] = {}
    for pr in gh_paginated(f"repos/{repo}/pulls?state=all"):
        head = pr.get("head") or {}
        head_repo = (head.get("repo") or {}).get("full_name")
        # Fork pull requests do not own a branch here.
        if head_repo is None or head_repo.lower() != repo.lower():
            continue
        summary = summaries.setdefault(head["ref"], PullSummary())
        if pr["state"] == "open":
            summary.open = True
        else:
            summary.finished.append((pr["number"], pr.get("merged_at") is not None, head["sha"]))
    return summaries


def classify(branch: Branch, *, default: str, protected: set[str], merged: set[str],
             pulls: dict[str, PullSummary], cutoff: int, include_unmerged: bool) -> None:
    name = branch.name
    if name == default or name in protected or name.startswith(PROTECTED_PREFIXES):
        branch.category = "protected"
        return
    summary = pulls.get(name, PullSummary())
    if summary.open:
        branch.category = "open_pr"
        return
    if name in EXPLICIT:
        branch.category, branch.delete = "explicit", True
        return
    if branch.commit_time > cutoff:
        branch.category = "too_new"
        return
    # The newest finished pull request whose last head is this branch's tip.
    matching = sorted((pr for pr in summary.finished if pr[2] == branch.sha), reverse=True)
    if matching:
        number, was_merged, _ = matching[0]
        branch.category = "merged_pr" if was_merged else "closed_pr"
        branch.note = f"#{number}"
        branch.delete = True
    elif name in merged:
        branch.category, branch.delete = "ancestor", True
    elif summary.finished:
        branch.category = "moved_after_pr"
        branch.note = ", ".join(f"#{pr[0]}" for pr in sorted(summary.finished, reverse=True))
    else:
        branch.category = "unmerged_no_pr"
        branch.delete = include_unmerged


def delete_branches(git_dir: str, remote: str, doomed: list[Branch], batch: int) -> None:
    """Delete each ref only if it still points at the examined commit."""
    for start in range(0, len(doomed), batch):
        chunk = doomed[start:start + batch]
        args = ["git", "-C", git_dir, "push", "--porcelain", "--no-verify"]
        args += [f"--force-with-lease=refs/heads/{b.name}:{b.sha}" for b in chunk]
        args += [remote] + [f":refs/heads/{b.name}" for b in chunk]
        done = subprocess.run(args, capture_output=True, text=True)
        status = {}
        for line in done.stdout.splitlines():
            # Porcelain lines: "<flag>\t(delete):refs/heads/<name>\t<summary>".
            fields = line.split("\t")
            if len(fields) >= 3 and ":refs/heads/" in fields[1]:
                name = fields[1].split(":refs/heads/", 1)[1]
                status[name] = (fields[0], fields[2])
        for b in chunk:
            tail = done.stderr.strip().splitlines()
            flag, summary = status.get(b.name, ("!", tail[-1] if tail else "no result"))
            b.result = "deleted" if flag == "-" else f"failed: {summary}"
        print(f"deleted batch {start // batch + 1}: "
              f"{sum(b.result == 'deleted' for b in chunk)}/{len(chunk)}", flush=True)


def report(branches: list[Branch], *, dry_run: bool, args, default: str,
           capped: int) -> str:
    lines = ["# Branch cleanup " + ("(dry run)" if dry_run else "(deletion run)"), ""]
    doomed = [b for b in branches if b.delete]
    lines.append(f"Repository `{args.repo}`, default branch `{default}`, "
                 f"{len(branches)} branches examined, minimum age {args.min_age_hours} h, "
                 f"unmerged no-PR branches {'included' if args.include_unmerged_no_pr else 'kept'}.")
    lines.append("")
    if dry_run:
        lines.append(f"**Would delete {len(doomed)} branches.** Nothing was deleted.")
    else:
        ok = sum(b.result == "deleted" for b in doomed)
        lines.append(f"**Deleted {ok} of {len(doomed)} branches; "
                     f"{len(doomed) - ok} failed.**")
    if capped:
        lines.append(f"\n{capped} further candidates exceeded max_delete={args.max_delete} "
                     "and were left for a later run.")
    lines += ["", "| Category | Deleted? | Count |", "| --- | --- | ---: |"]
    for key, title, deletes in CATEGORIES:
        count = sum(b.category == key for b in branches)
        verdict = "yes" if deletes or (key == "unmerged_no_pr" and args.include_unmerged_no_pr) else "no"
        lines.append(f"| {title} | {verdict} | {count} |")
    lines.append("")
    now = time.time()
    for key, title, _ in CATEGORIES:
        group = sorted((b for b in branches if b.category == key), key=lambda b: b.name)
        if not group:
            continue
        lines += [f"<details><summary>{title} ({len(group)})</summary>", "", "```text"]
        for b in group:
            age = (now - b.commit_time) / 86400
            extra = f"  {b.note}" if b.note else ""
            result = f"  [{b.result}]" if b.result else ""
            skip = "  [over max_delete]" if b.delete is None else ""
            lines.append(f"{b.name}  {b.sha[:12]}  {age:.1f}d{extra}{result}{skip}")
        lines += ["```", "", "</details>", ""]
    failures = [b for b in doomed if b.result.startswith("failed")]
    if failures:
        lines += ["## Failures", "", "```text"]
        lines += [f"{b.name}: {b.result}" for b in failures]
        lines += ["```", ""]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY", "CathedralOS/Omega"))
    parser.add_argument("--git-dir", default=".", help="repository whose remote is examined")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--checkout", default=None,
                        help="checkout of the default branch searched for branch links "
                             "(defaults to --git-dir)")
    parser.add_argument("--min-age-hours", type=float, default=48)
    parser.add_argument("--max-delete", type=int, default=2000)
    parser.add_argument("--include-unmerged-no-pr", action="store_true")
    parser.add_argument("--delete", action="store_true",
                        help="actually delete; without it the run only reports")
    parser.add_argument("--batch", type=int, default=50)
    parser.add_argument("--no-fetch", action="store_true", help="use already fetched remote refs")
    parser.add_argument("--out", default="branch-cleanup", help="report directory")
    args = parser.parse_args()

    default = json.loads(run("gh", "api", f"repos/{args.repo}"))["default_branch"]
    if not args.no_fetch:
        fetch_heads(args.git_dir, args.remote)
    branches = list_heads(args.git_dir, args.remote)
    names = {b.name for b in branches}
    if default not in names:
        sys.exit(f"default branch {default!r} was not fetched; refusing to classify")
    linked = linked_branches(args.checkout or args.git_dir, args.repo, names)
    protected = PROTECTED | {default} | linked
    merged = merged_into(args.git_dir, args.remote, default)
    pulls = pull_summaries(args.repo)
    cutoff = int(time.time() - args.min_age_hours * 3600)
    for b in branches:
        classify(b, default=default, protected=protected, merged=merged, pulls=pulls,
                 cutoff=cutoff, include_unmerged=args.include_unmerged_no_pr)
        if b.category == "protected" and b.name in linked and b.name not in PROTECTED:
            b.note = "linked from a repository file"

    doomed = sorted((b for b in branches if b.delete), key=lambda b: b.commit_time)
    capped = max(0, len(doomed) - args.max_delete)
    for b in doomed[args.max_delete:]:
        b.delete = None  # oldest first; the rest wait for the next run
    doomed = doomed[:args.max_delete]
    if args.delete:
        delete_branches(args.git_dir, args.remote, doomed, args.batch)

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    text = report(branches, dry_run=not args.delete, args=args, default=default, capped=capped)
    (out / "summary.md").write_text(text)
    (out / "branches.json").write_text(json.dumps(
        [b.__dict__ for b in sorted(branches, key=lambda b: (b.category, b.name))], indent=1))
    print(text.split("\n<details>")[0])
    return 1 if any(b.result.startswith("failed") for b in doomed) else 0


if __name__ == "__main__":
    sys.exit(main())
