"""Unit tests for the release packager; no compiler or network required."""
import importlib.util
from pathlib import Path
import tempfile
import sys
import unittest
from unittest.mock import patch
import zipfile
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
SPEC = importlib.util.spec_from_file_location("release_package", Path(__file__).resolve().parents[1] / "release/package.py")
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class ReleasePackageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omega package test ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.library = self.root / "library"
        (self.library / "core").mkdir(parents=True)
        (self.library / "std").mkdir()
        (self.library / "core/a.omg").write_bytes(b"data A {}\n")
        (self.library / "std/build.omg").write_bytes(b"machine build() {}\n")
        self.binary = self.root / "omega"
        self.binary.write_bytes(b"test executable")
        self.metadata = dict(version="0.1.0-alpha.1", commit="abc", host="test-host",
                             library_sha256=package.library_digest(self.library))

    def stage(self):
        bundle = self.root / "bundle"
        package.stage_bundle(bundle, self.binary, self.library, self.metadata)
        return bundle

    def test_streamed_builds_still_check_exit_status(self):
        result = package.subprocess.CompletedProcess(["compiler"], 1)
        with patch.object(package.subprocess, "run", return_value=result) as execute:
            with self.assertRaisesRegex(RuntimeError, "returned 1, expected 0"):
                package.run(["compiler"], capture=False)
            self.assertIsNone(execute.call_args.kwargs["stdout"])
            self.assertEqual(execute.call_args.kwargs["stderr"], package.subprocess.STDOUT)

    def test_digest_is_location_independent_and_content_sensitive(self):
        bundle = self.stage()
        self.assertEqual(package.library_digest(bundle / "lib/omega/library"), self.metadata["library_sha256"])
        self.assertEqual(self.metadata["library_sha256"], "fb0cd4651037cc6a991c017ff861bcf92c050498945c49a286f887dbb1fd9541")
        (self.library / "core/a.omg").write_bytes(b"data B {}\n")
        self.assertNotEqual(package.library_digest(self.library), self.metadata["library_sha256"])

    def test_added_and_renamed_sources_change_digest(self):
        (self.library / "core/a.omg").rename(self.library / "core/b.omg")
        self.assertNotEqual(package.library_digest(self.library), self.metadata["library_sha256"])
        renamed = package.library_digest(self.library)
        (self.library / "core/c.omg").write_bytes(b"data C {}\n")
        self.assertNotEqual(package.library_digest(self.library), renamed)

    def test_only_runtime_sources_and_licenses_are_staged(self):
        (self.library / "README.md").write_text("not runtime input")
        bundle = self.stage()
        manifest = package.verify_manifest(bundle)
        self.assertEqual(len(manifest["files"]), 5)
        self.assertFalse((bundle / "lib/omega/library/README.md").exists())

    def test_dependency_notices_participate_in_bundle_manifest(self):
        notices = self.root / "notices"
        notices.mkdir()
        (notices / "LICENSE").write_text("upstream license")
        bundle = self.root / "bundle"
        package.stage_bundle(bundle, self.binary, self.library, self.metadata,
                             dependency_notices=notices)
        manifest = package.verify_manifest(bundle)
        self.assertIn("licenses/third-party/LICENSE", manifest["files"])

    def test_git_fixture_is_pinned_and_offline_after_review(self):
        metadata = dict(host="x86_64-pc-windows-msvc", commit="a" * 40)
        calls = []
        def execute(command, **options):
            calls.append((command, options))
            project = self.root / "git source smoke"
            if "update" in command and "--resume" not in command:
                review = project / "build/package-manager/review-windows_x86_64.txt"
                review.parent.mkdir(parents=True)
                review.write_bytes(b"decision fixed-fixture pending\n")
            if "--resume" in command:
                (project / "omega.lock").write_bytes(b"accepted")
            return ""
        with patch.object(package, "run", side_effect=execute), \
             patch.object(package.shutil, "which", return_value="git"), \
             patch.dict(package.os.environ, GH_TOKEN="not-for-child", GITHUB_TOKEN="not-for-child"):
            package.git_source_smoke(self.binary, metadata, self.root)
        source = (self.root / "git source smoke/build.omg").read_text()
        self.assertIn('revision: "' + "a" * 40 + '"', source)
        self.assertIn('PackageSelection::Named { package: "omega_language_std" }', source)
        self.assertEqual(len(calls), 4)
        self.assertNotIn("--offline", calls[0][0])
        for command, options in calls[1:3]:
            self.assertIn("--offline", command)
            self.assertNotIn("GH_TOKEN", options["environment"])
            self.assertNotIn("GITHUB_TOKEN", options["environment"])
        self.assertIn("omega-program", str(calls[3][0][0]))

    def test_manifest_detects_corruption_and_extra_files(self):
        bundle = self.stage()
        (bundle / "extra").write_text("unexpected")
        with self.assertRaises(ValueError):
            package.verify_manifest(bundle)
        (bundle / "extra").unlink()
        (bundle / "bin/omega").write_bytes(b"wrong")
        with self.assertRaises(ValueError):
            package.verify_manifest(bundle)

    def test_manifest_detects_omitted_file(self):
        bundle = self.stage()
        (bundle / "lib/omega/library/core/a.omg").unlink()
        with self.assertRaises(ValueError):
            package.verify_manifest(bundle)

    def test_zip_and_tar_roundtrip(self):
        bundle = self.stage()
        for suffix in (".zip", ".tar.gz"):
            archive = self.root / ("bundle" + suffix)
            package.archive_bundle(bundle, archive)
            extracted = self.root / ("extracted" + suffix)
            extracted.mkdir()
            if suffix == ".zip":
                with zipfile.ZipFile(archive) as reader:
                    reader.extractall(extracted)
            else:
                with tarfile.open(archive) as reader:
                    reader.extractall(extracted)
            self.assertEqual(package.verify_manifest(extracted / "bundle")["version"], "0.1.0-alpha.1")

    def test_incomplete_library_rejects(self):
        (self.library / "std/build.omg").unlink()
        with self.assertRaises(ValueError):
            package.library_digest(self.library)

    def test_publication_preserves_existing_archive(self):
        destination = self.root / "release.zip"
        destination.write_bytes(b"existing release")
        with self.assertRaises(FileExistsError):
            package.publish_archive(self.binary, destination)
        self.assertEqual(destination.read_bytes(), b"existing release")

    def test_publication_writes_archive_and_checksum(self):
        destination = self.root / "release.zip"
        package.publish_archive(self.binary, destination)
        self.assertEqual(destination.read_bytes(), self.binary.read_bytes())
        expected = package.hashlib.sha256(self.binary.read_bytes()).hexdigest()
        self.assertEqual((self.root / "release.zip.sha256").read_bytes(),
                         f"{expected}  release.zip\n".encode("ascii"))


if __name__ == "__main__":
    unittest.main()
