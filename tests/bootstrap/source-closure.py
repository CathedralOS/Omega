"""Byte-only closure custody tests for manifested bootstrap source languages."""

import hashlib
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(os.environ["OMEGA_REPO_ROOT"])
TOOL = ROOT / "tools/bootstrap/source_closure.py"
HEADERS = {
    "GammaSourceClosureV1": ".gamma",
    "DeltaSourceClosureV1": ".delta",
    "EpsilonSourceClosureV1": ".epsilon",
}


def member(identity, path, data):
    return f"member {identity:064x} {len(data)} {hashlib.sha256(data).hexdigest()} {path}"


class SourceClosure(unittest.TestCase):
    def invoke(self, manifest, output, prefix=None):
        return subprocess.run(
            ["python3", str(TOOL), str(manifest), str(output)]
            + ([] if prefix is None else ["--prefix", str(prefix)]),
            capture_output=True,
        )

    def fixture(self, directory, header):
        closure = directory / "closure"
        closure.mkdir()
        suffix = HEADERS[header]
        first = closure / f"parts/first{suffix}"
        first.parent.mkdir()
        first.write_bytes(b"; first\r\n")
        second = closure / f"second{suffix}"
        second.write_bytes(b"\t; second\n")
        rows = [
            header,
            member(1, first.relative_to(closure).as_posix(), first.read_bytes()),
            member(2, second.name, second.read_bytes()),
        ]
        manifest = closure / "compiler.sources"
        manifest.write_text("\n".join(rows) + "\n", encoding="ascii")
        return manifest, first, second, rows

    def test_exact_order_and_bytes(self):
        for header in HEADERS:
            with self.subTest(header=header), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                manifest, first, second, rows = self.fixture(directory, header)
                # Reverse paths and corresponding data, not ascending identities.
                rows[1:] = [
                    member(1, second.name, second.read_bytes()),
                    member(2, first.relative_to(manifest.parent).as_posix(), first.read_bytes()),
                ]
                manifest.write_text("\n".join(rows), encoding="ascii")
                output = directory / "output"
                result = self.invoke(manifest, output)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(output.read_bytes(), second.read_bytes() + first.read_bytes())

    def test_prefix_order_and_failure_preservation(self):
        for header in HEADERS:
            for case in ("valid", "missing", "control", "nonascii", "symlink", "stale_member"):
                with self.subTest(header=header, case=case), tempfile.TemporaryDirectory() as temporary:
                    directory = Path(temporary)
                    manifest, first, second, rows = self.fixture(directory, header)
                    prefix = directory / ("entry" + HEADERS[header])
                    prefix.write_bytes(b"; exact entry\r\n\t")
                    expected = prefix.read_bytes() + first.read_bytes() + second.read_bytes()
                    if case == "missing": prefix.unlink()
                    elif case == "control": prefix.write_bytes(b"\x00")
                    elif case == "nonascii": prefix.write_bytes(b"\x80")
                    elif case == "symlink":
                        target = directory / "real-entry"
                        prefix.rename(target)
                        prefix.symlink_to(target)
                    elif case == "stale_member": first.write_bytes(b"changed")
                    output = directory / "output"
                    for existing in (False, True):
                        if existing:
                            output.write_bytes(b"keep prior output")
                        result = self.invoke(manifest, output, prefix)
                        if case == "valid":
                            self.assertEqual(result.returncode, 0, result.stderr)
                            self.assertEqual(output.read_bytes(), expected)
                        else:
                            self.assertNotEqual(result.returncode, 0)
                            self.assertIn(b"source closure:", result.stderr)
                            self.assertEqual(result.stdout, b"")
                            if existing:
                                self.assertEqual(output.read_bytes(), b"keep prior output")
                            else:
                                self.assertFalse(output.exists())

    def test_rejections_preserve_output(self):
        cases = [
            "header", "empty", "malformed", "reordered", "duplicate_identity",
            "duplicate_path", "length_spelling", "length", "digest", "missing",
            "extra", "foreign_source", "wrong_suffix", "wrong_language_member",
            "dot", "double_slash", "inner_dot",
            "parent", "absolute", "backslash", "drive", "source_byte", "source_nonascii",
            "manifest_ascii", "manifest_byte", "member_symlink",
            "directory_symlink", "manifest_symlink",
        ]
        for header in HEADERS:
            for case in cases:
                with self.subTest(header=header, case=case), tempfile.TemporaryDirectory() as temporary:
                    directory = Path(temporary)
                    manifest, first, second, rows = self.fixture(directory, header)
                    spelling = first.relative_to(manifest.parent).as_posix()
                    foreign_suffix = ".epsilon" if HEADERS[header] == ".gamma" else ".gamma"
                    replacement_paths = {
                        "dot": "./" + spelling,
                        "double_slash": spelling.replace("/", "//"),
                        "inner_dot": spelling.replace("/", "/./"),
                        "parent": "../" + spelling,
                        "absolute": first.as_posix(),
                        "backslash": spelling.replace("/", "\\"),
                        "drive": "C:/" + spelling,
                        "wrong_suffix": "first.txt",
                    }
                    if case in replacement_paths:
                        rows[1] = rows[1].replace(spelling, replacement_paths[case])
                    elif case == "header": rows[0] = "UnknownSourceClosureV1"
                    elif case == "empty": rows = rows[:1]
                    elif case == "malformed": rows[1] = "member broken"
                    elif case == "reordered": rows[1:] = reversed(rows[1:])
                    elif case == "duplicate_identity": rows[2] = rows[2].replace(f"{2:064x}", f"{1:064x}", 1)
                    elif case == "duplicate_path": rows[2] = member(2, spelling, first.read_bytes())
                    elif case == "length_spelling": rows[1] = rows[1].replace(" 9 ", " 09 ")
                    elif case == "length": rows[1] = rows[1].replace(" 9 ", " 8 ")
                    elif case == "digest": rows[1] = member(1, spelling, b"different")
                    elif case == "missing": first.unlink()
                    elif case == "extra": (manifest.parent / ("unlisted" + HEADERS[header])).write_bytes(b"")
                    elif case == "foreign_source":
                        (manifest.parent / ("unlisted" + foreign_suffix)).write_bytes(b"")
                    elif case == "wrong_language_member":
                        foreign = first.with_suffix(foreign_suffix)
                        first.rename(foreign)
                        rows[1] = member(1, foreign.relative_to(manifest.parent).as_posix(), foreign.read_bytes())
                    elif case in ("source_byte", "source_nonascii"):
                        first.write_bytes(b"\x01" if case == "source_byte" else b"\x80")
                        rows[1] = member(1, spelling, first.read_bytes())
                    elif case in ("member_symlink", "directory_symlink"):
                        target = first if case == "member_symlink" else first.parent
                        moved = directory / "moved"
                        target.rename(moved)
                        target.symlink_to(moved, target_is_directory=moved.is_dir())
                    manifest.write_text("\n".join(rows) + "\n", encoding="ascii")
                    if case == "manifest_ascii": manifest.write_bytes(manifest.read_bytes() + b"\xff")
                    elif case == "manifest_byte": manifest.write_bytes(manifest.read_bytes() + b"\x0b")
                    elif case == "manifest_symlink":
                        moved = directory / "manifest"
                        manifest.rename(moved)
                        manifest.symlink_to(moved)
                    output = directory / "output"
                    for existing in (False, True):
                        if existing:
                            output.write_bytes(b"keep prior output")
                        result = self.invoke(manifest, output)
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn(b"source closure:", result.stderr)
                        self.assertEqual(result.stdout, b"")
                        if existing:
                            self.assertEqual(output.read_bytes(), b"keep prior output")
                        else:
                            self.assertFalse(output.exists())

    def test_selected_compiler_closures(self):
        closures = [
            (ROOT / "tests/bootstrap/epsilon-source-closure/fixture.sources", 89,
             "528f65b2e2d9666db1c1f3930c9f5784bbfc1497e3b7225b26cb3eee34d2924c", None),
            (Path(os.environ["OMEGA_PATH_OMEGA_COMPILER_SOURCES"]), 464741,
             "621f507b214f0f26ba3c9d4d36a1bb54a26bdeecbcdffcc24a2cb1a266ab8cde", None),
            (Path(os.environ["OMEGA_PATH_EPSILON_COMPILER_SOURCES"]), 568684,
             "68c0a974718e933bd9ca023b3b929ade0acda913aadd39112c422f29c7cd120b", None),
            (Path(os.environ["OMEGA_PATH_DELTA_COMPILER_SOURCES"]), 111464,
             "469a007e0114cdf833b61463161a4e6ff7e246b4d01f8861ea39c948bcb3b9b6",
             Path(os.environ["OMEGA_PATH_DELTA_COMPILER_SOURCE"])),
            (Path(os.environ["OMEGA_PATH_DELTA_COMPILER_SOURCES"]), 111241,
             "824d48eb159d2afc2dd8efd279dd2c0b854ffab604f366617d684e2c77a9022c",
             Path(os.environ["OMEGA_PATH_DELTA_COMPILER_DEVELOPMENT_ENTRY"])),
        ]
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            for manifest, length, digest, prefix in closures:
                with self.subTest(manifest=manifest, prefix=prefix):
                    result = self.invoke(manifest, output, prefix)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(len(output.read_bytes()), length)
                    self.assertEqual(hashlib.sha256(output.read_bytes()).hexdigest(), digest)


if __name__ == "__main__":
    unittest.main()
