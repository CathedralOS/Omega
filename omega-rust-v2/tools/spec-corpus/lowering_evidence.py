"""Inventory L5's contract outputs and measure what a full readiness run observed.

Four kinds of evidence stay separate:

- A capability sentence in the L5 contract states what lowering emits. It is a claim.
- A Psi output form emitted by a published example shows that the form occurs. It does not show meaning.
- An assertion receipt from a registered L5 test that passed in this run checks one claim.
- Anything else is unmeasured. A passing linked test file shows only that the file ran green.

Typed obligations are classified from the rows the runner already validated:
`report["obligations"]` (kind -> spec label -> count, printed before `lower_package`),
`report["package_fixtures"]` (one row per shared-helper call, printed after it) and
`report["cases"]`. This module parses no OMEGA_INVENTORY, OMEGA_OBLIGATION or OMEGA_FIXTURE
line itself. Reaching L5, lowering and a checked Psi property stay separate. The only
registration of a Psi property for an obligation kind is the `obligations` tuple of an
entry in `ASSERTIONS`. Kinds are never matched to Psi forms by name.

The spec aggregate fails while any example has an open gap, so a spec input's standing comes from
that example's own final receipt (`OMEGA_L5_SPEC_RECEIPT`, printed by the aggregate after every
frontend, lowering-snapshot and semantic check of every example ran). Only a consistent receipt
with verdict `passed` confirms an example. A missing, repeated, misplaced or inconsistent receipt
confirms nothing.

A run that stops at L4 records nothing here. Call `finish(report, directory)` after the
suites ran and `summary(report)` while writing report.md.
"""
import csv
from collections import Counter
from pathlib import Path
import re
from typing import NamedTuple

import reviewed_comparisons

LAYER = Path("layers/05_prepared-trees-to-terminal-psi")
# "Introduction" names the text under the title. "Evidence for sign-off" describes the review process and is excluded.
CONTRACT_SECTIONS = ("Introduction", "First handoff", "Operations and declarations", "Handoffs and remaining work")
OBLIGATION_TABLE = "Typed obligations to reconcile before sign-off"
PRODUCED = {"executable product inspected", "executable product uninspected", "declaration or empty product"}
# A contextual control wraps the source. The unwrapped fence still stopped.
STOPPED_AT_L5 = {"open gap", "missing context", "contextual product inspected"}
# `L3-resumed` is an L3 refusal of a retype that consumed L4 answers (tests/support/preparation.rs).
RESUMED_TYPING = "L3-resumed"
STAGES = {"L0", "L1", "L2", "L3", RESUMED_TYPING, "L4", "L5"}
PROBE = ("tests/output_inventory.rs", "published_examples_expose_their_psi_output_forms")
ENTRY_STATES = ("claim asserted", "linked tests passed, claims unregistered", "linked test file has failures", "unmeasured")
# The aggregate test that prints each spec example's obligation counts and its OMEGA_CASE row.
SPEC_AGGREGATE = ("tests/spec_corpus.rs", "spec_fragments_have_reviewed_lowering_outcomes")
# Strongest observation first. Only the first state involves a check of Psi content, and only of the registered property.
LIBRARY_KIND = "VerifyLibraryImplementation"
LIBRARY_AUDIT = ("L3", "unittests src/lib.rs",
                 "toolchain_declarations::core_interface::tests::audit_closed_catalog_guarantees_against_their_implementation_obligations")
LIBRARY_CONSUMER = ("L5", "tests/obligation_library.rs", "library_definition_rows_stay_in_the_library_forest_for_callers_and_non_callers")
LIBRARY_COMPARISON = "library-definition-guarantees/consumer"
CATALOG_INAPPLICABLE = "no qualifying declaration in the audited toolchain catalog"
OBLIGATION_STATES = ("registered Psi property checked", "lowered in a passing test, no Psi property checked",
                     "lowered only in a failed or unreported test", "reached L5, outcome unattributed",
                     "reached L5, every input refused", "no observed input", CATALOG_INAPPLICABLE)
# The emitting test passed, failed (it did not finish its checks), or reported no single result.
# For a spec example: its own receipt passed, its receipt reports a gap or failure, or it has no valid receipt.
STANDINGS = ("confirmed", "partial", "unreported")
# Final per-example receipt of SPEC_AGGREGATE: label, case status, verdict and the checks behind it.
SPEC_RECEIPT = "OMEGA_L5_SPEC_RECEIPT"
RECEIPT_CHECKS = {"frontend": {"matched", "mismatched", "absent", "unchecked"},
                  "lowering": {"matched", "mismatched", "absent", "extra", "none"},
                  "semantic": {"witness", "declarations", "missing", "none", "panicked"}}
RECEIPT_VERDICTS = ("passed", "recorded refusal", "known gap", "failed", "panicked",
                    "mismatched expectation", "absent expectation", "extra expectation")
# The semantic check an example reaching L5 must report for its case status.
RECEIPT_SEMANTIC = {"executable product inspected": "witness", "contextual product inspected": "witness",
                    "declaration or empty product": "declarations", "executable product uninspected": "missing",
                    "open gap": "none", "missing context": "none"}

# Sentence classification follows the contract's wording. Review the details file before relying on a count.
LIMIT = re.compile(r"\bstill (?:needs?|requires?)\b|\bremain open\b|\bhas an open\b|\bcannot emit\b|\breports unavailable\b|\bit refuses\b")
DELEGATED = re.compile(r"^(?:The (?:lowered-form )?checker|L6|Artifact encoding)\b|\bL6 (?:owns|checks|judges)\b")
LINK = re.compile(r"\[([^\]]*)\]\(([^)\s]+)\)")
SENTENCE = re.compile(r"(?<=[.!?])\s+(?=[A-Z`])")
TARGET = re.compile(r"tests/[^/]+\.rs")
FORM = re.compile(r"([A-Za-z]+::[A-Za-z_]+)=([1-9]\d*)")


class Assertion(NamedTuple):
    entry: str
    role: str
    target: str
    test: str
    claim: str
    # Typed obligation kinds whose one Psi property `claim` states. Credit also requires
    # that a source in the lowered package owned an occurrence of the kind.
    obligations: tuple = ()


