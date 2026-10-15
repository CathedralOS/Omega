#!/usr/bin/env python3
"""Real commands and Git integration; no remote coordination or compiler mocks."""

import copy
import io
import json
import os
from pathlib import Path
import subprocess
import stat
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import worktree_testkit as kit

TOOLS = Path(__file__).resolve().parents[1]
qualification = kit.load_module(TOOLS / "qualification.py", "qualification_under_test")
landing = kit.load_module(TOOLS / "landing.py", "landing_under_test")


class QualificationTests(unittest.TestCase):
    def setUp(self):
        self.root, self.repository = kit.seed_repository(self, "omega-qualification-")
        (self.repository / "src").mkdir()
        (self.repository / "src" / "program").write_text("stable source\n")
        (self.repository / "check.py").write_text("print('validated output')\n")
        (self.repository / ".gitignore").write_text("/build/\n")
        self.git("add", ".")
        self.git("commit", "-m", "tools: sample validation")
        self.base = self.git("rev-parse", "HEAD")
        self.library = self.root / "producer.rlib"
        self.library.write_bytes(b"pinned compiler dependency")
        self.state = self.root / "evidence.json"
        self.config = dict(context=dict(host="local-fixture", target="fixture", toolchain="python"), checks=[
            dict(id="values", kind="tests", command=[sys.executable, "check.py"],
                 inputs=["src", "check.py"], artifacts=[str(self.library)], closure_reason="complete tiny fixture",
                 final=False, expected_exit=0, timeout_seconds=10)])
        qualification.start(self.repository, self.state, self.base, self.config)

    def git(self, *arguments):
        return kit.git(self, self.repository, *arguments)

    def run_check(self, name="values"):
        record = qualification.load(self.state)
        return qualification.run_check(self.repository, self.state, record["revision"], name)

    def assess(self, base=None):
        record = qualification.load(self.state)
        return qualification.assess(self.repository, self.state, record["revision"], base or self.base)

    def commit(self, path, text):
        (self.repository / path).write_text(text)
        self.git("add", ".")
        self.git("commit", "-m", "tools: incoming change")
        return self.git("rev-parse", "HEAD")

    def test_source_input_batch_preserves_exact_existing_and_missing_objects(self):
        names = ["source with spaces Ω.txt"]
        if os.name != "nt":
            names += ["source\twith\ttabs", "source\nwith\nnewlines", os.fsdecode(b"source-\xff")]
        for name in names:
            (self.repository / name).write_text("source payload\n")
        self.git("add", ".")
        self.git("commit", "-m", "tools: unusual source paths")
        head = self.git("rev-parse", "HEAD")
        paths = ["src", "src/", "src/program", *names, "absent", "absent\npath", "src"]
        expected = {}
        for path in paths:
            query = subprocess.run(["git", "-C", str(self.repository), "rev-parse", head + ":" + path],
                                   capture_output=True, text=True)
            expected[path] = query.stdout.strip() if query.returncode == 0 else None
        actual_run = subprocess.run
        with mock.patch.object(qualification.subprocess, "run", wraps=actual_run) as commands:
            self.assertEqual(qualification.source_inputs(self.repository, head, iter(paths)), expected)
        batches = [call for call in commands.call_args_list if "cat-file" in call.args[0]]
        self.assertEqual(len(batches), 1)
        self.assertEqual(qualification.source_inputs(self.repository, self.base, names),
                         {name: None for name in names})

    def test_source_input_batch_retains_unmaterialized_gitlink_entry(self):
        unavailable = "1234567890abcdef1234567890abcdef12345678"
        self.git("update-index", "--add", "--cacheinfo", "160000," + unavailable + ",vendor/library")
        self.git("commit", "-m", "tools: unavailable gitlink object")
        head = self.git("rev-parse", "HEAD")
        self.assertEqual(self.git("rev-parse", head + ":vendor/library"), unavailable)
        self.assertEqual(qualification.source_inputs(self.repository, head, ["vendor/library", "absent"]),
                         {"vendor/library": unavailable, "absent": None})

    def test_source_input_batch_empty_paths_and_invalid_revision(self):
        self.assertEqual(qualification.source_inputs(self.repository, self.base, []), {})
        self.git("tag", "-a", "source-input-tag", "-m", "annotated tag")
        invalid = ["not-a-revision", "f" * 40,
                   self.git("rev-parse", self.base + ":src/program"),
                   self.git("rev-parse", "refs/tags/source-input-tag")]
        for value in invalid:
            for paths in ([], ["src"]):
                with self.subTest(revision=value, paths=paths):
                    with self.assertRaises(qualification.RecoveryError):
                        qualification.source_inputs(self.repository, value, paths)

    def test_source_input_batch_rejects_failed_truncated_and_extra_replies(self):
        header = self.base.encode("ascii") + b"\n"
        replies = [(1, b""), (0, header + b"not an object\n"),
                   (0, header + b"a" * 41 + b"\n"), (0, header + b"a" * 40),
                   (0, header + b"a" * 40 + b"\nextra\n"),
                   (0, b"0" * 40 + b"\n" + b"a" * 40 + b"\n")]
        for status, output in replies:
            with self.subTest(status=status, output=output):
                with mock.patch.object(qualification.subprocess, "run",
                        return_value=subprocess.CompletedProcess([], status, output, b"")):
                    with self.assertRaises(qualification.RecoveryError):
                        qualification.source_inputs(self.repository, self.base, ["src"])
    def test_summary_run_preserves_full_evidence_and_failed_exit(self):
        def execute():
            output = io.StringIO()
            revision = qualification.load(self.state)["revision"]
            with mock.patch("sys.stdout", output):
                status = qualification.main(["run", "--repository", str(self.repository),
                    "--state", str(self.state), "--revision", str(revision),
                    "--check", "values", "--summary-output"])
            return status, json.loads(output.getvalue())
        status, summary = execute()
        self.assertEqual(status, 0)
        self.assertEqual(summary["schema"], "omega-qualification-summary-v1")
        self.assertEqual(summary["state"], str(self.state.resolve()))
        presentation = self.root / "summary.json"
        presentation.write_text(json.dumps(summary))
        with self.assertRaisesRegex(qualification.RecoveryError, "unsupported qualification record"):
            qualification.load(presentation)
        record = qualification.load(self.state)
        self.assertEqual(summary["latest_run"]["receipt"], record["runs"][-1]["receipt"])
        self.assertTrue(record["runs"][-1]["source_stamps"])
        receipt = Path(record["runs"][-1]["receipt"]["path"])
        original_receipt = receipt.read_bytes()
        original_run = copy.deepcopy(record["runs"][-1])
        self.assertEqual(json.loads(qualification.state_output(record, self.state, False)), record)
        self.assertEqual(json.loads(qualification.state_output(dict(binding="full"), None, True)),
                         dict(binding="full"))
        failing = self.commit("check.py", "raise SystemExit(7)\n")
        self.assess(failing)
        status, summary = execute()
        self.assertEqual(status, 1)
        self.assertEqual(summary["latest_run"]["exit"], 7)
        record = qualification.load(self.state)
        self.assertEqual(record["runs"][0], original_run)
        self.assertEqual(receipt.read_bytes(), original_receipt)
        with self.assertRaisesRegex(qualification.RecoveryError, "requires affected/final checks"):
            qualification.validate(self.repository, self.state, failing, failing)
        revision = record["revision"]
        with mock.patch("sys.stdout", io.StringIO()), mock.patch("sys.stderr", io.StringIO()):
            self.assertEqual(qualification.main(["run", "--repository", str(self.repository),
                "--state", str(self.state), "--revision", str(revision - 1),
                "--check", "values", "--summary-output"]), 1)
        self.assertEqual(qualification.load(self.state)["revision"], revision)

    def test_snapshot_preserves_normal_utf8_encoding_refusals(self):
        text = self.state.read_text(encoding="utf-8")
        for payload in (text.encode("utf-16"), b"\xef\xbb\xbf" + text.encode("utf-8")):
            with self.subTest(prefix=payload[:4]):
                self.state.write_bytes(payload)
                with self.assertRaisesRegex(qualification.RecoveryError, "missing or invalid JSON"):
                    qualification.load(self.state)
                with self.assertRaisesRegex(qualification.RecoveryError, "missing or invalid JSON"):
                    qualification._RunSnapshot.read(self.state)

    def test_snapshot_isolated_projection_and_single_use(self):
        initial = qualification.load(self.state)
        snap = qualification._RunSnapshot.read(self.state)
        context = snap.context('values')
        context['config']['checks'][0]['expected_exit'] = 99
        with mock.patch.object(qualification, 'load', side_effect=AssertionError('unexpected decode')):
            reserved = snap.update(self.state, 0, lambda r: r.update(marker='reserved'), retain=True)
            self.assertEqual(reserved.revision, 1)
            self.assertEqual(reserved.context('values')['config'], initial['config'])
            with self.assertRaisesRegex(qualification.RecoveryError, 'already consumed'):
                snap.context('values')
            result = reserved.update(self.state, 1, lambda r: r.update(marker='finished'), retain=False)
        self.assertEqual(result['config'], initial['config'])
        self.assertEqual(result['revision'], 2)
        self.assertEqual(qualification.load(self.state), result)
        with self.assertRaisesRegex(qualification.RecoveryError, 'already consumed'):
            reserved.context('values')

    def test_snapshot_intervening_writer_preserves_stale_revision_guard(self):
        snap = qualification._RunSnapshot.read(self.state)
        current = qualification.update(self.state, 0, lambda r: r.update(marker='other writer'))
        before = self.state.read_bytes()
        with mock.patch.object(qualification, 'load', wraps=qualification.load) as loader:
            with self.assertRaisesRegex(qualification.RecoveryError, 'stale qualification revision'):
                snap.update(self.state, 0, lambda r: r.update(marker='lost writer'), retain=False)
            self.assertEqual(loader.call_count, 1)
        self.assertEqual(self.state.read_bytes(), before)
        self.assertEqual(qualification.load(self.state), current)
        with self.assertRaisesRegex(qualification.RecoveryError, 'already consumed'):
            snap.context('values')

    def test_snapshot_malformed_replacement_refuses(self):
        snap = qualification._RunSnapshot.read(self.state)
        self.state.write_bytes(b'{malformed')
        with self.assertRaisesRegex(qualification.RecoveryError, 'missing or invalid JSON'):
            snap.update(self.state, 0, lambda r: None, retain=False)
        self.assertEqual(self.state.read_bytes(), b'{malformed')
        with self.assertRaisesRegex(qualification.RecoveryError, 'already consumed'):
            snap.context('values')

    def test_snapshot_same_content_replacement_is_safe(self):
        snap = qualification._RunSnapshot.read(self.state)
        before = self.state.read_bytes()
        qualification.atomic_json(self.state, qualification.load(self.state), indent=None)
        self.assertEqual(self.state.read_bytes(), before)
        with mock.patch.object(qualification, 'load', side_effect=AssertionError('unnecessary decode')):
            result = snap.update(self.state, 0, lambda r: r.update(marker='same bytes'), retain=False)
        self.assertEqual(result['revision'], 1)
        self.assertEqual(qualification.load(self.state), result)

    def test_snapshot_path_mismatch_consumes_without_writing(self):
        snap = qualification._RunSnapshot.read(self.state)
        before = self.state.read_bytes()
        other = self.root / 'other.json'
        other.write_bytes(before)
        with self.assertRaisesRegex(qualification.RecoveryError, 'snapshot path mismatch'):
            snap.update(other, 0, lambda r: None, retain=False)
        self.assertEqual(self.state.read_bytes(), before)
        self.assertEqual(other.read_bytes(), before)
        with self.assertRaisesRegex(qualification.RecoveryError, 'already consumed'):
            snap.context('values')

    def test_snapshot_callback_failure_discards_dirty_memory(self):
        snap = qualification._RunSnapshot.read(self.state)
        before = self.state.read_bytes()

        def fail(record):
            record['runs'].append({'dirty': True})
            raise RuntimeError('callback fault')
        with self.assertRaisesRegex(RuntimeError, 'callback fault'):
            snap.update(self.state, 0, fail, retain=True)
        self.assertEqual(self.state.read_bytes(), before)
        with self.assertRaisesRegex(qualification.RecoveryError, 'already consumed'):
            snap.context('values')
        self.assertEqual(qualification._RunSnapshot.read(self.state).revision, 0)

    def test_snapshot_write_failure_discards_dirty_memory(self):
        snap = qualification._RunSnapshot.read(self.state)
        before = self.state.read_bytes()
        with mock.patch.object(qualification.os, 'replace', side_effect=OSError('replace fault')):
            with self.assertRaisesRegex(OSError, 'replace fault'):
                snap.update(self.state, 0, lambda r: r.update(marker='dirty'), retain=True)
        self.assertEqual(self.state.read_bytes(), before)
        self.assertEqual(list(self.root.glob('.evidence.json.*')), [])
        with self.assertRaisesRegex(qualification.RecoveryError, 'already consumed'):
            snap.context('values')

    def test_snapshot_real_run_retains_receipts_without_redundant_decode(self):
        with mock.patch.object(qualification, 'load', side_effect=AssertionError('redundant full-state decode')):
            first, accepted = qualification.run_check(self.repository, self.state, 0, 'values')
            self.assertTrue(accepted)
            second, accepted = qualification.run_check(self.repository, self.state, first['revision'], 'values')
            self.assertTrue(accepted)
        self.assertEqual(len(second['runs']), 2)
        self.assertEqual(second['runs'][0], first['runs'][0])
        self.assertTrue(second['runs'][-1]['repeated_work'])
        self.assertEqual(qualification.load(self.state), second)
        for run in second['runs']:
            receipt = Path(run['receipt']['path'])
            self.assertEqual(qualification.file_digest(receipt), run['receipt']['sha256'])
            self.assertEqual(json.loads(receipt.read_bytes()), {k: v for k, v in run.items() if k != 'receipt'})

    def test_snapshot_running_reservation_is_not_overwritten(self):
        qualification.update(self.state, 0, lambda r: r.update(running={'id': 'other', 'check': 'values', 'candidate': self.base}))
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, 'already running'):
            qualification.run_check(self.repository, self.state, 1, 'values')
        self.assertEqual(self.state.read_bytes(), before)


    def test_unrelated_main_change_reuses_without_running_command_again(self):
        self.assertTrue(self.run_check()[1])
        incoming = self.commit("README.md", "unrelated documentation")
        record = self.assess(incoming)
        decision = record["assessments"][-1]
        self.assertEqual(decision["incoming_paths"], ["README.md"])
        self.assertEqual(decision["checks"][0]["action"], "reuse")
        self.assertEqual(decision["checks"][0]["observed_revision"], self.base)
        self.assertEqual(len(record["runs"]), 1)
        self.assertGreaterEqual(record["runs"][0]["duration_seconds"], 0)
        qualification.validate(self.repository, self.state, incoming, incoming)

    def test_affected_addition_and_deletion_invalidate_whole_dependency_tree(self):
        self.run_check()
        incoming = self.commit("src/added", "new behavior")
        record = self.assess(incoming)
        self.assertIn("source_changed:src", record["assessments"][-1]["checks"][0]["reasons"])
        self.assertTrue(self.run_check()[1])
        self.git("rm", "src/added")
        self.git("commit", "-m", "tools: remove behavior")
        record = self.assess(self.git("rev-parse", "HEAD"))
        self.assertIn("source_changed:src", record["assessments"][-1]["checks"][0]["reasons"])

    def source_reuse_review(self, base):
        record = qualification.load(self.state)
        check = record["config"]["checks"][0]
        review = dict(binding=qualification.reuse_binding(self.repository, record, check, record["runs"][-1],
                                                        self.git("rev-parse", "HEAD"), base),
                      accepted=True, reviewer="independent fixture reviewer",
                      rationale="The changed source branch is outside the captured command's exercised behavior.")
        path = self.root / "scope-review.json"
        path.write_text(json.dumps(review))
        return path

    def attest_source_reuse(self, base):
        review = self.source_reuse_review(base)
        record = qualification.load(self.state)
        return qualification.attest_source_reuse(self.repository, self.state, record["revision"], "values", base, review)

    def prepare_batch_source_reuse(self):
        configuration = copy.deepcopy(self.config)
        second = copy.deepcopy(configuration["checks"][0])
        second["id"] = "other-values"
        other_library = self.root / "other-producer.rlib"
        other_library.write_bytes(b"other pinned dependency")
        second["artifacts"] = [str(other_library)]
        configuration["checks"].append(second)
        record = qualification.load(self.state)
        qualification.amend(self.state, record["revision"], configuration,
                            "exercise independent review collection", base=self.base)
        self.assertTrue(self.run_check()[1])
        self.assertTrue(self.run_check("other-values")[1])
        incoming = self.commit("src/program", "changed unexercised branch")
        record = qualification.load(self.state)
        reviews = []
        for check in record["config"]["checks"]:
            run = [value for value in record["runs"] if value["check"] == check["id"]][-1]
            reviews.append(dict(binding=qualification.reuse_binding(
                self.repository, record, check, run, incoming, incoming),
                accepted=True, reviewer="independent fixture reviewer",
                rationale="Complete exact delta does not affect the exercised branch."))
        path = self.root / "batch-review.json"
        document = dict(schema="qualification-source-reuse-batch-review-1", reviews=reviews)
        path.write_text(json.dumps(document))
        return incoming, record, path, document, other_library

    def test_batch_source_reuse_registers_all_reviews_in_one_update(self):
        incoming, before, path, _, _ = self.prepare_batch_source_reuse()
        with mock.patch.object(qualification, "update", wraps=qualification.update) as update:
            with mock.patch("sys.stdout", io.StringIO()):
                status = qualification.main(["attest-source-reuse-batch", "--repository", str(self.repository),
                    "--state", str(self.state), "--revision", str(before["revision"]),
                    "--base", incoming, "--review", str(path), "--summary-output"])
        self.assertEqual(status, 0)
        self.assertEqual(update.call_count, 1)
        record = qualification.load(self.state)
        self.assertEqual(record["revision"], before["revision"] + 1)
        self.assertEqual(record["runs"], before["runs"])
        self.assertEqual(len(record["source_reuse"]), 2)
        rows = self.assess(incoming)["assessments"][-1]["checks"]
        self.assertEqual([row["action"] for row in rows], ["reuse", "reuse"])
        self.assertEqual([row["observed_revision"] for row in rows], [self.base, self.base])
        qualification.validate(self.repository, self.state, incoming, incoming)
        path.write_text("{}")
        self.assertTrue(all(row["action"] == "rerun" for row in self.assess(incoming)["assessments"][-1]["checks"]))

    def test_batch_source_reuse_refuses_invalid_second_binding_without_partial_registration(self):
        incoming, record, path, document, _ = self.prepare_batch_source_reuse()
        document["reviews"][1]["binding"]["candidate"] = self.base
        path.write_text(json.dumps(document))
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "exact observation"):
            qualification.attest_source_reuse(self.repository, self.state, record["revision"], None, incoming, path)
        self.assertEqual(self.state.read_bytes(), before)
        self.assertEqual(list(self.state.with_suffix(".records").glob("*.reuse.json")), [])

    def test_batch_source_reuse_preserves_each_artifact_boundary(self):
        incoming, record, path, _, other_library = self.prepare_batch_source_reuse()
        other_library.write_bytes(b"substituted dependency")
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "only source changes"):
            qualification.attest_source_reuse(self.repository, self.state, record["revision"], None, incoming, path)
        self.assertEqual(self.state.read_bytes(), before)
        self.assertEqual(list(self.state.with_suffix(".records").glob("*.reuse.json")), [])

    def test_batch_source_reuse_rejects_duplicate_or_empty_collections(self):
        incoming, record, path, document, _ = self.prepare_batch_source_reuse()
        for reviews in ([], [document["reviews"][0], document["reviews"][0]]):
            with self.subTest(reviews=len(reviews)):
                path.write_text(json.dumps(dict(schema=document["schema"], reviews=reviews)))
                before = self.state.read_bytes()
                with self.assertRaises(qualification.RecoveryError):
                    qualification.attest_source_reuse(self.repository, self.state, record["revision"], None, incoming, path)
                self.assertEqual(self.state.read_bytes(), before)
                self.assertEqual(list(self.state.with_suffix(".records").glob("*.reuse.json")), [])

    def test_reviewed_unexercised_source_change_retains_original_observation(self):
        self.run_check()
        incoming = self.commit("src/program", "changed unexercised branch")
        self.attest_source_reuse(incoming)
        record = self.assess(incoming)
        row = record["assessments"][-1]["checks"][0]
        self.assertEqual(row["action"], "reuse")
        self.assertEqual(row["reasons"], ["reviewed_source_scope_unchanged"])
        self.assertEqual(row["observed_revision"], self.base)
        self.assertEqual(len(record["runs"]), 1)
        qualification.validate(self.repository, self.state, incoming, incoming)

    def test_source_reuse_binds_candidate_base_and_complete_blob_changes(self):
        self.run_check()
        self.commit("src/program", "changed branch")
        incoming = self.commit("README.md", "outside closure, still part of review delta")
        review = self.source_reuse_review(incoming)
        value = json.loads(review.read_text())
        del value["binding"]["changes"]["README.md"]
        review.write_text(json.dumps(value))
        record = qualification.load(self.state)
        with self.assertRaisesRegex(qualification.RecoveryError, "complete blob delta"):
            qualification.attest_source_reuse(self.repository, self.state, record["revision"], "values", incoming, review)
        self.attest_source_reuse(incoming)
        wrong_base = self.assess(self.base)["assessments"][-1]["checks"][0]
        self.assertIn("source_changed:src", wrong_base["reasons"])
        later = self.commit("src/uncovered", "another source change")
        changed = self.assess(later)["assessments"][-1]["checks"][0]
        self.assertIn("source_changed:src", changed["reasons"])

    def test_source_reuse_preserves_artifact_environment_and_receipt_invalidations(self):
        self.run_check()
        incoming = self.commit("src/program", "unexercised branch")
        self.attest_source_reuse(incoming)
        self.library.write_bytes(b"replaced producer")
        row = self.assess(incoming)["assessments"][-1]["checks"][0]
        self.assertIn("compiler_or_artifact_changed", row["reasons"])
        self.library.write_bytes(b"pinned compiler dependency")
        with mock.patch.dict(os.environ, {"CARGO_PROFILE_DEV_OPT_LEVEL": "changed"}):
            row = self.assess(incoming)["assessments"][-1]["checks"][0]
        self.assertIn("environment_changed", row["reasons"])
        run = qualification.load(self.state)["runs"][-1]
        Path(run["receipt"]["path"]).write_text("{}")
        row = self.assess(incoming)["assessments"][-1]["checks"][0]
        self.assertIn("observation_record_mismatch", row["reasons"])

    def test_changed_reuse_review_or_attestation_cannot_cover_source_changes(self):
        self.run_check()
        incoming = self.commit("src/program", "unexercised branch")
        record = self.attest_source_reuse(incoming)
        receipt = Path(record["source_reuse"][-1]["path"])
        captured = receipt.read_text()
        receipt.write_text("{}")
        row = self.assess(incoming)["assessments"][-1]["checks"][0]
        self.assertIn("source_changed:src", row["reasons"])
        receipt.write_text(captured)
        (self.root / "scope-review.json").write_text("{}")
        row = self.assess(incoming)["assessments"][-1]["checks"][0]
        self.assertIn("source_changed:src", row["reasons"])

    def test_source_reuse_cannot_cover_final_candidate_checks(self):
        config = copy.deepcopy(self.config)
        config["checks"][0]["final"] = True
        qualification.amend(self.state, 0, config, "final review required")
        self.run_check()
        incoming = self.commit("src/program", "unexercised branch")
        with self.assertRaisesRegex(qualification.RecoveryError, "final-candidate"):
            self.attest_source_reuse(incoming)
        row = self.assess(incoming)["assessments"][-1]["checks"][0]
        self.assertIn("final_candidate_required", row["reasons"])

    def test_source_reuse_cannot_survive_changed_check_definition(self):
        self.run_check()
        incoming = self.commit("src/program", "unexercised branch")
        record = self.attest_source_reuse(incoming)
        config = copy.deepcopy(self.config)
        config["checks"][0]["timeout_seconds"] = 11
        qualification.amend(self.state, record["revision"], config, "check definition changed")
        row = self.assess(incoming)["assessments"][-1]["checks"][0]
        self.assertIn("check_definition_changed", row["reasons"])
        self.assertIn("source_changed:src", row["reasons"])
        self.assertNotIn("source_reuse", row)

    def test_landing_revalidates_source_reuse_receipt(self):
        self.run_check()
        incoming = self.commit("src/program", "unexercised branch")
        record = self.attest_source_reuse(incoming)
        self.assess(incoming)
        with mock.patch("sys.stdout"):
            self.assertEqual(landing.verify(self.repository, incoming), 0)
        self.assertIsNone(landing.receipt_failure(self.repository, incoming, set(), base=incoming,
                                                qualification_path=self.state))
        Path(record["source_reuse"][-1]["path"]).write_text("{}")
        refusal = landing.receipt_failure(self.repository, incoming, set(), base=incoming,
                                          qualification_path=self.state)
        self.assertIn("qualification no longer valid", refusal)

    def test_reuse_attestation_does_not_relax_new_capture_producer_source_equality(self):
        config = copy.deepcopy(self.config)
        config["checks"][0]["producer"] = dict(revision=self.base, inputs=["src"])
        qualification.amend(self.state, 0, config, "pin producer inputs")
        self.run_check()
        incoming = self.commit("src/program", "unexercised branch")
        self.attest_source_reuse(incoming)
        self.assess(incoming)
        with self.assertRaisesRegex(qualification.RecoveryError, "producer source/dependencies"):
            self.run_check()

    def test_run_pending_executes_only_affected_and_final_checks(self):
        self.run_check()
        incoming = self.commit("README.md", "unrelated")
        revision = qualification.load(self.state)["revision"]
        with mock.patch("sys.stdout"):
            exit_code = qualification.main(["run-pending", "--repository", str(self.repository),
                "--state", str(self.state), "--revision", str(revision), "--base", incoming])
        self.assertEqual(exit_code, 0)
        self.assertEqual(len(qualification.load(self.state)["runs"]), 1)
        affected = self.commit("src/program", "changed")
        revision = qualification.load(self.state)["revision"]
        with mock.patch("sys.stdout"):
            exit_code = qualification.main(["run-pending", "--repository", str(self.repository),
                "--state", str(self.state), "--revision", str(revision), "--base", affected])
        self.assertEqual(exit_code, 0)
        self.assertEqual(len(qualification.load(self.state)["runs"]), 2)

    def test_final_check_cannot_inherit_previous_candidate_acceptance(self):
        path = self.root / "final.json"
        config = copy.deepcopy(self.config)
        config["checks"][0]["final"] = True
        qualification.amend(self.state, 0, config, "final review of candidate required")
        qualification.run_check(self.repository, self.state, 1, "values")
        incoming = self.commit("README.md", "unrelated")
        record = qualification.assess(self.repository, self.state, 3, incoming)
        self.assertEqual(record["assessments"][-1]["checks"][0]["reasons"], ["final_candidate_required"])
        with self.assertRaisesRegex(qualification.RecoveryError, "affected/final"):
            qualification.validate(self.repository, self.state, incoming, incoming)

    def test_replaced_linked_library_invalidates_even_when_git_is_unchanged(self):
        self.run_check()
        self.library.write_bytes(b"another concurrent producer")
        record = self.assess()
        self.assertIn("compiler_or_artifact_changed", record["assessments"][-1]["checks"][0]["reasons"])

    def test_library_replaced_during_successful_capture_is_not_accepted(self):
        script = "from pathlib import Path\nPath(" + repr(str(self.library)) + ").write_bytes(b'replaced')\nprint('PASS')\n"
        head = self.commit("check.py", script)
        self.assess(head)
        record, accepted = self.run_check()
        self.assertFalse(accepted)
        self.assertEqual(record["runs"][-1]["exit"], 0)
        self.assertIn("artifact_changed_during_run", record["runs"][-1]["invalidations"])

    def test_environment_change_requires_requalification(self):
        self.run_check()
        with mock.patch.dict(os.environ, {"CARGO_PROFILE_DEV_OPT_LEVEL": "changed"}):
            record = self.assess()
        self.assertIn("environment_changed", record["assessments"][-1]["checks"][0]["reasons"])

    def test_recorded_source_and_observation_tampering_refused(self):
        self.run_check()
        original = qualification.load(self.state)
        for mutate, reason in ((lambda r: r["runs"][0]["source_inputs"].update(src="wrong"), "recorded_source"),
                               (lambda r: Path(r["runs"][0]["stdout"]["path"]).write_text("tampered"), "observation_changed")):
            record = copy.deepcopy(original)
            mutate(record)
            qualification.atomic_json(self.state, record)
            assessment = self.assess()["assessments"][-1]["checks"][0]
            self.assertTrue(any(reason in value for value in assessment["reasons"]))

    def test_stale_writer_and_mismatched_definition_refused(self):
        self.run_check()
        with self.assertRaisesRegex(qualification.RecoveryError, "stale"):
            qualification.assess(self.repository, self.state, 0, self.base)
        record = qualification.load(self.state)
        record["config"]["checks"][0]["expected_exit"] = 99
        qualification.atomic_json(self.state, record)
        with self.assertRaisesRegex(qualification.RecoveryError, "definition mismatch"):
            qualification.load(self.state)

    def test_concurrent_process_cannot_overwrite_or_duplicate_running_check(self):
        self.commit("check.py", "import time\ntime.sleep(0.5)\nprint('validated')\n")
        self.assess(self.git("rev-parse", "HEAD"))
        revision = qualification.load(self.state)["revision"]
        command = [sys.executable, str(TOOLS / "qualification.py"), "run", "--repository", str(self.repository),
                   "--state", str(self.state), "--revision", str(revision), "--check", "values"]
        processes = [subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE) for _ in range(2)]
        results = [process.communicate(timeout=15) for process in processes]
        self.assertEqual(sorted(process.returncode for process in processes), [0, 1], results)
        record = qualification.load(self.state)
        self.assertEqual(len(record["runs"]), 1)
        self.assertIsNone(record["running"])

    def test_moved_head_without_integration_assessment_refused(self):
        self.commit("README.md", "main advanced")
        with self.assertRaisesRegex(qualification.RecoveryError, "HEAD moved"):
            self.run_check()

    def test_stale_candidate_and_wrong_worktree_refused(self):
        self.run_check()
        self.assess()
        candidate = self.commit("README.md", "changed candidate")
        with self.assertRaisesRegex(qualification.RecoveryError, "stale"):
            qualification.validate(self.repository, self.state, candidate, self.base)
        with self.assertRaisesRegex(qualification.RecoveryError, "worktree mismatch"):
            qualification.validate(self.root, self.state, self.base, self.base)

    def test_failed_validator_remains_pending_and_explicit_repeat_is_measured(self):
        self.run_check()
        record, accepted = self.run_check()
        self.assertTrue(accepted)
        self.assertTrue(record["runs"][-1]["repeated_work"])
        candidate = self.commit("check.py", "raise SystemExit(4)\n")
        self.assess(candidate)
        self.assertFalse(self.run_check()[1])
        self.assertIn("prior_run_not_accepted", self.assess(candidate)["assessments"][-1]["checks"][0]["reasons"])

    def test_exact_landing_receipt_keeps_qualification_and_rechecks_artifacts(self):
        self.run_check()
        candidate = self.commit("README.md", "ready change")
        self.assess(self.base)
        with mock.patch("sys.stdout"):
            self.assertEqual(landing.verify(self.repository, self.base, self.state), 0)
        self.assertIsNone(landing.receipt_failure(self.repository, candidate, set()))
        self.library.write_bytes(b"replaced before publication")
        self.assertIn("qualification no longer valid", landing.receipt_failure(self.repository, candidate, set()))

    def test_stale_compiler_source_refused_before_command_runs(self):
        path = self.root / "producer.json"
        config = copy.deepcopy(self.config)
        config["checks"][0]["kind"] = "native"
        config["checks"][0]["producer"] = dict(revision=self.base, inputs=["src"])
        qualification.amend(self.state, 0, config, "compiler provenance added")
        candidate = self.commit("src/program", "new compiler semantics")
        qualification.assess(self.repository, self.state, 1, candidate)
        with self.assertRaisesRegex(qualification.RecoveryError, "producer source/dependencies"):
            qualification.run_check(self.repository, self.state, 2, "values")

    def test_rebuilt_producer_amendment_invalidates_and_retains_prior_evidence(self):
        self.run_check()
        config = copy.deepcopy(self.config)
        config["checks"][0]["producer"] = dict(revision=self.base, inputs=["src"])
        record = qualification.amend(self.state, 2, config, "reviewed exact rebuilt producer")
        self.assertEqual(len(record["runs"]), 1)
        self.assertEqual(len(record["definitions"]), 1)
        self.assertIn("check_definition_changed", self.assess()["assessments"][-1]["checks"][0]["reasons"])
        self.assertTrue(self.run_check()[1])

    def test_ordinary_verify_automatically_requires_active_qualification(self):
        self.commit("README.md", "candidate")
        with self.assertRaisesRegex(landing.LandingError, "unfinished"):
            landing.verify(self.repository, self.base)

    def test_missing_declared_dependency_cannot_establish_acceptance(self):
        config = copy.deepcopy(self.config)
        config["checks"][0]["inputs"] = ["does-not-exist"]
        qualification.amend(self.state, 0, config, "test incorrect closure")
        with self.assertRaisesRegex(qualification.RecoveryError, "dependency absent"):
            self.run_check()

    def test_semantic_mapping_requires_final_review_and_prior_canonical_authority(self):
        (self.repository / "wiki/spec").mkdir(parents=True)
        base = self.commit("wiki/spec/contract.md", "existing canonical contract")
        self.assess(base)
        config = copy.deepcopy(self.config)
        config["semantic_contracts"] = [dict(implementation=["src/program"], contract="wiki/spec/contract.md",
            classification="missing_implementation", reason="implement the existing declared behavior",
            passages=["existing canonical contract"])]
        with self.assertRaisesRegex(qualification.RecoveryError, "final-candidate review"):
            qualification.amend(self.state, 1, config, "semantic repair")
        review = copy.deepcopy(config["checks"][0])
        review.update(id="review", kind="review", final=True)
        config["checks"].append(review)
        qualification.amend(self.state, 1, config, "independent reviewer checks canonical mapping")
        candidate = self.commit("wiki/spec/contract.md", "invented semantics in the same candidate")
        with self.assertRaisesRegex(qualification.RecoveryError, "retroactive authority"):
            self.assess(base)

    def canonical_amendment_fixture(self):
        (self.repository / "wiki/spec").mkdir(parents=True)
        original_base = self.commit("wiki/spec/contract.md", "The prior canonical rule.\n")
        self.assess(original_base)
        original = copy.deepcopy(self.config)
        review = copy.deepcopy(original["checks"][0])
        review.update(id="review", kind="review", final=True)
        original["checks"].append(review)
        original["semantic_contracts"] = [dict(implementation=["src/program"],
            contract="wiki/spec/contract.md", classification="missing_implementation",
            reason="implement the existing canonical behavior", passages=["The prior canonical rule."])]
        qualification.amend(self.state, 1, original, "retain prior canonical context")
        self.assertTrue(self.run_check()[1])
        historical = qualification.load(self.state)
        next_base = self.commit("wiki/spec/contract.md", "The revised canonical rule.\n")
        candidate = self.commit("src/program", "implementation of revised rule\n")
        revised = copy.deepcopy(original)
        revised["semantic_contracts"][0]["passages"] = ["The revised canonical rule."]
        return original_base, next_base, candidate, revised, historical

    def test_amend_transitions_to_explicit_canonical_base_and_preserves_history(self):
        original_base, base, candidate, config, historical = self.canonical_amendment_fixture()
        configuration = self.root / "revised-config.json"
        configuration.write_text(json.dumps(config))
        output, errors = io.StringIO(), io.StringIO()
        with mock.patch("sys.stdout", output), mock.patch("sys.stderr", errors):
            status = qualification.main(["amend", "--repository", str(self.repository),
                "--state", str(self.state), "--revision", str(historical["revision"]),
                "--config", str(configuration), "--base", base, "--summary-output",
                "--reason", "reviewed incoming canonical change"])
        self.assertEqual(status, 0, errors.getvalue())
        record = qualification.load(self.state)
        self.assertEqual(record["canonical_context"][0]["revision"], base)
        self.assertEqual(record["canonical_context"][0]["blob"],
                         self.git("rev-parse", base + ":wiki/spec/contract.md"))
        self.assertEqual(record["runs"], historical["runs"])
        self.assertEqual(record["assessments"], historical["assessments"])
        self.assertEqual(record["definitions"][-1]["config"], historical["config"])
        self.assertEqual(record["assessments"][-1]["base"], original_base)
        self.assertEqual(json.loads(output.getvalue())["revision"], record["revision"])
        assessment = qualification.assess(self.repository, self.state, record["revision"], base)
        self.assertEqual(assessment["assessments"][-1]["candidate"], candidate)
        self.assertTrue(all(row["action"] == "rerun" for row in assessment["assessments"][-1]["checks"]))

    def test_canonical_amendment_without_new_base_still_refuses(self):
        _, base, _, config, historical = self.canonical_amendment_fixture()
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "retroactive authority"):
            qualification.amend(self.state, historical["revision"], config, "new passages")
        with self.assertRaisesRegex(qualification.RecoveryError, "missing/stale"):
            qualification.assess(self.repository, self.state, historical["revision"], base)
        self.assertEqual(self.state.read_bytes(), before)

    def test_canonical_amendment_refuses_backwards_base(self):
        _, _, _, config, historical = self.canonical_amendment_fixture()
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "compatible ancestor"):
            qualification.amend(self.state, historical["revision"], config,
                                "cannot roll authority backwards", base=self.base)
        self.assertEqual(self.state.read_bytes(), before)

    def test_canonical_amendment_refuses_unrelated_base(self):
        original, _, candidate, config, historical = self.canonical_amendment_fixture()
        self.git("checkout", "-b", "unrelated-authority", original)
        self.commit("check.py", "print('another branch')\n")
        unrelated = self.commit("wiki/spec/contract.md", "The revised canonical rule.\n")
        self.git("checkout", candidate)
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "compatible ancestor"):
            qualification.amend(self.state, historical["revision"], config,
                                "unrelated authority branch", base=unrelated)
        self.assertEqual(self.state.read_bytes(), before)

    def test_canonical_amendment_refuses_candidate_authored_contract_change(self):
        _, base, _, config, historical = self.canonical_amendment_fixture()
        self.commit("wiki/spec/contract.md", "The revised canonical rule.\nCandidate invention.\n")
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "retroactive authority"):
            qualification.amend(self.state, historical["revision"], config,
                                "candidate text cannot create authority", base=base)
        self.assertEqual(self.state.read_bytes(), before)

    def test_language_change_mapping_cannot_replace_owner_authorization(self):
        config = copy.deepcopy(self.config)
        config["semantic_contracts"] = [dict(implementation=["src/program"], contract="wiki/spec/contract.md",
            classification="owner_decision", reason="a proposed new feature", passages=["prior contract"])]
        with self.assertRaisesRegex(qualification.RecoveryError, "explicit owner's decision"):
            qualification.definition(config)

    def test_failed_or_mutated_run_fields_cannot_manufacture_acceptance(self):
        candidate = self.commit("check.py", "raise SystemExit(4)\n")
        self.assess(candidate)
        self.run_check()
        original = qualification.load(self.state)
        changes = {"exit": 0, "invalidations": [], "producer": {"revision": candidate},
                   "command": ["a different command"], "duration_seconds": 0}
        for name, value in changes.items():
            with self.subTest(field=name):
                record = copy.deepcopy(original)
                record["runs"][-1][name] = value
                # The empty invalidation field originally has no reason; mutate
                # it to a fabricated reason so the observation digest differs.
                if name == "invalidations":
                    record["runs"][-1][name] = ["edited"]
                qualification.atomic_json(self.state, record)
                decision = self.assess(candidate)["assessments"][-1]["checks"][0]
                self.assertIn("observation_record_mismatch", decision["reasons"])
                with self.assertRaises(qualification.RecoveryError):
                    qualification.validate(self.repository, self.state, candidate, candidate)

    def test_replaced_read_then_restored_library_is_not_stable_capture(self):
        code = ("from pathlib import Path\np=Path(" + repr(str(self.library)) + ")\n"
                "original=p.read_bytes()\np.write_bytes(b'replacement producer')\n"
                "print(p.read_bytes())\np.write_bytes(original)\n")
        candidate = self.commit("check.py", code)
        self.assess(candidate)
        record, accepted = self.run_check()
        self.assertEqual(self.library.read_bytes(), b"pinned compiler dependency")
        self.assertFalse(accepted)
        self.assertIn("artifact_mutated_during_run", record["runs"][-1]["invalidations"])

    def test_publication_receipt_cannot_use_another_base_or_registration(self):
        self.run_check()
        candidate = self.commit("README.md", "publish")
        self.assess(self.base)
        with mock.patch("sys.stdout"):
            landing.verify(self.repository, self.base)
        self.assertIn("reserved publication base", landing.receipt_failure(
            self.repository, candidate, set(), base=candidate))
        self.assertIn("active worktree registration", landing.receipt_failure(
            self.repository, candidate, set(), base=self.base, qualification_path=self.root / "other.json"))

    def test_source_replaced_and_restored_during_capture_cannot_pass(self):
        code = ("from pathlib import Path\np=Path('src/program')\noriginal=p.read_bytes()\n"
                "p.write_bytes(b'temporary other source')\nprint(p.read_bytes())\np.write_bytes(original)\n")
        candidate = self.commit("check.py", code)
        self.assess(candidate)
        record, accepted = self.run_check()
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertFalse(accepted)
        self.assertIn("source_mutated_during_run", record["runs"][-1]["invalidations"])

    def test_canonical_context_contains_exact_passage_revision_and_blob(self):
        (self.repository / "wiki/spec").mkdir(parents=True)
        base = self.commit("wiki/spec/contract.md", "The actual canonical rule.\nOther unrelated text.\n")
        config = copy.deepcopy(self.config)
        review = copy.deepcopy(config["checks"][0])
        review.update(id="review", kind="review", final=True)
        config["checks"].append(review)
        config["semantic_contracts"] = [dict(implementation=["src/program"], contract="wiki/spec/contract.md",
            classification="missing_implementation", reason="repair existing rule", passages=["The actual canonical rule."])]
        qualification.definition(config)
        context = qualification.contract_mapping(self.repository, config, base, base)
        self.assertEqual(context[0]["revision"], base)
        self.assertEqual(context[0]["blob"], self.git("rev-parse", base + ":wiki/spec/contract.md"))
        self.assertEqual(context[0]["passages"], ["The actual canonical rule."])
        config["semantic_contracts"][0]["passages"] = ["An invented rule"]
        with self.assertRaisesRegex(qualification.RecoveryError, "missing/stale"):
            qualification.contract_mapping(self.repository, config, base, base)

    def test_assignment_renderer_carries_passages_and_rejects_unretrieved_context(self):
        launch = kit.load_module(TOOLS / "swarm" / "launch.py", "qualification_launch_under_test")
        (self.repository / "wiki/spec").mkdir(parents=True)
        base = self.commit("wiki/spec/contract.md", "The actual rule.\n")
        session = dict(semantic_contracts=[dict(implementation=["src/program"], contract="wiki/spec/contract.md",
            classification="missing_implementation", reason="existing rule", passages=["The actual rule."])])
        with self.assertRaisesRegex(launch.SwarmError, "verify canonical passages"):
            launch.canonical_context_block(session)
        session["canonical_context"] = [dict(contract="wiki/spec/contract.md", revision=self.base,
            blob="source-blob", classification="missing_implementation", passages=["The actual rule."])]
        with self.assertRaisesRegex(launch.SwarmError, "verify canonical passages"):
            launch.canonical_context_block(session)
        session["semantic_handoff"] = qualification.work_handoff(self.repository, session, base, "implementation")
        text = launch.canonical_context_block(session)
        self.assertIn("The actual rule.", text)
        self.assertIn(base, text)
        self.assertIn("independent final review", text)

    def test_handoff_and_continuation_refresh_only_focused_changed_context(self):
        (self.repository / "wiki/spec").mkdir(parents=True)
        base = self.commit("wiki/spec/contract.md", "The actual rule.\n")
        config = copy.deepcopy(self.config)
        config["semantic_contracts"] = [dict(implementation=["src/program"], contract="wiki/spec/contract.md",
            classification="missing_implementation", reason="repair existing rule", passages=["The actual rule."])]
        initial = qualification.handoff(self.repository, config, base, "implementation")
        self.assertEqual(initial["reinject_context"][0]["passages"], ["The actual rule."])
        self.assertEqual(initial["canonical_context"][0]["revision"], base)
        self.commit("src/program", "implemented rule\n")
        review = qualification.handoff(self.repository, config, base, "review", initial)
        self.assertEqual(review["changed_paths"], ["src/program"])
        self.assertTrue(review["context_refresh"])
        resumed = qualification.handoff(self.repository, config, base, "continuation", initial)
        self.assertFalse(resumed["context_refresh"])
        self.assertEqual(resumed["reinject_context"], [])
        unchanged = self.commit("README.md", "unrelated integrated main\n")
        self.assertFalse(qualification.handoff(self.repository, config, unchanged, "continuation", initial)["context_refresh"])
        changed = self.commit("wiki/spec/contract.md", "The revised rule.\n")
        with self.assertRaisesRegex(qualification.RecoveryError, "missing/stale"):
            qualification.handoff(self.repository, config, changed, "continuation", initial)
        config["semantic_contracts"][0]["passages"] = ["The revised rule."]
        refreshed = qualification.handoff(self.repository, config, changed, "continuation", initial)
        self.assertTrue(refreshed["context_refresh"])
        self.assertEqual(refreshed["reinject_context"][0]["passages"], ["The revised rule."])
        initial["canonical_context"][0]["passages"] = ["forged context"]
        with self.assertRaisesRegex(qualification.RecoveryError, "corrupt prior"):
            qualification.handoff(self.repository, config, changed, "continuation", initial)

    def test_work_dispatch_defaults_semantic_and_uncertain_retention_reinjects(self):
        with self.assertRaisesRegex(qualification.RecoveryError, "requires focused"):
            qualification.work_handoff(self.repository, self.config, self.base, "implementation")
        (self.repository / "wiki/spec").mkdir(parents=True)
        base=self.commit("wiki/spec/contract.md", "The actual rule.\n")
        config=copy.deepcopy(self.config)
        config['semantic_contracts']=[dict(implementation=['src/program'],contract='wiki/spec/contract.md',
            classification='missing_implementation',reason='existing rule',passages=['The actual rule.'])]
        prior=qualification.work_handoff(self.repository,config,base,'implementation')
        resumed=qualification.work_handoff(self.repository,config,base,'continuation',prior)
        self.assertTrue(resumed['context_refresh'])
        self.assertIn('The actual rule.',qualification.work_context_prompt(resumed))
        retained=qualification.work_handoff(self.repository,config,base,'continuation',prior,retained=True)
        self.assertFalse(retained['context_refresh'])
        self.assertNotIn('The actual rule.',qualification.work_context_prompt(retained))
        config['work_classification']=dict(kind='non_semantic',reason='Transport only')
        with self.assertRaisesRegex(qualification.RecoveryError,'cannot also declare'):
            qualification.work_handoff(self.repository,config,base,'implementation')

    @unittest.skipIf(os.name == "nt", "symlink creation requires host-specific privileges on Windows")
    def test_artifact_symlink_retarget_and_restore_is_not_accepted(self):
        other = self.root / "other.rlib"
        other.write_bytes(b"another producer")
        link = self.root / "current.rlib"
        link.symlink_to(self.library)
        config = copy.deepcopy(self.config)
        config["checks"][0]["artifacts"] = [str(link)]
        qualification.amend(self.state, 0, config, "exact declared producer link")
        code = ("from pathlib import Path\np=Path(" + repr(str(link)) + ")\n"
                "original=p.readlink()\np.unlink()\np.symlink_to(" + repr(str(other)) + ")\n"
                "print(p.read_bytes())\np.unlink()\np.symlink_to(original)\n")
        candidate = self.commit("check.py", code)
        self.assess(candidate)
        record, accepted = self.run_check()
        self.assertFalse(accepted)
        self.assertIn("artifact_mutated_during_run", record["runs"][-1]["invalidations"])

    def test_added_consumed_then_deleted_source_is_not_accepted(self):
        code = ("from pathlib import Path\np=Path('src/added')\np.write_text('unrecorded source')\n"
                "print(p.read_text())\np.unlink()\n")
        candidate = self.commit("check.py", code)
        self.assess(candidate)
        record, accepted = self.run_check()
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertFalse(accepted)
        self.assertIn("source_mutated_during_run", record["runs"][-1]["invalidations"])

    def test_observed_source_remains_reachable_after_candidate_replacement(self):
        record, accepted = self.run_check()
        self.assertTrue(accepted)
        reference = record["runs"][-1]["source_ref"]
        self.commit("README.md", "new candidate")
        self.assertEqual(self.git("rev-parse", reference), self.base)

    @unittest.skipIf(os.name == "nt", "symlink creation requires host-specific privileges on Windows")
    def test_intermediate_file_and_directory_links_cannot_hide_another_producer(self):
        for directory in (False, True):
            with self.subTest(directory=directory):
                original = self.root / ("original-dir" if directory else "original-file")
                alternate = self.root / ("alternate-dir" if directory else "alternate-file")
                if directory:
                    original.mkdir(); alternate.mkdir()
                    (original / "producer").write_bytes(b"original")
                    (alternate / "producer").write_bytes(b"alternate")
                else:
                    original.write_bytes(b"original"); alternate.write_bytes(b"alternate")
                middle = self.root / ("middle-dir" if directory else "middle-file")
                declared = self.root / ("declared-dir" if directory else "declared-file")
                middle.symlink_to(original, target_is_directory=directory)
                declared.symlink_to(middle, target_is_directory=directory)
                artifact = declared / "producer" if directory else declared
                before = qualification.artifact_stamps([str(artifact)])
                middle.unlink(); middle.symlink_to(alternate, target_is_directory=directory)
                self.assertEqual(artifact.read_bytes(), b"alternate")
                middle.unlink(); middle.symlink_to(original, target_is_directory=directory)
                after = qualification.artifact_stamps([str(artifact)])
                self.assertNotEqual(before, after)
                self.assertIn(str(middle), before)

    @unittest.skipIf(os.name == "nt", "symlink creation requires host-specific privileges on Windows")
    def test_chained_link_replacement_and_restoration_refuses_actual_run(self):
        other = self.root / "chain-other.rlib"
        other.write_bytes(b"alternate producer")
        middle = self.root / "middle.rlib"; middle.symlink_to(self.library)
        declared = self.root / "declared.rlib"; declared.symlink_to(middle)
        config = copy.deepcopy(self.config); config["checks"][0]["artifacts"] = [str(declared)]
        qualification.amend(self.state, 0, config, "declared chained producer")
        code = ("from pathlib import Path\nm=Path(" + repr(str(middle)) + ")\n"
                "target=m.readlink()\nm.unlink()\nm.symlink_to(" + repr(str(other)) + ")\n"
                "print(Path(" + repr(str(declared)) + ").read_bytes())\nm.unlink()\nm.symlink_to(target)\n")
        candidate = self.commit("check.py", code); self.assess(candidate)
        record, accepted = self.run_check()
        self.assertFalse(accepted)
        self.assertIn("artifact_mutated_during_run", record["runs"][-1]["invalidations"])

    def test_windows_junction_detection_uses_portable_lstat_tags(self):
        # Python 3.9-3.11 have no Path.is_junction. These tag records exercise
        # the same owning judgment without requiring a Windows mount privilege.
        junction = SimpleNamespace(st_mode=stat.S_IFDIR, st_reparse_tag=0xA0000003)
        self.assertTrue(qualification.is_capture_link(junction))
        self.assertFalse(qualification.is_capture_link(SimpleNamespace(st_mode=stat.S_IFDIR)))
        with self.assertRaisesRegex(qualification.RecoveryError, "unsupported artifact reparse"):
            qualification.is_capture_link(SimpleNamespace(st_mode=stat.S_IFDIR, st_reparse_tag=0xDEADBEEF))


