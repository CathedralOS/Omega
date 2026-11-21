"""Account for every L5 refusal pinned by a test fixture.

A green test can pin the diagnostic of a valid program. The shared L5 helper
prints each fixture outcome, and this module reconciles every refusal against
l5_refusals.tsv. A registration names the exact test, site file, message and
number of occurrences, and classifies the refusal:

- invalid: deliberately invalid input whose refusal the L5 contract assigns to
  lowering. Only these are credited, and only from a passing test. A registry
  label preserves the case identity when a former gap is an intended rejection.
- limitation: a valid input L5 does not lower yet. It is an open gap.
- checker: L5 stopped on a proof that the lowered-form checker must judge.
- unresolved: validity or ownership is not settled by the L5 contract, code or
  tests. It blocks like an open gap until someone decides it.

An unregistered refusal, a changed occurrence count, a registered refusal that
a passing test no longer produced, and a missing or unregistered gap report are
measurement errors. Matching is exact; no message wording is trusted.
"""
from collections import Counter
import csv
from functools import cache
from pathlib import Path
import re
from typing import NamedTuple

REGISTRY = Path(__file__).with_name("l5_refusals.tsv")
OPEN_CLASSES = {
    "limitation": ("open gap", "Missing lowering handoff"),
    "checker": ("checker input blocked", "L5 stopped before the checker could judge the obligation"),
    "unresolved": ("unresolved refusal", "Unresolved refusal (validity or owner undecided)"),
}
CLASSES = {"invalid", *OPEN_CLASSES}
# Wording in which lowering admits that it lacks a form. Such a refusal is
# never accepted as an intended rejection of invalid input.
LIMITATION_WORDING = re.compile(
    r"unsupported|requires .*lowering|no Psi lowering|cannot lower|cannot carry|no specified|needs .* lowering", re.IGNORECASE)
REFUSED = re.compile(r"^refused: (?P<message>.+) at (?P<file>[^ ]+)$")
CONTRACT_DOCUMENTS = ("CONTRACT.md", "LOCK_EVIDENCE.md")


class Refusal(NamedTuple):
    target: str
    test: str
    file: str
    message: str
    occurrences: str
    cls: str
    gap: str
    reference: str
    justification: str

    @property
    def key(self):
        return self.target, self.test, self.file, self.message

    @property
    def gap_source(self):
        return self.gap.partition(":")[0] if self.gap != "-" else None

    @property
    def gap_labels(self):
        return self.gap.partition(":")[2].split("|") if self.gap != "-" else []


def validate(rows):
    """Reject a registry that could credit a refusal without evidence."""
    keys, labels = set(), set()
    for row in rows:
        if not all(row) or not row.target.startswith("tests/") or not row.target.endswith(".rs"):
            raise ValueError(f"incomplete L5 refusal registration: {row}")
        if row.key in keys:
            raise ValueError(f"duplicate L5 refusal registration: {row.key}")
        keys.add(row.key)
        if row.cls not in CLASSES or not row.occurrences.isdecimal() or int(row.occurrences) < 1:
            raise ValueError(f"invalid class or occurrence count: {row}")
        if row.cls == "invalid":
            if row.gap != "-" and (row.gap_source != "registry" or not all(row.gap_labels)):
                raise ValueError(f"an intended refusal needs a registry case label or a dash: {row}")
            if LIMITATION_WORDING.search(row.message):
                raise ValueError(f"message admits a missing lowering and cannot be an intended refusal: {row.message}")
            if not any(document in row.reference for document in CONTRACT_DOCUMENTS):
                raise ValueError(f"an intended refusal must cite the public L5 contract: {row}")
        elif row.gap_source not in {"test", "registry"} or not all(row.gap_labels):
            raise ValueError(f"an open refusal needs a test or registry gap label: {row}")
        for label in row.gap_labels:
            if label in labels:
                raise ValueError(f"duplicate gap label: {label}")
            labels.add(label)
    return rows


@cache
def registrations():
    with REGISTRY.open(encoding="utf-8", newline="") as source:
        reader = csv.reader(source, delimiter="\t", quoting=csv.QUOTE_NONE)
        header = next(reader, None)
        if header != [*Refusal._fields[:5], "class", *Refusal._fields[6:]]:
            raise ValueError("invalid L5 refusal registry header")
        rows = []
        for fields in reader:
            if len(fields) != len(Refusal._fields):
                raise ValueError(f"malformed L5 refusal registration: {fields}")
            rows.append(Refusal(*fields))
    if not rows:
        raise ValueError("empty L5 refusal registry")
    return tuple(validate(rows))


def observed_refusals(report):
    """Count exact refusals per (target, test, file, message) from the L5 helper."""
    counts, errors = Counter(), []
    for fixture in report.get("package_fixtures", ()):
        outcome = fixture["outcome"]
        if outcome == "produced Psi":
            continue
        match = REFUSED.match(outcome)
        if not match:
            errors.append(f"L5 fixture outcome is neither Psi nor a located refusal: {fixture['test']}: {outcome}")
            continue
        counts[(fixture["target"].replace("\\", "/"), fixture["test"], match["file"], match["message"])] += 1
    return counts, errors


