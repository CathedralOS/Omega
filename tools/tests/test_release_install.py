"""Controls for the real package-manager CI test; these tests install nothing."""

import hashlib
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
import install_upgrade as installation


class InstallationTests(unittest.TestCase):
    def test_winget_cleanup_uses_the_last_installed_local_manifest(self):
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            inputs = []
            for number in (1, 2):
                version = f"0.1.0-alpha.{number}"
                archive = workspace / f"omega-{version}-x86_64-pc-windows-msvc.zip"
                archive.write_bytes(b"fixture")
                inputs.append((archive, dict(version=version, host="x86_64-pc-windows-msvc")))
            with patch.dict(os.environ, LOCALAPPDATA=str(workspace)), \
                 patch.object(installation.package, "run") as run, \
                 patch.object(installation, "installed_identity"), \
                 patch.object(installation.package, "git_source_smoke"), \
                 patch.object(installation, "rebuild_existing_project", return_value="digest"):
                self.assertEqual(installation.winget_upgrade(*inputs, "http://127.0.0.1:1234", workspace), "digest")
            self.assertEqual(run.call_args.args[0][:4],
                             ["winget", "uninstall", "--manifest", workspace / "upgrade.yaml"])

    def test_installer_mutation_rejects_non_ci_hosts(self):
        arguments = ["install_upgrade", "check", "--manager", "winget",
                     "--host", "x86_64-pc-windows-msvc",
                     "--previous-tag", "v0.1.0-alpha.2", "--tag", "v0.1.0-alpha.3"]
        with patch.object(sys, "argv", arguments), patch.dict(os.environ, GITHUB_ACTIONS="false"), \
             patch.object(installation, "release_input") as release_input:
            with self.assertRaisesRegex(ValueError, "disposable"):
                installation.main()
            release_input.assert_not_called()

    def test_installer_refuses_existing_command(self):
        arguments = ["install_upgrade", "check", "--manager", "homebrew",
                     "--host", "aarch64-apple-darwin",
                     "--previous-tag", "v0.1.0-alpha.2", "--tag", "v0.1.0-alpha.3"]
        with patch.object(sys, "argv", arguments), patch.dict(os.environ, GITHUB_ACTIONS="true"), \
             patch.object(installation.shutil, "which", return_value="existing-omega"), \
             patch.object(installation, "release_input") as release_input:
            with self.assertRaisesRegex(ValueError, "existing omega"):
                installation.main()
            release_input.assert_not_called()

    def test_upgraded_compiler_reuses_home_project_and_lock_offline(self):
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            project = workspace / "git source smoke"
            project.mkdir()
            (project / "omega.lock").write_bytes(b"already accepted")
            with patch.object(installation.package, "run", return_value="") as run, \
                 patch.dict(os.environ, GH_TOKEN="secret", GITHUB_TOKEN="secret"):
                digest = installation.rebuild_existing_project(Path("registered-omega"), workspace)
            self.assertEqual(digest, hashlib.sha256(b"already accepted").hexdigest())
            self.assertEqual(run.call_count, 2)
            build, execute = run.call_args_list
            self.assertIn("--offline", build.args[0])
            self.assertEqual(build.args[0][-1], project / "main.omg")
            self.assertEqual(build.kwargs["environment"]["HOME"], str(workspace / "git smoke home"))
            self.assertNotIn("GH_TOKEN", build.kwargs["environment"])
            self.assertNotIn("GITHUB_TOKEN", build.kwargs["environment"])
            self.assertEqual(execute.kwargs["expected"], 42)

    def test_upgrade_cannot_silently_replace_lock(self):
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            project = workspace / "git source smoke"
            project.mkdir()
            lock = project / "omega.lock"
            lock.write_bytes(b"accepted")
            def execute(*unused, **options):
                lock.write_bytes(b"changed")
            with patch.object(installation.package, "run", side_effect=execute):
                with self.assertRaisesRegex(ValueError, "rewrote"):
                    installation.rebuild_existing_project(Path("omega"), workspace)


if __name__ == "__main__":
    unittest.main()