def registered_assertions():
    with Path(__file__).with_name("l5_output_checks.tsv").open(encoding="utf8", newline="") as file:
        reader = csv.DictReader(file, delimiter="\t", quoting=csv.QUOTE_NONE)
        if reader.fieldnames != ["label", *Assertion._fields]:
            raise ValueError("invalid L5 output-check header")
        checks = {}
        for row in reader:
            label = row.pop("label")
            kinds = row["obligations"]
            row["obligations"] = tuple(kinds.split(",")) if kinds else ()
            if (not label or label in checks or row["role"] not in {"producer", "refusal"} or
                    not all(row[field] for field in ("entry", "target", "test", "claim"))):
                raise ValueError(f"incomplete or duplicate L5 output check: {label}")
            checks[label] = Assertion(**row)
        return checks


ASSERTIONS = registered_assertions()


def contract_entries(source):
    """Paragraphs of the handoff sections. A paragraph can contain several promises."""
    entries, paragraph = [], []
    section, first = None, 0

    def flush():
        if paragraph:
            entries.append(entry(first, section, " ".join(paragraph)))
            paragraph.clear()

    for number, line in enumerate(source.splitlines(), 1):
        if line.startswith("#"):
            flush()
            level = len(line) - len(line.lstrip("#"))
            if level == 1:
                section = "Introduction"
            elif level == 2:
                section = line[2:].strip()
            continue
        if section in CONTRACT_SECTIONS and line.strip():
            if not paragraph:
                first = number
            paragraph.append(line.strip())
        else:
            flush()
    flush()
    missing = [name for name in CONTRACT_SECTIONS if not any(row["section"] == name for row in entries)]
    if missing:
        raise ValueError(f"no L5 contract entries extracted for {missing}")
    return entries


def entry(line, section, text):
    links = [target.split("#", 1)[0] for _, target in LINK.findall(text)]
    prose = " ".join(re.sub(r"(?:^|\s)·(?=\s|$)", " ", LINK.sub("", text)).split())
    row = {"line": line, "section": section, "text": text, "capabilities": [], "limits": [], "delegated": []}
    for sentence in filter(None, (part.strip() for part in SENTENCE.split(prose))):
        kind = "limits" if LIMIT.search(sentence) else "delegated" if DELEGATED.search(sentence) else "capabilities"
        row[kind].append(sentence)
    row["local_links"] = sorted({path for path in links if path.startswith(("tests/", "src/"))})
    row["targets"] = [path for path in row["local_links"] if TARGET.fullmatch(path)]
    return row


def obligation_rows(source):
    """Rows of the LOCK_EVIDENCE table that assigns each typed obligation kind a place to verify it."""
    rows, section = [], None
    for number, line in enumerate(source.splitlines(), 1):
        if line.startswith("#"):
            section = line.lstrip("#").strip()
            continue
        if section != OBLIGATION_TABLE or not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if cells[0] == "Typed input" or cells[0].startswith("---"):
            continue
        rows.append({"line": number, "kinds": re.findall(r"`([A-Za-z]+)`", cells[0]),
                     "links": [target.split("#", 1)[0] for _, target in LINK.findall(line)]})
    if not rows:
        raise ValueError(f"no rows under {OBLIGATION_TABLE!r}")
    return rows


def scan(log):
    """Yield (target, test, line) for each L5 receipt, attributed like the runner's own parser."""
    target, current = "", None
    for physical_line in log.split("\n"):
        line = physical_line.removesuffix("\r")
        if header := re.match(r"^\s*Running (.+?) \(", line):
            target, current = header[1].replace("\\", "/"), None
        elif line.lstrip().startswith("Doc-tests "):
            target, current = "doctests", None
        elif captured := re.match(r"^---- (.+?) stdout ----$", line):
            current = captured[1]
        elif line in {"successes:", "failures:"} or line.startswith("test result: ") or re.fullmatch(r"---- .+? stderr ----", line):
            current = None
        elif line.startswith("OMEGA_L5_"):
            yield target, current, line


def observe(log, errors):
    receipts = {}
    invalid = set()
    probe = {"inventory": None, "count": None, "examples": {}}
    for target, test, line in scan(log):
        fields = line.split("\t")
        if fields[0] == "OMEGA_L5_ASSERT":
            assertion = ASSERTIONS.get(fields[1]) if len(fields) in (2, 3) else None
            comparison = fields[2] if len(fields) == 3 else None
            needs_comparison = len(fields) >= 2 and fields[1].startswith("psi-output/")
            if (not assertion or (target, test) != (assertion.target, assertion.test)
                    or needs_comparison and not re.fullmatch(r"assert(?:_eq|_ne)?!\s*\(.*\S.*\)", comparison or "")):
                errors.append(f"unregistered or misplaced L5 assertion receipt: {line} ({target}::{test})")
                if len(fields) > 1 and fields[1] in ASSERTIONS:
                    invalid.add(fields[1])
            elif fields[1] in receipts:
                errors.append(f"duplicate L5 assertion receipt: {fields[1]} ({target}::{test})")
                invalid.add(fields[1])
            else:
                receipts[fields[1]] = comparison
            continue
        if fields[0] == SPEC_RECEIPT:
            continue  # Validated by spec_receipts.
        if (target, test) != PROBE:
            errors.append(f"L5 output observation outside {PROBE[0]}::{PROBE[1]}: {line}")
            continue
        tag = fields[0]
        if tag == "OMEGA_L5_OUTPUT_INVENTORY" and len(fields) == 2 and probe["inventory"] is None:
            forms = fields[1].split(",")
            if len(set(forms)) != len(forms) or not all(re.fullmatch(r"[A-Za-z]+::[A-Za-z_]+", form) for form in forms):
                errors.append(f"invalid L5 output inventory: {line}")
                continue
            probe["inventory"] = forms
        elif tag == "OMEGA_L5_OUTPUT_COUNT" and len(fields) == 2 and fields[1].isdecimal() and probe["count"] is None:
            probe["count"] = int(fields[1])
        elif tag == "OMEGA_L5_OUTPUT" and len(fields) == 4 and fields[1] not in probe["examples"]:
            _, label, status, detail = fields
            pairs = [FORM.fullmatch(pair) for pair in detail.split(",")] if detail else []
            if status == "produced" and all(pairs) and len({match[1] for match in pairs}) == len(pairs):
                probe["examples"][label] = {"status": status, "forms": {match[1]: int(match[2]) for match in pairs}}
            elif status == "stopped" and detail in STAGES:
                probe["examples"][label] = {"status": status, "stage": detail}
            elif status == "panicked" and detail == "-":
                probe["examples"][label] = {"status": status}
            else:
                errors.append(f"malformed L5 output observation: {line}")
        else:
            errors.append(f"malformed or duplicate L5 output observation: {line}")
    for label in invalid:
        receipts.pop(label, None)
    return receipts, probe


