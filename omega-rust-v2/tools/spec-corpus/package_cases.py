"""Keep confirmed L5 limits in the case ledger until a native test measures them.

Some valid programs prepare and then stop in lowering, but no L5 test lowers them through the
shared helper, so `refusal_evidence` cannot report them. Each row of `l5_package_cases.tsv`
names one such behavior by its case label. The row gives a normal input and a control that
differs in one input, both stored under the layer's
`tests/fixtures/package_cases/<fixture>/{case,control}/`. It also gives the exact first
diagnostic. After the package CLI is built, `apply` runs `source prepare` and `source lower` on
both directories:

- The control must prepare and lower silently. The case must prepare silently and stop at
  `<file>:<line>:<col>: lowering: <message> (bytes <start>..<end>)`. Line and column are derived
  from the fixture bytes, so the registry stores no line numbers.
- A reproduced row becomes the open gap `handoff/<case>`, marked `measured_by: package command`.
  It blocks readiness like any open gap. The report lists it apart from natively measured cases,
  because no Psi product or independent consumer has been checked.
- A row whose case is already in the ledger (a native refusal, closure or gap report), whose case
  now lowers, prepares differently or changes its refusal, or whose control stops, is a
  measurement error. The row must then be reconciled, never ignored. When the case lowers, add
  a native product test and closure, then delete the row.
- Whenever the commands run, `package_cases.inputs[<case>]` keeps the exact case and control
  source, argv and results. Deleting a row deletes its fixture, so this record is what lets
  `archive_replay.py` replay the original program against a later compiler.
"""
import csv
from pathlib import Path
import re
import subprocess

import lowering_cli

REGISTRY = Path(__file__).with_name("l5_package_cases.tsv")
LAYER = Path(__file__).absolute().parents[2] / "layers/05_prepared-trees-to-terminal-psi"
FIXTURES = LAYER / "tests/fixtures/package_cases"
FIELDS = ["case", "spec", "contract", "fixture", "file", "byte_start", "byte_end", "message", "justification"]
SPEC = re.compile(r"wiki/spec/[a-z_\-/]+\.md#[a-z0-9\-]+")
SLUG = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*")


def heading_anchor(line):
    text = re.sub(r"`", "", line.lstrip("#").strip()).lower()
    return re.sub(r"[^\w\- ]", "", text).replace(" ", "-")


def contract_anchors(layer=LAYER):
    return {heading_anchor(line) for line in (layer / "CONTRACT.md").read_text(encoding="utf-8").splitlines()
            if line.startswith("## ")}


def validate(rows, fixtures=FIXTURES, anchors=None):
    anchors = contract_anchors() if anchors is None else anchors
    labels = set()
    for row in rows:
        if any(not row[field] or row[field] != row[field].strip() for field in FIELDS):
            raise ValueError(f"incomplete package case: {row}")
        if row["case"] in labels:
            raise ValueError(f"duplicate package case: {row['case']}")
        labels.add(row["case"])
        if not SPEC.fullmatch(row["spec"]):
            raise ValueError(f"{row['case']}: spec must name a wiki/spec page and section anchor")
        page, _, anchor = row["contract"].partition("#")
        if page != "CONTRACT.md" or anchor not in anchors:
            raise ValueError(f"{row['case']}: contract must name a section of the L5 contract: {row['contract']}")
        if not SLUG.fullmatch(row["fixture"]):
            raise ValueError(f"{row['case']}: invalid fixture directory {row['fixture']!r}")
        for role in ("case", "control"):
            if not (fixtures / row["fixture"] / role / row["file"]).is_file():
                raise ValueError(f"{row['case']}: missing {role} fixture {row['fixture']}/{role}/{row['file']}")
        if not (row["byte_start"].isdecimal() and row["byte_end"].isdecimal()):
            raise ValueError(f"{row['case']}: byte range must be decimal")
        source = (fixtures / row["fixture"] / "case" / row["file"]).read_bytes()
        start, end = int(row["byte_start"]), int(row["byte_end"])
        if not 0 <= start <= end <= len(source):
            raise ValueError(f"{row['case']}: byte range outside the case source")
        source[:start].decode("utf-8")
        source[start:end].decode("utf-8")
    return rows


def registrations(path=REGISTRY):
    with Path(path).open(encoding="utf-8", newline="") as file:
        reader = csv.DictReader(file, delimiter="\t", quoting=csv.QUOTE_NONE)
        if reader.fieldnames != FIELDS:
            raise ValueError("invalid L5 package case header")
        return list(reader)


def command(binary, directory, action, file):
    done = subprocess.run([str(binary), "source", action, "--", file], cwd=directory, stdin=subprocess.DEVNULL,
                          capture_output=True, timeout=120)
    return done.returncode, done.stdout.decode("utf-8", "replace"), done.stderr.decode("utf-8", "replace")


def expected_diagnostic(source, row):
    start, end = int(row["byte_start"]), int(row["byte_end"])
    prefix = source[:start]
    line, column = 1 + prefix.count(b"\n"), start - prefix.rfind(b"\n")
    return f"{row['file']}:{line}:{column}: lowering: {row['message']} (bytes {start}..{end})", line, column


def snapshot(directory, file):
    """The package-only source a command is about to compile, as [file, text] rows, or None if not UTF-8."""
    data = (Path(directory) / file).read_bytes()
    try:
        return [[file, data.decode("utf-8")]]
    except UnicodeDecodeError:
        return None


