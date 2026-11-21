"""Replay the original programs behind an earlier report's blocking cases on immutable compilers.

Run from the V2 Cargo workspace:

    python tools/spec-corpus/archive_replay.py REPORT --binary LATER [--baseline EARLIER]
        [--tsv] [--json OUT] [--work DIR]

A later readiness report often cannot say whether a case stopped blocking because the
compiler improved or because its test now compiles a different program: a successful
fixture records no source. This optional comparison answers that question for programs the
earlier report actually stored, by running them again. It builds nothing and runs no checker.

Comparable programs are only those a normal package command compiled during measurement:
- an L5 native refusal the package CLI reproduced (`lowering_cli.results` with status
  `verified`), whose files are in `package_fixtures[i].replay.files`;
- a package-command case and its control (`package_cases.inputs`).
A native fixture the CLI did not reproduce is never used, even if its text equals another
program or its entry is `prepared`: its test may select targets, change the prepared product
or supply evidence that the CLI does not. The current fixture tree and the Rust emitters are
never read; a report written before archiving shows its package cases as missing input.

Each program runs `source prepare` and `source lower` with the argv recorded at measurement,
in a fresh directory created through the package-namespace check of `lowering_cli.program`.
The earlier report fixes the original diagnostic; `--baseline` replays it on the earlier
compiler as well. A case counts as a confirmed fix only when every original program now
prepares and lowers silently, the baseline reproduced every original diagnostic exactly, and
every package control still prepares and lowers silently on both compilers.
The command exits 0 whenever it can read its inputs and is not a readiness gate.
"""
import argparse
from collections import Counter, namedtuple
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from tempfile import TemporaryDirectory

from check_layer_readiness import case_blocks_readiness
import compare_readiness as compare
from diagnostic_fields import readable
import lowering_cli
from sample_diagnostics import escape

# Ordered from least to most evidence of a repair. A case takes one verdict.
VERDICTS = {
    "missing-input": "Missing historical input: the earlier report stored no comparable package program",
    "incomplete-input": "Some original programs are missing; the replayed ones cannot stand for the case",
    "baseline-mismatch": "The earlier compiler does not reproduce the archived diagnostic",
    "control-changed": "A package control no longer prepares and lowers silently",
    "unexpected": "The command failed, timed out or printed an unparsed result",
    "mixed": "The original programs of one case now behave differently from each other",
    "earlier-phase": "The original program now stops before lowering",
    "wrong-site": "Lowering now refuses the original program at a different file or byte range",
    "changed-diagnostic": "Lowering refuses the original program at the same site with a different message",
    "identical-refusal": "The original diagnostic is unchanged",
    "lowers": "The original program now prepares and lowers silently; no product was checked",
}
NATIVE, PACKAGE = "native CLI", "package command"
ACTIONS = ("prepare", "lower")
FRONT_STAGES = {"lexer", "parser", "resolver", "typer", "preparation"}
SILENT = (0, "", "")
DIAGNOSTIC = re.compile(r"(?P<file>[^\n]+?):(?P<line>[1-9]\d*):(?P<column>[1-9]\d*): (?P<producer>[^:\n]+): "
                        r"(?P<message>[^\n]*) \(bytes (?P<start>0|[1-9]\d*)\.\.(?P<end>0|[1-9]\d*)\)\n?")

Program = namedtuple("Program", "digest rows files argv original decode role")
Inputs = namedtuple("Inputs", "key cohort pairs holes")


def normal_argv(action, names):
    return ["source", action, "--", *names]


def program_files(rows, recorded_digest=None):
    files = lowering_cli.program(rows)  # path safety, collisions and device names
    digest = lowering_cli.program_digest(rows)
    if recorded_digest is not None and recorded_digest != digest:
        raise ValueError("archive corrupted: the stored files differ from the recorded digest")
    return files, digest


def native_program(row, fixture):
    replay = fixture.get("replay")
    if not isinstance(replay, dict):
        raise ValueError("the fixture kept no replay source")
    files, digest = program_files(replay.get("files"), row.get("program_sha256"))
    names, original, argv = list(files), {}, {}
    for action, code in (("prepare", 0), ("lower", 1)):
        check = row.get(action)
        if not isinstance(check, dict) or not isinstance(check.get("args"), list):
            raise ValueError(f"no recorded {action} command")
        argv[action] = check["args"][1:]
        if argv[action] != normal_argv(action, names):
            raise ValueError(f"the {action} command was not a normal package command over these files")
        expected = check.get("expected_stderr")
        observed = (check.get("exit"), check.get("stdout"), check.get("stderr"))
        if (check.get("matched") is not True or not isinstance(expected, str) or check.get("expected_exit") != code
                or observed != (code, "", expected) or (action == "prepare" and expected)):
            raise ValueError(f"the recorded {action} command did not match its expected result")
        original[action] = observed
    return Program(digest, replay["files"], files, argv, original, "backslashreplace", "case")


