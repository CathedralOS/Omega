#!/usr/bin/env python3
"""Native-queue client controls; no network or compiler builds."""

from pathlib import Path
import json
import subprocess
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import landing

HEAD = "a" * 40
PULL = dict(id="PR_example", number=1, url="https://github.com/example/repo/pull/1",
            state="OPEN", isDraft=False, baseRefName="main", headRefOid=HEAD,
            mergeStateStatus="CLEAN", mergeQueueEntry=None, mergeCommit=None)


def check_run(status="COMPLETED", conclusion="SUCCESS", name="compiler-check", started="2026-11-01T00:00:00Z"):
    return dict(__typename="CheckRun", name=name, status=status, conclusion=conclusion, startedAt=started)


class FakeGitHub:
    """Stands in for `git remote` and `gh` at the subprocess boundary.

    `script` holds one callable per pull-request read; each mutates the fake's
    state before that read is answered, so a test describes what GitHub does
    over time. After the script runs out the state stays as it was left.
    """

    def __init__(self, script=(), checks=None, **pull):
        self.pull = dict(PULL, **pull)
        self.checks = [check_run()] if checks is None else checks
        self.script = list(script)
        self.calls = []

    def __call__(self, repository, *arguments):
        self.calls.append(arguments)
        if arguments[:2] == ("git", "remote"):
            return "https://github.com/example/repo.git"
        if arguments[:3] == ("gh", "pr", "ready"):
            self.pull["isDraft"] = False
            return ""
        assert arguments[:3] == ("gh", "api", "graphql"), arguments
        query = next(a for a in arguments if a.startswith("query="))
        if "enqueuePullRequest" in query:
            self.pull["mergeQueueEntry"] = {"position": 1, "state": "QUEUED"}
            return json.dumps(dict(data=dict(enqueuePullRequest=dict(mergeQueueEntry=self.pull["mergeQueueEntry"]))))
        if "statusCheckRollup" in query:
            rollup = dict(contexts=dict(pageInfo=dict(hasNextPage=False), nodes=list(self.checks)))
            return json.dumps(dict(data=dict(repository=dict(object=dict(statusCheckRollup=rollup)))))
        assert "pullRequest(number" in query, query
        if self.script:
            self.script.pop(0)(self)
        return json.dumps(dict(data=dict(repository=dict(pullRequest=self.pull))))

    def ran(self, *prefix):
        return [call for call in self.calls if call[:len(prefix)] == prefix]

    @property
    def readied(self):
        return self.ran("gh", "pr", "ready")

    @property
    def enqueued(self):
        return [call for call in self.calls if any("enqueuePullRequest" in a for a in call)]


def update(**change):
    return lambda fake: fake.pull.update(change)


def run_land(test, fake, seconds=60):
    with mock.patch.object(landing, "command", fake), mock.patch.object(landing.time, "sleep"):
        return landing.land(".", "origin", 1, HEAD, seconds)


