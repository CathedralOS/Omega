#!/usr/bin/env python3
"""`git bisect run` over corpus_gate.py blames the right commit.

A throwaway repository commits a fake corpus_runner (a python script that
writes records from a committed data file) across six commits: one fixture
regresses at a known commit, and a later commit breaks the runner outright.
The gate's --bisect mapping must name the regression — never the
infrastructure failure, which maps to skip.

The scripted runner is a supplied producer like any other, so it answers the
gate's profile preflight through the same protocol the real corpus_runner
implements: its bytes carry the schema and `--build-profile` reports the
profile embedded when the file was written. The gate is never told to skip
the check for it, and a runner that declines the protocol is still refused
(test_gate_refuses_a_runner_that_declines_the_profile_protocol).

    python3 tools/tests/test_corpus_bisect.py -v
"""

import os
from pathlib import Path
import re
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
# The producer protocol the gate preflights, answered before anything else:
# the query carries no fixture environment, and a supplied binary that cannot
# name its own profile is refused rather than executed.
if sys.argv[1:2] == ["--build-profile"]:
    if len(sys.argv) != 2:
        print("corpus_runner --build-profile takes no arguments", file=sys.stderr)
        sys.exit(64)
    print('{"schema": "omega-compiler-build-profile-v1", "opt_level": "1"}')
    sys.exit(0)
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

# A supplied runner that answers no profile query at all, like a stale binary
# built before the protocol existed: the gate must refuse it, not run it.
UNPROFILED_RUNNER = """#!/usr/bin/env python3
import sys
sys.exit(0)
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

    def gate_at(self, commit):
        """Run the gate once at `commit` and return (returncode, output)."""
        self.git(self.repository, "bisect", "reset")
        self.git(self.repository, "checkout", "--detach", commit)
        result = subprocess.run(
            [sys.executable, str(GATE), "--runner",
             str(self.repository / "runner.py"), "--golden",
             str(self.golden), "--bisect"],
            capture_output=True, text=True, encoding="utf-8",
            env=self.environment, timeout=60)
        return result.returncode, result.stdout + result.stderr

    # The gate launches a supplied --runner as a program, and this fixture
    # supplies a script, which only POSIX executes directly from its shebang.
    # Handing the gate an interpreter instead would stop measuring that
    # --runner launches a program, so the mechanism is POSIX-only: the test is
    # not broken on Windows, it cannot reach the mechanism there, and saying so
    # beats a failure that reads like a defect. The refusal test below needs no
    # guard -- the gate refuses an unprofiled runner from its bytes without
    # executing it, so that half measures on every host.
    @unittest.skipUnless(
        os.name == "posix",
        "corpus_gate launches a supplied --runner as a program and this fixture "
        "supplies a script, which only POSIX executes directly from its shebang; "
        "the mechanism is POSIX-only rather than this test being broken on "
        "Windows")
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
        # Assert the commit the bisect NAMES, not merely that it exited: a
        # run that skipped every step still exits, and reports no answer
        # while looking like a tool that measured and found nothing.
        named = re.search(r"([0-9a-f]{40}) is the first bad commit", output)
        self.assertIsNotNone(named, output)
        self.assertEqual(named.group(1), self.commits[2], output)
        self.assertNotIn("could be any of", output)
        self.assertNotIn("We cannot bisect more", output)
        # The skip mapping itself, asserted directly rather than relying on
        # which midpoint git probed: the good commit is 0, the regressing one
        # bad (1), and the broken-runner commit untestable (125).
        self.assertEqual(self.gate_at(self.commits[0])[0], 0)
        self.assertEqual(self.gate_at(self.commits[2])[0], 1)
        self.assertEqual(self.gate_at(self.commits[3])[0], 125)

    def test_gate_refuses_a_runner_that_declines_the_profile_protocol(self):
        """The fixture's own profile record must not have opened a bypass."""
        unprofiled = self.repository / "unprofiled.py"
        unprofiled.write_text(UNPROFILED_RUNNER, encoding="utf-8")
        unprofiled.chmod(0o755)
        result = subprocess.run(
            [sys.executable, str(GATE), "--runner", str(unprofiled),
             "--golden", str(self.golden), "--bisect"],
            capture_output=True, text=True, encoding="utf-8",
            env=self.environment, timeout=60)
        output = result.stdout + result.stderr
        self.assertEqual(result.returncode, 125, output)
        self.assertIn("optimized compiler required before validation", output)
        self.assertIn("unknown compiler build profile", output)

    def tearDown(self):
        subprocess.run(["git", "-C", str(self.repository), "bisect", "reset"],
                       capture_output=True, env=self.environment)


if __name__ == "__main__":
    unittest.main()