def package_program(record, role, expected):
    if not isinstance(record, dict):
        raise ValueError(f"no archived {role}")
    if "unarchived" in record:
        raise ValueError(f"{role} not archived: {record['unarchived']}")
    files, digest = program_files(record.get("files"), record.get("sha256"))
    names = list(files)
    argv, results = record.get("argv"), record.get("results")
    if not isinstance(argv, dict) or not isinstance(results, dict) or set(argv) != set(ACTIONS) or set(results) != set(ACTIONS):
        raise ValueError(f"the archived {role} lacks its prepare and lower commands")
    if any(argv[action] != normal_argv(action, names) for action in ACTIONS):
        raise ValueError(f"the archived {role} commands were not normal package commands over these files")
    original = {action: tuple(results[action]) for action in ACTIONS}
    lower = original["lower"]
    if role == "control":
        consistent = original["prepare"] == SILENT and lower == SILENT
    else:
        consistent = (original["prepare"] == SILENT and lower[0] == 1 and lower[1] == ""
                      and isinstance(lower[2], str) and lower[2].strip() == expected)
    if not consistent:
        raise ValueError(f"the archived {role} results disagree with the registered measurement")
    return Program(digest, record["files"], files, {a: list(argv[a]) for a in ACTIONS}, original, "replace", role)


def native_inputs(key, case, report):
    stop = compare.first_stop(key, case, report)
    sites, found, pairs, holes = set(stop["locations"]), set(), {}, []
    fixtures = report.get("package_fixtures", ())
    for row in report.get("lowering_cli", {}).get("results", ()):
        index = row.get("fixture_index")
        if row.get("status") != "verified" or not isinstance(index, int) or not 0 <= index < len(fixtures):
            continue
        fixture = fixtures[index]
        location, replay = fixture.get("location"), fixture.get("replay") or {}
        emitter = f"{str(fixture.get('target', '')).replace(chr(92), '/')}::{fixture.get('test')}"
        if not location or emitter != stop["test"] or replay.get("message") != stop["reason"]:
            continue
        position = compare.site(location, location.get("package"))
        if position not in sites:
            continue
        try:
            program = native_program(row, fixture)
        except (KeyError, TypeError, ValueError) as error:
            holes.append(f"fixture {index}: {error}")
            continue
        pairs[(program.digest, json.dumps(program.argv))] = (program, None)
        found.add(position)
    if missing := sites - found:
        holes.append(f"{len(missing)} of {len(sites)} stop locations have no package program the CLI reproduced")
    return Inputs(key, NATIVE, [pairs[k] for k in sorted(pairs)], holes)


def package_inputs(key, report):
    archive = report.get("package_cases", {}).get("inputs")
    label = key.removeprefix("handoff/")
    if archive is None:
        return Inputs(key, PACKAGE, [], ["the report predates package-input archiving"])
    entry = archive.get(label)
    if not isinstance(entry, dict):
        return Inputs(key, PACKAGE, [], ["no archived input for this case"])
    if entry.get("reproduced") is not True:
        return Inputs(key, PACKAGE, [], ["the original commands did not reproduce the registered refusal"])
    try:
        case = package_program(entry.get("case"), "case", entry.get("expected"))
        control = package_program(entry.get("control"), "control", None)
    except (KeyError, TypeError, ValueError) as error:
        return Inputs(key, PACKAGE, [], [str(error)])
    return Inputs(key, PACKAGE, [(case, control)], [])


def collect(report):
    """The comparable original programs of every case that blocked in the report."""
    inputs = {}
    for key in sorted(report["cases"]):
        case = report["cases"][key]
        if not case_blocks_readiness(case):
            continue
        if case.get("measured_by") == "package command":
            inputs[key] = package_inputs(key, report)
        else:
            inputs[key] = native_inputs(key, case, report)
    return inputs


def subprocess_runner(binary, directory, argv, decode):
    try:
        done = subprocess.run([str(binary), *argv], cwd=directory, stdin=subprocess.DEVNULL, capture_output=True, timeout=120)
    except (OSError, subprocess.TimeoutExpired) as error:
        return ("error", "", str(error))
    return (done.returncode, done.stdout.decode("utf-8", decode), done.stderr.decode("utf-8", decode))