def receipt_verdict(status, checks):
    """The verdict the receipt's own checks imply. Mirrors `Receipt::verdict` in tests/spec_corpus.rs."""
    if checks["semantic"] == "panicked":
        return "panicked"
    if "mismatched" in (checks["frontend"], checks["lowering"]):
        return "mismatched expectation"
    if "absent" in (checks["frontend"], checks["lowering"]):
        return "absent expectation"
    if checks["lowering"] == "extra":
        return "extra expectation"
    if checks["semantic"] == "missing":
        return "failed"
    if status in {"open gap", "missing context"}:
        return "known gap"
    if status == "recorded refusal":
        return "recorded refusal"
    return "passed"


def parse_receipt(fields):
    if len(fields) != 5 or not fields[1] or fields[3] not in RECEIPT_VERDICTS:
        return None
    checks = {}
    for pair in fields[4].split(","):
        name, _, value = pair.partition("=")
        if name not in RECEIPT_CHECKS or name in checks or value not in RECEIPT_CHECKS[name]:
            return None
        checks[name] = value
    if checks.keys() != RECEIPT_CHECKS.keys():
        return None
    return {"label": fields[1], "status": fields[2], "verdict": fields[3], "checks": checks}


def receipt_inconsistency(case, receipt):
    """Why a receipt cannot describe the example's OMEGA_CASE row, or None."""
    status, checks = receipt["status"], receipt["checks"]
    if status != case["status"]:
        return f"receipt status {status}, case status {case['status']}"
    if case["stage"] == "unattributed":
        allowed = {"frontend": {"unchecked"}, "lowering": {"none", "extra"}, "semantic": {"panicked"}}
    elif case["stage"] == "L5":
        allowed = {"frontend": {"matched", "mismatched", "absent"}, "lowering": {"matched", "mismatched", "absent"},
                   "semantic": {RECEIPT_SEMANTIC.get(status)}}
    else:
        frontend = {"matched"} if status == "recorded refusal" else {"mismatched", "absent"}
        allowed = {"frontend": frontend, "lowering": {"none", "extra"}, "semantic": {"none"}}
    for name, values in allowed.items():
        if checks[name] not in values:
            return f"{name}={checks[name]} cannot describe {status} at {case['stage']}"
    implied = receipt_verdict(status, checks)
    if receipt["verdict"] != implied:
        return f"verdict {receipt['verdict']}, its checks imply {implied}"
    return None


def spec_receipts(log, report, tests, errors):
    """Label -> the validated final receipt of each spec example.

    A receipt counts only from the captured output of SPEC_AGGREGATE, once per label, for a spec case
    the runner recorded, and only when its status, checks and verdict agree with that case.
    """
    raw, repeated = {}, set()
    for target, test, line in scan(log):
        fields = line.split("\t")
        if fields[0] != SPEC_RECEIPT:
            continue
        if (target, test) != SPEC_AGGREGATE:
            errors.append(f"spec example receipt outside {SPEC_AGGREGATE[0]}::{SPEC_AGGREGATE[1]} ({target}::{test}): {line}")
            continue
        receipt = parse_receipt(fields)
        if receipt is None:
            errors.append(f"malformed spec example receipt: {line}")
            continue
        label = receipt["label"]
        if label in raw or label in repeated:
            errors.append(f"repeated spec example receipt for {label}: {line}")
            raw.pop(label, None)
            repeated.add(label)
            continue
        raw[label] = receipt
    aggregate = tests.get(SPEC_AGGREGATE)
    cases = {key.removeprefix("spec/"): case for key, case in report.get("cases", {}).items() if key.startswith("spec/")}
    accepted = {}
    for label, receipt in sorted(raw.items()):
        case = cases.get(label)
        problem = "no spec case result" if case is None else receipt_inconsistency(case, receipt)
        if not problem and aggregate == "ok" and receipt["verdict"] not in {"passed", "recorded refusal", "known gap"}:
            problem = f"verdict {receipt['verdict']}, but the aggregate passed"
        if problem:
            errors.append(f"inconsistent spec example receipt for {label}: {problem}")
        else:
            accepted[label] = receipt
    missing = sorted(cases.keys() - raw.keys() - repeated)
    if cases and not raw and not repeated:
        errors.append(f"{SPEC_AGGREGATE[0]}::{SPEC_AGGREGATE[1]} printed no example receipts; {len(cases)} spec examples unreported")
    else:
        errors.extend(f"spec/{label}: no final example receipt from {SPEC_AGGREGATE[0]}::{SPEC_AGGREGATE[1]}" for label in missing)
    return accepted


def spec_standing(aggregate, receipt):
    """A spec example is confirmed only by its own passed receipt from a test that reported a result."""
    if aggregate not in {"ok", "FAILED"} or receipt is None:
        return "unreported"
    return "confirmed" if receipt["verdict"] == "passed" else "partial"


def test_key(target, test):
    return target.replace("\\", "/"), test.removesuffix(" - should panic")


def test_results(report):
    suite = report.get("suites", {}).get("L5", {})
    results = {}
    for row in suite.get("tests", []):
        key = test_key(row["target"], row["test"])
        results[key] = "ambiguous" if key in results else row["status"]
    return results


def credit_assertions(receipts, tests, errors):
    states = {}
    for label, assertion in ASSERTIONS.items():
        status = tests.get((assertion.target, assertion.test))
        if status == "FAILED":
            states[label] = "failed"
        elif status == "ok" and label in receipts:
            states[label] = "credited"
        else:
            states[label] = "missing"
            errors.append(f"missing L5 assertion receipt: {label} ({assertion.target}::{assertion.test}, test {status or 'absent'})")
    return states


