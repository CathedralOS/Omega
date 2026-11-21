"""Run from the V2 Cargo workspace: python tools/spec-corpus/check_layer_readiness.py."""
from collections import Counter
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from time import monotonic

from check_frontend_readiness import PRODUCT_CASES, STAGE_TARGETS, STAGE_TESTS, context_launch_error, context_results
import frontend_diagnostics
import frontend_fixture_cli
import diagnostic_kinds
from diagnostic_fields import unescape
import handoff_evidence
import handoff_closures
import lowering_evidence
import lowering_origins
import lowering_sites
import lowering_cli
import preparation_reason_evidence
import test_inventory
import typing_sites
import typing_reason_evidence
import product_controls
import refusal_evidence
import reviewed_comparisons
import rule_diagnostics
import sample_diagnostics
import resolver_kind_evidence

LAYERS = ("omega-lexer", "omega-parser", "omega-resolver", "omega-typer", "omega-preparation", "omega-lowering")
TEST = test_inventory.TEST_RESULT
RESULT = re.compile(r"^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored")
BLOCKERS = {"open gap", "checker input blocked", "unresolved refusal", "inspection failed", "compatibility blocked", "missing context", "executable product uninspected", "unexpected Psi product"}
CASE_STATUSES = BLOCKERS | {"recorded refusal", "executable product inspected", "declaration or empty product", "intended rejection verified", "expected L6 refusal unverified", "contextual product inspected"}


def git(*args):
    return subprocess.check_output(["git", *args], text=True, encoding="utf-8").strip()


def source_fingerprint():
    digest = hashlib.sha256()
    digest.update(subprocess.check_output(["git", "diff", "--binary", "HEAD", "--", "."]))
    digest.update(subprocess.check_output(["git", "diff", "--binary", "HEAD", "--", ":(top)wiki/spec"]))
    paths = subprocess.check_output(["git", "ls-files", "-z", "--others", "--exclude-standard", "--", "."])
    for name in paths.split(b"\0"):
        if name:
            path = Path(os.fsdecode(name))
            digest.update(name + b"\0")
            digest.update(hashlib.sha256(os.fsencode(os.readlink(path)) if path.is_symlink() else path.read_bytes()).digest())
    return digest.hexdigest()


def reflection_exports(lines, errors):
    package = "@omega/toolchain"
    expected = {rf"language\core\reflect\{member}" for member in ("field", "callable", "FieldInfo", "CallableInfo")}
    observed = {}
    for line in lines:
        if not line.startswith("OMEGA_CONTEXT_EXPORT"):
            continue
        fields = line.split("\t")
        if (len(fields) != 5 or fields[:3] != ["OMEGA_CONTEXT_EXPORT", "language/reflection.md:77", package]
                or fields[3] not in expected or fields[3] in observed or not re.fullmatch(r"absent|[A-Z][A-Za-z]*(?:,[A-Z][A-Za-z]*)*", fields[4])):
            errors.append(f"invalid or duplicate contextual export observation: {line}")
            continue
        observed[fields[3]] = [] if fields[4] == "absent" else fields[4].split(",")
    for path in sorted(expected - observed.keys()):
        errors.append(f"missing contextual export observation: {package} {path}")
    return {"package": package, "members": observed}


def contextual_examples(report, directory, environment):
    results = {}
    for stage, target in STAGE_TARGETS.items():
        for test in report["suites"][stage]["tests"]:
            if test["target"].replace("\\", "/") == target:
                results.setdefault((stage, test["test"]), []).append(test["status"])
    for label, witnesses in STAGE_TESTS.items():
        outcomes = {f"{stage} {STAGE_TARGETS[stage]}::{name}": results.get((stage, name), []) for stage, name in witnesses}
        unavailable = [name for name, results in outcomes.items() if results not in (["ok"], ["FAILED"])]
        failed = [name for name, results in outcomes.items() if "FAILED" in results]
        if unavailable:
            report["measurement_errors"].append(f"{label}: missing unique executed test result for {', '.join(unavailable)}")
        status = "contextual witnesses verified"
        if failed:
            status = "inspection failed"
        elif unavailable:
            status = "test evidence unavailable"
        report["cases"]["context/" + label] = {
            "cohort": "context", "stage": "/".join(sorted({stage for stage, _ in witnesses})), "status": status,
            "detail": ", ".join(f"{name} ({', '.join(results) or 'absent'})" for name, results in outcomes.items()),
        }
    command = ["mbx", "run", "--locked", "-p", "spec-corpus", "--features", "readiness", "--bin", "published_readiness"]
    log = directory / "context.log"
    launch_error = None
    try:
        with log.open("wb") as stream:
            code = subprocess.run(command, env=environment, stdin=subprocess.DEVNULL, stdout=stream, stderr=subprocess.STDOUT).returncode
    except OSError as error:
        code = 1
        launch_error = str(error)
        log.write_text(launch_error, encoding="utf-8")
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()
    report["context_command"] = {"command": command, "exit": code}
    launch_error = launch_error or context_launch_error(code, lines)
    if launch_error:
        report["context_command"]["launch_error"] = launch_error
        report["measurement_errors"].append(f"published_readiness could not start: {launch_error}")
        for label in PRODUCT_CASES:
            report["cases"]["context/" + label] = {
                "cohort": "context", "stage": "runner", "status": "probe did not start", "detail": launch_error,
            }
        return
    exports = reflection_exports(lines, report["measurement_errors"])
    for label in PRODUCT_CASES:
        passed, details, error = context_results(label, lines)
        if error:
            report["measurement_errors"].append(error)
        stops = [re.match(r"^FAIL .*? — (L[0-4])\b", detail) for detail in details if detail.startswith("FAIL ")]
        stages = sorted({match[1] for match in stops if match})
        observed = "/".join(stages) if stops and all(stops) else "front end"
        report["cases"]["context/" + label] = {
            "cohort": "context", "stage": observed,
            "status": "contextual witnesses verified" if passed else "inspection failed",
            "detail": " / ".join(details) or "see context.log",
        }
        if label == "language/reflection.md:77":
            report["cases"]["context/" + label]["toolchain_exports"] = exports
    if code and all("PASS " + label in lines for label in PRODUCT_CASES):
        report["measurement_errors"].append(f"context command exited {code} after reporting every case passed")