class InterruptedQualificationTests(unittest.TestCase):
    setUp = QualificationTests.setUp
    git = QualificationTests.git
    # Exercise real locked state/receipt files; process evidence is an explicit
    # unit-test adapter judgment, never evidence about a live executor.
    def interruption(self, stopped_pids=None):
        record = qualification.load(self.state)
        operation = "interrupted-fixture"
        def reserve(value):
            value["running"] = dict(id=operation, check="values", candidate=self.base)
        record = qualification.update(self.state, record["revision"], reserve)
        directory = self.state.with_suffix(".records")
        directory.mkdir(exist_ok=True)
        for name in ("stdout", "stderr"):
            (directory / (operation + "." + name)).write_bytes(b"partial evidence\n")
        binding = dict(state=str(self.state.resolve()), state_sha256=qualification.file_digest(self.state),
                       state_revision=record["revision"], running=record["running"],
                       config_digest=record["config_digest"])
        evidence = self.root / "stopped.json"
        qualification.atomic_json(evidence, dict(schema="qualification-stopped-process-tree-v1",
            observed_utc=qualification.datetime.now(qualification.timezone.utc).isoformat(),
            controller_stopped=True, qualification_stopped=True, children_stopped=True,
            unresolved_processes=[], stopped_pids=stopped_pids or [], binding=binding))
        review = self.root / "interruption-review.json"
        qualification.atomic_json(review, dict(schema="qualification-interruption-review-v1",
            accepted=True, reviewer="independent test adapter", rationale="Stopped test process tree",
            recovered_at_head=self.base, binding=binding,
            process_evidence=dict(path=str(evidence), sha256=qualification.file_digest(evidence))))
        return record, review, evidence

    def test_recovery_preserves_history_and_forces_fresh_run(self):
        previous, accepted = qualification.run_check(self.repository, self.state, 0, "values")
        self.assertTrue(accepted)
        record, review, _ = self.interruption()
        result = qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        self.assertIsNone(result["running"])
        self.assertEqual(result["runs"][:-1], previous["runs"])
        failed = result["runs"][-1]
        self.assertIsNone(failed["exit"])
        self.assertIsNone(failed["duration_seconds"])
        self.assertEqual(failed["source_inputs"], {})
        self.assertIn("prior_run_not_accepted", qualification.reasons(
            self.repository, result, self.config["checks"][0], failed, self.base))
        rerun, accepted = qualification.run_check(self.repository, self.state, result["revision"], "values")
        self.assertTrue(accepted)
        self.assertEqual(len(rerun["runs"]), 3)

    def test_recovery_refuses_stale_or_mismatched_binding(self):
        record, review, _ = self.interruption()
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "stale qualification"):
            qualification.recover_interrupted(self.repository, self.state, record["revision"] - 1, review)
        value = json.loads(review.read_text());value["binding"]["running"]["id"] = "different"
        qualification.atomic_json(review, value)
        with self.assertRaisesRegex(qualification.RecoveryError, "exact state and reservation"):
            qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        self.assertEqual(self.state.read_bytes(), before)

    def test_recovery_refuses_live_process_and_tampered_evidence(self):
        record, review, evidence = self.interruption([os.getpid()])
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "still live"):
            qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        evidence.write_text(evidence.read_text() + " ")
        with self.assertRaisesRegex(qualification.RecoveryError, "evidence changed"):
            qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        self.assertEqual(self.state.read_bytes(), before)

    def test_recovery_refuses_missing_tree_judgment_and_expired_evidence(self):
        record, review, evidence = self.interruption()
        value = json.loads(evidence.read_text());value["children_stopped"] = False
        qualification.atomic_json(evidence, value)
        document = json.loads(review.read_text());document["process_evidence"]["sha256"] = qualification.file_digest(evidence)
        qualification.atomic_json(review, document)
        with self.assertRaisesRegex(qualification.RecoveryError, "complete stopped"):
            qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        value["children_stopped"] = True;value["observed_utc"] = "2000-01-01T00:00:00+00:00"
        qualification.atomic_json(evidence, value)
        document["process_evidence"]["sha256"] = qualification.file_digest(evidence)
        qualification.atomic_json(review, document)
        with self.assertRaisesRegex(qualification.RecoveryError, "stale interruption"):
            qualification.recover_interrupted(self.repository, self.state, record["revision"], review)

    def test_recovery_preserves_orphan_receipt_after_atomic_failure(self):
        record, review, _ = self.interruption()
        before = self.state.read_bytes();original = qualification.atomic_json
        def fail_state(path, *arguments, **keywords):
            if Path(path) == self.state:raise OSError("replacement failure")
            return original(path, *arguments, **keywords)
        with mock.patch.object(qualification, "atomic_json", side_effect=fail_state):
            with self.assertRaisesRegex(OSError, "replacement failure"):
                qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        self.assertEqual(self.state.read_bytes(), before)
        orphan = next(self.state.with_suffix(".records").glob("*.interrupted.json"))
        saved = orphan.read_bytes()
        result = qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        self.assertIsNone(result["running"])
        self.assertEqual(orphan.read_bytes(), saved)

    def test_recovery_keeps_original_candidate_after_tool_commit(self):
        record, review, _ = self.interruption()
        (self.repository / "recovery-tool.txt").write_text("tool repair\n")
        self.git("add", "recovery-tool.txt");self.git("commit", "-m", "tools: recover interrupted records")
        head = self.git("rev-parse", "HEAD")
        value = json.loads(review.read_text());value["recovered_at_head"] = head
        qualification.atomic_json(review, value)
        result = qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        self.assertEqual(result["runs"][-1]["revision"], self.base)
        self.assertEqual(result["runs"][-1]["recovery"]["recovered_at_head"], head)

    def test_windows_process_probe_is_read_only(self):
        result = SimpleNamespace(returncode=0, stdout='"omega.exe","25","Console","1","512 K"\n')
        with mock.patch.object(qualification.os, "name", "nt"), \
             mock.patch.object(qualification.os, "kill") as kill, \
             mock.patch.object(qualification.subprocess, "run", return_value=result) as query:
            with self.assertRaisesRegex(qualification.RecoveryError, "still live"):
                qualification.require_stopped_pid(25)
            result.stdout = "INFO: No tasks match the specified criteria.\n"
            qualification.require_stopped_pid(25)
            kill.assert_not_called()
            self.assertEqual(query.call_args.args[0][0], "tasklist")

    @unittest.skipIf(os.name == "nt", "POSIX process-query protocol")
    def test_process_probe_accepts_confirmed_zombie_but_refuses_live_or_unknown(self):
        result = SimpleNamespace(returncode=0, stdout="25 Z\n", stderr="")
        with mock.patch.object(qualification.subprocess, "run", return_value=result):
            qualification.require_stopped_pid(25)
            result.stdout = "25 S\n"
            with self.assertRaisesRegex(qualification.RecoveryError, "still live"):
                qualification.require_stopped_pid(25)
            result.returncode = 1;result.stdout = ""
            qualification.require_stopped_pid(25)
            result.stderr = "permission denied"
            with self.assertRaisesRegex(qualification.RecoveryError, "cannot establish"):
                qualification.require_stopped_pid(25)

    def test_recovery_refuses_completed_observation(self):
        record, review, _ = self.interruption()
        (self.state.with_suffix(".records") / "interrupted-fixture.json").write_text("{}")
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "completed observation exists"):
            qualification.recover_interrupted(self.repository, self.state, record["revision"], review)
        self.assertEqual(self.state.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
