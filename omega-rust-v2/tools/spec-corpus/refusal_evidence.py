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

A damaged registry is also a measurement error, not a crash: the report still
gets written and blocks. A row that fails validation, and every row sharing
its duplicate key or gap label, is not credited; its observed refusals are
listed as unaccounted and its registry case stays a blocking case.
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


class Registry(NamedTuple):
    """The refusal registrations as read, with every reason not to trust them.

    `problems` pairs a message with the indexes of the rows it affects. An
    untrusted registry could not be read completely (file, header or a row's
    field count), so none of its rows can be credited and an observed refusal
    may belong to a row that was lost.
    """
    rows: tuple
    problems: tuple = ()
    trusted: bool = True
    source: str = "L5 refusal registrations"
    first_line: int | None = None

    @property
    def affected(self):
        if not self.trusted:
            return frozenset(range(len(self.rows)))
        return frozenset(index for indexes, _ in self.problems for index in indexes)

    def describe(self, index):
        row = self.rows[index]
        where = f"line {self.first_line + index}" if self.first_line else f"registration {index + 1}"
        return f"{where} {row.target}::{row.test}"


def problems(rows):
    """List every reason the registry could credit a refusal without evidence.

    Each entry pairs the indexes of the affected rows with a message. A
    duplicate key or gap label affects every row that carries it, so neither
    registration of a duplicate is credited.
    """
    found, keys, labels = [], {}, {}
    for index, row in enumerate(rows):
        if not all(row) or not row.target.startswith("tests/") or not row.target.endswith(".rs"):
            found.append(((index,), f"incomplete L5 refusal registration: {row}"))
        if row.key in keys:
            found.append(((keys[row.key], index), f"duplicate L5 refusal registration: {row.key}"))
        keys.setdefault(row.key, index)
        try:
            positive_count = row.occurrences.isdecimal() and int(row.occurrences) >= 1
        except ValueError:
            positive_count = False
        if row.cls not in CLASSES or not positive_count:
            found.append(((index,), f"invalid class or occurrence count: {row}"))
        if row.cls == "invalid":
            if row.gap != "-" and (row.gap_source != "registry" or not all(row.gap_labels)):
                found.append(((index,), f"an intended refusal needs a registry case label or a dash: {row}"))
            if LIMITATION_WORDING.search(row.message):
                found.append(((index,), f"message admits a missing lowering and cannot be an intended refusal: {row.message}"))
            if not any(document in row.reference for document in CONTRACT_DOCUMENTS):
                found.append(((index,), f"an intended refusal must cite the public L5 contract: {row}"))
        elif row.gap_source not in {"test", "registry"} or not all(row.gap_labels):
            found.append(((index,), f"an open refusal needs a test or registry gap label: {row}"))
        for label in row.gap_labels:
            if label in labels:
                found.append(((labels[label], index), f"duplicate gap label: {label}"))
            labels.setdefault(label, index)
    return found


def validate(rows):
    """Reject a registry that could credit a refusal without evidence."""
    if found := problems(rows):
        raise ValueError(found[0][1])
    return rows


def read(path=None):
    """Read and check a registry file without raising; `load` caches the authoritative one."""
    path = REGISTRY if path is None else Path(path)
    source = path.name
    try:
        with path.open(encoding="utf-8", newline="") as text:
            reader = csv.reader(text, delimiter="\t", quoting=csv.QUOTE_NONE)
            header = next(reader, None)
            if header != [*Refusal._fields[:5], "class", *Refusal._fields[6:]]:
                return Registry((), (((), "invalid L5 refusal registry header"),), False, source)
            rows, damaged = [], []
            for fields in reader:
                if len(fields) != len(Refusal._fields):
                    damaged.append(((), f"malformed L5 refusal registration at line {reader.line_num}: {fields}"))
                else:
                    rows.append(Refusal(*fields))
    except (OSError, UnicodeDecodeError, csv.Error) as error:
        return Registry((), (((), f"cannot read the L5 refusal registry: {error}"),), False, source)
    if damaged:
        # The surviving rows no longer have their file lines, so they are counted, not located.
        return Registry(tuple(rows), tuple(damaged), False, source)
    if not rows:
        return Registry((), (((), "empty L5 refusal registry"),), False, source)
    return Registry(tuple(rows), tuple(problems(rows)), True, source, first_line=2)


