"""Compare two layer-readiness reports case by case.

Run from the V2 Cargo workspace:

    python tools/spec-corpus/compare_readiness.py BEFORE AFTER [--verbose] [--tsv]

BEFORE and AFTER are report directories under target/layer-readiness/ or their
evidence.json files. Cases are matched by their stable report key, such as
`handoff/<label>` or `spec/<chapter>.md:<line>`, and `case_blocks_readiness`
decides whether each side blocks. A case that disappears is listed as removed and
is never counted as unblocked. Blocked cases in both reports are compared by their
first stop: status, stage, typing phase, reason and source locations. Explanatory prose, directory timestamps and durations are not compared.
The comparison is diagnostic only and exits 0 whenever both reports can be read.
"""
import argparse
import json
from pathlib import Path
import re
import sys

from check_layer_readiness import case_blocks_readiness
from diagnostic_fields import readable
from sample_diagnostics import escape

# Ordered for display. The keys are the first column of the tab-separated output.
GROUPS = {
    "regression": "Previously passing cases that now block",
    "new-blocker": "Newly measured blockers, absent from the earlier report",
    "fixtures-added": "Additional fixtures reached the same refusal; original inputs and stops were preserved",
    "witness-changed": "Same compiler stop, different emitting test",
    "stop-changed": "Blocked in both reports; the first stop changed",
    "removed-blocker": "Blocked before and absent now. The case was not measured, so this is not a repair",
    "unblocked": "Blocked before, measured now, and no longer blocking",
    "removed": "Passing before and absent now",
    "new": "Newly measured cases that do not block",
    "outcome-changed": "Passing in both reports with a different status or stage",
    "text-only": "Blocked in both reports; only explanatory text changed",
}
TSV_HEADER = ("group", "case", "field", "before", "after")
STOP_FIELDS = ("status", "stage", "after_static_evaluation", "reason", "locations", "test", "verdict")
# `compare_fragment_runs` renders a front-end refusal as `file:line:col: reason (bytes a..b)`
# for the example file and `package/file: reason (bytes a..b)` elsewhere.
SPEC_REFUSAL = re.compile(r"(?P<where>.+?): (?P<reason>.*) \(bytes (?P<start>0|[1-9]\d*)\.\.(?P<end>0|[1-9]\d*)\)")
SPEC_POSITION = re.compile(r"(?P<file>.+):(?P<line>[1-9]\d*):(?P<column>[1-9]\d*)")
# `refusal_evidence.apply` ends a registered handoff with `Observed <message> at <file> in <target>::<test>.`
REGISTERED_TEST = re.compile(r" in (?P<target>\S+\.rs)::(?P<test>\S+)\.$")