class LandTests(unittest.TestCase):
    def test_draft_waits_for_green_then_readies_enqueues_and_waits_for_merge(self):
        fake = FakeGitHub(isDraft=True, checks=[check_run("IN_PROGRESS", None)],
                          script=[lambda fake: None,
                                  lambda fake: fake.checks.__setitem__(0, check_run()),
                                  lambda fake: None, lambda fake: None,
                                  update(state="MERGED")])
        pull, code = run_land(self, fake)
        self.assertEqual((code, pull["state"]), (0, "MERGED"))
        self.assertEqual(len(fake.readied), 1)
        self.assertEqual(len(fake.enqueued), 1)
        self.assertLess(fake.calls.index(fake.readied[0]), fake.calls.index(fake.enqueued[0]))
        self.assertEqual(fake.readied[0][3:], ("1", "--repo", "example/repo"))

    def test_check_not_yet_started_is_pending(self):
        fake = FakeGitHub(checks=[], script=[lambda fake: None, lambda fake: fake.checks.append(check_run()),
                                             lambda fake: None, lambda fake: None, update(state="MERGED")])
        self.assertEqual(run_land(self, fake)[1], 0)
        self.assertEqual(len(fake.enqueued), 1)

    def test_other_checks_and_older_runs_do_not_decide(self):
        checks = [check_run(name="lint", conclusion="FAILURE"),
                  check_run(conclusion="FAILURE", started="2026-11-01T00:00:00Z"),
                  check_run(conclusion="SUCCESS", started="2026-11-01T01:00:00Z")]
        fake = FakeGitHub(checks=checks, script=[lambda fake: None, lambda fake: None, update(state="MERGED")])
        self.assertEqual(run_land(self, fake)[1], 0)

    def test_ready_pull_request_is_not_marked_ready_again(self):
        fake = FakeGitHub(script=[lambda fake: None, lambda fake: None, update(state="MERGED")])
        self.assertEqual(run_land(self, fake)[1], 0)
        self.assertEqual(fake.readied, [])
        self.assertEqual(len(fake.enqueued), 1)

    def test_red_check_refuses_without_ready_or_enqueue(self):
        for conclusion in ("FAILURE", "CANCELLED", "TIMED_OUT"):
            with self.subTest(conclusion=conclusion):
                fake = FakeGitHub(isDraft=True, checks=[check_run(conclusion=conclusion)])
                with self.assertRaisesRegex(landing.LandingError, f"compiler-check concluded {conclusion.lower()}"):
                    run_land(self, fake)
                self.assertEqual((fake.readied, fake.enqueued), ([], []))

    def test_changed_head_refuses_while_waiting(self):
        fake = FakeGitHub(isDraft=True, checks=[check_run("QUEUED", None)],
                          script=[lambda fake: None, update(headRefOid="b" * 40)])
        with self.assertRaisesRegex(landing.LandingError, "differs from --expected-head"):
            run_land(self, fake)
        self.assertEqual((fake.readied, fake.enqueued), ([], []))

    def test_merge_conflict_refuses_before_ready(self):
        for change in (dict(mergeStateStatus="DIRTY"), dict(mergeable="CONFLICTING")):
            with self.subTest(change=change):
                fake = FakeGitHub(isDraft=True, **change)
                with self.assertRaisesRegex(landing.LandingError, "merge conflict"):
                    run_land(self, fake)
                self.assertEqual((fake.readied, fake.enqueued), ([], []))

    def test_conflict_found_by_the_queue_is_reported_as_conflict(self):
        fake = FakeGitHub(script=[lambda fake: None, lambda fake: None, update(mergeStateStatus="DIRTY")])
        with self.assertRaisesRegex(landing.LandingError, "merge conflict"):
            run_land(self, fake)

    def test_removal_from_queue_refuses_after_one_confirming_read_and_never_reenqueues(self):
        remove = update(mergeQueueEntry=None)
        fake = FakeGitHub(script=[lambda fake: None, lambda fake: None, remove, lambda fake: None])
        with self.assertRaisesRegex(landing.LandingError, "left the merge queue"):
            run_land(self, fake)
        self.assertEqual(len(fake.enqueued), 1)

    def test_queue_entry_that_lags_the_mutation_is_not_called_removal(self):
        lag = update(mergeQueueEntry=None)
        fake = FakeGitHub(script=[lambda fake: None, lambda fake: None, lag,
                                  update(mergeQueueEntry={"position": 1}), update(state="MERGED")])
        self.assertEqual(run_land(self, fake)[1], 0)

    def test_closed_pull_request_refuses(self):
        for script in ([], [lambda fake: None, lambda fake: None, update(state="CLOSED", mergeQueueEntry=None)]):
            with self.subTest(script=len(script)):
                fake = FakeGitHub(script=script, **({} if script else dict(state="CLOSED")))
                with self.assertRaisesRegex(landing.LandingError, "closed without merging"):
                    run_land(self, fake)

    def test_deadline_while_checking_or_queued_exits_unfinished(self):
        fake = FakeGitHub(isDraft=True, checks=[check_run("IN_PROGRESS", None)])
        self.assertEqual(run_land(self, fake, seconds=0)[1], 2)
        self.assertEqual((fake.readied, fake.enqueued), ([], []))
        queued = FakeGitHub(mergeQueueEntry={"position": 2})
        self.assertEqual(run_land(self, queued, seconds=0)[1], 2)

    def test_already_merged_or_queued_resumes_without_mutations(self):
        merged = FakeGitHub(state="MERGED")
        self.assertEqual(run_land(self, merged)[1], 0)
        queued = FakeGitHub(isDraft=False, mergeQueueEntry={"position": 1}, script=[lambda fake: None, update(state="MERGED")])
        self.assertEqual(run_land(self, queued)[1], 0)
        for fake in (merged, queued):
            self.assertEqual((fake.readied, fake.enqueued), ([], []))

    def test_command_line_exit_codes_and_reasons(self):
        arguments = ["land", "1", "--expected-head", HEAD]
        cases = [(FakeGitHub(script=[lambda fake: None, lambda fake: None, update(state="MERGED")]), 0, ""),
                 (FakeGitHub(checks=[check_run(conclusion="FAILURE")]), 1, "compiler-check concluded failure"),
                 (FakeGitHub(checks=[check_run("QUEUED", None)]), 2, "not merged after 0s")]
        for fake, code, reason in cases:
            with self.subTest(code=code), mock.patch.object(landing, "command", fake), \
                    mock.patch.object(landing.time, "sleep"), \
                    mock.patch.object(sys, "stdout", new=mock.MagicMock()), \
                    mock.patch.object(sys, "stderr", new=mock.MagicMock()) as stderr:
                options = arguments + (["--wait-seconds", "0"] if code == 2 else [])
                self.assertEqual(landing.main(options), code)
                self.assertIn(reason, "".join(call.args[0] for call in stderr.write.call_args_list))
        with mock.patch.object(sys, "stderr", new=mock.MagicMock()):
            self.assertEqual(landing.main(["land", "1"]), 1)
            self.assertEqual(landing.main(arguments + ["--wait-seconds", "86401"]), 1)