def run(binary, program, work, runner):
    with TemporaryDirectory(dir=work, prefix="archive-replay-") as temp:
        lowering_cli.materialize(temp, program.files)
        return {action: tuple(runner(binary, temp, program.argv[action], program.decode)) for action in ACTIONS}


def classify(original, observed):
    prepare, lower = observed["prepare"], observed["lower"]
    if any(type(result[0]) is not int for result in (prepare, lower)):
        return "unexpected"
    if prepare != SILENT:
        refusal = DIAGNOSTIC.fullmatch(prepare[2])
        if prepare[0] == 1 and not prepare[1] and refusal and refusal["producer"] in FRONT_STAGES:
            return "earlier-phase"
        return "unexpected"
    if lower == SILENT:
        return "lowers"
    if lower == original["lower"]:
        return "identical-refusal"
    now, then = DIAGNOSTIC.fullmatch(lower[2]), DIAGNOSTIC.fullmatch(original["lower"][2].strip() + "\n")
    if lower[0] != 1 or lower[1] or not now or not then:
        return "unexpected"
    if now["producer"] != "lowering":
        return "earlier-phase" if now["producer"] in FRONT_STAGES else "unexpected"
    same_site = (now["file"], now["start"], now["end"]) == (then["file"], then["start"], then["end"])
    return "changed-diagnostic" if same_site else "wrong-site"


def replay(report, binary, baseline=None, work=None, runner=subprocess_runner):
    """Return (inputs, results, verdicts). Each unique program runs once per compiler."""
    inputs = collect(report)
    results = {}
    for entry in inputs.values():
        for pair in entry.pairs:
            for program in filter(None, pair):
                identity = (program.digest, json.dumps(program.argv), program.decode)
                if identity not in results:
                    results[identity] = {"later": run(binary, program, work, runner),
                                         "baseline": run(baseline, program, work, runner) if baseline else None}
    verdicts = {}
    for key, entry in inputs.items():
        outcomes = []
        for case, control in entry.pairs:
            observed = results[(case.digest, json.dumps(case.argv), case.decode)]
            outcome = classify(case.original, observed["later"])
            if observed["baseline"] is not None and classify(case.original, observed["baseline"]) != "identical-refusal":
                outcome = "baseline-mismatch"
            elif control is not None:
                checks = results[(control.digest, json.dumps(control.argv), control.decode)]
                if any(r is not None and (r["prepare"], r["lower"]) != (SILENT, SILENT) for r in checks.values()):
                    outcome = "control-changed"
            outcomes.append(outcome)
        if not entry.pairs:
            verdict = "missing-input"
        elif entry.holes:
            verdict = "incomplete-input"
        elif len(set(outcomes)) == 1:
            verdict = outcomes[0]
        else:
            verdict = "mixed"
        verdicts[key] = {"verdict": verdict, "outcomes": outcomes}
    return inputs, results, verdicts


def confirmed_fix(verdict, baseline):
    return verdict == "lowers" and baseline is not None


def binary_identity(path):
    return None if path is None else hashlib.sha256(Path(path).read_bytes()).hexdigest()


def program_text(program):
    return f"{program.role} {', '.join(name for name, _ in program.rows)} sha256:{program.digest[:16]}"


def result_text(result):
    return "not replayed" if result is None else " · ".join(
        f"{action} exit {code}" + (f": {readable((err or out).strip())}" if (err or out) else "") for action, (code, out, err) in result.items())


def records(inputs, results, verdicts):
    """One record per case and per replayed program, in a stable order."""
    for key in sorted(inputs):
        entry, verdict = inputs[key], verdicts[key]
        yield {"case": key, "cohort": entry.cohort, "verdict": verdict["verdict"], "holes": list(entry.holes),
               "programs": [{"role": program.role, "program": program.digest, "files": [name for name, _ in program.rows],
                             "argv": program.argv, "original": program.original,
                             "outcome": outcome if program.role == "case" else None,
                             **{name: results[(program.digest, json.dumps(program.argv), program.decode)][name]
                                for name in ("baseline", "later")}}
                            for pair, outcome in zip(entry.pairs, verdict["outcomes"]) for program in filter(None, pair)]}


def summary_rows(inputs, verdicts):
    counts = Counter((inputs[key].cohort, verdict["verdict"]) for key, verdict in verdicts.items())
    return [(name, counts[(NATIVE, name)], counts[(PACKAGE, name)]) for name in VERDICTS
            if counts[(NATIVE, name)] or counts[(PACKAGE, name)]]


