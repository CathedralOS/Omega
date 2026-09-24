#!/usr/bin/env python3
"""Advisory failure triage: name the suspect commit and layer for a landed
failure, given its output and a window of recent commits.

Feed it captured failure output plus the command that produced it; it prints
advisory lines naming the most likely responsible commit (or
`none_in_window` for infrastructure/environment/older history), the
compiler/repo layer, and a recent-change confidence. Advisory only: it never
gates anything, exits 0 on any of its own failures, and is silent without a
TypeSafe key. Proven shape: build/experiments/failure-triage attributed 2/3
witnessed failures exactly (suspect + layer) and refused to blame a commit
for a host platform wall.

    tools/triage_advisor.py --command "mbx run -p omega -- --check std/main.omg" \
        --stderr-file fail.log
    some-command 2>&1 | tools/triage_advisor.py --command "some-command"

Key resolution: TYPESAFE_API_KEY, --key-file, build/typesafe.env.txt,
~/.config/typesafe/typesafe.env.txt (worktree-aware).
"""
import argparse
import json
import os
import subprocess
import sys
import urllib.request
from pathlib import Path

ENDPOINT = "https://api.typesafe.ai/v1/systemone"
MODEL = "jev-1.13.0"
NONE = "none_in_window"
LAYERS = {
    "name_resolution": "lexing/parsing/name resolution",
    "typing_checking": "type checking and semantic validation",
    "lowering": "Psi lowering and unit transforms",
    "native_backend": "native realization, ISA, ABI, object/image",
    "packages_build": "package resolution, build evaluation, trust ledger",
    "cli_host": "CLI dispatch, host infrastructure, process/stack limits",
    "test_infra": "test harnesses, catalogs, goldens, fixtures",
    "docs_tooling": "documentation, boards, repository tools",
    "unknown": "cannot tell from this evidence"}


def repository_root():
    done = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                          capture_output=True, text=True)
    if done.returncode != 0:
        raise ValueError("not inside a git checkout")
    return Path(done.stdout.strip())


def git(root, *arguments):
    return subprocess.run(["git", *arguments], cwd=root, capture_output=True,
                          text=True, check=True).stdout


def window_commits(root, count, since=None):
    if since:
        shas = git(root, "rev-list", "--first-parent", f"{since}..HEAD").split()
    else:
        shas = git(root, "rev-list", "--first-parent", f"-{count}", "HEAD").split()
    commits = []
    for sha in shas[:60]:
        subject = git(root, "show", "-s", "--format=%s", sha).strip()
        parents = git(root, "show", "-s", "--format=%P", sha).split()
        files = (git(root, "diff", "--name-only", parents[0], sha)
                 if len(parents) > 1
                 else git(root, "show", "--format=", "--name-only", sha)).split()
        commits.append({"sha": sha[:10], "subject": subject,
                        "files": files[:15], "file_count": len(files)})
    return commits


def make_request(command, failure, commits):
    options = {c["sha"]: c["subject"] for c in commits}
    options[NONE] = ("the failure is infrastructure, environment, or a change "
                     "older than this window — no listed commit introduced it")
    return {"model": MODEL,
            "state": {"command": command,
                      "failure_output": failure[:12000],
                      "recent_commits": commits},
            "questions": {
                "suspect": {
                    "type": "choice",
                    "instructions": (
                        "Given `failure_output` for `command` and the ranked "
                        "`recent_commits` (newest first), choose the single "
                        "commit most likely responsible for this failure — "
                        "the change whose touched files or described behavior "
                        "could produce exactly this output. Choose "
                        f"`{NONE}` only when the output points at "
                        "infrastructure, environment, or history older than "
                        "the window rather than any listed change."),
                    "criteria": options},
                "layer": {
                    "type": "choice",
                    "instructions": "Which compiler or repository layer does "
                                    "this failure live in?",
                    "criteria": LAYERS},
                "recent_change_responsible": {
                    "type": "noul",
                    "instructions": (
                        "Is this failure attributable to a change inside the "
                        "recent window at all? High means a listed commit "
                        "plausibly introduced it; low means it looks "
                        "environmental, infrastructural, or older than the "
                        "window."),
                    "criteria": {
                        "true": "a listed commit plausibly introduced this "
                                "failure",
                        "false": "infrastructure/environment/older history"}}}}


