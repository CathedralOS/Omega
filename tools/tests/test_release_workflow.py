"""Release identity, archive safety, and publication guards; no network."""

from pathlib import Path
import stat
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
import github_release as release
import package


class ReleaseWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omega release tests ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_only_concrete_prerelease_versions_are_accepted(self):
        self.assertEqual(release.version_from_tag("v0.1.0-alpha.1"), "0.1.0-alpha.1")
        for tag in ("v0.1.0", "v01.1.0-alpha.1", "v1.0.0-alpha.01", "../tag", "v1.0.0-alpha.1\n", "v1.0.0-alpha.1;echo"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.version_from_tag(tag)

    def test_matrix_matches_packager_hosts(self):
        self.assertEqual({row["host"] for row in release.MATRIX}, set(package.HOST_TARGETS))
        self.assertEqual(len(release.MATRIX), 4)

    def test_zip_rejects_escape_and_symlink(self):
        for member in ("../outside", "/outside", "bundle/../outside", "bundle/./file", "bundle/C:file", "bundle\\outside", "bundle/file\0hidden"):
            archive = self.root / "bundle.zip"
            with zipfile.ZipFile(archive, "w") as writer:
                entry = zipfile.ZipInfo()
                # Bypass ZipInfo's host-dependent normalization to exercise
                # actual untrusted archive names, not the writer's repair.
                entry.filename = member
                writer.writestr(entry, b"bad")
            with self.subTest(member=member), self.assertRaises(ValueError):
                package.extract_bundle(archive, self.root / "output")
        with zipfile.ZipFile(archive, "w") as writer:
            member = zipfile.ZipInfo("bundle/link")
            member.create_system = 3
            member.external_attr = (stat.S_IFLNK | 0o777) << 16
            writer.writestr(member, "../outside")
        with self.assertRaises(ValueError):
            package.extract_bundle(archive, self.root / "output")
        self.assertFalse((self.root / "output").exists())

    def test_tar_rejects_hard_links_and_devices(self):
        archive = self.root / "bundle.tar.gz"
        for kind in (tarfile.LNKTYPE, tarfile.SYMTYPE, tarfile.CHRTYPE, tarfile.FIFOTYPE):
            with tarfile.open(archive, "w:gz") as writer:
                member = tarfile.TarInfo("bundle/entry")
                member.type = kind
                member.linkname = "../outside"
                writer.addfile(member)
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                package.extract_bundle(archive, self.root / "output")

    def make_archive(self, directory, row, commit="a" * 40):
        library = self.root / "library"
        (library / "core").mkdir(parents=True, exist_ok=True)
        (library / "std").mkdir(exist_ok=True)
        (library / "core/a.omg").write_bytes(b"data A {}\n")
        (library / "std/build.omg").write_bytes(b"machine build() {}\n")
        binary = self.root / "omega"
        binary.write_bytes(b"fixture")
        name = release.archive_name("0.1.0-alpha.1", row)
        bundle = self.root / name.removesuffix(row["suffix"])
        metadata = dict(version="0.1.0-alpha.1", commit=commit, host=row["host"], dirty=False,
                        library_sha256=package.library_digest(library))
        package.stage_bundle(bundle, binary, library, metadata)
        archive = directory / name
        package.archive_bundle(bundle, archive)
        checksum = package.hashlib.sha256(archive.read_bytes()).hexdigest()
        archive.with_name(archive.name + ".sha256").write_text(f"{checksum}  {archive.name}\n")
        return archive

    def test_complete_asset_set_is_required_and_commits_must_agree(self):
        assets = self.root / "assets"
        assets.mkdir()
        for row in release.MATRIX:
            self.make_archive(assets, row)
        self.assertEqual(len(release.checked_assets(assets, "0.1.0-alpha.1", "a" * 40)), 8)
        with self.assertRaises(ValueError):
            release.checked_assets(assets, "0.1.0-alpha.1", "b" * 40)
        next(assets.glob("*.sha256")).unlink()
        with self.assertRaises(ValueError):
            release.checked_assets(assets, "0.1.0-alpha.1", "a" * 40)

    def test_checksum_failure_precedes_extraction(self):
        archive = self.make_archive(self.root, release.MATRIX[0])
        archive.with_name(archive.name + ".sha256").write_text("wrong")
        with self.assertRaisesRegex(ValueError, "checksum"):
            package.verify_archive(archive, self.root / "output", version="0.1.0-alpha.1",
                                   commit="a" * 40, host=release.MATRIX[0]["host"])
        self.assertFalse((self.root / "output").exists())

    def test_publication_rejects_existing_release_before_mutation(self):
        with patch.object(release, "checked_assets", return_value=[]), \
             patch.object(release, "existing_release", return_value={"id": 1}), \
             patch.object(release, "command") as command:
            with self.assertRaisesRegex(ValueError, "already exists"):
                release.publish("v0.1.0-alpha.1", "owner/repo", "a" * 40, self.root)
            command.assert_not_called()

    def test_release_lookup_does_not_hide_api_failure(self):
        with patch.object(release, "command", side_effect=RuntimeError("offline")):
            with self.assertRaisesRegex(RuntimeError, "offline"):
                release.existing_release("owner/repo", "v0.1.0-alpha.1")


if __name__ == "__main__":
    unittest.main()
