"""The sample queue lists failing sample programs, smallest first."""

import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "sample_queue", Path(__file__).resolve().parents[1] / "sample_queue.py")
queue_module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(queue_module)


class SampleQueueTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        patcher = patch.object(queue_module, "ROOT", self.root)
        patcher.start()
        self.addCleanup(patcher.stop)

    def sample(self, path, lines):
        directory = self.root / path
        directory.mkdir(parents=True)
        (directory / "build.omg").write_text("machine build() {}\n" * 50)
        (directory / "main.omg").write_text("// line\n" * lines)
        return path

    def record(self, *rows):
        return {"results": [{"sample": sample, "state": state, "detail": detail}
                            for sample, state, detail in rows]}

    def test_failing_programs_come_smallest_first(self):
        big = self.sample("samples/cli/big", 300)
        small = self.sample("samples/cli/small", 20)
        passing = self.sample("samples/cli/passing", 5)
        rows = queue_module.queue(self.record(
            (big, "no-native-build", "big refusal"),
            (small, "oracle-failed", "small refusal"),
            (passing, "pass", "")))
        self.assertEqual([row["sample"] for row in rows], [small, big])
        self.assertEqual(rows[0]["lines"], 20)

    def test_nested_projects_and_build_files_do_not_count(self):
        parent = self.sample("samples/cli/lab", 10)
        self.sample("samples/cli/lab/controls/invalid", 400)
        rows = queue_module.queue(self.record((parent, "no-native-build", "")))
        self.assertEqual(rows[0]["lines"], 10)

    def test_probes_and_controls_only_with_all(self):
        probe = self.sample("samples/cli/probes/gap", 5)
        control = self.sample("samples/cli/lab/controls/invalid_case", 5)
        program = self.sample("samples/cli/app", 50)
        record = self.record((probe, "check-fail", ""),
                             (control, "rejection-failed", ""),
                             (program, "no-native-build", ""))
        self.assertEqual([row["sample"] for row in queue_module.queue(record)], [program])
        everything = queue_module.queue(record, include_all=True)
        self.assertEqual([row["kind"] for row in everything], ["program", "probe", "control"])


if __name__ == "__main__":
    unittest.main()