def apply(report, rows=None):
    """Hook: call after the L5 suite is collected and before the verdict."""
    if report.get("last_layer") != "L5":
        return
    rows = registrations() if rows is None else validate(list(rows))
    errors = report["measurement_errors"]
    statuses = {(test["target"].replace("\\", "/"), test["test"]): test["status"]
                for test in report["suites"].get("L5", {}).get("tests", ())}
    counts, malformed = observed_refusals(report)
    locations = {}
    for fixture in report.get("package_fixtures", ()):
        if (site := fixture.get("location")) and (refusal := REFUSED.match(fixture["outcome"])):
            key = (fixture["target"].replace("\\", "/"), fixture["test"], refusal["file"], refusal["message"])
            locations.setdefault(key, []).append(site)
    errors.extend(malformed)
    registered = {row.key: row for row in rows}
    accounting = {"registered": len(rows), "credited": [], "open": [], "not_credited": [], "unclassified": []}
    report["refusal_accounting"] = accounting

    for key, count in sorted(counts.items()):
        target, test, file, message = key
        if key not in registered:
            accounting["unclassified"].append(f"{target}::{test} · {file} · {message} ×{count}")
            errors.append(f"unclassified L5 refusal: {target}::{test} refused {file}: {message} ({count}×); "
                          "register it in tools/spec-corpus/l5_refusals.tsv")

    expected_labels = set()
    for row in rows:
        site = f"{row.target}::{row.test}"
        status = statuses.get((row.target, row.test))
        count = counts.get(row.key, 0)
        expected_labels.update(row.gap_labels if row.gap_source == "test" else ())
        if status == "FAILED":
            accounting["not_credited"].append(f"{site} · {row.message} · test failed")
            continue
        if status != "ok" or count == 0:
            errors.append(f"registered L5 refusal not observed in a passing test: {site} · {row.file}: {row.message} (test {status or 'absent'})")
            continue
        if count != int(row.occurrences):
            errors.append(f"L5 refusal occurrence count changed: {site} · {row.message}: observed {count}, registered {row.occurrences}")
            continue
        entry = f"{site} · {row.file}: {row.message} ×{count} · {row.reference}"
        if row.cls == "invalid":
            accounting["credited"].append(entry)
            case_status, kind = "intended rejection verified", "Invalid input"
        else:
            accounting["open"].append(f"{row.cls} · {entry}")
            case_status, kind = OPEN_CLASSES[row.cls]
        for label in row.gap_labels:
            key = "handoff/" + label
            case = report["cases"].get(key)
            if row.gap_source == "test":
                evidence = f"Observed {row.message} at {row.file}:"
                if case is None:
                    errors.append(f"missing gap report for registered L5 limitation: {key} ({site})")
                elif case["status"] != "open gap" or case["stage"] != "L5" or evidence not in case["detail"]:
                    errors.append(f"gap report {key} does not record the registered refusal {row.file}: {row.message}")
                else:
                    case["refusal_reason"] = row.message
            elif case is not None:
                errors.append(f"registry gap label collides with a reported case: {key}")
            else:
                report["cases"][key] = {
                    "cohort": "handoff", "stage": "L5", "status": case_status,
                    "detail": f"{kind}. {row.justification} Observed {row.message} at {row.file} in {site}.",
                    "registered_refusal": row.cls,
                    "refusal_reason": row.message,
                }
            if key in report["cases"] and row.key in locations:
                if row.gap_source == "registry":
                    report["cases"][key]["fixture_locations"] = locations[row.key]
                elif (not case.get("fixture_locations") or
                      any(site not in locations[row.key] for site in case["fixture_locations"])):
                    errors.append(f"{key}: labelled stop does not match a fixture location for {site}")

    for key, case in report["cases"].items():
        label = key.removeprefix("handoff/")
        if (case["cohort"] == "handoff" and case["stage"] == "L5" and case["status"] == "open gap"
                and "registered_refusal" not in case and label not in expected_labels):
            errors.append(f"unregistered L5 gap report: {key}; add its refusal to tools/spec-corpus/l5_refusals.tsv")


def summary(report):
    """Hook: Markdown lines for report.md."""
    accounting = report.get("refusal_accounting")
    if not accounting:
        return []
    lines = ["", "## Refusals pinned by L5 tests", "",
             f"{accounting['registered']} registered refusals. Intended rejections credited: {len(accounting['credited'])}. "
             f"Open (missing handoff, blocked checker input, or unresolved): {len(accounting['open'])}. Not credited because the test failed: {len(accounting['not_credited'])}. "
             f"Unclassified: {len(accounting['unclassified'])}.",
             "", "Each open refusal also appears as an L5 handoff open gap. [Registry](../../../tools/spec-corpus/l5_refusals.tsv)", ""]
    lines += [f"- open · {entry}" for entry in accounting["open"]]
    lines += [f"- unclassified · {entry}" for entry in accounting["unclassified"]]
    lines += [f"- not credited · {entry}" for entry in accounting["not_credited"]]
    return lines
