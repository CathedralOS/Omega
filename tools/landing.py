#!/usr/bin/env python3
"""Observe or enqueue GitHub's native merge queue; never build, rebase or push main.

Git and `gh pr create` publish the author's branch and evidence. GitHub owns
integration and required checks. This adapter supplies expected-head fencing,
bounded waiting, and JSON reads on Windows, macOS and Linux. The old lease and
build-receipt publication path is retired; see landing.md.
"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import time


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
        id number url state baseRefName headRefOid mergeStateStatus
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


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("status", "enqueue", "wait"))
    parser.add_argument("pull_request", nargs="?", type=int)
    parser.add_argument("--repository", type=Path, default=Path.cwd())
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--expected-head")
    parser.add_argument("--wait-seconds", type=int, default=60)
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
                if not 0 <= options.wait_seconds <= 3600:
                    raise LandingError("Wait must be between 0 and 3600 seconds.")
                result, code = wait(options.repository, options.remote, options.pull_request,
                                    options.expected_head, options.wait_seconds)
        print(json.dumps(result))
        return code
    except (LandingError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"landing: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
