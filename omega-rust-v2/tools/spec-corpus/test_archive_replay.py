"""Check the original-program replay without a compiler: a fake package CLI reads the files it is given.

`access_plan.json` is one case copied verbatim from target/layer-readiness/20261121T122154Z-tg8nywyw.
Its program digest is the one target/l5-loop-01/integration/combined-parent/all-prior-inputs.json
recorded when both immutable binaries replayed it.
"""
from contextlib import redirect_stdout
import copy
from io import StringIO
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import archive_replay as replay
import lowering_cli
import package_cases

FIXTURES = Path(__file__).with_name("fixtures") / "archive_replay"
ACCESS = "handoff/access plan evaluation at its domain"
ACCESS_DIGEST = "6792a9e44c8e74ccfde8a92e7fbcb27a68474f16a9b2259c674ab724647f69fc"
KEY = "handoff/a native case"
MESSAGE = "unsupported thing"
EARLIER, LATER = "earlier.exe", "later.exe"


def position(text, token):
    data = text.encode()
    start = data.find(token.encode())
    prefix = data[:start]
    return start, start + len(token.encode()), 1 + prefix.count(b"\n"), start - prefix.rfind(b"\n")


def diagnostic(name, text, token, message, producer="lowering"):
    start, end, line, column = position(text, token)
    return f"{name}:{line}:{column}: {producer}: {message} (bytes {start}..{end})\n"


class Compiler:
    """A package CLI whose behaviour per program is chosen by a marker in the source.

    REFUSE refuses at that token; after `fixed` markers it lowers, `typer` stops in
    preparation, `moved` refuses at `machine`, `renamed` keeps the site with another message."""

    def __init__(self, later=None, earlier=None):
        self.modes = {LATER: later or {}, EARLIER: earlier or {}}
        self.calls = []

    def __call__(self, binary, directory, argv, decode):
        action, names = argv[1], argv[3:]
        files = {name: (Path(directory) / name).read_bytes() for name in names}
        self.calls.append((binary, action, tuple(sorted(files.items()))))
        if argv[2] != "--":
            return (2, "", "unexpected option\n")
        source = {name: data.decode("utf-8") for name, data in files.items()}
        mode = next((m for marker, m in self.modes[binary].items() if any(marker in text for text in source.values())), "refuse")
        target = next((name for name, text in source.items() if "REFUSE" in text), None)
        if mode == "typer":
            name = target or names[0]
            return (1, "", diagnostic(name, source[name], "REFUSE" if target else "machine", "wrong type", "typer"))
        if action == "prepare" or target is None or mode == "lowers":
            return (0, "", "")
        if mode == "noisy":
            return (0, "lowered\n", "")
        text = source[target]
        return (1, "", {"refuse": diagnostic(target, text, "REFUSE", MESSAGE), "renamed": diagnostic(target, text, "REFUSE", "another thing"),
                        "moved": diagnostic(target, text, "machine", MESSAGE)}[mode])


def native_report(*programs, entry="source", key=KEY, extra_sites=()):
    """One blocking native case whose every program the package CLI reproduced at measurement."""
    fixtures, results, locations = [], [], list(extra_sites)
    for index, rows in enumerate(programs):
        name, text = next((n, t) for n, t in rows if "REFUSE" in t)
        start, end, line, column = position(text, "REFUSE")
        location = {"package": "app", "occurrence": "REFUSE", "file": name, "byte_start": start, "byte_end": end,
                    "line": line, "byte_column": column}
        locations.append(location)
        fixtures.append({"target": "tests\\native.rs", "test": "refuses", "files": ",".join(n for n, _ in rows),
                         "outcome": f"refused: {MESSAGE} at {name}", "location": location,
                         "replay": {"entry": entry, "message": MESSAGE, "root_message": MESSAGE, "files": copy.deepcopy(rows)}})
        expected = diagnostic(name, text, "REFUSE", MESSAGE)
        names = [n for n, _ in rows]
        check = lambda action, code, err: {"args": ["omega.exe", "source", action, "--", *names], "expected_exit": code,  # noqa: E731
                                           "expected_stdout": "", "expected_stderr": err, "matched": True, "exit": code,
                                           "stdout": "", "stderr": err}
        results.append({"fixture_index": index, "target": "tests\\native.rs", "test": "refuses", "entry": entry,
                        "origin_status": "observed", "status": "verified", "program_sha256": lowering_cli.program_digest(rows),
                        "prepare": check("prepare", 0, ""), "lower": check("lower", 1, expected)})
    case = {"cohort": "handoff", "stage": "L5", "status": "open gap", "detail": f"Observed {MESSAGE}.",
            "refusal_reason": MESSAGE, "target": "tests\\native.rs", "test": "refuses", "fixture_locations": locations}
    return {"last_layer": "L5", "measurement_errors": [], "cases": {key: case}, "package_fixtures": fixtures,
            "lowering_cli": {"results": results}}