def attach(entries, states, tests, errors):
    for row in entries:
        row.update(asserted=[], failed_assertions=[], target_results={})
    if entries:
        for label, assertion in ASSERTIONS.items():
            matches = [row for row in entries if row["text"].startswith(assertion.entry)]
            if len(matches) != 1:
                errors.append(f"L5 assertion {label}: expected one contract entry starting {assertion.entry!r}, found {len(matches)}")
            elif states[label] == "credited":
                matches[0]["asserted"].append(label)
            elif states[label] == "failed":
                matches[0]["failed_assertions"].append(label)
    silent = set()
    for row in entries:
        for path in row["local_links"]:
            if not (LAYER / path).exists():
                errors.append(f"L5 CONTRACT.md line {row['line']}: linked file missing: {path}")
        for target in row["targets"]:
            counts = Counter(status for (name, _), status in tests.items() if name == target)
            row["target_results"][target] = dict(counts)
            if not counts and (LAYER / target).exists():
                silent.add(target)
        results = row["target_results"].values()
        if row["failed_assertions"] or any(counts.get("FAILED") for counts in results):
            row["status"] = "linked test file has failures"
        elif row["asserted"]:
            row["status"] = "claim asserted"
        elif results and all(counts.get("ok") for counts in results):
            row["status"] = "linked tests passed, claims unregistered"
        else:
            row["status"] = "unmeasured"
    for target in sorted(silent):
        errors.append(f"L5 contract links {target}, which reported no test results in this run")


def agrees(case, row):
    if case is None:
        return False
    if case["stage"] == "unattributed":
        return row["status"] == "panicked" and case["status"] == "inspection failed"
    if row["status"] == "produced":
        return case["status"] in PRODUCED
    if row["status"] == "stopped" and row["stage"] == "L5":
        return case["status"] in STOPPED_AT_L5
    if row["status"] == "stopped":
        # Both runners can reach the same typing error on different passes.
        resumed = row["stage"] == RESUMED_TYPING
        owner = "L3" if resumed else row["stage"]
        return (case["status"] in {"recorded refusal", "inspection failed"} and case["stage"] == owner
                and case.get("after_static_evaluation", False) == resumed)
    return False


def measure_outputs(probe, status, report, errors):
    inventory = probe["inventory"] or []
    result = {"probe": f"{PROBE[0]}::{PROBE[1]}", "test": status or "absent", "measured": False, "inventory": inventory,
              "emitted": {}, "examples": {"produced": 0, "stopped": 0, "panicked": 0}}
    if status != "ok":
        errors.append(f"L5 output probe {PROBE[0]}::{PROBE[1]}: test {status or 'absent'}; Psi output forms unmeasured")
        return result
    if not inventory:
        errors.append("L5 output probe printed no output inventory")
        return result
    examples = probe["examples"]
    expected = report.get("expected", {}).get("spec")
    if probe["count"] != len(examples) or probe["count"] != expected:
        errors.append(f"L5 output probe: observed {len(examples)} examples, the probe announced {probe['count']} and the corpus {expected}")
    emitted = {form: {"occurrences": 0, "examples": []} for form in inventory}
    for label, row in sorted(examples.items()):
        result["examples"][row["status"]] += 1
        case = report.get("cases", {}).get(f"spec/{label}")
        # A disagreeing probe row did not lower the product the corpus judged. Its forms are not credited.
        credited = agrees(case, row)
        if not credited:
            phase = " after static evaluation" if case and case.get("after_static_evaluation") else ""
            corpus = f"{case['status']} at {case['stage']}{phase}" if case else "absent"
            errors.append(f"L5 output probe and spec corpus disagree for {label}: probe {row['status']} {row.get('stage', '')}".rstrip()
                          + f", corpus {corpus}")
        for form, count in row.get("forms", {}).items():
            if form not in emitted:
                errors.append(f"L5 output probe: {label} emitted uninventoried form {form}")
                continue
            if not credited:
                continue
            emitted[form]["occurrences"] += count
            emitted[form]["examples"].append(label)
    result.update(emitted=emitted, measured=True)
    return result


def standing(status):
    return "confirmed" if status == "ok" else "partial" if status == "FAILED" else "unreported"


def obligation_inputs(report, tests, errors, receipts=None):
    """One row per prepared package that entered `lower_package` with its typed obligation counts.

    Rows come from the runner's validated `obligations`, `package_fixtures` and `cases`. A spec
    example is one input. Each helper record is one input, so a test that lowers the same files
    twice contributes two rows. A fixture's standing comes only from the emitting test's result; a
    spec example's from its validated receipt in `receipts` (see `spec_receipts`). The runner
    already reports malformed rows and spec counts whose case is missing or stopped before L5;
    such an input is classified here as unattributed without repeating that error.
    """
    labels = {}
    for kind, counts in report.get("obligations", {}).items():
        for label, count in counts.items():
            labels.setdefault(label, {})[kind] = count
    inputs = []
    receipts = receipts or {}
    aggregate = tests.get(SPEC_AGGREGATE)
    if labels and aggregate is None:
        errors.append(f"spec obligation counts have no result for {SPEC_AGGREGATE[0]}::{SPEC_AGGREGATE[1]}")
    for label, counts in sorted(labels.items()):
        case = report.get("cases", {}).get(f"spec/{label}") or {}
        # Counts are printed after preparation. A later panic does not say whether lowering returned.
        lowering = ("lowered" if case.get("status") in PRODUCED else "refused" if case.get("status") in STOPPED_AT_L5
                    else "unattributed")
        receipt = receipts.get(label)
        inputs.append({"source": "spec", "site": f"spec/{label}", "target": SPEC_AGGREGATE[0], "test": SPEC_AGGREGATE[1],
                       "obligations": counts, "lowering": lowering, "status": aggregate or "absent",
                       "receipt": receipt["verdict"] if receipt else "absent", "standing": spec_standing(aggregate, receipt)})
    unreported = set()
    for index, fixture in enumerate(report.get("package_fixtures", ())):
        target, test = test_key(fixture["target"], fixture["test"])
        status = tests.get((target, test))
        if status is None and (target, test) not in unreported:
            unreported.add((target, test))
            errors.append(f"package fixture {target}::{test} has no test result in this run")
        outcome = fixture["outcome"]
        lowering = "lowered" if outcome == "produced Psi" else "refused" if outcome.startswith("refused: ") else "unattributed"
        exact = None
        # A live instrumented run expects the versioned frame on every refusal. An
        # archive read without that requirement can legitimately predate the frame.
        if (lowering == 'refused' and report.get('lowering_origins', {}).get('origins_required_by_reader')
                and 'obligation_sites' not in fixture.get('replay', {}) and fixture.get('origin_status') != 'malformed capture'):
            errors.append(f'L5 {target}::{test}: fixture {index} missing prepared obligation sites')
        if (lowering == "lowered" and report.get("lowering_origins", {}).get("origins_required_by_reader")
                and "local_obligation_kinds" not in fixture):
            errors.append(f"L5 {target}::{test}: fixture {index} missing local obligation kinds")
        if status == 'ok' and fixture.get('obligation_sites_observed'):
            location = fixture['location']
            exact = Counter(row['kind'] for row in fixture['replay']['obligation_sites']
                            if all(row[k] == location[k] for k in ('package', 'file', 'byte_start', 'byte_end')))
        inputs.append({"source": "fixture", "site": f"{target}::{test} ({fixture['files']})", "target": target, "test": test,
                       "fixture_index": index, "exact_sites": exact,
                       "local_obligation_kinds": fixture.get("local_obligation_kinds"),
                       "obligations": dict(fixture["obligations"]), "lowering": lowering, "status": status or "absent",
                       "standing": standing(status)})
    return inputs


