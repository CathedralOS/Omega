"""A hosted depth-one checkout must export restorable measured source."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from ci import compiler_source_export as export


class SourceExportTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.origin = self.directory / "origin"
        self.origin.mkdir()
        self.git(self.origin, "init", "-q", "-b", "main")
        self.git(self.origin, "config", "user.name", "Source export test")
        self.git(self.origin, "config", "user.email", "source@example.invalid")
        (self.origin / "source").write_text("older published source")
        self.git(self.origin, "add", ".")
        self.git(self.origin, "commit", "-qm", "published")
        (self.origin / "source").write_text("published")
        self.git(self.origin, "commit", "-qam", "published prerequisite with a parent")
        self.base = self.git(self.origin, "rev-parse", "HEAD")
        self.git(self.origin, "checkout", "-qb", "work")
        self.sources = []
        for number in range(3):
            (self.origin / "source").write_text(str(number))
            self.git(self.origin, "commit", "-qam", "feature")
            self.sources.append(self.git(self.origin, "rev-parse", "HEAD"))
        self.checkout = self.directory / "checkout"
        self.git(self.directory, "clone", "-q", "--depth", "1", "--branch", "work",
                 self.origin.as_uri(), str(self.checkout))
        self.output = self.checkout / "build/pilot"
        self.output.mkdir(parents=True)
        self.root = patch.object(export.corpus_gate, "ROOT", self.checkout)
        self.root.start()
        self.addCleanup(self.root.stop)

    def git(self, directory, *arguments):
        return subprocess.check_output(["git", *arguments], cwd=directory, text=True,
                                       stderr=subprocess.PIPE).strip()

    def test_shallow_source_bundle_restores_with_only_its_published_prerequisite(self):
        self.assertEqual(self.git(self.checkout, "rev-parse", "--is-shallow-repository"), "true")
        records = export.export(self.output, [self.sources[-1], self.sources[0]])
        restored = self.directory / "restored"
        self.git(self.directory, "clone", "-q", "--branch", "main", "--single-branch",
                 self.origin.as_uri(), str(restored))
        absent = subprocess.run(["git", "cat-file", "-e", self.sources[-1] + "^{commit}"],
                                cwd=restored, capture_output=True)
        self.assertNotEqual(absent.returncode, 0)
        for record in records:
            self.assertEqual(record["prerequisite"], self.base)
            bundle = Path(record["bundle"])
            self.assertEqual(hashlib.sha256(bundle.read_bytes()).hexdigest(), record["sha256"])
            self.git(restored, "bundle", "unbundle", str(bundle))
            self.git(restored, "fsck", "--full", record["revision"])
        self.assertEqual(self.git(restored, "show", self.sources[-1] + ":source"), "2")
        self.assertEqual(self.git(self.checkout, "rev-parse", "HEAD"), self.sources[-1])
        self.assertEqual(json.loads((self.output / "source/manifest.json").read_text()), records)
        self.assertEqual(export.export(self.output, [self.sources[-1], self.sources[0]]), records)

    def test_preexisting_shallow_bundle_is_refused_without_overwriting_it(self):
        revision = self.sources[-1]
        directory = self.checkout / "build/corpus-source"
        directory.mkdir()
        bundle = directory / (revision + ".bundle")
        reference = "refs/corpus-observations/" + revision
        self.git(self.checkout, "update-ref", reference, revision)
        self.git(self.checkout, "bundle", "create", str(bundle), reference)
        digest = hashlib.sha256(bundle.read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError, "cannot restore its bundle"):
            export.export(self.output, [revision])
        self.assertEqual(hashlib.sha256(bundle.read_bytes()).hexdigest(), digest)
        self.assertFalse((self.output / "source/manifest.json").exists())
        self.assertFalse((self.output / "source" / bundle.name).exists())

    def test_unknown_original_source_or_missing_remote_refuses_without_a_manifest(self):
        for missing in (False, True):
            with self.subTest(missing_remote=missing):
                if missing:
                    self.git(self.checkout, "remote", "set-url", "origin", (self.directory / "missing").as_uri())
                with self.assertRaisesRegex(ValueError, "source export"):
                    export.export(self.output, ["0" * 40])
                self.assertFalse((self.output / "source/manifest.json").exists())

    def test_bundle_cannot_require_an_unpublished_feature_parent(self):
        self.git(self.checkout, "fetch", "--deepen=64", "origin", "work")
        revision = self.sources[-1]
        directory = self.checkout / "build/corpus-source"
        directory.mkdir()
        bundle = directory / (revision + ".bundle")
        reference = "refs/corpus-observations/" + revision
        self.git(self.checkout, "update-ref", reference, revision)
        self.git(self.checkout, "bundle", "create", str(bundle), reference, "^" + self.sources[0])
        with self.assertRaisesRegex(ValueError, "source export"):
            export.export(self.output, [revision])
        self.assertFalse((self.output / "source/manifest.json").exists())

    def test_existing_bundle_owner_refusal_stops_artifact_copy(self):
        with patch.object(export.corpus_gate, "retain_record_source", return_value="bad source bundle"):
            with self.assertRaisesRegex(ValueError, "bad source bundle"):
                export.export(self.output, [self.sources[-1]])
        self.assertFalse((self.output / "source/manifest.json").exists())
        self.assertFalse(any((self.output / "source").glob("*.bundle")))


if __name__ == "__main__":
    unittest.main()
