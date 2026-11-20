"""Credit a reviewed comparison only in a run that observed its successful test."""
import csv
from functools import cache
from pathlib import Path
from typing import NamedTuple

import frontend_products


class Comparison(NamedTuple):
    owner: str
    rule: str
    side: str
    suite: str
    target: str
    test: str
    comparison: str


@cache
def registrations():
    with Path(__file__).with_name("reviewed_handoffs.tsv").open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source, delimiter="\t")
        if reader.fieldnames != list(Comparison._fields):
            raise ValueError("invalid reviewed handoff table header")
        rows = [Comparison(**row) for row in reader]
    if not rows or len(set(rows)) != len(rows):
        raise ValueError("empty or duplicate handoff registration")
    for row in rows:
        if (not all(row) or row.owner not in {"L0", "L1", "L2", "L3", "L4"}
                or row.suite not in {"L0", "L1", "L2", "L3", "L4", "L5"}
                or row.side not in {"direct", "consumer", "refusals"}):
            raise ValueError(f"invalid handoff registration: {row}")
    return rows


@cache
def sources():
    # A product comparison has an owner even when it is not evidence for a handoff rule.
    result = {frontend_products.comparison(stage): (stage, f"tests/{target}.rs", test)
              for stage, (target, test, _) in frontend_products.PRODUCTS.items()}
    for row in registrations():
        if row.comparison.startswith("fixture/"):
            continue
        source = (row.suite, row.target, row.test)
        if result.setdefault(row.comparison, source) != source:
            raise ValueError(f"conflicting assertion sites: {row.comparison}")
    return result


def record(report, suite, target, current_test, line):
    parts = line.split("\t", 2)
    source = (suite, target.replace("\\", "/"), current_test[1] if current_test else None)
    if len(parts) != 3 or not parts[2].strip() or sources().get(parts[1]) != source:
        report["measurement_errors"].append(f"unrecognized or misplaced reviewed comparison: {line}")
        if len(parts) >= 2 and parts[1] in sources():
            report.setdefault("reviewed_comparisons", {})[parts[1]] = None
        return
    failed = parts[0] == "OMEGA_FAILED_COMPARISON"
    receipts = report.setdefault("failed_comparisons" if failed else "reviewed_comparisons", {})
    if parts[1] in receipts:
        if not failed or receipts[parts[1]] != parts[2]:
            report["measurement_errors"].append(f"duplicate reviewed comparison: {parts[1]}")
            if not failed:
                receipts[parts[1]] = None
    else:
        receipts[parts[1]] = parts[2]


def test_results(report):
    """Require a single libtest result for the exact suite, target and test."""
    results = {}
    for stage, suite in report["suites"].items():
        for row in suite.get("tests", []):
            key = stage, row["target"].replace("\\", "/"), row["test"]
            results[key] = "ambiguous" if key in results else row["status"]
    return results


def apply(report, outlines, unconfirmed_fixtures=()):
    receipts = report.get("reviewed_comparisons", {})
    cutoff = report.get("last_layer", "L4")
    tests = test_results(report)
    observed = set()
    errors = report["measurement_errors"]
    counts = {"registered": len(registrations()), "observed": 0, "outside_run": 0}
    report["reviewed_registration_counts"] = counts
    for registration in registrations():
        entries = [row for row in outlines[registration.owner]
                   if row["rule"] == registration.rule or row["rule"].startswith(registration.rule + " ")]
        if len(entries) != 1:
            errors.append(f"{registration.owner}: expected one rule matching {registration.rule!r}")
            continue
        entry = entries[0]
        site = f"{registration.suite} {registration.target}::{registration.test}"
        if registration.suite > cutoff:
            counts["outside_run"] += 1
            entry.setdefault("outside_run", []).append(f"{registration.side}: {site}")
            continue
        result = tests.get((registration.suite, registration.target, registration.test))
        expression = receipts.get(registration.comparison)
        if registration.comparison.startswith("fixture/"):
            fixture = registration.comparison.removeprefix("fixture/")
            goldens = report.get("handoff_observations", {}).get("goldens", {})
            key = f"{registration.suite}/{fixture}"
            expression = fixture if key not in unconfirmed_fixtures and goldens.get(key) == registration.test else None
        failure = report.get("failed_comparisons", {}).get(registration.comparison)
        if failure:
            entry.setdefault("failed_checks", []).append(f"{site} · {registration.comparison} · {failure}")
            if result != "FAILED":
                errors.append(f"{site}: comparison {registration.comparison} failed but test result was {result}")
        elif result == "FAILED":
            detail = "comparison returned before the test failed" if expression else "comparison not observed"
            entry.setdefault("failed_tests", []).append(f"{site} · {registration.comparison} · {detail}")
        elif result != "ok" or expression is None:
            errors.append(f"missing reviewed comparison: {registration.comparison} ({site}, test {result or 'absent'})")
        else:
            counts["observed"] += 1
            observed.add(registration)
            bucket = entry.setdefault(registration.side, [])
            # The document's fixture column can name the very same comparison.
            if expression not in bucket:
                bucket.append(f"{site} · {expression}")
    return observed