def archived(before, directory, file, results):
    """Record what a package command compiled and returned, for a later replay of the same program.

    Keep the bytes only if they are unchanged after the commands ran, so the record
    names exactly what those commands compiled. The argv is the whole configuration:
    the package CLI reads nothing else."""
    entry = {"argv": {action: ["source", action, "--", file] for action in results},
             "results": {action: list(result) for action, result in results.items()}}
    if before is None:
        entry["unarchived"] = "the package source is not UTF-8"
    elif snapshot(directory, file) != before:
        entry["unarchived"] = "the package source changed while its commands ran"
    else:
        entry.update(files=before, sha256=lowering_cli.program_digest(before))
    return entry


def apply(report, rows=None, execute=None, fixtures=FIXTURES):
    """Hook: call after the package CLI is built and after the native case ledger is complete."""
    if report.get("last_layer") != "L5":
        return
    errors = report["measurement_errors"]
    rows = validate(registrations() if rows is None else list(rows), fixtures)
    ledger = report["package_cases"] = {"registered": len(rows), "reproduced": [], "unreconciled": {}, "inputs": {}}
    if execute is None:
        binary = report.get("lowering_cli", {}).get("build", {}).get("executable")
        if not binary:
            for row in rows:
                ledger["unreconciled"][row["case"]] = "unmeasured: no package CLI was built"
            errors.append(f"{len(rows)} confirmed L5 limits unmeasured: no package CLI was built")
            return
        execute = lambda directory, action, file: command(binary, directory, action, file)  # noqa: E731
    for row in rows:
        key, label = "handoff/" + row["case"], row["case"]
        case_dir, control_dir = fixtures / row["fixture"] / "case", fixtures / row["fixture"] / "control"

        def problem(text):
            ledger["unreconciled"][label] = text
            errors.append(f"package case {key}: {text}")

        if key in report["cases"]:
            problem(f"the native ledger already reports it ({report['cases'][key]['status']}); delete its l5_package_cases.tsv row")
            continue
        archive = ledger["inputs"][label] = {"fixture": row["fixture"], "reproduced": False}
        control_source = snapshot(control_dir, row["file"])
        control = [execute(control_dir, action, row["file"]) for action in ("prepare", "lower")]
        archive["control"] = archived(control_source, control_dir, row["file"], dict(zip(("prepare", "lower"), control)))
        if any(result != (0, "", "") for result in control):
            problem("its control no longer prepares and lowers silently: "
                    + next((err or out).strip() or f"exit {code}" for code, out, err in control if (code, out, err) != (0, "", "")))
            continue
        case_source = snapshot(case_dir, row["file"])
        prepare = execute(case_dir, "prepare", row["file"])
        if prepare != (0, "", ""):
            archive["case"] = archived(case_source, case_dir, row["file"], {"prepare": prepare})
            problem(f"its case no longer prepares silently: {(prepare[2] or prepare[1]).strip() or f'exit {prepare[0]}'}")
            continue
        code, out, err = execute(case_dir, "lower", row["file"])
        archive["case"] = archived(case_source, case_dir, row["file"], {"prepare": prepare, "lower": (code, out, err)})
        source = (case_dir / row["file"]).read_bytes()
        expected, line, column = expected_diagnostic(source, row)
        archive["expected"] = expected
        if code == 0:
            problem("its case now lowers; add a native product test and closure for it, then delete its row")
            continue
        if code != 1 or out or err.strip() != expected:
            problem(f"its refusal changed: expected {expected!r}, observed {(err or out).strip()!r} (exit {code})")
            continue
        start, end = int(row["byte_start"]), int(row["byte_end"])
        archive["reproduced"] = True
        ledger["reproduced"].append(label)
        report["cases"][key] = {
            "cohort": "handoff", "stage": "L5", "status": "open gap", "measured_by": "package command",
            "detail": (f"Missing lowering handoff, measured only by the package command; no native product or consumer "
                       f"check exists. {row['justification']} Observed {row['message']} at {row['file']}."),
            "registered_refusal": "limitation", "refusal_reason": row["message"],
            "spec": row["spec"], "contract": row["contract"], "fixture": row["fixture"],
            "fixture_locations": [{"package": "root", "file": row["file"], "byte_start": start, "byte_end": end,
                                   "line": line, "byte_column": column,
                                   "occurrence": source[start:end].decode("utf-8")}],
        }


def summary(report):
    ledger = report.get("package_cases")
    if not ledger:
        return []
    lines = ["", "## Confirmed limits measured only by the package command", "",
             f"{ledger['registered']} registered normal inputs prepare and stop in lowering while their one-input "
             f"control lowers. {len(ledger['reproduced'])} reproduced and block readiness as open gaps. Each still "
             "needs a native test that checks its Psi product and an independent consumer. "
             f"{sum('files' in entry.get('case', {}) for entry in ledger.get('inputs', {}).values())} case sources "
             "were archived for a later replay. "
             "[Registry](../../../tools/spec-corpus/l5_package_cases.tsv)", ""]
    for label in ledger["reproduced"]:
        case = report["cases"]["handoff/" + label]
        lines.append(f"- handoff/{label} · {case['refusal_reason']} · {case['spec']} · {case['contract']}")
    for label, text in sorted(ledger["unreconciled"].items()):
        lines.append(f"- unreconciled · handoff/{label} · {text}")
    return lines