def load(path):
    path = Path(path)
    file = path / "evidence.json" if path.is_dir() else path
    try:
        report = json.loads(file.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise ValueError(f"cannot read readiness evidence {file}: {error}") from None
    if not isinstance(report.get("cases"), dict):
        raise ValueError(f"{file} has no case results")
    return report


def report_name(path):
    path = Path(path)
    return path.parent.name if path.name == "evidence.json" else path.name


def site(record, package):
    """One source position as a comparable tuple; occurrence text is the source span itself."""
    return (package or "", record.get("file", ""), record.get("byte_start"), record.get("byte_end"),
            record.get("line"), record.get("byte_column"), record.get("occurrence", record.get("occurrence_hex", "")))


def render_site(position):
    package, file, start, end, line, column, occurrence = position
    where = f"{package}/{file}" if package else file
    if line is not None:
        where += f":{line}:{column}"
    text = f"{where} bytes {start}..{end}"
    return text + (" " + json.dumps(occurrence, ensure_ascii=False) if occurrence else "")


def observed_message(detail, files):
    """The registered refusal message after the last `Observed `, delimited by its fixture file."""
    at = detail.rfind("Observed ")
    if at < 0:
        return None
    rest = detail[at + len("Observed "):]
    ends = [match.start() for file in files
            if (match := re.search(rf" at {re.escape(file)}(?=: | in |\.?$)", rest))]
    return rest[:min(ends)] if ends else None


def first_stop(key, case, report):
    """The comparable facts about where and why a case stopped, separated from its explanatory text."""
    cohort, detail = case["cohort"], case["detail"]
    reason, sites = case.get("refusal_reason"), []
    if refusal := case.get("compiler_refusal"):
        reason = refusal.get("reason")
        sites.append(site(refusal, refusal.get("package")))
    elif cohort == "spec" and (match := SPEC_REFUSAL.fullmatch(detail)):
        reason = match["reason"]
        bounds = {"byte_start": int(match["start"]), "byte_end": int(match["end"])}
        if position := SPEC_POSITION.fullmatch(match["where"]):
            sites.append(site({"file": position["file"], "line": int(position["line"]),
                               "byte_column": int(position["column"]), **bounds}, "example"))
        else:
            package, _, file = match["where"].partition("/")
            sites.append(site({"file": file, **bounds}, package))
    fixtures = case.get("fixture_locations", ())
    sites.extend(site(location, location.get("package")) for location in fixtures)
    if cohort == "handoff" and reason is None:
        reason = observed_message(detail, sorted({location["file"] for location in fixtures}))
    if cohort == "spec":
        for kind, location in sorted(report.get("lowering_locations", {}).get(key.removeprefix("spec/"), {}).items()):
            sites.append(site(location, "spec " + kind))
    test = None
    if case.get("test"):
        test = f"{case['target'].replace(chr(92), '/')}::{case['test']}"
    elif match := REGISTERED_TEST.search(detail):
        test = f"{match['target']}::{match['test']}"
    return {"status": case["status"], "stage": case["stage"],
            "after_static_evaluation": bool(case.get("after_static_evaluation", False)),
            # Without a structured or delimited reason the whole detail stands in, so a change still shows.
            "reason": detail if reason is None else reason,
            # Distinct sites: one stop reported by two fixtures of the same test is still one stop.
            "locations": tuple(sorted(set(sites), key=lambda s: tuple("" if v is None else str(v) for v in s))),
            "test": test, "verdict": (case.get("expectation_check") or {}).get("verdict")}


def value_text(field, value):
    if value is None:
        return ""
    if field == "after_static_evaluation":
        return "true" if value else "false"
    if field == "locations":
        return "; ".join(map(render_site, value))
    return str(value)


def replayed_refusals(stop, report):
    """Pair each exact source package with its measured refusal locations."""
    observations = set()
    observed_sites = set()
    locations = set(stop["locations"])
    for fixture in report.get("package_fixtures", ()):
        emitter = f"{fixture['target'].replace(chr(92), '/')}::{fixture['test']}"
        replay = fixture.get("replay", {})
        location = fixture.get("location")
        if emitter != stop["test"] or replay.get("message") != stop["reason"] or not location or not replay.get("files"):
            continue
        position = site(location, location.get("package"))
        if position in locations:
            source = (replay.get("entry"), tuple((name, text) for name, text in replay["files"]))
            observations.add((source, position))
            observed_sites.add(position)
    return observations if observed_sites == locations else set()


def only_added_controls(earlier, later, before, after):
    old = replayed_refusals(earlier, before)
    new = replayed_refusals(later, after)
    if not old or not old < new:
        return False
    original_inputs = {source for source, _ in old}
    return all(source not in original_inputs for source, _ in new - old)


def compare(before, after):
    """Return {group: [entry]} where each entry has the case key and its changed fields."""
    groups = {name: [] for name in GROUPS}
    earlier, later = before["cases"], after["cases"]
    unchanged = []
    for key in sorted(earlier.keys() | later.keys()):
        old, new = earlier.get(key), later.get(key)
        if new is None:
            stop = first_stop(key, old, before)
            groups["removed-blocker" if case_blocks_readiness(old) else "removed"].append(
                {"case": key, "before": stop, "changes": [("status", stop["status"], None)]})
            continue
        if old is None:
            stop = first_stop(key, new, after)
            groups["new-blocker" if case_blocks_readiness(new) else "new"].append(
                {"case": key, "after": stop, "changes": [("status", None, stop["status"])]})
            continue
        was, now = case_blocks_readiness(old), case_blocks_readiness(new)
        a, b = first_stop(key, old, before), first_stop(key, new, after)
        changes = [(field, a[field], b[field]) for field in STOP_FIELDS if a[field] != b[field]]
        entry = {"case": key, "before": a, "after": b, "changes": changes}
        if was != now:
            groups["regression" if now else "unblocked"].append(entry)
        elif was and changes:
            group = "witness-changed" if all(field == "test" for field, _, _ in changes) else "stop-changed"
            if [field for field, _, _ in changes] == ["locations"] and only_added_controls(a, b, before, after):
                group = "fixtures-added"
            groups[group].append(entry)
        elif was and old["detail"] != new["detail"]:
            entry["changes"] = [("detail", old["detail"], new["detail"])]
            groups["text-only"].append(entry)
        elif not was and any(field in {"status", "stage"} for field, _, _ in changes):
            entry["changes"] = [change for change in changes if change[0] in {"status", "stage"}]
            groups["outcome-changed"].append(entry)
        elif was:
            unchanged.append(key)
    return groups, unchanged


def side(report):
    cases = report["cases"].values()
    return {"revision": report.get("revision", ""), "fingerprint": report.get("fingerprint", ""),
            "last_layer": report.get("last_layer", ""), "cases": len(report["cases"]),
            "blocking": sum(map(case_blocks_readiness, cases)),
            "measurement_errors": len(report.get("measurement_errors", ())),
            "failed_suites": sorted(stage for stage, suite in report.get("suites", {}).items() if suite.get("exit"))}


def warnings(before, after):
    a, b = side(before), side(after)
    lines = []
    if a["last_layer"] != b["last_layer"]:
        lines.append(f"The reports measured different layer ranges (through {a['last_layer']} and {b['last_layer']}). "
                     "Cases outside the shorter range appear as removed or new.")
    for name, values in (("earlier", a), ("later", b)):
        if values["measurement_errors"]:
            lines.append(f"The {name} report has {values['measurement_errors']} incomplete measurements. "
                         "A removed or new case can reflect a measurement fault rather than a compiler change.")
        if values["failed_suites"]:
            lines.append(f"The {name} report has suites with a nonzero exit: {', '.join(values['failed_suites'])}.")
    for cohort in sorted(set(before.get("expected", {})) | set(after.get("expected", {}))):
        x, y = before.get("expected", {}).get(cohort), after.get("expected", {}).get(cohort)
        if x != y:
            lines.append(f"The expected {cohort} count changed from {x if x is not None else 'none'} to {y if y is not None else 'none'}.")
    return lines


def tsv_rows(before, after, groups):
    a, b = side(before), side(after)
    for field in ("revision", "fingerprint", "last_layer", "cases", "blocking", "measurement_errors"):
        yield ("report", "", field, str(a[field]), str(b[field]))
    for group, entries in groups.items():
        for entry in entries:
            for field, old, new in entry["changes"]:
                yield (group, entry["case"], field, value_text(field, old), value_text(field, new))


def tsv(before, after, groups):
    lines = ["\t".join(TSV_HEADER)]
    for row in tsv_rows(before, after, groups):
        fields = [escape(value) for value in row]
        # The optional final empty column is omitted, as for a removed case's absent outcome.
        lines.append("\t".join(fields[:-1] if fields[-1] == "" else fields))
    return "\n".join(lines) + "\n"


def headline(stop):
    text = f"{stop['stage']} · {stop['status']}"
    return text + (f" · {readable(stop['reason'])}" if stop["reason"] and stop["reason"] != stop["status"] else "")


def markdown(before, after, groups, unchanged, verbose=False, names=("earlier", "later")):
    a, b = side(before), side(after)
    lines = ["# Readiness comparison", "",
             f"Blocking cases: {a['blocking']} → {b['blocking']}. "
             f"Newly blocking: {len(groups['regression'])} regressions and {len(groups['new-blocker'])} newly measured. "
             f"No longer blocking while still measured: {len(groups['unblocked'])}. "
             f"Blockers no longer measured: {len(groups['removed-blocker'])}. "
             f"Blocked in both: {len(groups['stop-changed'])} with a changed first stop, "
             f"{len(groups['fixtures-added'])} with additional refusal fixtures, "
             f"{len(groups['witness-changed'])} with a changed emitting test, "
             f"{len(groups['text-only'])} with changed text only, {len(unchanged)} unchanged.", "",
             "| Report | Revision | Fingerprint | Layers | Cases | Blocking | Incomplete measurements |",
             "| --- | --- | --- | --- | ---: | ---: | ---: |"]
    for name, values in zip(names, (a, b)):
        lines.append(f"| {name} | {values['revision'][:12]} | {values['fingerprint'][:12]} | through {values['last_layer']} | "
                     f"{values['cases']} | {values['blocking']} | {values['measurement_errors']} |")
    if notes := warnings(before, after):
        lines += ["", "## Measurement differences", "", *("- " + note for note in notes)]
    for group, title in GROUPS.items():
        entries = groups[group]
        if not entries:
            continue
        lines += ["", f"## {title} ({len(entries)})", ""]
        if group == "text-only" and not verbose:
            lines.append("Run with --verbose to list them.")
            continue
        for entry in entries:
            if group in {"removed", "removed-blocker"}:
                lines.append(f"- {entry['case']} · was {headline(entry['before'])}")
            elif group in {"new", "new-blocker"}:
                lines.append(f"- {entry['case']} · {headline(entry['after'])}")
            elif group in {"regression", "unblocked"}:
                lines.append(f"- {entry['case']} · was {headline(entry['before'])} · now {headline(entry['after'])}")
            else:
                lines.append(f"- {entry['case']}")
                for field, old, new in entry["changes"]:
                    lines.append(f"  - {field}: {readable(value_text(field, old)) or 'none'} → {readable(value_text(field, new)) or 'none'}")
            # Locate the blocking side: the earlier stop for a removed or unblocked case.
            stop = entry["before"] if group in {"removed-blocker", "unblocked"} else entry.get("after")
            if group in {"removed-blocker", "new-blocker", "regression", "unblocked"} and stop["locations"]:
                lines.append("  - at " + value_text("locations", stop["locations"]))
    if verbose and unchanged:
        lines += ["", f"## Blocked in both reports without change ({len(unchanged)})", "", *("- " + key for key in unchanged)]
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("before", help="earlier report directory or evidence.json")
    parser.add_argument("after", help="later report directory or evidence.json")
    parser.add_argument("--verbose", action="store_true", help="list text-only changes and unchanged blockers")
    parser.add_argument("--tsv", action="store_true", help="print one tab-separated row per changed field")
    options = parser.parse_args(argv)
    try:
        before, after = load(options.before), load(options.after)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    groups, unchanged = compare(before, after)
    if options.tsv:
        sys.stdout.write(tsv(before, after, groups))
    else:
        sys.stdout.write(markdown(before, after, groups, unchanged, options.verbose,
                                  tuple(report_name(path) for path in (options.before, options.after))))
    return 0


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.exit(main())
