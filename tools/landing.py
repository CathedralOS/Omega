#!/usr/bin/env python3
"""Observe, enqueue or land through GitHub's native merge queue; never build, rebase or push main.

Git and `gh pr create` publish the author's branch and evidence. GitHub owns
integration and required checks. This adapter supplies expected-head fencing,
bounded waiting, and JSON reads on Windows, macOS and Linux. `land` is the one
step a session runs once its work is finished and pushed: it marks the draft
ready, enqueues a green head or asks GitHub to queue it when the check passes.
It returns once that request is registered; an armed request is not a queue
entry or publication. A later `land` rechecks the head and enqueues it when
green even if the request remains armed. `--wait-seconds` also observes MERGED
where a host allows that. The old lease and build-receipt publication path is
retired; see landing.md.
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
LAND_MAX_SECONDS = 86400


class LandingError(RuntimeError):
    pass


class GraphQLUnavailable(LandingError):
    """The GitHub proxy refuses GraphQL, as Claude Code cloud sessions do.

    Those sessions reach GitHub only through REST. The proxy names REST routes
    for the writes GraphQL would make (ready for review, auto-merge), so each
    operation below falls back to them instead of leaving the PR unlanded.
    """


GRAPHQL_REFUSAL = "GraphQL is not available"
QUEUE_RUN_ACTIVE = {"queued", "in_progress", "waiting", "requested", "pending"}


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
    try:
        output = command(repository, *arguments)
    except LandingError as error:
        if GRAPHQL_REFUSAL in str(error):
            raise GraphQLUnavailable(str(error)) from None
        raise
    result = json.loads(output)
    if result.get("errors"):
        raise LandingError(json.dumps(result["errors"]))
    return result["data"]


def rest(repository, method, path, *fields):
    """One REST call through `gh api`; `fields` are `-f key=value` strings."""
    arguments = ["gh", "api", "-X", method, path]
    for field in fields:
        arguments.extend(["-f", field])
    output = command(repository, *arguments)
    return json.loads(output) if output else {}


def rest_queue_entry(repository, slug, number):
    """The PR's merge-queue entry, read from the queue's own CI run.

    REST exposes no merge-queue object. GitHub tests each queued PR on a
    `gh-readonly-queue/main/pr-<number>-<base>` branch, so an active
    `merge_group` run there is the queue entry. Its position is unknown.
    """
    runs = rest(repository, "GET", f"repos/{slug}/actions/runs?event=merge_group&per_page=50")
    prefix = f"gh-readonly-queue/main/pr-{number}-"
    for run in runs.get("workflow_runs", []):
        if run.get("head_branch", "").startswith(prefix) and run.get("status") in QUEUE_RUN_ACTIVE:
            return {"position": None, "state": "QUEUED"}
    return None


def queue_run_passed(repository, remote, number):
    """Whether the PR's newest merge-queue run succeeded, so GitHub merges it next.

    The queue's run completes before GitHub marks the PR merged: #1921 read as
    out of the queue and still OPEN for about 35 seconds after its run passed.
    Only a newest run that failed, was cancelled or never existed means the
    entry was removed.
    """
    slug = repository_name(repository, remote)
    runs = rest(repository, "GET", f"repos/{slug}/actions/runs?event=merge_group&per_page=50")
    prefix = f"gh-readonly-queue/main/pr-{number}-"
    for run in runs.get("workflow_runs", []):  # newest first
        if run.get("head_branch", "").startswith(prefix):
            return run.get("status") == "completed" and run.get("conclusion") == "success"
    return False


def rest_pull_request(repository, remote, number):
    """`pull_request`'s record built from REST, in the same GraphQL shape."""
    slug = repository_name(repository, remote)
    pull = rest(repository, "GET", f"repos/{slug}/pulls/{number}")
    merged = bool(pull.get("merged"))
    mergeable = pull.get("mergeable")
    return dict(
        id=pull["node_id"], number=pull["number"], url=pull["html_url"],
        state="MERGED" if merged else pull["state"].upper(), isDraft=pull["draft"],
        baseRefName=pull["base"]["ref"], headRefOid=pull["head"]["sha"],
        mergeStateStatus=str(pull.get("mergeable_state") or "unknown").upper(),
        mergeable="CONFLICTING" if mergeable is False else "MERGEABLE" if mergeable else "UNKNOWN",
        mergeCommit={"oid": pull["merge_commit_sha"]} if merged else None,
        mergeQueueEntry=None if merged or pull["state"] != "open"
        else rest_queue_entry(repository, slug, number),
        autoMergeRequest={"enabledAt": None} if pull.get("auto_merge") else None)


