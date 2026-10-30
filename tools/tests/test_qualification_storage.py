#!/usr/bin/env python3
"""Receipt-backed persistence keeps exact observations without repeated snapshots."""

import copy
import io
import json
from pathlib import Path
import unittest
from unittest import mock

import test_qualification as fixtures

qualification = fixtures.qualification


class QualificationStorageTests(unittest.TestCase):
    setUp = fixtures.QualificationTests.setUp
    git = fixtures.QualificationTests.git
    run_check = fixtures.QualificationTests.run_check
    assess = fixtures.QualificationTests.assess

    def test_receipt_backed_history_keeps_full_logical_observations(self):
        record, accepted = self.run_check()
        self.assertTrue(accepted)
        original = qualification.observation(record["runs"][0])
        receipt = Path(original["receipt"]["path"])
        before = receipt.read_bytes()
        self.assertNotIn("source_stamps", json.loads(self.state.read_text())["runs"][0])
        self.assertEqual(qualification.observation(qualification.load(self.state)["runs"][0]), original)
        self.assess()
        full = json.loads(qualification.state_output(qualification.load(self.state), self.state, False))
        self.assertEqual(full["runs"], [original])
        self.assertEqual(full["assessments"][-1]["incoming_paths"], [])
        output = io.StringIO()
        with mock.patch("sys.stdout", output):
            self.assertEqual(qualification.main(["status", "--repository", str(self.repository),
                "--state", str(self.state), "--full-output"]), 0)
        self.assertEqual(json.loads(output.getvalue()), full)
        self.assertEqual(receipt.read_bytes(), before)
        qualification.validate(self.repository, self.state, self.base, self.base)

    def legacy(self):
        self.run_check()
        record = qualification.expanded_record(self.assess())
        record["schema"] = qualification.LEGACY_SCHEMA
        for row in record["assessments"]:
            row.pop("receipt", None)
        qualification.atomic_json(self.state, record, indent=None)
        return record

    def test_legacy_migration_preserves_every_field_and_receipt_byte(self):
        legacy = self.legacy()
        run = legacy["runs"][0]
        receipt = Path(run["receipt"]["path"])
        before = receipt.read_bytes()
        record = qualification.update(self.state, legacy["revision"], lambda record: None)
        self.assertEqual(record["schema"], qualification.SCHEMA)
        self.assertEqual(record["revision"], legacy["revision"] + 1)
        full = qualification.expanded_record(record)
        self.assertEqual(full["runs"], legacy["runs"])
        self.assertEqual([{key:value for key,value in row.items() if key != "receipt"}
                          for row in full["assessments"]], legacy["assessments"])
        for key in legacy.keys() - {"runs", "assessments", "schema", "revision"}:
            self.assertEqual(full[key], legacy[key])
        self.assertEqual(receipt.read_bytes(), before)
        qualification.validate(self.repository, self.state, self.base, self.base)

    def test_compact_observation_rejects_every_missing_or_extra_semantic_field(self):
        record, _ = self.run_check()
        original = record["runs"][0]
        for key in original.keys() - {"receipt_fields", "receipt"}:
            changed = copy.deepcopy(original)
            changed.pop(key)
            with self.subTest(missing=key):
                with self.assertRaisesRegex(qualification.RecoveryError, "observation_record_mismatch"):
                    qualification.observation(changed)
        mutations = [dict(extra_effect="invented"), dict(exit=False), dict(repeated_work=0),
                     dict(source_stamps={}), dict(receipt_fields=[]),
                     dict(receipt_fields=["source_stamps", "artifact_stamps", "exit"]),
                     dict(receipt_fields=["artifact_stamps", "source_stamps"])]
        for mutation in mutations:
            changed = dict(original, **mutation)
            with self.subTest(mutation=mutation):
                with self.assertRaisesRegex(qualification.RecoveryError, "observation_record_mismatch"):
                    qualification.observation(changed)
                self.assertIn("observation_record_mismatch", qualification.reasons(
                    self.repository, record, record["config"]["checks"][0], changed, self.base))
        changed = dict(original)
        changed.pop("receipt_fields")
        with self.assertRaisesRegex(qualification.RecoveryError, "observation_record_mismatch"):
            qualification.observation(changed)

    def test_legacy_corruption_cannot_be_laundered_by_compaction(self):
        legacy = self.legacy()
        altered = legacy["runs"][0]
        altered["source_stamps"]["forged-entry"] = [1, 2, 3, 4, 5]
        altered["unknown_semantic_field"] = "must remain visible"
        before = copy.deepcopy(altered)
        qualification.atomic_json(self.state, legacy, indent=None)
        record = qualification.update(self.state, legacy["revision"], lambda record: None)
        self.assertEqual(record["runs"][0], before)
        self.assertNotIn("receipt_fields", record["runs"][0])
        self.assertIn("observation_record_mismatch", qualification.reasons(
            self.repository, record, record["config"]["checks"][0], record["runs"][0], self.base))

    def test_missing_changed_and_incomplete_receipts_never_hydrate(self):
        record, _ = self.run_check()
        row = record["runs"][0]
        path = Path(row["receipt"]["path"])
        original = path.read_bytes()
        path.unlink()
        with self.assertRaises(OSError):
            qualification.observation(row)
        path.write_bytes(original + b" ")
        with self.assertRaisesRegex(qualification.RecoveryError, "receipt digest"):
            qualification.observation(row)
        captured = json.loads(original)
        for name in qualification.RUN_RECEIPT_FIELDS:
            missing = dict(captured)
            missing.pop(name)
            qualification.atomic_json(path, missing)
            changed = dict(row, receipt=dict(path=str(path), sha256=qualification.file_digest(path)))
            with self.subTest(missing=name):
                with self.assertRaisesRegex(qualification.RecoveryError, "field roster"):
                    qualification.observation(changed)
        path.write_bytes(original)
        self.assertEqual(qualification.observation(row), dict(captured, receipt=row["receipt"]))

    def test_latest_assessment_is_authenticated_before_source_or_validation_use(self):
        self.run_check()
        record = self.assess()
        original = copy.deepcopy(record)
        for mutation in (dict(candidate="0" * 40), dict(extra_result="accepted"), dict(reused=True)):
            changed = copy.deepcopy(original)
            changed["assessments"][-1].update(mutation)
            qualification.atomic_json(self.state, changed, indent=None)
            with self.subTest(mutation=mutation):
                with self.assertRaisesRegex(qualification.RecoveryError, "observation_record_mismatch"):
                    qualification.validate(self.repository, self.state, self.base, self.base)
                with self.assertRaisesRegex(qualification.RecoveryError, "observation_record_mismatch"):
                    qualification.amend(self.state, record["revision"], self.config, "still requires source custody")
        qualification.atomic_json(self.state, original, indent=None)
        receipt = Path(original["assessments"][-1]["receipt"]["path"])
        receipt.unlink()
        with self.assertRaises(OSError):
            qualification._RunSnapshot.read(self.state).context("values")
        with self.assertRaises(OSError):
            qualification.state_output(qualification.load(self.state), self.state, False)

    def test_stripped_assessment_references_cannot_downgrade_to_inline_legacy(self):
        self.run_check()
        record = self.assess()
        row = record["assessments"][-1]
        row.pop("receipt")
        row.pop("receipt_fields")
        with self.assertRaisesRegex(qualification.RecoveryError, "complete inline legacy"):
            qualification.assessment(row)
        for fields in ({}, {"incoming_paths": []}):
            row.update(fields)
            qualification.atomic_json(self.state, record, indent=None)
            with self.subTest(fields=fields):
                with self.assertRaisesRegex(qualification.RecoveryError, "v2 assessment"):
                    qualification.validate(self.repository, self.state, self.base, self.base)
                with self.assertRaisesRegex(qualification.RecoveryError, "v2 assessment"):
                    qualification.amend(self.state, record["revision"], self.config, "no downgrade")
                with self.assertRaisesRegex(qualification.RecoveryError, "v2 assessment"):
                    qualification._RunSnapshot.read(self.state).context("values")

    def test_normal_save_never_reads_archived_snapshots(self):
        self.run_check()
        record = self.assess()
        with mock.patch.object(qualification, "observation", side_effect=AssertionError("historical run read")), \
                mock.patch.object(qualification, "assessment", side_effect=AssertionError("historical assessment read")):
            result = qualification.update(self.state, record["revision"], lambda record: None)
        self.assertEqual(result["revision"], record["revision"] + 1)
        self.assertEqual(result["runs"], record["runs"])
        self.assertEqual(result["assessments"], record["assessments"])

    def test_interrupted_migration_keeps_old_state_and_reuses_exact_archive(self):
        legacy = self.legacy()
        before = self.state.read_bytes()
        atomic = qualification.atomic_json
        def fail_state(path, *arguments, **keywords):
            if Path(path) == self.state:
                raise OSError("injected index replacement failure")
            return atomic(path, *arguments, **keywords)
        with mock.patch.object(qualification, "atomic_json", side_effect=fail_state):
            with self.assertRaisesRegex(OSError, "replacement failure"):
                qualification.update(self.state, legacy["revision"], lambda record: None)
        self.assertEqual(self.state.read_bytes(), before)
        archives = {path:path.read_bytes() for path in self.state.with_suffix(".records").glob("*.assessment.json")}
        result = qualification.update(self.state, legacy["revision"], lambda record: None)
        self.assertEqual(result["revision"], legacy["revision"] + 1)
        self.assertEqual({path:path.read_bytes() for path in archives}, archives)
        self.assertEqual(len(list(self.state.with_suffix(".records").glob("*.assessment.json"))), len(archives))
        qualification.validate(self.repository, self.state, self.base, self.base)


class CompletedStorageTests(unittest.TestCase):
    setUp = fixtures.CompletedQualificationTests.setUp
    git = fixtures.CompletedQualificationTests.git
    commit = fixtures.CompletedQualificationTests.commit
    executable_producer_fixture = fixtures.CompletedQualificationTests.executable_producer_fixture
    completed_orphan = fixtures.CompletedQualificationTests.completed_orphan
    completed_review = fixtures.CompletedQualificationTests.completed_review
    recover = fixtures.CompletedQualificationTests.recover

    def test_completed_recovery_refuses_stripped_assessment_receipt(self):
        record, receipt, _ = self.completed_orphan()
        row = record["assessments"][-1]
        row.pop("receipt")
        row.pop("receipt_fields")
        row["candidate"] = self.base
        qualification.atomic_json(self.state, record, indent=None)
        review, _ = self.completed_review(record, receipt)
        before = self.state.read_bytes()
        with self.assertRaisesRegex(qualification.RecoveryError, "v2 assessment"):
            self.recover(record, review)
        self.assertEqual(self.state.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