def run(report, compiler, baseline=True):
    with TemporaryDirectory() as work:
        return replay.replay(report, LATER, EARLIER if baseline else None, work, compiler)


def verdict(report, compiler, **options):
    return run(report, compiler, **options)[2]


def only(verdicts):
    [value] = verdicts.values()
    return value["verdict"]


class PackageMeasurement:
    """Measure one package case with `package_cases.apply`, so the archive is the one a run writes."""

    CASE = "machine f() -> f32 { REFUSE }\n"
    CONTROL = "machine f() -> u32 { 1 }\n"

    def __init__(self, case=CASE, control=CONTROL, during=None, compiler=None):
        self.temp = TemporaryDirectory()
        root = Path(self.temp.name)
        for role, data in (("case", case), ("control", control)):
            (root / "fx" / role).mkdir(parents=True)
            (root / "fx" / role / "case.omg").write_bytes(data if isinstance(data, bytes) else data.encode())
        text = case.decode("utf-8", "replace") if isinstance(case, bytes) else case
        start, end, _, _ = position(text, "REFUSE")
        self.row = {"case": "package case", "spec": "wiki/spec/language/x.md#y", "contract": "CONTRACT.md#z", "fixture": "fx",
                    "file": "case.omg", "byte_start": str(start), "byte_end": str(end), "message": MESSAGE, "justification": "J."}
        compiler = compiler or Compiler()

        def execute(directory, action, file):
            result = compiler(EARLIER, directory, ["source", action, "--", file], "replace")
            if during and action == "lower" and Path(directory).name == "case":
                during(Path(directory) / file)
            return result

        self.report = {"last_layer": "L5", "measurement_errors": [], "cases": {}}
        original = package_cases.validate
        with patch.object(package_cases, "validate", lambda rows, fixtures: original(rows, fixtures, {"z"})):
            package_cases.apply(self.report, [self.row], execute, root)
        self.temp.cleanup()


