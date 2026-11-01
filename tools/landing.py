#!/usr/bin/env python3
"""Observe, enqueue or land through GitHub's native merge queue; never build, rebase or push main.

Git and `gh pr create` publish the author's branch and evidence. GitHub owns
integration and required checks. This adapter supplies expected-head fencing,
bounded waiting, and JSON reads on Windows, macOS and Linux. `land` is the one
step a session runs right after its first push: it waits for the required check
on the exact head, marks the draft ready, enqueues, and waits for MERGED. The
old lease and build-receipt publication path is retired; see landing.md.
"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import time


REQUIRED_CHECK = "compiler-check"
CHECK_POLL_SECONDS = 15
LAND_DEFAULT_SECONDS = 7200
LAND_MAX_SECONDS = 86400


class LandingError(RuntimeError):
    pass


def command(repository, *arguments):
    result = subprocess.run(arguments, cwd=repository, capture_output=True,
                            text=True, timeout=60)
    if result.returncode:
        raise LandingError(result.stderr.strip() or result.stdout.strip()
                           or f"{arguments[0]} exited {result.returncode}")
    return result.stdout.strip()


def repository_name(repository, remote):
    urls = command(repository, "git", "remote", "get-url", "--all", "--push", remote).splitlines()
    if len(urls) != 1:
        raise LandingError("Landing requires one explicit GitHub push destination.")
    match = re.fullmatch(r"(?:https://github\.com/|git@github\.com:)([^/]+/[^/]+?)(?:\.git)?/?", urls[0])
    if not match:
        raise LandingError("The push destination must be an HTTPS or SSH github.com repository.")
    return match[1]


def graphql(repository, query, **variables):
    arguments = ["gh", "api", "graphql", "-f", "query=" + query]
    for name, value in variables.items():
        arguments.extend(["-F" if type(value) is int else "-f", f"{name}={value}"])
    result = json.loads(command(repository, *arguments))
    if result.get("errors"):
        raise LandingError(json.dumps(result["errors"]))
    return result["data"]


def pull_request(repository, remote, number):
    owner, name = repository_name(repository, remote).split("/")
    query = """query($owner:String!,$name:String!,$number:Int!){
      repository(owner:$owner,name:$name){pullRequest(number:$number){
        id number url state isDraft baseRefName headRefOid mergeStateStatus mergeable
        mergeCommit{oid} mergeQueueEntry{position state}
      }}
    }"""
    result = graphql(repository, query, owner=owner, name=name, number=number)["repository"]["pullRequest"]
    if result is None:
        raise LandingError(f"Pull request {number} does not exist.")
    return result


def queue_status(repository, remote):
    owner, name = repository_name(repository, remote).split("/")
    query = """query($owner:String!,$name:String!){repository(owner:$owner,name:$name){
      mergeQueue(branch:"main"){entries(first:100){pageInfo{hasNextPage}
        nodes{position state pullRequest{number url title headRefOid}}}}
    }}"""
    return graphql(repository, query, owner=owner, name=name)["repository"]["mergeQueue"]


def head_check(repository, remote, expected):
    """The newest required-check run on the exact commit, or None before it exists."""
    owner, name = repository_name(repository, remote).split("/")
    query = """query($owner:String!,$name:String!,$head:GitObjectID!){
      repository(owner:$owner,name:$name){object(oid:$head){... on Commit{
        statusCheckRollup{contexts(first:100){pageInfo{hasNextPage}
          nodes{__typename ... on CheckRun{name status conclusion startedAt}}}}
      }}}
    }"""
    commit = graphql(repository, query, owner=owner, name=name, head=expected)["repository"]["object"]
    rollup = (commit or {}).get("statusCheckRollup")
    if not rollup:
        return None
    contexts = rollup["contexts"]
    runs = [node for node in contexts["nodes"]
            if node.get("__typename") == "CheckRun" and node.get("name") == REQUIRED_CHECK]
    if not runs and contexts["pageInfo"]["hasNextPage"]:
        raise LandingError(f"{REQUIRED_CHECK} is not among the first 100 checks; inspect it with gh pr checks.")
    return max(runs, key=lambda run: run.get("startedAt") or "", default=None)


def check_outcome(run):
    """Return ("green" | "pending" | "red", description) for a required-check run."""
    if run is None:
        return "pending", f"{REQUIRED_CHECK} has not started"
    if run["status"] != "COMPLETED":
        return "pending", f"{REQUIRED_CHECK} is {run['status'].lower()}"
    if run["conclusion"] == "SUCCESS":
        return "green", f"{REQUIRED_CHECK} succeeded"
    return "red", f"{REQUIRED_CHECK} concluded {str(run['conclusion']).lower()}"


def has_conflict(pull):
    return pull["mergeStateStatus"] == "DIRTY" or pull.get("mergeable") == "CONFLICTING"


def validate_head(pull, expected):
    if pull["baseRefName"] != "main":
        raise LandingError("This command only submits PRs targeting main.")
    if not re.fullmatch(r"[0-9a-f]{40}", expected or "") or pull["headRefOid"] != expected:
        raise LandingError("PR source head differs from --expected-head; inspect the actual revision.")


def enqueue(repository, remote, number, expected):
    pull = pull_request(repository, remote, number)
    validate_head(pull, expected)
    if pull["state"] == "MERGED" or pull["mergeQueueEntry"]:
        return pull
    if pull["state"] != "OPEN":
        raise LandingError("Cannot enqueue a closed PR.")
    # Server rules decide eligibility. No admin override, jump, synthetic status,
    # auto-merge toggle, or retry of an uncertain mutation.
    query = """mutation($pullRequest:ID!,$head:GitObjectID!){enqueuePullRequest(
      input:{pullRequestId:$pullRequest,expectedHeadOid:$head}){
        mergeQueueEntry{position state}}}"""
    return graphql(repository, query, pullRequest=pull["id"], head=expected)["enqueuePullRequest"]


def wait(repository, remote, number, expected, seconds):
    deadline = time.monotonic() + seconds
    while True:
        pull = pull_request(repository, remote, number)
        validate_head(pull, expected)
        if pull["state"] == "MERGED":
            return pull, 0
        if pull["state"] == "CLOSED" or pull["mergeStateStatus"] == "DIRTY":
            return pull, 1
        if not pull["mergeQueueEntry"]:
            # Never silently re-enqueue a removed or failed group.
            return pull, 1
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return pull, 2
        time.sleep(min(10, remaining))


def mark_ready(repository, remote, number):
    command(repository, "gh", "pr", "ready", str(number), "--repo", repository_name(repository, remote))


def land(repository, remote, number, expected, seconds, interval=CHECK_POLL_SECONDS):
    """Wait for the required check, mark ready, enqueue, wait for MERGED.

    Returns (pull, 0) when merged and (pull, 2) at the deadline. Every other
    ending raises LandingError naming the reason. Reads only: nothing repeats a
    mutation, so a rerun after an interruption resumes from whatever state the
    PR is in.
    """
    deadline = time.monotonic() + seconds
    remaining = lambda: max(0.0, deadline - time.monotonic())
    pull = pull_request(repository, remote, number)
    while not pull["mergeQueueEntry"]:
        validate_head(pull, expected)
        if pull["state"] == "MERGED":
            return pull, 0
        if pull["state"] != "OPEN":
            raise LandingError(f"PR {number} was closed without merging.")
        if has_conflict(pull):
            raise LandingError(f"PR {number} has a merge conflict with main; resolve it and push.")
        verdict, description = check_outcome(head_check(repository, remote, expected))
        if verdict == "red":
            raise LandingError(f"{description} on {expected}; inspect the failed run before changing source.")
        if verdict == "green":
            if pull["isDraft"]:
                mark_ready(repository, remote, number)
            enqueue(repository, remote, number, expected)
            break
        if not remaining():
            return pull, 2
        time.sleep(min(interval, remaining()))
        pull = pull_request(repository, remote, number)
    confirmed = False
    while True:
        pull, code = wait(repository, remote, number, expected, remaining())
        if code != 1:
            return pull, code
        if pull["state"] == "CLOSED":
            raise LandingError(f"PR {number} was closed without merging.")
        if has_conflict(pull):
            raise LandingError(f"PR {number} has a merge conflict with main; resolve it and push.")
        if confirmed:
            raise LandingError(f"PR {number} left the merge queue without merging; inspect the queue "
                               "run with gh run list and do not re-enqueue blindly.")
        # The queue entry can lag the mutation; read once more before calling it removed.
        confirmed = True
        time.sleep(interval)


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("status", "enqueue", "wait", "land"))
    parser.add_argument("pull_request", nargs="?", type=int)
    parser.add_argument("--repository", type=Path, default=Path.cwd())
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--expected-head")
    parser.add_argument("--wait-seconds", type=int,
                        help="deadline in seconds (wait: default 60, at most 3600; land: default 7200, at most 86400)")
    options = parser.parse_args(arguments)
    try:
        if options.command == "status":
            result = (pull_request(options.repository, options.remote, options.pull_request)
                      if options.pull_request else queue_status(options.repository, options.remote))
            code = 0
        else:
            if not options.pull_request or not options.expected_head:
                raise LandingError("Supply a PR number and --expected-head FULL_SHA.")
            if options.command == "enqueue":
                result = enqueue(options.repository, options.remote, options.pull_request, options.expected_head)
                code = 0
            else:
                landing = options.command == "land"
                seconds = options.wait_seconds
                if seconds is None:
                    seconds = LAND_DEFAULT_SECONDS if landing else 60
                limit = LAND_MAX_SECONDS if landing else 3600
                if not 0 <= seconds <= limit:
                    raise LandingError(f"Wait must be between 0 and {limit} seconds.")
                step = land if landing else wait
                result, code = step(options.repository, options.remote, options.pull_request,
                                    options.expected_head, seconds)
                if landing and code == 2:
                    print(f"landing: PR {options.pull_request} is not merged after {seconds}s; "
                          "rerun land or inspect with status.", file=sys.stderr)
        print(json.dumps(result))
        return code
    except (LandingError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"landing: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