def collect(output, stage, report, *, require_lowering_origins=False):
    cases = report["cases"]
    errors = report["measurement_errors"]
    counts = Counter()
    results = []
    target = ""
    failure_output = {}
    current_test = None
    captures = Counter()
    invalid_fixtures = set()
    malformed_fixtures = 0
    first_fixture = len(report.get("package_fixtures", ()))

    def malformed_fixture():
        nonlocal malformed_fixtures
        malformed_fixtures += 1
        if current_test:
            invalid_fixtures.add((target.replace("\\", "/"), current_test[1]))

    # Each L5 record is credited only inside the captured output of the test that emits it.
    spec_emitter = ("tests/spec_corpus.rs", "spec_fragments_have_reviewed_lowering_outcomes")
    sample_emitter = ("tests/sample_baseline.rs", "fixed_sample_configurations_through_l5")
    cohort_emitters = {"spec": spec_emitter, "sample": sample_emitter}
    source_emitters = {"historical_samples": sample_emitter}
    layers = {f"L{i}" for i in range(6)}
    before_lowering = layers - {"L5"}
    # Statuses and stages each emitter can print. Validation only; the status meanings stay with the emitters.
    case_rules = {
        "spec": {"recorded refusal": before_lowering, "inspection failed": before_lowering | {"unattributed"},
                 "open gap": {"L5"}, "missing context": {"L5"}, "executable product inspected": {"L5"},
                 "declaration or empty product": {"L5"}, "contextual product inspected": {"L5"}, "executable product uninspected": {"L5"}},
        "sample": {"intended rejection verified": layers, "compatibility blocked": layers, "expected L6 refusal unverified": {"L5"},
                   "executable product inspected": {"L5"}, "unexpected Psi product": {"L5"}, "missing context": {"setup"}, "inspection failed": layers},
        "handoff": {"open gap": {"L5"}},
    }
    spec_label = re.compile(r"[^\t]+\.md:[1-9]\d*")
    kind_name = re.compile(r"[A-Za-z]+")
    positive = re.compile(r"[1-9]\d*")
    baseline_directory = Path("layers/05_prepared-trees-to-terminal-psi/tests")
    baseline = {}
    sample_rows = []
    sample_refusals = {}

    def emitting_test(kind, line, required=None):
        """Return the (target, test) printing this record, or None after reporting why it cannot be credited."""
        if stage != "L5":
            errors.append(f"{stage}: {kind} outside the L5 suite: {line}")
            return None
        if current_test is None:
            errors.append(f"{stage}: {kind} outside a captured test output: {line}")
            return None
        where = (current_test[0].replace("\\", "/"), current_test[1])
        if required and where != required:
            errors.append(f"{stage}: {kind} printed by {where[0]}::{where[1]}, expected {required[0]}::{required[1]}: {line}")
            return None
        if not required and not re.fullmatch(r"tests/[^/]+\.rs", where[0]):
            errors.append(f"{stage}: {kind} printed outside an L5 integration test ({where[0]}::{where[1]}): {line}")
            return None
        return where

    def sample_baseline():
        """Labels and outcomes of the versioned fixed-sample baseline, or None when it cannot be read."""
        if "rows" not in baseline:
            baseline["rows"] = None
            path = baseline_directory / "sample_baseline" / "observations.tsv"
            try:
                text = path.read_text(encoding="utf-8")
                revision = re.search(r'^const REVISION: &str = "([0-9a-f]{40})";$',
                                     (baseline_directory / "sample_baseline.rs").read_text(encoding="utf-8"), re.M)
            except OSError as error:
                errors.append(f"cannot read the versioned sample baseline: {error}")
                return None
            rows = {}
            baseline["refusals"] = {}
            for row in (line.split("\t") for line in text.split("\n") if line and not line.startswith("#")):
                if len(row) < 2 or not row[0] or row[0] in rows:
                    errors.append(f"{path}: malformed or duplicate baseline row {chr(9).join(row)}")
                    continue
                if row[1] in {"psi", "setup"} and len(row) == 3:
                    rows[row[0]] = ("L5" if row[1] == "psi" else "setup", row[2])
                    continue
                try:
                    site = sample_diagnostics.decode(["OMEGA_SAMPLE_REFUSAL", *row])
                except ValueError as error:
                    errors.append(f"{path}: invalid baseline refusal for {row[0]}: {error}")
                    continue
                rows[row[0]] = (row[1], sample_diagnostics.rendered(site))
                baseline["refusals"][row[0]] = site
            baseline["rows"] = rows
            baseline["revision"] = revision[1] if revision else None
        return baseline["rows"]

    # Rust writes LF-delimited records. Unicode line separators may belong to the source or reason.
    for physical_line in output.split("\n"):
        line = physical_line.removesuffix("\r")
        header = re.match(r"^\s*Running (.+?) \(", line)
        if header:
            target = header[1]
            current_test = None
        elif line.lstrip().startswith("Doc-tests "):
            target = "doctests"
            current_test = None
        if captured := re.match(r"^---- (.+?) stdout ----$", line):
            current_test = (target, captured[1])
            captures[target.replace("\\", "/"), captured[1]] += 1
            failure_output[current_test] = []
        elif not line or line in {"successes:", "failures:"} or RESULT.match(line) or re.fullmatch(r"---- .+? stderr ----", line):
            if line:
                current_test = None
        elif current_test and not line.startswith("OMEGA_"):
            failure_output[current_test].append(line)
        if current_test is None and (match := TEST.match(line)):
            results.append({"target": target, "test": match[1], "status": match[2]})
        if match := RESULT.match(line):
            counts.update(dict(zip(("passed", "failed", "ignored"), map(int, match.groups()))))
        if line.startswith(("OMEGA_RESOLVER_KIND", "OMEGA_RESOLVER_INVENTORY")):
            resolver_kind_evidence.record(report, stage, target, current_test, line)
        if line.startswith(preparation_reason_evidence.PREFIX):
            preparation_reason_evidence.record(report, stage, target, current_test, line)
        if line.startswith(typing_reason_evidence.PREFIX):
            typing_reason_evidence.record(report, stage, target, current_test, line)
        if line.startswith("OMEGA_TYPING_RETURN"):
            typing_sites.record_return(report, stage, target, current_test, line)
        if line.startswith(("OMEGA_IDENTITY_CONTROL\t", "OMEGA_CONSTANT_IDENTITY\t", "OMEGA_GOLDEN_MATCH\t", "OMEGA_HANDOFF_ASSERT\t")):
            handoff_evidence.record(report, stage, target, current_test, line)
        elif line.startswith(("OMEGA_REVIEWED_COMPARISON\t", "OMEGA_FAILED_COMPARISON\t")):
            reviewed_comparisons.record(report, stage, target, current_test, line)
            if line.startswith("OMEGA_FAILED_COMPARISON\t") and current_test:
                failure_output[current_test].append(line)
        if line.startswith("OMEGA_SPEC_LOCATION\t"):
            fields = line.split("\t")
            if (len(fields) != 7 or stage != "L5" or target.replace("\\", "/") != "tests/spec_corpus.rs"
                    or not current_test or current_test[1] != "spec_fragments_have_reviewed_lowering_outcomes"
                    or not spec_label.fullmatch(fields[1]) or fields[2] not in {"fragment", "context", "generated"}):
                errors.append(f"malformed or misplaced spec location: {line}")
                continue
            _, label, input_kind, row, column, start, end = fields
            try:
                location = frontend_diagnostics.location("example.omg", row, column, start, end)
                if input_kind != "generated":
                    location["spec_line"] = int(label.rsplit(":", 1)[1]) + location["line"]
            except (ValueError, KeyError):
                errors.append(f"invalid spec source position: {line}")
                continue
            sites = report.setdefault("lowering_locations", {}).setdefault(label, {})
            if input_kind in sites:
                errors.append(f"duplicate spec location for {label}: {input_kind}")
            sites[input_kind] = location
        if line.startswith("OMEGA_NEGATIVE_CONTROL\t"):
            product_controls.record(report, stage, target, current_test, line)
        if line.startswith("OMEGA_CASE\t"):
            fields = line.split("\t", 5)
            if len(fields) != 6 or not all(fields[1:]) or fields[1] not in case_rules or fields[4] not in CASE_STATUSES:
                errors.append(f"{stage}: malformed case result {line}")
                continue
            _, cohort, label, reached, status, detail = fields
            if cohort == "spec":
                try:
                    detail = unescape(detail).decode("utf-8")
                except ValueError as error:
                    errors.append(f"{stage}: invalid diagnostic text for {label}: {error}")
                    continue
            reported_stage = reached
            stop = frontend_diagnostics.compiler_stop(reached) if reached == "L3-resumed" or reached in layers else None
            if stop:
                reached = stop["stage"]
            if reached not in case_rules[cohort].get(status, ()):
                errors.append(f"{stage}: {cohort} emitter cannot report {status} at {reached}: {line}")
                continue
            where = emitting_test("case result", line, cohort_emitters.get(cohort))
            if where is None:
                continue
            if cohort == "spec" and not spec_label.fullmatch(label):
                errors.append(f"{stage}: unrecognized spec fragment label {line}")
                continue
            if cohort == "sample":
                rows = sample_baseline()
                if rows is not None and label not in rows:
                    errors.append(f"{stage}: sample label absent from the versioned baseline: {line}")
                    continue
            key = f"{cohort}/{label}"
            if key in cases:
                errors.append(f"duplicate result for {key}")
                continue
            cases[key] = {"cohort": cohort, "stage": reached, "status": status, "detail": detail,
                          "target": current_test[0], "test": current_test[1]}
            if cohort == "sample":
                sample_rows.append((label, reported_stage, detail, where))
            if stop and (status in {"recorded refusal", "compatibility blocked", "open gap", "intended rejection verified"}
                         or status == "inspection failed" and (cohort == "sample" or reached != "L5")):
                cases[key].update(stop)
        elif line.startswith("OMEGA_SAMPLE_REFUSAL"):
            if emitting_test("sample refusal", line, sample_emitter) is None:
                continue
            try:
                label, site = sample_diagnostics.parse(line)
                if label in sample_refusals:
                    raise ValueError(f"duplicate refusal for {label}")
                sample_refusals[label] = site
            except ValueError as error:
                errors.append(f"{stage}: invalid sample refusal: {error}: {line}")
        elif line.startswith("OMEGA_GAP_LOCATION\t"):
            where = emitting_test("gap location", line)
            if where:
                try:
                    _, label, package, file, start, end, row, col, text = line.split("\t")
                    case = cases["handoff/" + label]
                    if (case["target"].replace("\\", "/"), case["test"]) != where or case.get("fixture_locations") or not all((package, file, row, col)):
                        raise ValueError("location does not belong to this gap")
                    site = {"package": package, "occurrence": text, **frontend_diagnostics.location(file, row, col, start, end)}
                    case["fixture_locations"] = [site]
                except (KeyError, ValueError) as error:
                    errors.append(f"{stage}: invalid gap location in {where[0]}::{where[1]}: {error}")
        elif line.startswith("OMEGA_EXPECTED\t"):
            fields = line.split("\t")
            if len(fields) != 3 or fields[1] not in cohort_emitters or not positive.fullmatch(fields[2]):
                errors.append(f"{stage}: malformed or unrecognized expected count {line}")
                continue
            if emitting_test("expected count", line, cohort_emitters[fields[1]]) is None:
                continue
            if fields[1] in report["expected"]:
                errors.append(f"{stage}: duplicate expected count {line}")
                continue
            report["expected"][fields[1]] = int(fields[2])
            if fields[1] == "sample" and (rows := sample_baseline()) is not None and len(rows) != int(fields[2]):
                errors.append(f"{stage}: sample count {fields[2]} disagrees with {len(rows)} versioned baseline rows")
        elif line.startswith("OMEGA_SOURCE\t"):
            fields = line.split("\t")
            if len(fields) != 3 or fields[1] not in source_emitters or not re.fullmatch(r"[0-9a-f]{40}", fields[2]):
                errors.append(f"{stage}: malformed or unrecognized source identity {line}")
                continue
            if emitting_test("source identity", line, source_emitters[fields[1]]) is None:
                continue
            sources = report.setdefault("cohort_sources", {})
            if fields[1] in sources:
                errors.append(f"{stage}: duplicate source identity {line}")
                continue
            sample_baseline()
            if baseline.get("revision") != fields[2]:
                errors.append(f"{stage}: source identity disagrees with the versioned sample revision {baseline.get('revision')}: {line}")
                continue
            sources[fields[1]] = fields[2]
        elif line.startswith("OMEGA_INVENTORY\t"):
            fields = line.split("\t")
            kinds = fields[1].split(",") if len(fields) == 2 and fields[1] else []
            if not kinds or not all(map(kind_name.fullmatch, kinds)) or len(set(kinds)) != len(kinds):
                errors.append(f"{stage}: malformed obligation inventory {line}")
                continue
            if emitting_test("obligation inventory", line, spec_emitter) is None:
                continue
            if report["obligations"]:
                errors.append(f"{stage}: duplicate obligation inventory {line}")
                continue
            report["obligations"].update({name: {} for name in kinds})
        elif line.startswith("OMEGA_OBLIGATION\t"):
            fields = line.split("\t")
            if len(fields) != 4 or not spec_label.fullmatch(fields[1]) or fields[2] not in report["obligations"] or not positive.fullmatch(fields[3]):
                errors.append(f"{stage}: malformed or undeclared obligation {line}")
                continue
            if emitting_test("obligation count", line, spec_emitter) is None:
                continue
            if fields[1] in report["obligations"][fields[2]]:
                errors.append(f"{stage}: duplicate obligation count {line}")
                continue
            report["obligations"][fields[2]][fields[1]] = int(fields[3])
        elif line.startswith("OMEGA_FIXTURE"):
            fields = line.split("\t")
            # A replay frame follows the program site and optional Rust origin.
            replay = None
            for boundary in (12, 15):
                if fields[boundary:boundary + 1] == ["replay-v1"]:
                    replay, fields = fields[boundary + 1:], fields[:boundary]
                    break
            position_fields, origin_fields, fields = fields[5:12], fields[12:], fields[:5]
            pairs = fields[3].split(",") if len(fields) == 5 and fields[3] else []
            kinds = [pair.split("=")[0] for pair in pairs]
            if (len(fields) != 5 or fields[0] != "OMEGA_FIXTURE" or not all(fields[1:3]) or len(set(kinds)) != len(kinds)
                    or any(not re.fullmatch(r"[A-Za-z]+=[1-9]\d*", pair) for pair in pairs)
                    or not (fields[4] == "produced Psi" or re.fullmatch(r"refused: \S.*", fields[4]))):
                malformed_fixture()
                errors.append(f"{stage}: malformed package fixture {line}")
                continue
            where = emitting_test("package fixture", line)
            if where is None:
                malformed_fixture()
                continue
            if fields[1] != where[1]:
                malformed_fixture()
                errors.append(f"{stage}: package fixture names {fields[1]} but was printed by {where[0]}::{where[1]}: {line}")
                continue
            obligations = {kind: int(n) for kind, n in (pair.split("=") for pair in pairs)}
            fixture = {"target": current_test[0], "test": fields[1], "files": fields[2], "obligations": obligations, "outcome": fields[4]}
            if position_fields:
                try:
                    package, file, start, end, row, column, occurrence = position_fields
                    if not package or not file or not fields[4].startswith("refused: ") or not fields[4].endswith(" at " + file):
                        raise ValueError("location disagrees with lowering outcome")
                    if package == "app" and (file not in fields[2].split(",") or not row):
                        raise ValueError("local refusal lacks its original source")
                    fixture["location"] = {"package": package, "occurrence": occurrence, **frontend_diagnostics.location(file, row, column, start, end)}
                except ValueError as error:
                    malformed_fixture()
                    errors.append(f"{stage}: invalid fixture location in {where[0]}::{where[1]}: {error}")
                else:
                    if origin_fields:
                        try:
                            if (not all(re.fullmatch(r"0|[1-9][0-9]*", n) for n in (start, end))
                                    or row and not all(lowering_origins.POSITIVE.fullmatch(n) for n in (row, column))):
                                raise ValueError("noncanonical program coordinates")
                            fixture["reported_compiler_site"] = lowering_origins.decode(origin_fields)
                        except ValueError as error:
                            malformed_fixture()
                            errors.append(f"{stage}: invalid compiler origin in {where[0]}::{where[1]}: {error}")
                    if replay is not None:
                        try:
                            fixture["replay"] = lowering_origins.decode_replay(replay, fixture)
                        except ValueError as error:
                            malformed_fixture()
                            errors.append(f"{stage}: invalid replay inputs in {where[0]}::{where[1]}: {error}")
            report["package_fixtures"].append(fixture)
    # A passing comparison test must have printed the outcome it compared with the versioned baseline.
    statuses = {}
    for test in results:
        statuses.setdefault((test["target"].replace("\\", "/"), test["test"].removesuffix(" - should panic")), []).append(test["status"])
    for label, reached, detail, where in sample_rows:
        recorded = (baseline.get("rows") or {}).get(label)
        if recorded and statuses.get(where) == ["ok"] and (reached, detail) != recorded:
            errors.append(f"{stage}: sample/{label} reported {reached} · {detail}, but its passing test compared {recorded[0]} · {recorded[1]}")
        elif recorded and statuses.get(where) == ["ok"]:
            pinned = baseline["refusals"].get(label)
            actual = sample_refusals.get(label)
            if pinned and actual and "invalid" not in actual and pinned != actual:
                errors.append(f"{stage}: sample/{label} compiler refusal differs from its pinned source bytes or diagnostic")
    if stage == "L5":
        lowering_origins.finish(report, report.get("package_fixtures", ())[first_fixture:], statuses, captures,
                                invalid_fixtures, malformed_fixtures, require_lowering_origins)
        sample_diagnostics.attach(cases, sample_refusals, errors)
    if not results or not counts:
        report["measurement_errors"].append(f"{stage}: missing Rust test results")
    if counts["failed"] != sum(test["status"] == "FAILED" for test in results):
        report["measurement_errors"].append(f"{stage}: failed test count disagrees with named failures")
    if counts["passed"] != sum(test["status"] == "ok" for test in results):
        report["measurement_errors"].append(f"{stage}: passed test count disagrees with named successes")
    for test in results:
        if test["status"] == "FAILED":
            test["diagnostic"] = failure_output.get((test["target"], test["test"].removesuffix(" - should panic")), [])
    return results, dict(counts)


