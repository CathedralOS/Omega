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
            mergeStateStatus="CLEAN", mergeQueueEntry=None, mergeCommit=None, autoMergeRequest=None)


def check_run(status="COMPLETED", conclusion="SUCCESS", name="compiler-check", started="2026-11-01T00:00:00Z"):
    return dict(__typename="CheckRun", name=name, status=status, conclusion=conclusion, startedAt=started)


class FakeGitHub:
    """Stands in for `git remote` and `gh` at the subprocess boundary.

    `script` holds one callable per pull-request read; each mutates the fake's
    state before that read is answered, so a test describes what GitHub does
    over time. After the script runs out the state stays as it was left.
    """

    def __init__(self, script=(), checks=None, refuse_arming=None, **pull):
        self.pull = dict(PULL, **pull)
        self.refuse_arming = refuse_arming
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
        if "enablePullRequestAutoMerge" in query:
            if self.refuse_arming:
                raise landing.LandingError(self.refuse_arming)
            self.pull["autoMergeRequest"] = {"enabledAt": "2026-11-01T00:00:00Z"}
            return json.dumps(dict(data=dict(enablePullRequestAutoMerge=dict(pullRequest=dict(
                autoMergeRequest=self.pull["autoMergeRequest"], mergeQueueEntry=None)))))
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
    def armed(self):
        return [call for call in self.calls if any("enablePullRequestAutoMerge" in a for a in call)]

    @property
    def mutated(self):
        return self.readied + self.enqueued + self.armed

    @property
    def enqueued(self):
        return [call for call in self.calls if any("enqueuePullRequest" in a for a in call)]


def update(**change):
    return lambda fake: fake.pull.update(change)


PENDING = [check_run("IN_PROGRESS", None)]


def queued_by_github(fake):
    """GitHub's side of an armed PR: the check passes and the queue takes it."""
    fake.checks[:] = [check_run()]
    fake.pull.update(autoMergeRequest=None, mergeQueueEntry={"position": 1, "state": "QUEUED"})


def run_land(test, fake, seconds=0):
    with mock.patch.object(landing, "command", fake), mock.patch.object(landing.time, "sleep"):
        return landing.land(".", "origin", 1, HEAD, seconds)