def lock_reconciliation(inventory, lock, errors):
    rows = {}
    for row in lock:
        for kind in row["kinds"]:
            if kind in rows:
                errors.append(f"L5 LOCK_EVIDENCE.md line {row['line']}: {kind} is already listed on line {rows[kind]['line']}")
            rows.setdefault(kind, row)
    if inventory and lock:
        for kind in sorted(inventory - rows.keys()):
            errors.append(f"typed obligation {kind} has no reconciliation row in L5 LOCK_EVIDENCE.md")
        for kind in sorted(rows.keys() - inventory):
            errors.append(f"L5 LOCK_EVIDENCE.md lists {kind}, which the prepared obligation inventory lacks")
    return rows


def property_checks(inventory, inputs, states, errors):
    """Kind -> credited and failed assertion labels. `Assertion.obligations` is the only registration."""
    credited, failed = {}, {}
    for label, state in states.items():
        assertion = ASSERTIONS[label]
        for kind in assertion.obligations:
            if kind not in inventory:
                if inventory:  # A missing inventory is reported by the runner.
                    errors.append(f"L5 assertion {label} registers {kind}, which the prepared obligation inventory lacks")
                continue
            if state == "failed":
                failed.setdefault(kind, []).append(label)
            elif state == "credited":
                sources = [row for row in inputs if row["source"] == "fixture"
                           and (row["target"], row["test"]) == (assertion.target, assertion.test)
                           and row["lowering"] == "lowered" and row["obligations"].get(kind)]
                if any(kind in (row["local_obligation_kinds"] or ()) for row in sources):
                    credited.setdefault(kind, []).append(label)
                elif sources:
                    errors.append(f"L5 assertion {label} registers {kind}, but {assertion.target}::{assertion.test} "
                                  "has no observed occurrence in the lowered package's own sources")
                else:
                    errors.append(f"L5 assertion {label} registers {kind}, but {assertion.target}::{assertion.test} lowered no input carrying it")
    return credited, failed


def record_library_audit(report, stage, target, current_test, line):
    """Only the source-catalog audit that also exercised the modified toolchain can report these counts."""
    key = "library_implementation_audit"
    site = (stage, target.replace("\\", "/"), current_test[1] if current_test else None)
    fields = line.split("\t")
    if (key in report or site != LIBRARY_AUDIT or len(fields) != 5 or fields[0] != "OMEGA_LIBRARY_IMPLEMENTATION_AUDIT"
            or any(not re.fullmatch(r"0|[1-9][0-9]{0,8}", field) for field in fields[1:])):
        report["measurement_errors"].append(f"invalid, repeated or misplaced library implementation audit: {line}")
        report[key] = None
        return
    machines, supplied, exempt, eligible = map(int, fields[1:])
    if not (0 < supplied <= machines and exempt + eligible <= supplied):
        report["measurement_errors"].append(f"inconsistent library implementation catalog counts: {line}")
        report[key] = None
        return
    report[key] = {"machines": machines, "supplied": supplied, "collection_guarantees": exempt, "eligible": eligible}


def audited_library_catalog(report):
    audit = report.get("library_implementation_audit")
    if audit is None:
        return None
    results = reviewed_comparisons.test_results(report)
    if (results.get(LIBRARY_AUDIT) == results.get(LIBRARY_CONSUMER) == "ok"
            and report.get("reviewed_comparisons", {}).get(LIBRARY_COMPARISON)
            and LIBRARY_COMPARISON not in report.get("failed_comparisons", {})):
        return audit
    return None


def obligation_blocks(kind, row):
    if row["state"] == OBLIGATION_STATES[0]:
        return False
    return not (kind == LIBRARY_KIND and row["state"] == CATALOG_INAPPLICABLE and row.get("catalog", {}).get("eligible") == 0
                and row["occurrences"] == 0 and not row["failed_property_checks"])