def failure_excerpt(lines):
    # libtest captures ordinary output and caught panics in the same block. The
    # last panic carries the failing assertion and Rust's actual/expected values.
    start = next((i for i in reversed(range(len(lines))) if " panicked at " in lines[i]), 0)
    excerpt = []
    for line in lines[start:]:
        if line.startswith(("note: run with `RUST_BACKTRACE", "stack backtrace:")):
            break
        if line.strip() and not line.startswith("OMEGA_FAILED_COMPARISON\t"):
            excerpt.append(line)
    labels = [line.split("\t", 2)[1] for line in lines if line.startswith("OMEGA_FAILED_COMPARISON\t")]
    return [*(f"Failed comparison: {label}" for label in dict.fromkeys(labels)), *excerpt]


def summarize(report):
    failed = sum(suite["counts"].get("failed", 0) for suite in report["suites"].values())
    blocked = sum(case_blocks_readiness(row) for row in report["cases"].values())
    verdict = "Blocked" if readiness_exit(report) else "Passed"
    gaps = report["unverified_contracts"]
    deferred = sum(row.get("consumer_scope") == "later suite" and row["missing"] == ["consumer"] for row in gaps)
    lines = ["# Compiler layer measurements", "",
             f"**{verdict} through {report['last_layer']}.** Failed tests across layers and package commands: {failed}. Cases blocking readiness: {blocked}. Incomplete measurements: {len(report['measurement_errors'])}. "
             f"Of {len(gaps)} L0–L4 contract entries awaiting evidence at this cutoff, {deferred} have their outstanding comparison in a later suite.",
             "",
             f"Revision {report['revision']}. Working changes, source fingerprint and per-test results are in [evidence.json](evidence.json).",
             "", "| Suite | Passed | Failed | Ignored | Runner exit | Log |",
             "| --- | ---: | ---: | ---: | ---: | --- |"]
    for stage, suite in report["suites"].items():
        counts = suite["counts"]
        log = suite.get("log", f"{stage}.log")
        lines.append(f"| {stage} {suite['crate']} | {counts.get('passed', 0)} | {counts.get('failed', 0)} | {counts.get('ignored', 0)} | {suite['exit']} | [{stage}]({log}) |")
    launch_failures = [(stage, suite, issue) for stage, suite in report["suites"].items() for issue in suite.get("launch_failures", ())]
    probe_error = report.get("context_command", {}).get("launch_error")
    if launch_failures or probe_error:
        lines += ["", "## Executables that could not start", ""]
        if probe_error:
            lines.append(f"- Context examples · published_readiness · {sample_diagnostics.readable(probe_error)}. [Runner log](context.log)")
        for stage, suite, issue in launch_failures:
            site = issue["target"] + (f"::{issue['test']}" if issue.get("test") else "")
            log = suite["roster"]["log"] if issue["action"] == "list" else suite.get("log", f"{stage}.log")
            lines.append(f"- {stage} · {issue['action']} · {site} · {sample_diagnostics.readable(issue['reason'])}. [Runner log]({log})")
    if "lowering_evidence" in report:
        obligations = report["lowering_evidence"]["obligations"]
        checked = sum(row["state"] == lowering_evidence.OBLIGATION_STATES[0] for row in obligations.values())
        lines.extend(["", f"Of {len(obligations)} typed obligation kinds, {checked} have an observed check of a specific Psi property. "
                      "The report lists what reached lowering and what each output check establishes in [lowering.md](lowering.md)."])
    rosters = [f"[{stage}]({suite['roster']['log']})" for stage, suite in report["suites"].items() if "roster" in suite]
    if rosters:
        lines += ["", "Each suite lists its selected tests independently. Missing, extra, repeated and ignored tests appear under incomplete measurements. " + " · ".join(rosters)]
    lines.extend(rule_diagnostics.summary(report))
    lines.extend(typing_sites.summary(report))
    lines += ["", "## Failed tests", ""]
    for stage, suite in report["suites"].items():
        for test in suite["tests"]:
            if test["status"] == "FAILED":
                log = suite.get("log", f"{stage}.log")
                lines.append(f"- {stage} · {test['target']} · {test['test']}. [Diagnostic]({log})")
                target = re.fullmatch(r"tests[/\\](.+?)\.rs", test["target"])
                if target and "/" not in target[1] and "\\" not in target[1]:
                    name = test["test"].removesuffix(" - should panic")
                    lines.append(f"  Rerun with `mbx test --locked -p {suite['crate']} --test {target[1]} {name} -- --exact`.")
                excerpt = failure_excerpt(test.get("diagnostic", ()))
                if excerpt:
                    lines.extend(["", *["      " + message for message in excerpt], ""])
    if not any(suite["counts"].get("failed", 0) for suite in report["suites"].values()):
        lines.append("No individual test reported a failure.")
    lines += ["", "## Programs and spec fragments", "",
              "The stage identifies the producer of the refusal or the inspected product. An L3 refusal can follow L4 static evaluation. An isolated fragment can require declarations from its surrounding package. Contextual examples have separate positive and negative witnesses. [Front-end product checks](context.log)",
              "", "| Cohort | Stage | Observed outcome | Cases |", "| --- | --- | --- | ---: |"]
    groups = Counter((v["cohort"], v["stage"], v["status"]) for v in report["cases"].values())
    for (cohort, stage, status), count in sorted(groups.items()):
        lines.append(f"| {cohort} | {stage} | {status} | {count} |")
    lines += ["", "### Open gaps, blocked compatibility and missing context", ""]
    for key, observation in report["cases"].items():
        if observation["status"] in BLOCKERS or case_blocks_readiness(observation):
            lines.append(f"- {key} · {observation['stage']} · {observation['status']} · {sample_diagnostics.readable(observation['detail'])}")
            if check := observation.get("expectation_check"):
                lines.append("  - Example checks · " + ", ".join(f"{key} = {value}" for key, value in check.items()))
            if exports := observation.get("toolchain_exports"):
                for path, kinds in exports["members"].items():
                    lines.append(f"  - {exports['package']} · {path} · " + (", ".join(kinds) if kinds else "absent from the supplied public interface"))
            for site in observation.get("fixture_locations", ()):
                position = f"{site['file']}:{site['line']}:{site['byte_column']}" if "line" in site else site["file"]
                lines.append(f"  - {site['package']}/{position} · bytes {site['byte_start']}..{site['byte_end']} · {site['occurrence']}")
            if key.startswith("spec/"):
                chapter, _, _ = key[5:].rpartition(":")
                for variant, site in report.get("lowering_locations", {}).get(key[5:], {}).items():
                    position = f"{chapter}:{site['spec_line']}" if "spec_line" in site else f"generated wrapper:{site['line']}"
                    lines.append(f"  - {variant} · {position}, byte column {site['byte_column']}, bytes {site['byte_start']}..{site['byte_end']}")
    lines.extend(handoff_closures.summary(report))
    lines.extend(sample_diagnostics.summary(report["cases"]))
    lines.extend(frontend_diagnostics.section(report))
    lines.extend(handoff_evidence.summary(report))
    if report["last_layer"] == "L4":
        if report["measurement_errors"]:
            lines += ["", "## Incomplete measurements", "", *["- " + error for error in report["measurement_errors"]]]
        return "\n".join(lines) + "\n"
    agreement = report["fragment_agreement"]
    lines.extend(["", f"The frontend and lowering runs both inspected {agreement['compared']} published fragments. "
                  f"Disagreements or missing fragments: {len(agreement['differences'])}. Each refusal is compared by producing layer, typing phase, source location and reason."])
    lines.extend(lowering_evidence.summary(report))
    lines.extend(refusal_evidence.summary(report))
    lines.extend(lowering_origins.summary(report))
    lines.extend(lowering_sites.summary(report))
    lines.extend(lowering_cli.summary(report))
    controls = report["product_controls"]
    lines += ["", f"Psi corruption checks rejected {sum(row['verified'] for row in controls.values())} "
              f"of {len(controls)} registered changes to branch targets, call operands, restoration or residual cleanup. "
              "Each check first accepts the real product. The evidence JSON records why it rejected each changed product."]
    if report["measurement_errors"]:
        lines += ["", "## Incomplete measurements", "", *["- " + error for error in report["measurement_errors"]]]
    return "\n".join(lines) + "\n"


