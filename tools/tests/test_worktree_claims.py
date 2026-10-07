#!/usr/bin/env python3
"""Claim-attachment matching tests for worktree_status; stdlib only, no network.

attach_claims must attach a live claim only when the worktree identifies the
claim's task — the normalized worktree name or its task token appearing as the
claim owner's task tail. Substring coincidence is not a match.
"""

from pathlib import Path
import tempfile
import unittest

import worktree_testkit as kit


UNDER_TEST = Path(__file__).resolve().parents[1] / "swarm" / "worktree_status.py"




def claim(owner, item="ITEM-ONE"):
    return {"owner": owner, "item": item, "board": None, "ticket": "T-1",
            "expires_utc": "2030-01-01T00:00:00+00:00"}


class ClaimAttachTests(unittest.TestCase):
    def setUp(self):
        self.module = kit.load_status_module()
        self.temporary = tempfile.TemporaryDirectory(prefix="omega claims test ")
        self.addCleanup(self.temporary.cleanup)
        self.repository = Path(self.temporary.name)

    def attach(self, names, owners):
        rows = [{"name": name, "claims": []} for name in names]
        live = [claim(owner) for owner in owners]
        unmatched = self.module.attach_claims(self.repository, rows, live)
        return rows, unmatched

    def test_exact_worktree_name_attaches(self):
        rows, unmatched = self.attach(
            ["macw3-normalized-abi"], ["Jarod / macw3-normalized-abi"])
        self.assertEqual(len(rows[0]["claims"]), 1)
        self.assertEqual(unmatched, [])

    def test_task_token_attaches_to_owner_task_tail(self):
        rows, unmatched = self.attach(
            ["macw3-frame-layout"], ["Jarod / macw3 frame-layout"])
        self.assertEqual(len(rows[0]["claims"]), 1)
        self.assertEqual(unmatched, [])

    def test_prefix_of_longer_task_does_not_attach(self):
        rows, unmatched = self.attach(
            ["macw3-normalized"], ["Jarod / macw3-normalized-abi"])
        self.assertEqual(rows[0]["claims"], [],
                         "a worktree that is only a prefix of the claim's task "
                         "must not steal it")
        self.assertEqual(len(unmatched), 1)

    def test_partial_token_does_not_attach(self):
        rows, unmatched = self.attach(
            ["normalized"], ["Jarod / macw3-normalized-abi"])
        self.assertEqual(rows[0]["claims"], [])
        self.assertEqual(len(unmatched), 1)

    def test_unrelated_worktree_does_not_attach(self):
        rows, unmatched = self.attach(
            ["w9-other-thing", "macw3-normalized-abi"],
            ["Jarod / macw3-normalized-abi"])
        self.assertEqual(rows[0]["claims"], [])
        self.assertEqual(len(rows[1]["claims"]), 1)
        self.assertEqual(unmatched, [])


if __name__ == "__main__":
    unittest.main()