def markdown(report_name, report, binaries, inputs, results, verdicts):
    later, baseline = binaries
    with_programs = [k for k, e in inputs.items() if e.pairs]
    programs = {(p.digest, json.dumps(p.argv), p.decode) for e in inputs.values() for pair in e.pairs for p in pair if p}
    fixed = sum(confirmed_fix(v["verdict"], baseline) for v in verdicts.values())
    lines = ["# Original-program replay", "",
             f"Report {report_name} (revision {report.get('revision', '')[:12]}, fingerprint {report.get('fingerprint', '')[:12]}). "
             f"Later compiler sha256:{later[:16]}. Earlier compiler: "
             + (f"sha256:{baseline[:16]}." if baseline else "not replayed, so no fix is confirmed."), "",
             f"{len(inputs)} cases blocked in the report. {len(with_programs)} have archived original programs "
             f"({len(programs)} distinct programs and controls); {len(inputs) - len(with_programs)} have none. "
             f"Confirmed fixes: {fixed}.", "",
             f"| Verdict | {NATIVE} | {PACKAGE} | Total |", "| --- | ---: | ---: | ---: |",
             *(f"| {VERDICTS[name].split(':')[0]} | {n} | {p} | {n + p} |" for name, n, p in summary_rows(inputs, verdicts)),
             f"| Total | {sum(e.cohort == NATIVE for e in inputs.values())} | {sum(e.cohort == PACKAGE for e in inputs.values())} | {len(inputs)} |"]
    by_verdict = {}
    for record in records(inputs, results, verdicts):
        by_verdict.setdefault(record["verdict"], []).append(record)
    for name, title in VERDICTS.items():
        if name not in by_verdict:
            continue
        lines += ["", f"## {title} ({len(by_verdict[name])})", ""]
        for record in by_verdict[name]:
            lines.append(f"- {record['case']} · {record['cohort']}")
            lines += [f"  - missing: {hole}" for hole in record["holes"]]
            for row in record["programs"]:
                lines.append(f"  - {row['role']} {', '.join(row['files'])} sha256:{row['program'][:16]}"
                             + (f" · {row['outcome']}" if row["outcome"] else ""))
                if name not in {"identical-refusal"}:
                    lines.append(f"    - later: {result_text(row['later'])}")
                    if row["baseline"] is not None and name == "baseline-mismatch":
                        lines.append(f"    - earlier: {result_text(row['baseline'])}")
    return "\n".join(lines) + "\n"


def tsv(inputs, results, verdicts):
    lines = ["case\tcohort\tverdict\trole\tprogram\tcompiler\tprepare\tlower"]
    for record in records(inputs, results, verdicts):
        base = (record["case"], record["cohort"], record["verdict"])
        for hole in record["holes"]:
            lines.append("\t".join(escape(v) for v in (*base, "missing", "", "", hole, "")))
        for row in record["programs"]:
            for compiler in ("baseline", "later"):
                result = row[compiler]
                if result is None:
                    continue
                cells = [f"{code}|{escape(out)}|{escape(err)}" for code, out, err in (result[a] for a in ACTIONS)]
                lines.append("\t".join([*(escape(v) for v in (*base, row["role"], row["program"], compiler)), *cells]))
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("report", help="earlier report directory or evidence.json")
    parser.add_argument("--binary", required=True, help="the later package CLI (an existing omega executable)")
    parser.add_argument("--baseline", help="the earlier package CLI, to confirm each archived diagnostic")
    parser.add_argument("--tsv", action="store_true", help="print one row per program, compiler and archive hole")
    parser.add_argument("--json", help="also write every record to this new file")
    parser.add_argument("--work", help="directory for the temporary package directories")
    options = parser.parse_args(argv)
    try:
        report = compare.load(options.report)
        binaries = (binary_identity(options.binary), binary_identity(options.baseline))
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 2
    inputs, results, verdicts = replay(report, options.binary, options.baseline, options.work)
    if options.json:
        data = {"report": compare.report_name(options.report), "later_sha256": binaries[0], "baseline_sha256": binaries[1],
                "cases": list(records(inputs, results, verdicts))}
        with Path(options.json).open("x", encoding="utf-8", newline="\n") as out:
            json.dump(data, out, indent=1, ensure_ascii=False)
            out.write("\n")
    if options.tsv:
        sys.stdout.write(tsv(inputs, results, verdicts))
    else:
        sys.stdout.write(markdown(compare.report_name(options.report), report, binaries, inputs, results, verdicts))
    return 0


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.exit(main())
