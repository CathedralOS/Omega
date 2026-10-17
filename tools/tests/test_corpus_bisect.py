#!/usr/bin/env python3
"""`git bisect run` over corpus_gate.py blames the right commit.

A throwaway repository commits a fake corpus_runner (a python script that
writes records from a committed data file) across six commits: one fixture
regresses at a known commit, and a later commit breaks the runner outright.
The gate's --bisect mapping must name the regression — never the
infrastructure failure, which maps to skip.

    python3 tools/tests/test_corpus_bisect.py -v
"""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

GATE = Path(__file__).resolve().parents[1] / "corpus_gate.py"
RECORDS = Path(__file__).resolve().parents[1] / "corpus_records.py"

RUNNER = """#!/usr/bin/env python3
import os
import sys
from pathlib import Path
print("corpus_runner: fixture-root-v1 " + os.fsencode(os.environ["OMEGA_CORPUS_ROOT"]).hex(), file=sys.stderr)
data = (Path(__file__).resolve().parent / "data.txt").read_text()
if "runner-broken" in data:
    sys.exit(2)
status = "rejected" if "regressed" in data else "built"
Path(os.environ["OMEGA_CORPUS_RECORDS"]).write_text(
    "pass/alpha/kept checked 10ms\\n"
    "run/alpha/moving built 20ms exit:0\\n"
    f"run/alpha/subject {status} 30ms\\n", encoding="utf-8")
"""


class BisectRun(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(
            prefix="omega corpus bisect ")
        self.addCleanup(self.temporary.cleanup)
        self.repository = Path(self.temporary.name) / "repo"
        self.environment = {
            key: value for key, value in os.environ.items()
            if not key.startswith("GIT_")}
        self.environment.update(GIT_CONFIG_NOSYSTEM="1",
                                GIT_CONFIG_GLOBAL=os.devnull,
                                GIT_TERMINAL_PROMPT="0")
        self.git(self.repository.parent, "init", "-b", "main",
                 str(self.repository))
        for name, value in (("user.name", "Bisect Test"),
                            ("user.email", "test@example.invalid"),
                            ("commit.gpgSign", "false")):
            self.git(self.repository, "config", name, value)
        runner = self.repository / "runner.py"
        runner.write_text(RUNNER, encoding="utf-8")
        runner.chmod(0o755)
        self.commits = []
        # c0/c1 good; c2 regresses `subject` (bad); c3 breaks the runner
        # (skip); c4/c5 fix the runner but keep the regression. A skipped
        # commit adjacent to the first bad would leave the answer
        # ambiguous, so the breakage sits strictly inside the bad range.
        for data in ("plain-a", "plain-b", "regressed-a", "runner-broken",
                     "regressed-b", "regressed-c"):
            (self.repository / "data.txt").write_text(data + "\n",
                                                     encoding="utf-8")
            self.git(self.repository, "add", "-A")
            self.git(self.repository, "commit", "-m", data)
            self.commits.append(
                self.git(self.repository, "rev-parse", "HEAD"))
        # The golden is the record observed at the good commit; untracked
        # so `git bisect run` checkouts never touch it.
        self.golden = self.repository / "golden.txt"
        self.golden.write_text(
            "pass/alpha/kept checked 10ms\n"
            "run/alpha/moving built 20ms exit:0\n"
            "run/alpha/subject built 30ms\n", encoding="utf-8")

    def git(self, directory, *arguments, check=True):
        result = subprocess.run(["git", "-C", str(directory), *arguments],
                                capture_output=True, text=True,
                                encoding="utf-8", env=self.environment,
                                timeout=60)
        if check:
            self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def test_bisect_names_the_regressing_commit_and_skips_broken_runner(self):
        self.git(self.repository, "bisect", "start",
                 self.commits[5], self.commits[0])
        # git bisect run feeds each checkout to the gate; --runner skips the
        # build entirely, so the test never touches cargo/mbx.
        result = subprocess.run(
            ["git", "-C", str(self.repository), "bisect", "run",
             sys.executable, str(GATE), "--runner",
             str(self.repository / "runner.py"), "--golden",
             str(self.golden), "--bisect"],
            capture_output=True, text=True, encoding="utf-8",
            env=self.environment, timeout=120)
        output = result.stdout + result.stderr
        self.assertRegex(output, r"is the first '?bad'? commit")
        self.assertIn(self.commits[2][:10], output)
        # The skip mapping itself, asserted directly rather than relying on
        # which midpoint git probed: the broken-runner commit is untestable
        # (125), the regressing one bad (1).
        self.git(self.repository, "bisect", "reset")
        self.git(self.repository, "checkout", "--detach", self.commits[3])
        broken = subprocess.run(
            [sys.executable, str(GATE), "--runner",
             str(self.repository / "runner.py"), "--golden",
             str(self.golden), "--bisect"],
            capture_output=True, text=True, encoding="utf-8",
            env=self.environment, timeout=60)
        self.assertEqual(broken.returncode, 125, broken.stderr)

    def tearDown(self):
        subprocess.run(["git", "-C", str(self.repository), "bisect", "reset"],
                       capture_output=True, env=self.environment)


if __name__ == "__main__":
    unittest.main()