@cache
def load():
    """The authoritative registry, read and checked once per measurement."""
    return read()


@cache
def registrations():
    registry = load()
    if registry.problems:
        raise ValueError(registry.problems[0][1])
    return registry.rows


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
    if rows is None:
        registry = load()
    else:
        rows = tuple(rows)
        registry = Registry(rows, tuple(problems(rows)))
    rows, affected = registry.rows, registry.affected
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
    # A damaged registration is never credited and never merged with another
    # row; the refusals it named stay listed until the registry is repaired.
    damaged_keys = {rows[index].key for index in affected}
    registered = {row.key: row for index, row in enumerate(rows) if index not in affected}
    accounting = {"registered": len(rows), "credited": [], "open": [], "not_credited": [], "unclassified": []}
    report["refusal_accounting"] = accounting
    if registry.problems:
        accounting["registry_errors"] = []
        accounting["unaccounted"] = []
    for indexes, message in registry.problems:
        rows_named = "; ".join(registry.describe(index) for index in sorted(set(indexes)))
        problem = f"{message} ({rows_named})" if rows_named else message
        accounting["registry_errors"].append(problem)
        consequence = "these registrations are not credited" if registry.trusted else "no L5 refusal is credited from it"
        errors.append(f"{registry.source}: {problem}; {consequence}")

    for key, count in sorted(counts.items()):
        target, test, file, message = key
        if key in registered:
            continue
        raw = f"{target}::{test} · {file} · {message} ×{count}"
        if key in damaged_keys or not registry.trusted:
            accounting["unaccounted"].append(raw)
        else:
            accounting["unclassified"].append(raw)
            errors.append(f"unclassified L5 refusal: {target}::{test} refused {file}: {message} ({count}×); "
                          "register it in tools/spec-corpus/l5_refusals.tsv")
    if not registry.trusted and accounting["unaccounted"]:
        errors.append(f"{len(accounting['unaccounted'])} observed L5 refusals are unaccounted because "
                      f"{registry.source} could not be read completely")

    expected_labels = set()
    damaged_cases = {}
    for index, row in enumerate(rows):
        if index in affected:
            # Suppress a second report for a label the registry error already names.
            expected_labels.update(row.gap_labels if row.gap_source == "test" else ())
            for label in row.gap_labels if row.gap_source == "registry" else ():
                damaged_cases.setdefault(label, []).append(row)
            continue
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

    # A case the damaged registry names stays in the ledger as a blocker, so a
    # comparison never reads registry damage as a closed gap.
    for label, named in damaged_cases.items():
        key = "handoff/" + label
        if key in report["cases"]:
            continue
        observed = " ".join(f"Observed {row.message} at {row.file} in {row.target}::{row.test} ({counts.get(row.key, 0)}×)."
                            for row in named)
        case = report["cases"][key] = {
            "cohort": "handoff", "stage": "L5", "status": "inspection failed",
            "detail": f"Registration invalid; the refusal is neither classified nor credited. {observed}",
            "registered_refusal": "invalid registration", "measurement_blocker": REGISTRY.name,
        }
        if len({row.message for row in named}) == 1:
            case["refusal_reason"] = named[0].message
        if sites := [site for row in named for site in locations.get(row.key, ())]:
            case["fixture_locations"] = sites

    if not registry.trusted:
        return
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
    if accounting.get("registry_errors"):
        lines += [f"The registry is invalid ({len(accounting['registry_errors'])} problems), so its affected registrations are not credited. "
                  f"Observed refusals without a valid registration: {len(accounting['unaccounted'])}.", ""]
        lines += [f"- registry error · {entry}" for entry in accounting["registry_errors"]]
        lines += [f"- unaccounted · {entry}" for entry in accounting["unaccounted"]]
    lines += [f"- open · {entry}" for entry in accounting["open"]]
    lines += [f"- unclassified · {entry}" for entry in accounting["unclassified"]]
    lines += [f"- not credited · {entry}" for entry in accounting["not_credited"]]
    return lines