class LandTests(unittest.TestCase):
    def test_pending_draft_is_readied_then_armed_and_returns_without_waiting(self):
        fake = FakeGitHub(isDraft=True, checks=list(PENDING))
        pull, code = run_land(self, fake)
        self.assertEqual(code, 0)
        self.assertTrue(pull["autoMergeRequest"])
        self.assertEqual((len(fake.readied), len(fake.armed), fake.enqueued), (1, 1, []))
        self.assertLess(fake.calls.index(fake.readied[0]), fake.calls.index(fake.armed[0]))
        self.assertEqual(fake.readied[0][3:], ("1", "--repo", "example/repo"))
        self.assertIn("expectedHeadOid:$head", next(a for a in fake.armed[0] if a.startswith("query=")))

    def test_check_not_yet_started_is_pending(self):
        fake = FakeGitHub(checks=[])
        run_land(self, fake)
        self.assertEqual((len(fake.armed), fake.enqueued), (1, []))

    def test_green_check_enqueues_at_once_without_arming(self):
        fake = FakeGitHub(isDraft=True)
        pull, code = run_land(self, fake)
        self.assertEqual((code, len(fake.readied), len(fake.enqueued), fake.armed), (0, 1, 1, []))
        self.assertTrue(pull["mergeQueueEntry"])

    def test_other_checks_and_older_runs_do_not_decide(self):
        checks = [check_run(name="lint", conclusion="FAILURE"),
                  check_run(conclusion="FAILURE", started="2026-11-01T00:00:00Z"),
                  check_run(conclusion="SUCCESS", started="2026-11-01T01:00:00Z")]
        fake = FakeGitHub(checks=checks)
        run_land(self, fake)
        self.assertEqual(len(fake.enqueued), 1)

    def test_ready_pull_request_is_not_marked_ready_again(self):
        fake = FakeGitHub(checks=list(PENDING))
        run_land(self, fake)
        self.assertEqual((fake.readied, len(fake.armed)), ([], 1))

    def test_check_that_turns_green_before_arming_is_enqueued(self):
        fake = FakeGitHub(checks=list(PENDING), refuse_arming="pull request is in clean status")
        original = fake.__call__

        def green_on_refusal(repository, *arguments):
            if any("enablePullRequestAutoMerge" in a for a in arguments):
                fake.checks[:] = [check_run()]
            return original(repository, *arguments)

        with mock.patch.object(landing, "command", green_on_refusal), mock.patch.object(landing.time, "sleep"):
            pull, code = landing.land(".", "origin", 1, HEAD, 0)
        self.assertEqual((code, len(fake.enqueued)), (0, 1))

    def test_refused_arming_while_still_pending_is_reported(self):
        fake = FakeGitHub(checks=list(PENDING), refuse_arming="denied")
        with self.assertRaisesRegex(landing.LandingError, "denied"):
            run_land(self, fake)
        self.assertEqual(fake.enqueued, [])

    def test_red_check_refuses_without_any_mutation(self):
        for conclusion in ("FAILURE", "CANCELLED", "TIMED_OUT"):
            with self.subTest(conclusion=conclusion):
                fake = FakeGitHub(isDraft=True, checks=[check_run(conclusion=conclusion)])
                with self.assertRaisesRegex(landing.LandingError, f"compiler-check concluded {conclusion.lower()}"):
                    run_land(self, fake)
                self.assertEqual(fake.mutated, [])

    def test_changed_head_refuses_without_mutation(self):
        fake = FakeGitHub(isDraft=True, checks=list(PENDING), headRefOid="b" * 40)
        with self.assertRaisesRegex(landing.LandingError, "differs from --expected-head"):
            run_land(self, fake)
        self.assertEqual(fake.mutated, [])

    def test_merge_conflict_refuses_before_ready(self):
        for change in (dict(mergeStateStatus="DIRTY"), dict(mergeable="CONFLICTING")):
            with self.subTest(change=change):
                fake = FakeGitHub(isDraft=True, **change)
                with self.assertRaisesRegex(landing.LandingError, "merge conflict"):
                    run_land(self, fake)
                self.assertEqual(fake.mutated, [])

    def test_already_merged_queued_or_armed_repeats_no_mutation(self):
        for change in (dict(state="MERGED"), dict(mergeQueueEntry={"position": 1}),
                       dict(autoMergeRequest={"enabledAt": "x"}, isDraft=False)):
            with self.subTest(change=change):
                fake = FakeGitHub(checks=list(PENDING), **change)
                self.assertEqual(run_land(self, fake)[1], 0)
                self.assertEqual(fake.mutated, [])

    def test_closed_pull_request_refuses(self):
        fake = FakeGitHub(state="CLOSED")
        with self.assertRaisesRegex(landing.LandingError, "closed without merging"):
            run_land(self, fake)

    def test_waiting_follows_armed_queued_merged(self):
        fake = FakeGitHub(isDraft=True, checks=list(PENDING),
                          script=[lambda fake: None, lambda fake: None, lambda fake: None,
                                  queued_by_github, update(state="MERGED")])
        pull, code = run_land(self, fake, seconds=60)
        self.assertEqual((code, pull["state"], fake.enqueued), (0, "MERGED", []))

    def test_red_check_while_armed_is_reported_and_never_reenqueued(self):
        fake = FakeGitHub(checks=list(PENDING),
                          script=[lambda fake: None, lambda fake: None,
                                  lambda fake: fake.checks.__setitem__(0, check_run(conclusion="FAILURE"))])
        with self.assertRaisesRegex(landing.LandingError, "concluded failure.*stays armed"):
            run_land(self, fake, seconds=60)
        self.assertEqual(fake.enqueued, [])

    def test_conflict_found_by_the_queue_is_reported_as_conflict(self):
        fake = FakeGitHub(script=[lambda fake: None, lambda fake: None, update(mergeStateStatus="DIRTY")])
        with self.assertRaisesRegex(landing.LandingError, "merge conflict"):
            run_land(self, fake, seconds=60)

    def test_removal_from_queue_refuses_after_one_confirming_read_and_never_reenqueues(self):
        remove = update(mergeQueueEntry=None)
        fake = FakeGitHub(script=[lambda fake: None, lambda fake: None, remove, lambda fake: None])
        with self.assertRaisesRegex(landing.LandingError, "left the merge queue"):
            run_land(self, fake, seconds=60)
        self.assertEqual(len(fake.enqueued), 1)

    def test_queue_entry_that_lags_the_mutation_is_not_called_removal(self):
        lag = update(mergeQueueEntry=None)
        fake = FakeGitHub(script=[lambda fake: None, lambda fake: None, lag,
                                  update(mergeQueueEntry={"position": 1}), update(state="MERGED")])
        self.assertEqual(run_land(self, fake, seconds=60)[1], 0)

    def test_closed_while_waiting_refuses(self):
        fake = FakeGitHub(script=[lambda fake: None, lambda fake: None,
                                  update(state="CLOSED", mergeQueueEntry=None)])
        with self.assertRaisesRegex(landing.LandingError, "closed without merging"):
            run_land(self, fake, seconds=60)

    def test_deadline_while_armed_or_queued_exits_unfinished(self):
        for change in (dict(autoMergeRequest={"enabledAt": "x"}), dict(mergeQueueEntry={"position": 2})):
            with self.subTest(change=change), mock.patch.object(landing.time, "monotonic",
                                                                side_effect=[0.0] + [100.0] * 10):
                fake = FakeGitHub(checks=list(PENDING), **change)
                self.assertEqual(run_land(self, fake, seconds=1)[1], 2)

    def test_command_line_exit_codes_and_reasons(self):
        arguments = ["land", "1", "--expected-head", HEAD]
        cases = [(FakeGitHub(checks=list(PENDING)), [], 0, ""),
                 (FakeGitHub(checks=[check_run(conclusion="FAILURE")]), [], 1, "compiler-check concluded failure"),
                 (FakeGitHub(checks=list(PENDING)), ["--wait-seconds", "1"], 2, "not merged after 1s")]
        for fake, extra, code, reason in cases:
            with self.subTest(code=code), mock.patch.object(landing, "command", fake), \
                    mock.patch.object(landing.time, "sleep"), \
                    mock.patch.object(landing.time, "monotonic", side_effect=[0.0] + [100.0] * 10), \
                    mock.patch.object(sys, "stdout", new=mock.MagicMock()), \
                    mock.patch.object(sys, "stderr", new=mock.MagicMock()) as stderr:
                self.assertEqual(landing.main(arguments + extra), code)
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
        armed = dict(autoMergeRequest={"enabledAt": "x"})
        for checks, expected in (([check_run("IN_PROGRESS", None)], 2), ([check_run(conclusion="FAILURE")], 1)):
            with self.subTest(armed=expected), mock.patch.object(landing, "pull_request", return_value=dict(PULL, **armed)), \
                    mock.patch.object(landing, "head_check", return_value=checks[0]):
                self.assertEqual(landing.wait(".", "origin", 1, HEAD, 0)[1], expected)
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