def read_key(path):
    if not path.exists():
        return ""
    keys = [line.partition("=")[2].strip().strip("\"'")
            for line in path.read_text(encoding="utf-8-sig").splitlines()
            if line.partition("=")[0].strip() == "TYPESAFE_API_KEY"]
    return keys[0] if len(keys) == 1 else ""


def main_checkout_root():
    try:
        done = subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.SubprocessError):
        return None
    return Path(done.stdout.strip()).parent if done.returncode == 0 else None


def find_key(key_file):
    key = os.environ.get("TYPESAFE_API_KEY", "").strip()
    if key:
        return key
    candidates = ([key_file] if key_file else [])
    candidates.append(Path("build/typesafe.env.txt"))
    root = main_checkout_root()
    if root is not None:
        candidates.append(root / "build/typesafe.env.txt")
    candidates.append(Path.home() / ".config" / "typesafe" / "typesafe.env.txt")
    for path in candidates:
        key = read_key(path)
        if key:
            return key
    return ""


def evaluate(request, response):
    answers = response.get("answers", {})
    suspect = answers.get("suspect", {}).get("choice", "")
    layer = answers.get("layer", {}).get("choice", "unknown")
    recent = answers.get("recent_change_responsible", {}).get("noul", 0.0)
    subject = request["questions"]["suspect"]["criteria"].get(suspect, "")
    return suspect, subject, layer, recent


def self_test():
    request = make_request("cmd", "err", [
        {"sha": "abc1234567", "subject": "x", "files": [], "file_count": 0}])
    assert set(request["questions"]) == {
        "suspect", "layer", "recent_change_responsible"}
    assert NONE in request["questions"]["suspect"]["criteria"]
    assert set(LAYERS) == set(request["questions"]["layer"]["criteria"])
    print("self-test ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--command", default="(unspecified command)",
                        help="the invocation that produced the failure")
    parser.add_argument("--stderr-file", type=Path,
                        help="file containing the failure output (default stdin)")
    parser.add_argument("--window", type=int, default=40,
                        help="commits to rank (default 40)")
    parser.add_argument("--since", help="window lower bound: commits since REV")
    parser.add_argument("--key-file", type=Path)
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        self_test()
        return 0
    failure = (arguments.stderr_file.read_text(encoding="utf-8",
                                               errors="replace")
               if arguments.stderr_file else sys.stdin.read())
    if not failure.strip():
        print("triage_advisor: empty failure output", file=sys.stderr)
        return 0
    key = find_key(arguments.key_file)
    if not key:
        print("  = triage advisory unavailable: no TYPESAFE_API_KEY",
              file=sys.stderr)
        return 0
    try:
        root = repository_root()
        commits = window_commits(root, arguments.window, arguments.since)
    except (OSError, subprocess.CalledProcessError, ValueError) as error:
        print(f"  = triage advisory unavailable: {error}", file=sys.stderr)
        return 0
    if not commits:
        print("  = triage advisory unavailable: empty commit window",
              file=sys.stderr)
        return 0
    request = make_request(arguments.command, failure, commits)
    http = urllib.request.Request(
        ENDPOINT, data=json.dumps(request).encode(),
        headers={"Authorization": f"Bearer {key}",
                 "Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.build_opener(
                urllib.request.HTTPRedirectHandler()).open(
                http, timeout=60) as connection:
            response = json.load(connection)
    except (urllib.error.URLError, TimeoutError, urllib.error.HTTPError):
        print("  = triage advisory unavailable: TypeSafe request failed",
              file=sys.stderr)
        return 0
    if key in json.dumps(response):
        raise RuntimeError("response contains the credential; refusing output")
    suspect, subject, layer, recent = evaluate(request, response)
    if suspect == NONE:
        print(f"  = triage advisory (jev): no window commit blamed — likely "
              f"infrastructure/environment/pre-window history "
              f"| layer: {layer} ({LAYERS.get(layer, layer)}) "
              f"| recent-change {recent:.2f}", file=sys.stderr)
    else:
        print(f"  = triage advisory (jev): suspect {suspect} — {subject} "
              f"| layer: {layer} ({LAYERS.get(layer, layer)}) "
              f"| recent-change {recent:.2f}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError) as error:
        raise SystemExit(
            f"triage_advisor failed: {type(error).__name__}: {error}")
