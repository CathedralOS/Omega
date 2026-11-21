"""Keep migrated gap labels in the case ledger without accepting self-reported closures.

Use the original bare case label, product or checker-input, and all required
psi-output labels joined by '|'. The mapping is a reviewed assertion of relevance
for that exact fixture; a related test of a different fixture is not a migration.
While l5_refusals.tsv is invalid no closure is credited: an open refusal
registration for the same case cannot be ruled out.
"""
import csv
from pathlib import Path

import lowering_evidence
import refusal_evidence

REGISTRY = Path(__file__).with_name("l5_handoff_closures.tsv")
STATUS = {"product": "Psi product compared", "checker-input": "checker input compared; judgement pending"}


def parse(source):
    reader = csv.reader(source, delimiter="\t", quoting=csv.QUOTE_NONE)
    if next(reader, None) != ["case", "scope", "checks"]:
        raise ValueError("expected case, scope and checks columns")
    cases = {}
    for fields in reader:
        if len(fields) != 3:
            raise ValueError(f"malformed closure row {reader.line_num}: {fields}")
        case, scope, labels = fields
        if not case or case.strip() != case or case in cases or scope not in STATUS:
            raise ValueError(f"invalid or duplicate closure row {reader.line_num}: {fields}")
        checks = labels.split("|")
        if len(set(checks)) != len(checks):
            raise ValueError(f"{case}: repeated output check")
        for label in checks:
            check = lowering_evidence.ASSERTIONS.get(label)
            if not label.startswith("psi-output/") or not check or check.role != "producer":
                raise ValueError(f"{case}: not a registered Psi product comparison: {label!r}")
        cases[case] = scope, checks
    return cases


def apply(report):
    if report.get("last_layer") != "L5":
        return
    try:
        with REGISTRY.open(encoding="utf-8", newline="") as source:
            closures = parse(source)
    except (OSError, ValueError) as error:
        report["measurement_errors"].append(f"{REGISTRY.name}: {error}")
        return

    if not closures:
        return
    # Even if an open test disappeared and produced no case, its registration conflicts.
    # An invalid refusal registry cannot rule that conflict out, so it credits no closure.
    try:
        open_labels = {label for row in refusal_evidence.registrations() for label in row.gap_labels}
        registry_error = None
    except ValueError as error:
        open_labels, registry_error = set(), str(error)
        report["measurement_errors"].append(
            f"{REGISTRY.name}: {len(closures)} closures not credited because {refusal_evidence.REGISTRY.name} is invalid: {error}")
    assertions = report["lowering_evidence"]["assertions"]
    for case, (scope, labels) in closures.items():
        key = "handoff/" + case
        if case in open_labels or key in report["cases"]:
            report["measurement_errors"].append(f"{key}: closure conflicts with existing case or registered refusal")
            continue
        # 'credited' already requires a unique ok result and unique receipt from this
        # registration's exact target/test. Keep the expression printed after the assertion.
        receipts, missing = {}, []
        for label in labels:
            row = assertions.get(label, {})
            if row.get("state") == "credited" and row.get("comparison"):
                receipts[label] = {field: row[field] for field in ("target", "test", "claim", "comparison")}
            else:
                missing.append(label)
        detail = STATUS[scope]
        if missing:
            detail = "No passing comparison for " + ", ".join(missing)
            report["measurement_errors"].append(f"{key}: {detail}")
        if registry_error:
            detail = (f"Closure not credited: {refusal_evidence.REGISTRY.name} is invalid ({registry_error}), "
                      "so a registered open refusal for this case cannot be ruled out. " + detail)
        report["cases"][key] = {"cohort": "handoff", "stage": "L5",
                                "status": "inspection failed" if missing or registry_error else STATUS[scope],
                                "detail": detail, "required_output_checks": labels, "output_receipts": receipts}
        if registry_error and not missing:
            # All outputs were compared. Only registry validation prevents credit.
            report["cases"][key]["measurement_blocker"] = refusal_evidence.REGISTRY.name


def summary(report):
    lines = []
    for key, case in report["cases"].items():
        if case["status"] not in STATUS.values():
            continue
        lines.append(f"- {key} · {case['status']}")
        for label, row in case["output_receipts"].items():
            lines.append(f"  - {label} · `{row['target']}::{row['test']}` · {row['claim']}: `{row['comparison']}`")
    return ["", "### Formerly blocked handoffs with compared L5 output", "", *lines] if lines else []