def classify_obligations(report, inputs, states, lock, errors):
    """The one per-kind classifier. Its counts are inputs and occurrences, never meaning."""
    inventory = set(report.get("obligations", {}))
    rows = lock_reconciliation(inventory, lock, errors)
    credited, failed = property_checks(inventory, inputs, states, errors)
    result = {}
    for kind in sorted(inventory):
        carrying = [row for row in inputs if row["obligations"].get(kind)]

        def sites(lowering, standings=STANDINGS):
            return [row["site"] for row in carrying if row["lowering"] == lowering and row["standing"] in standings]

        lowered = {name: sites("lowered", (name,)) for name in STANDINGS}
        refused, unattributed = sites("refused"), sites("unattributed")
        if credited.get(kind):
            state = OBLIGATION_STATES[0]
        elif lowered["confirmed"]:
            state = OBLIGATION_STATES[1]
        elif lowered["partial"] or lowered["unreported"]:
            state = OBLIGATION_STATES[2]
        elif unattributed:
            state = OBLIGATION_STATES[3]
        elif refused:
            state = OBLIGATION_STATES[4]
        else:
            state = OBLIGATION_STATES[5]
        result[kind] = {
            "state": state, "lock_row": rows[kind]["line"] if kind in rows else None,
            "inputs": {source: sum(row["source"] == source for row in carrying) for source in ("spec", "fixture")},
            "distinct_inputs": len({row["site"] for row in carrying}),
            "occurrences": sum(row["obligations"][kind] for row in carrying),
            "lowered": lowered, "refused": refused, "unattributed": unattributed,
            "property_checks": credited.get(kind, []), "failed_property_checks": failed.get(kind, []),
        }
        if kind == LIBRARY_KIND and (audit := audited_library_catalog(report)) is not None:
            result[kind]["catalog"] = audit
            if audit["eligible"] == 0:
                if carrying:
                    errors.append(f"{kind} occurs in a prepared input although the audited catalog has no qualifying declaration")
                elif state == OBLIGATION_STATES[5] and kind in rows and not failed.get(kind):
                    result[kind]["state"] = CATALOG_INAPPLICABLE
        if state == OBLIGATION_STATES[4]:
            result[kind]['refusal_sites'] = refusal_sites(kind, carrying)
    return result


def refusal_sites(kind, carrying):
    """Partition calls (any exact row) and occurrences (each row separately)."""
    inputs = {key: 0 for key in ('exact', 'elsewhere', 'unknown')}
    occurrences = inputs.copy()
    witnesses = []
    for row in carrying:
        count = row['obligations'][kind]
        exact = row.get('exact_sites')
        if exact is None:  # Spec counts, legacy logs, or no validated capture.
            state = 'unknown'
            occurrences[state] += count
        else:
            state = 'exact' if exact[kind] else 'elsewhere'
            occurrences['exact'] += exact[kind]
            occurrences['elsewhere'] += count - exact[kind]
        inputs[state] += 1
        witnesses.append({'site': row['site'], 'fixture_index': row.get('fixture_index'), 'relation': state,
                          'exact_occurrences': None if exact is None else exact[kind], 'occurrences': count})
    return dict(inputs=inputs, occurrences=occurrences, witnesses=witnesses)


def input_table(inputs):
    """'source/standing' -> input counts by lowering outcome, for inputs carrying at least one obligation."""
    table = {}
    for row in inputs:
        if not row["obligations"]:
            continue
        cell = table.setdefault(f"{row['source']}/{row['standing']}",
                                {"inputs": 0, "distinct": set(), "lowered": 0, "refused": 0, "unattributed": 0})
        cell["inputs"] += 1
        cell["distinct"].add(row["site"])
        cell[row["lowering"]] += 1
    return {key: {**cell, "distinct": len(cell["distinct"])} for key, cell in sorted(table.items())}


def earlier_consumers(report, tests):
    """Earlier-layer rules whose registered consumer comparison runs in the L5 suite."""
    try:
        registrations = reviewed_comparisons.registrations()
    except (OSError, ValueError):
        return []  # The comparison registry reports its own failure.
    receipts = report.get("reviewed_comparisons", {})
    rows = []
    for row in registrations:
        if row.suite != "L5":
            continue
        status = tests.get((row.target, row.test))
        rows.append({"owner": row.owner, "rule": row.rule, "side": row.side, "site": f"{row.target}::{row.test}",
                     "comparison": row.comparison, "test": status or "absent",
                     "credited": status == "ok" and bool(receipts.get(row.comparison))
                                 and row.comparison not in report.get("failed_comparisons", {})})
    return rows