def incomplete_cohorts(report):
    for cohort in ("spec", "sample"):
        count = sum(row["cohort"] == cohort for row in report["cases"].values())
        if count != report["expected"].get(cohort):
            yield f"{cohort}: observed {count} results, expected {report['expected'].get(cohort, 'an inventory')}"
    if not report["obligations"]:
        yield "missing prepared obligation inventory"
    # The aggregate prints counts after preparation and its case row after lowering returns or panics.
    for label in sorted({label for hits in report["obligations"].values() for label in hits}):
        case = report["cases"].get("spec/" + label)
        if case is None:
            yield f"spec/{label}: obligation counts without a case result"
        elif case["stage"] not in {"L5", "unattributed"}:
            yield f"spec/{label}: obligation counts printed after preparation, but the case stopped at {case['stage']}"
    if not report.get("package_fixtures"):
        yield "missing observations from the shared L5 test helper"
    for fixture in report.get("package_fixtures", ()):
        if fixture["outcome"].startswith("refused: ") and "location" not in fixture:
            yield f"{fixture['test']}: lowering refusal without its source location"
        for unknown in fixture["obligations"].keys() - report["obligations"].keys():
            yield f"{fixture['test']}: undeclared obligation {unknown}"


def compare_fragment_runs(report):
    frontend = report["frontend_diagnostics"]["fragments"]
    through_lowering = {key.removeprefix("spec/"): row for key, row in report["cases"].items() if row["cohort"] == "spec"}
    differences = {}
    for label in sorted(frontend.keys() | through_lowering.keys()):
        earlier, later = frontend.get(label), through_lowering.get(label)
        if earlier is None or later is None:
            differences[label] = f"fragment missing from {'frontend' if earlier is None else 'lowering'} run"
            continue
        if earlier["status"] == "prepared":
            if later["stage"] != "L5":
                differences[label] = f"frontend prepared the program; lowering run reported {later['stage']} · {later['detail']}"
            continue
        if earlier["package"] == "example" and earlier["file"] == "example.omg" and "line" in earlier:
            where = f"{earlier['file']}:{earlier['line']}:{earlier['byte_column']}"
            message = f"{where}: {earlier['reason']} (bytes {earlier['byte_start']}..{earlier['byte_end']})"
        else:
            message = f"{earlier['package']}/{earlier['file']}: {earlier['reason']} (bytes {earlier['byte_start']}..{earlier['byte_end']})"
        if (later["stage"] != earlier["stage"] or later.get("after_static_evaluation", False) != earlier.get("after_static_evaluation", False)
                or later["detail"] != message or later["status"] not in {"recorded refusal", "inspection failed"}):
            differences[label] = (f"frontend reported {frontend_diagnostics.phase_label(earlier)} · {message}. "
                                  f"Lowering run reported {frontend_diagnostics.phase_label(later)} · {later['detail']}")
    return {"compared": len(frontend.keys() & through_lowering.keys()), "differences": differences}