class LandingTests(unittest.TestCase):
    def test_resolve_actual_push_destination(self):
        for url in ("https://github.com/example/repo.git", "git@github.com:example/repo.git",
                    "https://github.com/example/repo"):
            with self.subTest(url=url), mock.patch.object(landing, "command", return_value=url):
                self.assertEqual(landing.repository_name(Path.cwd(), "origin"), "example/repo")
        for url in ("/local/bare", "https://elsewhere.invalid/example/repo", "one\ntwo"):
            with self.subTest(url=url), mock.patch.object(landing, "command", return_value=url):
                with self.assertRaises(landing.LandingError):
                    landing.repository_name(Path.cwd(), "origin")

    def test_enqueue_fences_head_without_override(self):
        with mock.patch.object(landing, "pull_request", return_value=PULL), \
                mock.patch.object(landing, "graphql", return_value={"enqueuePullRequest": {"queued": True}}) as call:
            self.assertEqual(landing.enqueue(".", "origin", 1, HEAD), {"queued": True})
            self.assertIn("expectedHeadOid:$head", call.call_args.args[1])
            self.assertNotIn("jump", call.call_args.args[1])
            self.assertEqual(call.call_args.kwargs, dict(pullRequest="PR_example", head=HEAD))

    def test_changed_head_wrong_base_closed_or_missing_head_refuse(self):
        for change, expected in ((dict(headRefOid="b" * 40), HEAD),
                                 (dict(baseRefName="release"), HEAD),
                                 (dict(state="CLOSED"), HEAD), ({}, None)):
            with mock.patch.object(landing, "pull_request", return_value=dict(PULL, **change)), \
                    mock.patch.object(landing, "graphql") as mutation:
                with self.assertRaises(landing.LandingError):
                    landing.enqueue(".", "origin", 1, expected)
                mutation.assert_not_called()

    def test_merged_or_already_queued_does_not_repeat_mutation(self):
        for change in (dict(state="MERGED"), dict(mergeQueueEntry={"position": 1})):
            pull = dict(PULL, **change)
            with mock.patch.object(landing, "pull_request", return_value=pull), \
                    mock.patch.object(landing, "graphql") as mutation:
                self.assertEqual(landing.enqueue(".", "origin", 1, HEAD), pull)
                mutation.assert_not_called()

    def test_uncertain_enqueue_is_not_retried(self):
        with mock.patch.object(landing, "pull_request", return_value=PULL), \
                mock.patch.object(landing, "graphql", side_effect=landing.LandingError("network lost")) as mutation:
            with self.assertRaisesRegex(landing.LandingError, "network lost"):
                landing.enqueue(".", "origin", 1, HEAD)
            self.assertEqual(mutation.call_count, 1)

    def test_wait_distinguishes_merged_removed_conflict_and_pending(self):
        cases = [(dict(state="MERGED"), 0), ({}, 1),
                 (dict(mergeStateStatus="DIRTY"), 1),
                 (dict(mergeQueueEntry={"position": 1}), 2)]
        for change, expected in cases:
            with self.subTest(change=change), mock.patch.object(landing, "pull_request", return_value=dict(PULL, **change)):
                self.assertEqual(landing.wait(".", "origin", 1, HEAD, 0)[1], expected)

    def test_api_errors_do_not_become_success(self):
        with mock.patch.object(landing, "command", return_value='{"data": null, "errors": [{"message":"denied"}]}'):
            with self.assertRaisesRegex(landing.LandingError, "denied"):
                landing.graphql(".", "query")
        with mock.patch.object(subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "denied")):
            with self.assertRaisesRegex(landing.LandingError, "denied"):
                landing.command(".", "gh", "api")


if __name__ == "__main__":
    unittest.main()