def pull_request(repository, remote, number):
    try:
        return graphql_pull_request(repository, remote, number)
    except GraphQLUnavailable:
        return rest_pull_request(repository, remote, number)


def graphql_pull_request(repository, remote, number):
    owner, name = repository_name(repository, remote).split("/")
    query = """query($owner:String!,$name:String!,$number:Int!){
      repository(owner:$owner,name:$name){pullRequest(number:$number){
        id number url state isDraft baseRefName headRefOid mergeStateStatus mergeable
        mergeCommit{oid} mergeQueueEntry{position state} autoMergeRequest{enabledAt}
      }}
    }"""
    result = graphql(repository, query, owner=owner, name=name, number=number)["repository"]["pullRequest"]
    if result is None:
        raise LandingError(f"Pull request {number} does not exist.")
    return result


def queue_status(repository, remote):
    try:
        return graphql_queue_status(repository, remote)
    except GraphQLUnavailable:
        slug = repository_name(repository, remote)
        runs = rest(repository, "GET", f"repos/{slug}/actions/runs?event=merge_group&per_page=50")
        nodes = [{"position": None, "state": "QUEUED", "pullRequest": {"number": int(match[1])}}
                 for run in runs.get("workflow_runs", []) if run.get("status") in QUEUE_RUN_ACTIVE
                 for match in [re.match(r"gh-readonly-queue/main/pr-(\d+)-", run.get("head_branch", ""))]
                 if match]
        return {"entries": {"pageInfo": {"hasNextPage": False}, "nodes": nodes}}


def graphql_queue_status(repository, remote):
    owner, name = repository_name(repository, remote).split("/")
    query = """query($owner:String!,$name:String!){repository(owner:$owner,name:$name){
      mergeQueue(branch:"main"){entries(first:100){pageInfo{hasNextPage}
        nodes{position state pullRequest{number url title headRefOid}}}}
    }}"""
    return graphql(repository, query, owner=owner, name=name)["repository"]["mergeQueue"]


def head_check(repository, remote, expected):
    """The newest required-check run on the exact commit, or None before it exists."""
    try:
        return graphql_head_check(repository, remote, expected)
    except GraphQLUnavailable:
        slug = repository_name(repository, remote)
        listed = rest(repository, "GET",
                      f"repos/{slug}/commits/{expected}/check-runs?check_name={REQUIRED_CHECK}&per_page=100")
        runs = [dict(__typename="CheckRun", name=run["name"], status=run["status"].upper(),
                     conclusion=(run.get("conclusion") or "").upper() or None,
                     startedAt=run.get("started_at"))
                for run in listed.get("check_runs", []) if run.get("name") == REQUIRED_CHECK]
        return max(runs, key=lambda run: run.get("startedAt") or "", default=None)


def graphql_head_check(repository, remote, expected):
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
    try:
        return graphql(repository, query, pullRequest=pull["id"], head=expected)["enqueuePullRequest"]
    except GraphQLUnavailable:
        # REST has no enqueue. On a merge-queue branch GitHub queues a green PR
        # as soon as auto-merge is requested (observed on #1586, 2026-11-03).
        request_auto_merge(repository, remote, number)
        return {"mergeQueueEntry": pull_request(repository, remote, number)["mergeQueueEntry"]}


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
            # Armed and waiting for its check is pending; a red check is not.
            # Never silently re-enqueue a removed or failed group.
            if not pull.get("autoMergeRequest"):
                return pull, 1
            if check_outcome(head_check(repository, remote, expected))[0] == "red":
                return pull, 1
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return pull, 2
        time.sleep(min(10, remaining))


def mark_ready(repository, remote, number):
    slug = repository_name(repository, remote)
    try:
        command(repository, "gh", "pr", "ready", str(number), "--repo", slug)
    except LandingError as error:
        if GRAPHQL_REFUSAL not in str(error):
            raise
        rest(repository, "POST", f"repos/{slug}/pulls/{number}/ccr/ready_for_review")