def case_blocks_readiness(case):
    check = case.get("expectation_check")
    if check and check["verdict"] not in {"passed", "recorded refusal", "known gap"}:
        return True
    if case["status"] == "missing context":
        return case["cohort"] == "sample"
    return case["status"] in BLOCKERS


def readiness_exit(report):
    blocked = any(case_blocks_readiness(c) for c in report["cases"].values())
    lowering = report.get("lowering_evidence", {})
    pending_obligations = any(row["state"] != lowering_evidence.OBLIGATION_STATES[0]
                              for row in lowering.get("obligations", {}).values())
    missing_outputs = lowering_evidence.paragraphs_without_output_check(lowering)
    return int(bool(report["measurement_errors"] or any(s["exit"] for s in report["suites"].values())
                    or blocked or report.get("unverified_contracts") or pending_obligations or missing_outputs
                    or ("CLI" in report["suites"] and diagnostic_kinds.blocks(report.get("frontend_diagnostics", {}).get("diagnostic_kinds")))
                    or any(not rows for rows in report.get("resolver_diagnostic_kinds", {}).values())
                    or typing_reason_evidence.blocks(report.get("typing_sites", {}).get("typing_reasons"))
                    or preparation_reason_evidence.blocks(report.get("preparation_reasons"))))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before-l5", action="store_true", help="measure diagnostics and handoffs from lexing through preparation")
    options = parser.parse_args()
    layers = LAYERS[:5] if options.before_l5 else LAYERS
    if not Path("tools/spec-corpus/check_layer_readiness.py").is_file() or not Path("Cargo.toml").is_file():
        sys.exit("Start in the V2 Cargo workspace.")
    parent = Path("target/layer-readiness")
    parent.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix=datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ-"), dir=parent))
    report = {"revision": git("rev-parse", "HEAD"), "cohort_sources": {"spec_tree_at_head": git("rev-parse", "HEAD:wiki/spec")},
              "fingerprint": source_fingerprint(), "working_changes": git("status", "--short", "--", ".").splitlines(),
              "suites": {}, "cases": {}, "expected": {}, "obligations": {}, "package_fixtures": [], "measurement_errors": [], "last_layer": f"L{len(layers)-1}"}
    environment = dict(os.environ, OMEGA_LAYER_REPORT="1", OMEGA_BLESS="0", CARGO_BUILD_JOBS=os.environ.get("CARGO_BUILD_JOBS", "1"))
    print(f"Report directory: {directory}", flush=True)
    for i, crate in enumerate(layers):
        stage = f"L{i}"
        features = ["--features", "diagnostic-origins"] if stage in {"L3", "L5"} else []
        command = ["mbx", "test", "--locked", "-p", crate, *features, "--no-fail-fast", "--", "--include-ignored", "--show-output", "--format", "pretty"]
        started = monotonic()
        log = directory / f"{stage}.log"
        try:
            with log.open("wb") as out:
                result = subprocess.run(command, env=environment, stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT)
            exit_code = result.returncode
        except OSError as error:
            log.write_text(str(error), encoding="utf-8")
            exit_code = 1
        tests, counts = collect(log.read_text(encoding="utf-8", errors="replace"), stage, report, require_lowering_origins=stage == "L5")
        if exit_code and not counts.get("failed"):
            report["measurement_errors"].append(f"{stage}: command exited {exit_code} without a failing assertion; inspect {stage}.log")
        report["suites"][stage] = {"crate": crate, "command": command, "exit": exit_code, "seconds": round(monotonic() - started, 2),
                                   "counts": counts, "tests": tests}
        print(f"{stage} {crate}: {counts}, exit {exit_code}", flush=True)
    contextual_examples(report, directory, environment)
    frontend_diagnostics.run(report, directory, environment)
    cli_log = directory / "frontend-cli.log"
    cli_tests, cli_counts = collect(cli_log.read_text(encoding="utf-8", errors="replace"), "CLI", report)
    if report["frontend_cli_command"]["exit"] and not cli_counts.get("failed"):
        report["measurement_errors"].append(f"CLI command failed without a named failing test; inspect {cli_log.name}")
    report["suites"]["CLI"] = {**report["frontend_cli_command"], "crate": "omega", "log": cli_log.name,
                               "counts": cli_counts, "tests": cli_tests}
    handoff_evidence.collect_rules(report)
    for stage in report["suites"]:
        test_inventory.verify_suite(report, stage, directory, environment)
    rule_diagnostics.finish(report, directory)
    handoff_evidence.finish(report, directory)
    typing_sites.finish(report, directory, environment)
    if not options.before_l5:
        lowering_sites.finish(report, directory, environment)
        lowering_cli.finish(report, directory, environment)
        report["measurement_errors"].extend(incomplete_cohorts(report))
        lowering_evidence.finish(report, directory)
        product_controls.finish(report)
        refusal_evidence.apply(report)
        handoff_closures.apply(report)
        comparison = compare_fragment_runs(report)
        report["fragment_agreement"] = comparison
        report["measurement_errors"].extend(f"{label}: {detail}" for label, detail in comparison["differences"].items())
    frontend_fixture_cli.finish(report, directory, environment)
    if report["revision"] != git("rev-parse", "HEAD") or report["fingerprint"] != source_fingerprint():
        report["measurement_errors"].append("V2 sources changed during this run; repeat for a single-revision measurement")
    (directory / "evidence.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    (directory / "report.md").write_text(summarize(report), encoding="utf-8")
    print(f"Summary: {directory / 'report.md'}", flush=True)
    return readiness_exit(report)


if __name__ == "__main__":
    sys.exit(main())
