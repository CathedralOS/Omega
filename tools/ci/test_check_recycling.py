from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from check_recycling import violations


class RecyclingBoundary(unittest.TestCase):
    def check(self, files):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            paths = set()
            for name, text in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text)
                paths.add(path)
            return list(violations(root, paths))

    def test_dependency_kinds_and_workspace_alias(self):
        for table in ("dependencies", "build-dependencies", "dev-dependencies",
                      "workspace.dependencies", "target.'cfg(windows)'.dependencies"):
            with self.subTest(table=table):
                line = f'[{table}]\nrenamed = {{ package = "real", path = "../_recycling/real" }}'
                self.assertEqual(len(self.check({"live/Cargo.toml": line})), 1)

    def test_glob_members_respect_exclude_but_literal_members_win(self):
        files = {"_recycling/old/Cargo.toml": "", "src/Cargo.toml": ""}
        for members, excluded, result in [('["_recycling/*"]', '[]', 1),
                                         ('["_recycling/*"]', '["_recycling"]', 0),
                                         ('["_recycling/old"]', '["_recycling/old"]', 1)]:
            with self.subTest(members=members, excluded=excluded):
                files["Cargo.toml"] = f"[workspace]\nmembers={members}\nexclude={excluded}"
                self.assertEqual(len(self.check(files)), result)

    def test_direct_rust_paths(self):
        for expression in [
            '#[path = "../_recycling/old.rs"] mod old;',
            '#[cfg_attr(windows, path = r#"../_recycling/windows.rs"#)] mod chosen;',
            'include! { "../_recycling/old.rs" }',
            'include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/_rec", "ycling/old.rs"));',
            'const DATA: &[u8] = include_bytes!("../_recycling/old.bin");',
        ]:
            with self.subTest(expression=expression):
                self.assertEqual(len(self.check({"src/lib.rs": expression, "Cargo.toml": ""})), 1)

    def test_module_directory(self):
        files = {"src/lib.rs": "mod parent { mod r#_recycling; }", "src/parent/_recycling/mod.rs": ""}
        nested = "src/parent/_recycling/mod.rs: nested _recycling directory; use the repository-root _recycling"
        errors = self.check(files)
        self.assertIn(nested, errors)
        self.assertTrue(any("Rust module points into _recycling" in error for error in errors))
        files["src/lib.rs"] = 'mod parent { #[path = "normal.rs"] mod _recycling; }'
        self.assertEqual(self.check(files), [nested])

    def test_checkout_checks_all_tracked_directory_names(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            paths = ["docs/_recycling/design.md", "_recycling/owner/_recycling/dump.bin",
                     "_recycling/owner/dump.bin", "assets/live.bin",
                     "omega-rust-v2/owner/_recycling/legacy.bin", "notes/near_recycling/design.md"]
            for name in paths:
                path = repo / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"\xff")  # File contents are irrelevant to the layout.
            subprocess.run(["git", "init", "--quiet"], cwd=repo, check=True)
            subprocess.run(["git", "add", "--", *paths[2:]], cwd=repo, check=True)
            check = [sys.executable, "-B", str(Path(__file__).with_name("check_recycling.py").resolve())]
            self.assertEqual(subprocess.run(check, cwd=repo).returncode, 0)
            subprocess.run(["git", "add", "--", *paths[:2]], cwd=repo, check=True)
            refused = subprocess.run(check, cwd=repo, capture_output=True, text=True)
            self.assertEqual(refused.returncode, 1)
            self.assertCountEqual([row.split(":")[0] for row in refused.stderr.splitlines()], paths[:2])

    def test_checkout_command_checks_tracked_symlink_targets(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            (repo / "_recycling").mkdir()
            (repo / "_recycling" / "impl.rs").write_text("")
            (repo / "src").mkdir()
            (repo / "src/lib.rs").write_text("mod active;")
            (repo / "src/active.rs").symlink_to("../_recycling/impl.rs")
            (repo / "src/active.bin").symlink_to("../_recycling/impl.rs")
            subprocess.run(["git", "init", "--quiet"], cwd=repo, check=True)
            subprocess.run(["git", "add", "."], cwd=repo, check=True)
            check = [sys.executable, "-B", str(Path(__file__).with_name("check_recycling.py").resolve())]
            refused = subprocess.run(check, cwd=repo / "src", capture_output=True, text=True)
            self.assertEqual(refused.returncode, 1)
            self.assertIn("src/active.rs links into _recycling", refused.stderr)
            self.assertIn("src/active.bin links into _recycling", refused.stderr)
            (repo / "src/active.bin").unlink()
            (repo / "src/active.rs").unlink()
            (repo / "src/active.rs").write_text("")
            self.assertEqual(subprocess.run(check, cwd=repo).returncode, 0)

    def test_text_and_archive_internals_do_not_declare_active_edges(self):
        self.assertEqual(self.check({
            "Cargo.toml": '[package.metadata.docs]\npath="_recycling/lib.rs"\n[dependencies]\nnear={path="near_recycling"}',
            "src/lib.rs": '''// include!("_recycling/file.rs");
                /* nested /* comment */ #[path="_recycling/mod.rs"] mod wrong; */
                const DOC: &str = r##"#[path=r#"_recycling/old.rs"#] mod wrong;"##;
                include_str!("near_recycling/readme");
                #[path = "../_recycling/../live.rs"] mod active;
            ''',
            "_recycling/old/Cargo.toml": '[dependencies]\nunused={path="../other"}',
            "_recycling/lib.rs": 'include!("_recycling/inner.rs");',
        }), [])


if __name__ == "__main__":
    unittest.main()