class ArchiveReplayTests(unittest.TestCase):
    def test_a_verbatim_report_case_is_one_normal_package_program(self):
        report = json.loads((FIXTURES / "access_plan.json").read_text(encoding="utf-8"))
        inputs = replay.collect(report)
        [(program, control)] = inputs[ACCESS].pairs
        # The native entry is `prepared`, but the CLI reproduced the same refusal, so the files are an ordinary source.
        self.assertEqual((program.digest, control, inputs[ACCESS].holes), (ACCESS_DIGEST, None, []))
        self.assertEqual(program.argv["lower"], ["source", "lower", "--", "access_plan.omg"])
        self.assertEqual(program.original["lower"][2],
                         "access_plan.omg:4:38: lowering: unsupported domain member: Access (bytes 163..174)\n")
        # The same report without the CLI result is missing input, not a program.
        report["lowering_cli"]["results"][0]["status"] = "unverified"
        self.assertEqual(replay.collect(report)[ACCESS].holes, ["1 of 1 stop locations have no package program the CLI reproduced"])

    def test_build_owned_replay_requires_the_same_root_for_both_commands(self):
        report = native_report([["case.omg", "REFUSE"]], entry="package")
        row = report["lowering_cli"]["results"][0]
        for action in ("prepare", "lower"):
            row[action]["args"][3:3] = ["--package-root", "."]
        inputs = replay.collect(report)[KEY]
        self.assertEqual(inputs.holes, [])
        self.assertEqual(inputs.pairs[0][0].argv["lower"], ["source", "lower", "--package-root", ".", "--", "case.omg"])
        del row["lower"]["args"][3:5]
        damaged = replay.collect(report)[KEY]
        self.assertFalse(damaged.pairs)
        self.assertTrue(any("not a normal package command" in reason for reason in damaged.holes))

    def test_each_outcome_on_the_original_program_is_kept_apart(self):
        report = native_report([["a.omg", "machine f() { REFUSE }"]])
        for mode, expected in [("refuse", "identical-refusal"), ("lowers", "lowers"), ("typer", "earlier-phase"),
                               ("moved", "wrong-site"), ("renamed", "changed-diagnostic"), ("noisy", "unexpected")]:
            self.assertEqual(only(verdict(report, Compiler(later={"REFUSE": mode}))), expected, mode)
        # The earlier compiler must reproduce the archived diagnostic before anything is credited.
        self.assertEqual(only(verdict(report, Compiler(later={"REFUSE": "lowers"}, earlier={"REFUSE": "renamed"}))), "baseline-mismatch")
        # Without the earlier compiler a lowering program is reported, but no fix is confirmed.
        inputs, results, verdicts = run(report, Compiler(later={"REFUSE": "lowers"}), baseline=False)
        self.assertEqual(only(verdicts), "lowers")
        text = replay.markdown("r", report, ("0" * 64, None), inputs, results, verdicts)
        self.assertIn("Earlier compiler: not replayed, so no fix is confirmed.", text)
        self.assertIn("Confirmed fixes: 0.", text)

    def test_two_programs_in_one_case_must_agree_before_the_case_counts(self):
        report = native_report([["a.omg", "machine f() { REFUSE }"]], [["b.omg", "machine g() { REFUSE }\n// second"]])
        self.assertEqual(only(verdict(report, Compiler(later={"second": "lowers"}))), "mixed")
        inputs, results, verdicts = run(report, Compiler(later={"REFUSE": "lowers"}))
        self.assertEqual(only(verdicts), "lowers")
        self.assertIn("Confirmed fixes: 1.", replay.markdown("r", report, ("0" * 64, "1" * 64), inputs, results, verdicts))
        # A second stop site with no reproduced program leaves the case incomplete, whatever the first one does.
        partial = native_report([["a.omg", "machine f() { REFUSE }"]],
                                extra_sites=[{"package": "app", "file": "a.omg", "byte_start": 0, "byte_end": 7, "line": 1, "byte_column": 1}])
        self.assertEqual(only(verdict(partial, Compiler(later={"REFUSE": "lowers"}))), "incomplete-input")

    def test_a_changed_file_at_the_same_path_is_another_program(self):
        old, new = [["a.omg", "machine f() { REFUSE }"]], [["a.omg", "machine f() { REFUSE }\n// edited"]]
        report = native_report(old, new)
        inputs = replay.collect(report)
        self.assertEqual(len(inputs[KEY].pairs), 2)
        compiler = Compiler(later={"edited": "lowers"})
        self.assertEqual(only(verdict(report, compiler)), "mixed")
        # Each program ran from its own archived bytes, never from whatever is now at that path.
        compiled = {dict(files)["a.omg"] for binary, action, files in compiler.calls}
        self.assertEqual(compiled, {old[0][1].encode(), new[0][1].encode()})
        # A package source edited while its commands ran is not archived, so it cannot be replayed.
        edited = PackageMeasurement(during=lambda path: path.write_bytes(path.read_bytes() + b"// later\n")).report
        self.assertEqual(edited["package_cases"]["inputs"]["package case"]["case"]["unarchived"],
                         "the package source changed while its commands ran")
        self.assertEqual(only(verdict(edited, Compiler())), "missing-input")

    def test_a_package_case_needs_its_control_on_both_compilers(self):
        measured = PackageMeasurement().report
        archive = measured["package_cases"]["inputs"]["package case"]
        self.assertTrue(archive["reproduced"])
        self.assertEqual(archive["case"]["files"], [["case.omg", PackageMeasurement.CASE]])
        self.assertEqual(archive["control"]["argv"]["lower"], ["source", "lower", "--", "case.omg"])
        self.assertEqual(only(verdict(measured, Compiler(later={"REFUSE": "lowers"}))), "lowers")
        self.assertEqual(only(verdict(measured, Compiler())), "identical-refusal")
        # A control that now refuses makes the case's lowering uncreditable.
        self.assertEqual(only(verdict(measured, Compiler(later={"REFUSE": "lowers", "u32": "typer"}))), "control-changed")
        # An archive whose control did not lower silently at measurement is not a usable control.
        wrong = copy.deepcopy(measured)
        wrong["package_cases"]["inputs"]["package case"]["control"]["results"]["lower"] = [1, "", "x\n"]
        self.assertEqual(only(verdict(wrong, Compiler(later={"REFUSE": "lowers"}))), "missing-input")

    def test_missing_original_sources_are_holes_not_outcomes(self):
        measured = PackageMeasurement().report
        old_format = copy.deepcopy(measured)
        del old_format["package_cases"]["inputs"]
        [hole] = replay.collect(old_format)["handoff/package case"].holes
        self.assertEqual(hole, "the report predates package-input archiving")
        compiler = Compiler()
        self.assertEqual(only(verdict(old_format, compiler)), "missing-input")
        self.assertEqual(compiler.calls, [])
        # A native fixture that kept no replay files is a hole too.
        report = native_report([["a.omg", "machine f() { REFUSE }"]])
        del report["package_fixtures"][0]["replay"]["files"]
        self.assertEqual(only(verdict(report, Compiler())), "missing-input")
        # So is a verified row whose recorded command output is not the original diagnostic it claims.
        for field, value in (("stderr", "a.omg:1:1: lowering: other (bytes 0..1)\n"), ("matched", False), ("exit", 0)):
            report = native_report([["a.omg", "machine f() { REFUSE }"]])
            report["lowering_cli"]["results"][0]["lower"][field] = value
            self.assertEqual(only(verdict(report, Compiler())), "missing-input", field)

    def test_line_ends_and_non_ascii_bytes_reach_the_compiler_unchanged(self):
        text = "\ufeff// é\r\nmachine f() {\r\n REFUSE }\n"
        report = native_report([["a.omg", text]])
        compiler = Compiler()
        self.assertEqual(only(verdict(report, compiler)), "identical-refusal")
        self.assertEqual({dict(files)["a.omg"] for _, _, files in compiler.calls}, {text.encode("utf-8")})
        # Bytes that are not UTF-8 cannot be archived as text, so the case has no comparable input.
        latin = PackageMeasurement(case="machine f() { REFUSE } // \xe9".encode("latin-1"),
                                   compiler=lambda binary, directory, argv, decode: (1, "", "x\n") if argv[1] == "lower" and Path(directory).name == "case" else (0, "", "")).report
        self.assertEqual(latin["package_cases"]["inputs"]["package case"]["case"]["unarchived"], "the package source is not UTF-8")

    def test_entry_and_argv_define_the_compared_program(self):
        rows = [["a.omg", "machine f() { REFUSE }"]]
        report = native_report(rows, entry="prepared")
        # The same CLI program reproduced under two entries runs once per compiler.
        second = native_report(rows, entry="source")
        report["package_fixtures"] += second["package_fixtures"]
        report["lowering_cli"]["results"].append({**second["lowering_cli"]["results"][0], "fixture_index": 1})
        compiler = Compiler()
        self.assertEqual(only(verdict(report, compiler)), "identical-refusal")
        self.assertEqual(len(compiler.calls), 4)
        # A recorded command with another option or file order is not a normal package command over these files.
        for args in (["omega.exe", "source", "lower", "--target", "x", "--", "a.omg"], ["omega.exe", "source", "lower", "a.omg"]):
            changed = native_report(rows)
            changed["lowering_cli"]["results"][0]["lower"]["args"] = args
            self.assertEqual(only(verdict(changed, Compiler())), "missing-input", args)
        two = [["a.omg", "machine f() { REFUSE }"], ["b.omg", "machine g() {}"]]
        swapped = native_report(two)
        swapped["lowering_cli"]["results"][0]["lower"]["args"] = ["omega.exe", "source", "lower", "--", "b.omg", "a.omg"]
        self.assertEqual(only(verdict(swapped, Compiler())), "missing-input")

    def test_corrupted_archives_and_unsafe_paths_are_refused_before_anything_is_written(self):
        measured = PackageMeasurement().report
        corrupt = copy.deepcopy(measured)
        corrupt["package_cases"]["inputs"]["package case"]["case"]["files"][0][1] += " "
        self.assertEqual(replay.collect(corrupt)["handoff/package case"].holes,
                         ["archive corrupted: the stored files differ from the recorded digest"])
        native = native_report([["a.omg", "machine f() { REFUSE }"]])
        native["package_fixtures"][0]["replay"]["files"][0][1] += " "
        self.assertEqual(only(verdict(native, Compiler())), "missing-input")
        refusing = "machine f() { REFUSE }"
        for rows in ([["../escape.omg", refusing]], [["C:/escape.omg", refusing]], [["nul.omg", refusing]],
                     [["a\\b.omg", refusing]], [["a.omg", "machine f() {}"], ["A.omg", refusing]]):
            unsafe = native_report(rows)
            name = rows[-1][0]
            with TemporaryDirectory() as work:
                inputs, _, verdicts = replay.replay(unsafe, LATER, EARLIER, work, Compiler())
                self.assertEqual(list(Path(work).iterdir()), [])
            self.assertEqual(only(verdicts), "missing-input", name)
            self.assertTrue(any("package path" in hole for hole in inputs[KEY].holes), (name, inputs[KEY].holes))

    def test_an_abnormal_prepare_exit_cannot_establish_an_earlier_stage_refusal(self):
        expected = (1, "", diagnostic("a.omg", "machine f() { REFUSE }", "REFUSE", MESSAGE))
        original = {"prepare": replay.SILENT, "lower": expected}
        for prepare in ((101, "", expected[2]), (0, "unexpected output", ""), (1, "", "unknown error")):
            self.assertEqual(replay.classify(original, {"prepare": prepare, "lower": expected}), "unexpected")

    def test_outputs_are_stable_and_the_command_refuses_unreadable_input(self):
        report = native_report([["b.omg", "machine g() { REFUSE }"]], [["a.omg", "machine f() { REFUSE }"]])
        rendered = []
        for order in (report, {**report, "lowering_cli": {"results": report["lowering_cli"]["results"][::-1]}}):
            inputs, results, verdicts = run(order, Compiler(later={"g()": "lowers"}))
            rendered.append((replay.markdown("r", order, ("0" * 64, "1" * 64), inputs, results, verdicts),
                             replay.tsv(inputs, results, verdicts)))
        self.assertEqual(rendered[0], rendered[1])
        self.assertIn("| The original programs of one case now behave differently from each other | 1 | 0 | 1 |", rendered[0][0])
        self.assertEqual(len(rendered[0][1].splitlines()), 1 + 2 * 2)
        with TemporaryDirectory() as directory, redirect_stdout(StringIO()), patch("sys.stderr", StringIO()) as errors:
            self.assertEqual(replay.main([directory, "--binary", str(Path(directory) / "absent.exe")]), 2)
        self.assertIn("cannot read readiness evidence", errors.getvalue())


if __name__ == "__main__":
    unittest.main()