def request_auto_merge(repository, remote, number):
    """The REST route Claude Code's GitHub proxy offers in place of GraphQL auto-merge.

    It cannot fence on the head, so callers validate the head just before it.
    """
    slug = repository_name(repository, remote)
    return rest(repository, "PUT", f"repos/{slug}/pulls/{number}/ccr/auto_merge", "merge_method=squash")


def merge_when_green(repository, pull, expected, remote="origin"):
    """Ask GitHub to add the PR to the queue once the required check passes.

    On a merge-queue branch an auto-merge request means "add to the queue when
    ready". GitHub accepts it only when the repository allows auto-merge, which
    it does since 2026-11-01. The request is fenced by the head; a red check
    leaves the PR out of the queue.
    """
    query = """mutation($pullRequest:ID!,$head:GitObjectID!){enablePullRequestAutoMerge(
      input:{pullRequestId:$pullRequest,expectedHeadOid:$head}){
        pullRequest{autoMergeRequest{enabledAt} mergeQueueEntry{position state}}}}"""
    try:
        return graphql(repository, query, pullRequest=pull["id"], head=expected)["enablePullRequestAutoMerge"]
    except GraphQLUnavailable:
        return request_auto_merge(repository, remote, pull["number"])


def arm(repository, remote, number, expected):
    """Mark ready and hand the wait for compiler-check to GitHub.

    Enqueues at once when the check is already green. Refuses a red check, a
    conflict, a closed PR or a changed head without any mutation. A PR that is
    already merged or queued is returned unchanged. An existing pending
    request stays armed; a later green check takes the ordinary enqueue path.
    """
    pull = pull_request(repository, remote, number)
    validate_head(pull, expected)
    if pull["state"] == "MERGED" or pull["mergeQueueEntry"]:
        return pull
    if pull["state"] != "OPEN":
        raise LandingError(f"PR {number} was closed without merging.")
    if has_conflict(pull):
        raise LandingError(f"PR {number} has a merge conflict with main; resolve it and push.")
    verdict, description = check_outcome(head_check(repository, remote, expected))
    if verdict == "red":
        raise LandingError(f"{description} on {expected}; inspect the failed run before changing source.")
    if verdict == "pending" and pull.get("autoMergeRequest"):
        return pull
    if pull["isDraft"]:
        mark_ready(repository, remote, number)
    if verdict == "green":
        enqueue(repository, remote, number, expected)
    else:
        try:
            merge_when_green(repository, pull, expected, remote)
        except LandingError:
            # The check may have passed between the read and the request, and
            # GitHub refuses to arm a PR it could queue now.
            if check_outcome(head_check(repository, remote, expected))[0] != "green":
                raise
            enqueue(repository, remote, number, expected)
    return pull_request(repository, remote, number)


def land(repository, remote, number, expected, seconds, interval=CHECK_POLL_SECONDS):
    """Arm the PR, then optionally wait up to `seconds` for MERGED.

    Returns (pull, 0) when armed, queued or merged with no wait asked for, or
    when merged within the wait; (pull, 2) at the deadline. Every other ending
    raises LandingError naming the reason.
    """
    pull = arm(repository, remote, number, expected)
    if pull["state"] == "MERGED" or not seconds:
        return pull, 0
    deadline = time.monotonic() + seconds
    remaining = lambda: max(0.0, deadline - time.monotonic())
    confirmed = False
    while True:
        pull, code = wait(repository, remote, number, expected, remaining())
        if code != 1:
            return pull, code
        if pull["state"] == "CLOSED":
            raise LandingError(f"PR {number} was closed without merging.")
        if has_conflict(pull):
            raise LandingError(f"PR {number} has a merge conflict with main; resolve it and push.")
        if pull.get("autoMergeRequest"):
            description = check_outcome(head_check(repository, remote, expected))[1]
            raise LandingError(f"{description} on {expected}; the merge request stays armed, so a "
                               "pushed fix lands when its check passes.")
        if confirmed:
            if not queue_run_passed(repository, remote, number):
                raise LandingError(f"PR {number} left the merge queue without merging; inspect the "
                                   "queue run with gh run list and do not re-enqueue blindly.")
            # Its queue run passed, so GitHub is merging it: wait for MERGED.
            if not remaining():
                return pull, 2
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
                        help="deadline in seconds (wait: default 60, at most 3600; land: default 0, "
                             "meaning return once armed, at most 86400)")
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
                    seconds = 0 if landing else 60
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
