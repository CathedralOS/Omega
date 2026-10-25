#!/usr/bin/env python3
"""Native-queue client controls; no network or compiler builds."""

from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import landing

HEAD = "a" * 40
PULL = dict(id="PR_example", number=1, url="https://github.com/example/repo/pull/1",
            state="OPEN", baseRefName="main", headRefOid=HEAD,
            mergeStateStatus="CLEAN", mergeQueueEntry=None, mergeCommit=None)


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
