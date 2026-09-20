#!/usr/bin/env python3
"""Guard the Rust-producer omission audit: the documented checks and exit
statuses cannot drift from the tool unnoticed.

Runs locally only; no network access or non-standard Python packages:

    python3 tools/tests/test_rust_producer_omission.py -v
"""

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools" / "rust_producer_omission.py"


def load_tool():
    specification = importlib.util.spec_from_file_location(
        "rust_producer_omission_under_test", TOOL
    )
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


class OmissionAuditTests(unittest.TestCase):
    def setUp(self):
        self.module = load_tool()
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def manifest(self, name: str, members: list[str]) -> Path:
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n".join(members) + "\n", encoding="ascii")
        return path

    def steps(self, name: str, lines: list[str]) -> Path:
        path = self.root / name
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return path

    def audit(self, manifests=(), steps=()):
        return self.module.audit(
            manifests, steps, self.root.resolve()
        )

    def test_clean_set_omits_producer(self):
        manifest = self.manifest(
            "closure.txt",
            ["source/omega/main.omg", "source/library/core/strict.omg", "README.md"],
        )
        record = self.audit(manifests=[manifest])
        self.assertEqual(record["result"], "omitted")
        self.assertEqual(record["member_count"], 3)
        self.assertEqual(record["findings"], [])

    def test_sources_manifest_reads_member_spellings(self):
        path = self.root / "closure.epsilon.sources"
        path.write_text(
            "EpsilonSourceClosureV1\n"
            "member " + "a" * 64 + " 3 " + "b" * 64 + " src/one.epsilon\n"
            "member " + "c" * 64 + " 4 " + "d" * 64 + " omega-rust/omega/x.rs\n",
            encoding="ascii",
        )
        record = self.audit(manifests=[path])
        self.assertEqual(record["result"], "present")
        self.assertEqual(record["member_count"], 2)
        self.assertEqual(len(record["findings"]), 1)
        self.assertEqual(record["findings"][0]["check"], "producer-artifact")

    def test_omega_rust_member_is_a_producer_artifact(self):
        manifest = self.manifest(
            "closure.txt", ["source/omega/main.omg", "omega-rust/omega/src/main.rs"]
        )
        record = self.audit(manifests=[manifest])
        self.assertEqual(record["result"], "present")
        self.assertEqual(record["findings"][0]["check"], "producer-artifact")

    def test_traversal_escaping_root_is_checkout_derived(self):
        manifest = self.manifest(
            "nested/closure.txt", ["../omega-rust/source/library/std"]
        )
        record = self.audit(manifests=[manifest])
        self.assertEqual(record["result"], "present")
        checks = {finding["check"] for finding in record["findings"]}
        self.assertIn("checkout-derived-path", checks)

    def test_absolute_member_rejects(self):
        manifest = self.manifest("closure.txt", ["/opt/omega-rust/bin/omega"])
        record = self.audit(manifests=[manifest])
        self.assertEqual(record["result"], "present")

    def test_rust_toolchain_step_is_producer_build_step(self):
        steps = self.steps(
            "steps.txt",
            ["python3 tools/bootstrap/source_closure.py m.sources out.bin",
             "cargo build -p omega --release",
             "mbx nextest run -p compiler"],
        )
        record = self.audit(steps=[steps])
        self.assertEqual(record["result"], "present")
        checks = {finding["check"] for finding in record["findings"]}
        self.assertEqual(checks, {"producer-build-step"})
        # cargo, mbx and nextest each flag once.
        self.assertEqual(len(record["findings"]), 3)

    def test_checkout_derived_step_markers(self):
        steps = self.steps(
            "steps.txt",
            ["cp $CARGO_MANIFEST_DIR/../../../../source/library/std omega_lib/",
             "install -D target/release/omega bin/omega"],
        )
        record = self.audit(steps=[steps])
        self.assertEqual(record["result"], "present")
        checks = {finding["check"] for finding in record["findings"]}
        self.assertIn("checkout-derived-path", checks)

    def test_omega_rust_step_reference_is_producer_build_step(self):
        steps = self.steps("steps.txt", ["./omega-rust/target/release/omega build app.omg"])
        record = self.audit(steps=[steps])
        self.assertEqual(record["result"], "present")
        self.assertEqual(record["findings"][0]["check"], "producer-build-step")

    def test_main_exit_status_and_record(self):
        manifest = self.manifest("ok.txt", ["a/b.omg"])
        bad = self.manifest("bad.txt", ["omega-rust/x.rs"])
        record_out = self.root / "record.json"
        argv = [
            "--manifest", str(manifest),
            "--record", str(record_out),
            "--root", str(self.root),
        ]
        previous = sys.argv
        try:
            sys.argv = ["rust_producer_omission.py", *argv]
            self.assertEqual(self.module.main(), 0)
        finally:
            sys.argv = previous
        record = json.loads(record_out.read_text(encoding="utf-8"))
        self.assertEqual(record["schema"], "ReleaseRecordSubstrateV1")
        self.assertEqual(record["result"], "omitted")

        previous = sys.argv
        try:
            sys.argv = [
                "rust_producer_omission.py",
                "--manifest", str(bad),
                "--root", str(self.root),
            ]
            self.assertEqual(self.module.main(), 1)
            sys.argv = [
                "rust_producer_omission.py",
                "--manifest", str(bad),
                "--root", str(self.root),
                "--require", "present",
            ]
            self.assertEqual(self.module.main(), 0)
        finally:
            sys.argv = previous


if __name__ == "__main__":
    unittest.main()