def finish(report, directory):
    if report.get("last_layer") != "L5":
        return
    errors = report["measurement_errors"]
    directory = Path(directory)
    tests = test_results(report)
    try:
        log = (directory / "L5.log").read_text(encoding="utf-8", errors="replace")
    except OSError as error:
        errors.append(f"L5 contract outputs: cannot read L5.log: {error}")
        log = ""
    receipts, probe = observe(log, errors)
    for label, observation in probe["examples"].items():
        if observation["status"] == "stopped" and observation["stage"] == "L5":
            sites = report.get("lowering_locations", {}).get(label, {})
            if "fragment" not in sites:
                errors.append(f"{label}: lowering stopped without an observed source position")
            detail = report.get("cases", {}).get(f"spec/{label}", {}).get("detail", "")
            if "machine-wrapped fragment refused:" in detail and not (sites.keys() & {"generated", "context"}):
                errors.append(f"{label}: wrapped fragment refused without an observed source position")
    states = credit_assertions(receipts, tests, errors)
    examples = spec_receipts(log, report, tests, errors)
    for key, case in report["cases"].items():
        if case["cohort"] == "spec":
            receipt = examples.get(key.removeprefix("spec/"))
            case["expectation_check"] = ({"verdict": receipt["verdict"], **receipt["checks"]}
                                         if receipt else {"verdict": "unverified"})
    try:
        entries = contract_entries((LAYER / "CONTRACT.md").read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        errors.append(f"L5 contract inventory: {error}")
        entries = []
    attach(entries, states, tests, errors)
    try:
        lock = obligation_rows((LAYER / "LOCK_EVIDENCE.md").read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        errors.append(f"L5 obligation inventory: {error}")
        lock = []
    inputs = obligation_inputs(report, tests, errors, examples)
    spec_cases = sum(key.startswith("spec/") for key in report.get("cases", {}))
    data = {
        "contract": entries,
        "assertions": {label: {**ASSERTIONS[label]._asdict(), "state": state, "comparison": receipts.get(label)} for label, state in states.items()},
        "outputs": measure_outputs(probe, tests.get(PROBE), report, errors),
        "obligation_inputs": input_table(inputs),
        "spec_aggregate": tests.get(SPEC_AGGREGATE, "absent"),
        "spec_receipts": {"cases": spec_cases, "accepted": len(examples),
                          "verdicts": dict(Counter(row["verdict"] for row in examples.values()))},
        "obligations": classify_obligations(report, inputs, states, lock, errors),
        "earlier_consumers": earlier_consumers(report, tests),
    }
    report["lowering_evidence"] = data
    write_details(directory, data)


def write_details(directory, data):
    lines = ["# L5 contract outputs", "",
             "Each entry is a paragraph of the L5 contract. Sentences are sorted by wording into capabilities, acknowledged limits and statements assigned to the checker or a later consumer. An asserted claim has a receipt from a registered test that passed in this run. A linked test file only shows that the file ran.", ""]
    for row in data["contract"]:
        lines.append(f"## Line {row['line']} · {row['section']} · {row['status']}")
        lines.append("")
        for field, title in (("capabilities", "Capability"), ("limits", "Acknowledged limit"), ("delegated", "Checker or later consumer")):
            lines.extend(f"- {title}: {sentence}" for sentence in row[field])
        for label in row["asserted"]:
            assertion = data["assertions"][label]
            lines.append(f"- Asserted {assertion['role']}: {assertion['claim']} ({assertion['target']}::{assertion['test']})")
            if assertion.get("comparison"):
                lines.append(f"  - `{assertion['comparison']}`")
        for label in row["failed_assertions"]:
            assertion = data["assertions"][label]
            lines.append(f"- Failed {assertion['role']} check: {assertion['claim']} ({assertion['target']}::{assertion['test']})")
        for target, counts in row["target_results"].items():
            observed = ", ".join(f"{status} {count}" for status, count in sorted(counts.items())) or "no results"
            lines.append(f"- Linked test file {target}: {observed}")
        lines.append("")
    outputs = data["outputs"]
    lines += ["## Psi output forms", ""]
    if outputs["measured"]:
        lines += ["Counts come from lowering each published example through the package path. Emission is occurrence only. A form with no occurrence is unmeasured here.", "",
                  "| Form | Occurrences | Examples |", "| --- | ---: | --- |"]
        for form in outputs["inventory"]:
            seen = outputs["emitted"][form]
            lines.append(f"| {form} | {seen['occurrences']} | {', '.join(seen['examples'][:5]) + (' …' if len(seen['examples']) > 5 else '')} |")
    else:
        lines.append(f"Unmeasured. The probe {outputs['probe']} reported {outputs['test']}.")
    lines += ["", "## Typed obligations at L5", "",
              "Inputs are prepared packages carrying the kind into `lower_package`; each helper call counts once, so a repeated call is a repeated input. Occurrences count obligations of the kind in those inputs. A lowered input shows that Psi was produced, not what Psi records for the kind. A property check names one registered assertion and states only its claim.", "",
              "| Kind | Observation | LOCK_EVIDENCE line | Inputs spec / fixture (distinct) | Occurrences | Lowered confirmed / partial / unreported | Refused | Unattributed | Property checks |",
              "| --- | --- | ---: | --- | ---: | --- | ---: | ---: | --- |"]
    for kind, row in data["obligations"].items():
        lowered = " / ".join(str(len(row["lowered"][name])) for name in STANDINGS)
        checks = [f"{label}: {data['assertions'][label]['claim']}" for label in row["property_checks"]]
        checks += [f"{label} (failed)" for label in row["failed_property_checks"]]
        lines.append(f"| {kind} | {row['state']} | {row['lock_row'] or 'none'} | {row['inputs']['spec']} / {row['inputs']['fixture']} ({row['distinct_inputs']}) | "
                     f"{row['occurrences']} | {lowered} | {len(row['refused'])} | {len(row['unattributed'])} | {'; '.join(checks) or 'none'} |")
    if audit := data["obligations"].get(LIBRARY_KIND, {}).get("catalog"):
        lines += ["", f"The library implementation audit examined {audit['machines']} machine declarations. "
                  f"The closed catalog supplies {audit['supplied']}. Its collection primitives publish {audit['collection_guarantees']} "
                  f"guarantee-bearing declarations with fixed semantics. Qualifying implementation declarations: {audit['eligible']}. "
                  "The changed-catalog control adds a guarantee to a supplied ranking machine and checks the resulting library debt and its importer."]
    lines += refusal_site_table(data['obligations'])
    lines += ["", "## Earlier-layer rules checked by an L5 consumer", ""]
    for row in data["earlier_consumers"]:
        verdict = "credited" if row["credited"] else f"not credited (test {row['test']})"
        lines.append(f"- {row['owner']} · {row['rule']} · {row['side']} · {row['site']} · {verdict}")
    if not data["earlier_consumers"]:
        lines.append("No registered comparison runs in the L5 suite.")
    (directory / "lowering.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def paragraphs_without_output_check(data):
    return [row for row in data.get("contract", ()) if row["capabilities"]
            and not any(data["assertions"][label]["role"] == "producer" for label in row["asserted"])]


def summary(report):
    data = report.get("lowering_evidence")
    if not data:
        return []
    entries = data["contract"]
    states = Counter(row["status"] for row in entries)
    sentences = Counter({kind: sum(len(row[kind]) for row in entries) for kind in ("capabilities", "limits", "delegated")})
    assertions = data["assertions"].values()
    credited = [row for row in assertions if row["state"] == "credited"]
    lines = ["", "## L5 contract outputs", "",
             f"The L5 contract has {len(entries)} paragraphs in its introduction and handoff sections. By wording they contain {sentences['capabilities']} capability sentences, {sentences['limits']} acknowledged limits and {sentences['delegated']} statements assigned to the checker or a later consumer. [Lowering evidence](lowering.md)",
             "", "| Evidence for a contract paragraph | Paragraphs |", "| --- | ---: |"]
    lines += [f"| {state} | {states[state]} |" for state in ENTRY_STATES]
    missing = paragraphs_without_output_check(data)
    if missing:
        lines += ["", f"{len(missing)} paragraphs state capabilities and have no verified producer assertion. "
                  + "See CONTRACT.md lines " + ", ".join(str(row["line"]) for row in missing) + "."]
    lines += ["", f"A claim counts as asserted only when its registered test printed a receipt and passed. {len(credited)} of {len(data['assertions'])} registered checks were credited, {sum(row['role'] == 'producer' for row in credited)} on emitted Psi and {sum(row['role'] == 'refusal' for row in credited)} on exact refusals. A passing linked test file does not show which sentence it checks."]
    outputs = data["outputs"]
    if outputs["measured"]:
        inventory = outputs["inventory"]
        emitted = {form for form in inventory if outputs["emitted"][form]["occurrences"]}
        examples = outputs["examples"]
        lines += ["", f"The probe inventories {len(inventory)} Psi output forms. The {examples['produced']} published examples that produced Psi emitted {len(emitted)} of them. {examples['stopped']} examples stopped before Psi and {examples['panicked']} panicked in the probe. Emission records occurrence, not meaning. A form no example emitted is unmeasured; a targeted test may still produce it.",
                  "", "| Form family | Inventoried | Emitted by examples | Unmeasured |", "| --- | ---: | ---: | ---: |"]
        families = Counter(form.split("::")[0] for form in inventory)
        seen = Counter(form.split("::")[0] for form in emitted)
        lines += [f"| {family} | {count} | {seen[family]} | {count - seen[family]} |" for family, count in families.items()]
    else:
        lines += ["", f"Psi output forms are unmeasured. The probe {outputs['probe']} reported {outputs['test']}."]
    if data["obligations"]:
        lines += obligation_summary(data)
    consumers = data["earlier_consumers"]
    if consumers:
        lines += ["", f"{sum(row['credited'] for row in consumers)} of {len(consumers)} earlier-layer consumer comparisons registered in the L5 suite were credited."]
    return lines


def refusal_site_table(kinds):
    refused = [(kind, row) for kind, row in kinds.items() if row['state'] == OBLIGATION_STATES[4]]
    if not refused:
        return []
    lines = ['', '### Where always-refused inputs stopped', '',
             'Exact means the returned refusal has the same package, file and byte range as a prepared obligation. '
             'An input counts as exact if any occurrence of the kind matches; other occurrences still count separately. '
             'Elsewhere means a different site in the same input. Without validated site evidence the relation is unknown. '
             'Co-location does not establish that the obligation caused the refusal. '
             'In `evidence.json`, each fixture index identifies the helper call.', '',
             '| Kind | Inputs: exact / elsewhere / unknown | Occurrences: exact / elsewhere / unknown |',
             '| --- | --- | --- |']

    def cell(counts, total):
        if counts is None:  # Rendering archived evidence.json without the new measurement.
            return f'0 / 0 / {total}'
        return ' / '.join(str(counts[k]) for k in ('exact', 'elsewhere', 'unknown'))

    for kind, row in refused:
        sites = row.get('refusal_sites', {})
        calls = cell(sites.get('inputs'), len(row['refused']))
        occurrences = cell(sites.get('occurrences'), row['occurrences'])
        lines.append(f'| {kind} | {calls} | {occurrences} |')
    return lines


def obligation_summary(data):
    kinds, table = data["obligations"], data["obligation_inputs"]
    names = {"spec": "Spec examples", "fixture": "Helper fixtures"}
    meaning = {"confirmed": "test passed", "partial": "test failed before finishing its checks", "unreported": "no test result"}
    spec_meaning = {"confirmed": "example receipt passed", "partial": "example receipt reports a gap or failure",
                    "unreported": "no valid example receipt"}
    lines = ["", "## Typed obligations at L5", "",
             "Inputs are prepared packages that carried at least one typed obligation into `lower_package`. Reaching L5 is not lowering, and a lowered input is not a check of what Psi records for a kind. Each helper call is one input; distinct inputs merge repeated calls with the same test and files. Tests that call `lower_package` directly are not observed.",
             "", "| Inputs | Emitting test | Inputs (distinct) | Lowered | Refused | Unattributed |", "| --- | --- | ---: | ---: | ---: | ---: |"]
    for key, cell in table.items():
        source, standing = key.split("/")
        emitter = (spec_meaning if source == "spec" else meaning)[standing]
        lines.append(f"| {names[source]} | {emitter} | {cell['inputs']} ({cell['distinct']}) | {cell['lowered']} | {cell['refused']} | {cell['unattributed']} |")
    if any(key.startswith("spec/") for key in table):
        receipts = data.get("spec_receipts", {})
        verdicts = receipts.get("verdicts", {})
        observed = ", ".join(f"{count} {verdict}" for verdict, count in sorted(verdicts.items())) or "none"
        lines += ["", f"The spec aggregate {SPEC_AGGREGATE[0]}::{SPEC_AGGREGATE[1]} reported {data['spec_aggregate']}. "
                  f"Spec standing comes from each example's final receipt, printed after all of its checks ran: "
                  f"{receipts.get('accepted', 0)} valid receipts for {receipts.get('cases', 0)} spec cases ({observed}). "
                  "Only a passed receipt confirms an example; failure text is never read."]
    by_state = {state: [kind for kind, row in kinds.items() if row["state"] == state] for state in OBLIGATION_STATES}
    lines += ["", f"Kinds by strongest observation ({len(kinds)} declared). Absence of an input is unmeasured, not a failure.", "",
              "| Observation | Kinds | Names |", "| --- | ---: | --- |"]
    lines += [f"| {state} | {len(by_state[state])} | {', '.join(by_state[state]) or 'none'} |" for state in OBLIGATION_STATES]
    lines += refusal_site_table(kinds)
    checks = [(kind, label) for kind, row in kinds.items() for label in row["property_checks"]]
    failed = [(kind, label) for kind, row in kinds.items() for label in row["failed_property_checks"]]
    lines += ["", f"Registered Psi property checks credited for an obligation kind: {len(checks)}, covering {len({kind for kind, _ in checks})} of {len(kinds)} kinds. Each credit requires the kind to occur in a source owned by the lowered package in the registered test."]
    lines += [f"- {kind}: {data['assertions'][label]['claim']} ({label})" for kind, label in checks]
    lines += [f"- {kind}: failed check {label}" for kind, label in failed]
    return lines
