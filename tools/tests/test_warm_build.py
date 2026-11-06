#!/usr/bin/env python3
"""Warm build pack/restore controls on a throwaway repository; no compiler build or network."""

import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import warm_build

TOOLCHAIN = {"rustc": "rustc 1.0.0-test", "cargo": "cargo 1.0.0-test"}


def git(cwd, *arguments):
    return subprocess.run(["git", *arguments], cwd=cwd, check=True, text=True,
                          capture_output=True).stdout.strip()


class WarmBuildTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name, "Omega")
        self.root.mkdir()
        git(self.root, "init", "-q")
        git(self.root, "config", "user.email", "test@example.com")
        git(self.root, "config", "user.name", "Test")
        Path(self.root, "source.rs").write_text("fn main() {}\n")
        git(self.root, "add", ".")
        git(self.root, "commit", "-q", "-m", "base")
        self.commit = git(self.root, "rev-parse", "HEAD")
        self.built = self.root / ".agents" / "worktree" / "built"
        git(self.root, "worktree", "add", "-q", "--detach", str(self.built), self.commit)
        output = self.built / "target" / "debug" / "build" / "crate" / "hash"
        output.mkdir(parents=True)
        (output / "artifact").write_bytes(b"compiled")
        (self.built / "target/debug/incremental/crate-abc/s-session").mkdir(parents=True)
        patch = mock.patch.object(warm_build, "toolchain", return_value=dict(TOOLCHAIN))
        patch.start()
        self.addCleanup(patch.stop)

    def packed(self):
        return warm_build.pack(self.built, Path(self.directory.name, "pack")).parent

    def test_restore_unpacks_the_build_and_ages_only_the_published_sources(self):
        source = self.packed()
        warm = warm_build.warm_worktree(self.root)
        manifest = warm_build.restore(self.root, source=str(source))
        self.assertEqual(manifest["commit"], self.commit)
        self.assertEqual((warm / "target/debug/build/crate/hash/artifact").read_bytes(), b"compiled")
        self.assertEqual(os.stat(warm / "source.rs").st_mtime, warm_build.OLD_TIME)
        self.assertTrue((warm / "target" / warm_build.MARKER).exists())

    def test_files_changed_after_the_published_commit_keep_new_times(self):
        source = self.packed()
        Path(self.root, "source.rs").write_text("fn main() { changed(); }\n")
        Path(self.root, "untouched.rs").write_text("\n")
        git(self.root, "add", ".")
        git(self.root, "commit", "-q", "-m", "next")
        later = git(self.root, "rev-parse", "HEAD")
        warm = warm_build.warm_worktree(self.root)
        warm_build.restore(self.root, source=str(source), revision=later)
        self.assertEqual(git(warm, "rev-parse", "HEAD"), later)
        # Cargo must see the edited file as newer than the restored build.
        self.assertGreater(os.stat(warm / "source.rs").st_mtime, warm_build.OLD_TIME)

    def test_the_manifest_records_the_unpacked_size(self):
        manifest = json.loads((self.packed() / warm_build.MANIFEST).read_text())
        self.assertGreater(manifest["unpacked_bytes"], len(b"compiled"))

    def test_too_little_disk_is_refused_before_unpacking(self):
        # A session's first restore failed with a bare tar exit status 2 when
        # the disk filled; the shortfall must be named before anything unpacks.
        source = self.packed()
        full = mock.Mock(free=1024)
        with mock.patch.object(warm_build.shutil, "disk_usage", return_value=full), \
                self.assertRaisesRegex(warm_build.WarmBuildError, "GB free to unpack"):
            warm_build.restore(self.root, source=str(source))
        self.assertFalse((warm_build.warm_worktree(self.root) / "target").exists())

    def test_a_dirty_worktree_is_refused_before_anything_changes(self):
        source = self.packed()
        warm = warm_build.warm_worktree(self.root)
        git(self.root, "worktree", "add", "-q", "--detach", str(warm), self.commit)
        (warm / "source.rs").write_text("local edit\n")
        with self.assertRaisesRegex(warm_build.WarmBuildError, "uncommitted changes"):
            warm_build.restore(self.root, source=str(source))
        self.assertEqual((warm / "source.rs").read_text(), "local edit\n")
        self.assertFalse((warm / "target").exists())

    def test_a_different_toolchain_is_refused_and_leaves_no_partial_build(self):
        source = self.packed()
        with mock.patch.object(warm_build, "toolchain",
                               return_value={"rustc": "rustc other", "cargo": "cargo other"}):
            with self.assertRaisesRegex(warm_build.WarmBuildError, "toolchain differs"):
                warm_build.restore(self.root, source=str(source))
        self.assertFalse((warm_build.warm_worktree(self.root) / "target").exists())

    def test_a_corrupted_part_is_refused(self):
        source = self.packed()
        manifest = json.loads((source / warm_build.MANIFEST).read_text())
        with open(source / manifest["parts"][0]["name"], "ab") as part:
            part.write(b"tampered")
        with self.assertRaisesRegex(warm_build.WarmBuildError, "does not match"):
            warm_build.restore(self.root, source=str(source))
        self.assertFalse((warm_build.warm_worktree(self.root) / "target").exists())

    def test_only_the_newest_incremental_session_of_each_crate_is_packed(self):
        crate = self.built / "target/debug/incremental/crate-abc"
        crate.mkdir(parents=True, exist_ok=True)
        (crate / "s-session").rmdir()
        for name, when in (("s-old", 1000), ("s-new", 2000)):
            (crate / name).mkdir()
            (crate / (name + ".lock")).write_text("")
            os.utime(crate / name, (when, when))
        stale = warm_build.stale_incremental_sessions(self.built)
        self.assertEqual(sorted(stale), ["target/debug/incremental/crate-abc/s-old",
                                         "target/debug/incremental/crate-abc/s-old.lock"])

    def test_a_build_without_incremental_state_is_not_published(self):
        import shutil
        shutil.rmtree(self.built / "target/debug/incremental")
        with self.assertRaisesRegex(warm_build.WarmBuildError, "CARGO_INCREMENTAL=1"):
            self.packed()

    def test_session_start_does_nothing_outside_cloud_sessions(self):
        with mock.patch.dict(os.environ, {"CLAUDE_CODE_REMOTE": "false"}), \
                mock.patch.object(warm_build, "restore") as restore:
            self.assertEqual(warm_build.session_start(self.root), 0)
        restore.assert_not_called()

    def test_session_start_never_touches_a_warm_worktree_already_in_use(self):
        (warm_build.warm_worktree(self.root) / "target").mkdir(parents=True)
        with mock.patch.dict(os.environ, {"CLAUDE_CODE_REMOTE": "true"}), \
                mock.patch.object(warm_build, "restore") as restore:
            self.assertEqual(warm_build.session_start(self.root), 0)
        restore.assert_not_called()

    def test_an_interrupted_part_download_resumes_at_the_next_unread_byte(self):
        def download(payload, status):
            process = mock.Mock()
            process.stdout = io.BytesIO(payload)
            process.wait.return_value = status
            return process

        # The proxy cut the first connection after 3 bytes (curl exit 56).
        attempts = [download(b"abc", 56), download(b"def", 0)]
        with mock.patch.object(warm_build.subprocess, "Popen",
                               side_effect=attempts) as popen:
            data = b"".join(warm_build.part_blocks("https://host/release", "part.00",
                                                   block_size=2))
        self.assertEqual(data, b"abcdef")
        commands = [call.args[0] for call in popen.call_args_list]
        self.assertNotIn("-C", commands[0])
        self.assertEqual(commands[1][1:3], ["-C", "3"])

    def test_a_part_that_never_progresses_fails_after_bounded_attempts(self):
        def refused(*_arguments, **_options):
            process = mock.Mock()
            process.stdout = io.BytesIO(b"")
            process.wait.return_value = 56
            return process

        with mock.patch.object(warm_build.subprocess, "Popen", side_effect=refused) as popen:
            with self.assertRaisesRegex(warm_build.WarmBuildError, "curl exit 56"):
                list(warm_build.part_blocks("https://host/release", "part.00"))
        self.assertEqual(popen.call_count, warm_build.PART_ATTEMPTS)

    def test_session_start_without_zstd_reports_it_instead_of_downloading(self):
        with mock.patch.dict(os.environ, {"CLAUDE_CODE_REMOTE": "true"}), \
                mock.patch.object(warm_build.shutil, "which", return_value=None), \
                mock.patch.object(warm_build.subprocess, "run") as run, \
                mock.patch.object(warm_build, "restore") as restore:
            self.assertEqual(warm_build.session_start(self.root), 0)
        restore.assert_not_called()
        installs = [call.args[0] for call in run.call_args_list]
        self.assertEqual(installs[0], ["apt-get", "update", "-qq"])
        status = warm_build.status_path(self.root).read_text(encoding="utf-8")
        self.assertIn("zstd is not installed", status)


if __name__ == "__main__":
    unittest.main()
