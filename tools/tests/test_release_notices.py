"""Distribution notice closure and fail-closed coverage; no network."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
import notices


class NoticeTests(unittest.TestCase):
    def test_new_license_expression_stops_packaging(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata = dict(packages=[
                dict(id="omega", name="omega", version="1", source=None),
                dict(id="external", name="external", version="1", source="registry", license="UNKNOWN"),
            ], resolve=dict(nodes=[
                dict(id="omega", deps=[dict(pkg="external", dep_kinds=[dict(kind=None)])]),
                dict(id="external", deps=[]),
            ]))
            with self.assertRaisesRegex(ValueError, "unreviewed license"):
                notices.stage_dependency_notices(root / "notices", metadata, root / "rust")

    def test_closure_includes_build_tools_not_dev_only_dependencies(self):
        packages = [dict(id=name, name=name, version="1.0", source=None if name == "omega" else "registry")
                    for name in ("omega", "normal", "build", "dev", "transitive")]
        def dependency(name, kind):
            return dict(pkg=name, dep_kinds=[dict(kind=kind)])
        metadata = dict(packages=packages, resolve=dict(nodes=[
            dict(id="omega", deps=[dependency("normal", None), dependency("build", "build"), dependency("dev", "dev")]),
            dict(id="normal", deps=[dependency("transitive", None)]),
            *[dict(id=name, deps=[]) for name in ("build", "dev", "transitive")],
        ]))
        self.assertEqual([package["name"] for package in notices.dependency_closure(metadata)],
                         ["build", "normal", "transitive"])

    def test_notices_include_nested_vendored_code_and_missing_text_rejects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = dict(id="example", name="example", version="1.0", manifest_path=str(root / "Cargo.toml"))
            with self.assertRaisesRegex(ValueError, "no license text"):
                notices.license_files(package)
            (root / "LICENSE-MIT").write_text("license")
            (root / "vendor").mkdir()
            (root / "vendor/NOTICE.txt").write_text("notice")
            self.assertEqual([str(relative).replace("\\", "/") for relative, _ in notices.license_files(package)],
                             ["LICENSE-MIT", "vendor/NOTICE.txt"])

    def test_iced_fallback_does_not_cover_future_versions(self):
        with tempfile.TemporaryDirectory() as temporary:
            package = dict(id="iced", name="iced-x86", version="1.21.0", manifest_path=str(Path(temporary) / "Cargo.toml"))
            self.assertIn("Copyright", notices.license_files(package)[0][1].read_text())
            package["version"] = "1.22.0"
            with self.assertRaises(ValueError):
                notices.license_files(package)


if __name__ == "__main__":
    unittest.main()
