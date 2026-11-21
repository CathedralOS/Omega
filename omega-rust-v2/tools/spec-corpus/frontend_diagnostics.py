"""Collect source stops, CLI refusals and product witnesses through lowering.

The before-L5 run skips the L5 CLI probes and witnesses. The full run requires them.
"""
import re
import subprocess
import diagnostic_kinds
import frontend_fixture_cli
from diagnostic_fields import readable, unescape
from frontend_products import PRODUCTS, comparison as product_comparison
import rule_diagnostics
import reviewed_comparisons

# Test name -> (first stopping layer, producer named in the CLI diagnostic).
CLI_CASES = {
    "invalid_byte_after_crlf": ("L0", "lexer"),
    "truncated_utf8_after_multibyte_comment_text": ("L0", "lexer"),
    "missing_closing_brace_at_eof": ("L1", "parser"),
    "unresolved_call_in_the_second_file": ("L2", "resolver"),
    "wrong_return_type_during_initial_typing": ("L3", "typer"),
    "deeply_grouped_result_reports_the_type_mismatch_at_the_result": ("L3", "typer"),
    "wrong_array_identity_after_evaluation": ("L3", "typer (after static evaluation)"),
    "a_domain_alias_with_no_matching_evaluated_width_identifies_its_target": ("L3", "typer (after static evaluation)"),
    "failed_precondition_during_static_evaluation": ("L4", "preparation"),
    "unavailable_target_for_a_required_extent": ("L4", "preparation (target input)"),
    "a_carry_annotated_alias_stops_at_its_attributes": ("L5", "lowering"),
    "a_transparent_alias_cannot_change_its_targets_multiplicity": ("L3", "typer"),
    "a_crashing_helper_cannot_supply_a_total_predicate_application": ("L5", "lowering"),
}
# Each stop must carry the controls that separate it from its neighbouring layer.
CONTROLS = {
    False: ("prepare", "good-twin-prepares,lower-repeats-stop"),
    True: ("lower", "good-twin-lowers,bad-input-prepares"),
}
# L5 CLI refusals must agree with the layer test that pins the same message and span.
LAYER_WITNESSES = {
    "a_crashing_helper_cannot_supply_a_total_predicate_application": ("predicate_call_handoff", "a_published_crash_route_prevents_a_total_predicate_application"),
    "a_carry_annotated_alias_stops_at_its_attributes": ("declarations", "alias_with_carry_stops_at_its_attributes"),
}


def includes_l5(report):
    return report.get("last_layer", "L4") == "L5"


def active_cases(report):
    full = includes_l5(report)
    return {name: case for name, case in CLI_CASES.items() if full or case[0] != "L5"}


def active_products(report):
    full = includes_l5(report)
    return {stage: witness for stage, witness in PRODUCTS.items() if full or stage != "L5"}


def compiler_stop(raw_stage):
    resumed = raw_stage == "L3-resumed"
    stage = "L3" if resumed else raw_stage
    if stage not in PRODUCTS:
        raise ValueError(f"unknown compiler stage {raw_stage!r}")
    furthest = 4 if resumed else int(stage[1:])
    return {"stage": stage, "after_static_evaluation": resumed,
            "later_layers_not_reached": [f"L{i}" for i in range(furthest + 1, 6)]}


def phase_label(row):
    if row["stage"] == "L3":
        return "L3 after static evaluation" if row.get("after_static_evaluation") else "L3 initial typing"
    return row["stage"]


PREPARATION_TESTS = {
    "Typing": ("preparation-refusal/initial-typing", "wrong_return_type_during_initial_typing"),
    "ResumedTyping": ("preparation-refusal/resumed-typing", "wrong_array_identity_after_evaluation"),
    "Preparation": ("preparation-refusal/evaluation", "failed_precondition_during_static_evaluation"),
    "MissingTarget": ("preparation-refusal/missing-target", "unavailable_target_for_a_required_extent"),
}
# Typing phases carry the typer's error. Preparation phases may carry the refusal with its typed cause.
PREPARATION_PAYLOADS = ("TypeError", "PreparationRefusal")


def preparation_phases(report):
    """Check each public failure variant against its producer assertion and its package CLI twin."""
    errors = report["measurement_errors"]
    path = "layers/04_typed-trees-to-prepared-trees/src/preparation/rounds.rs"
    try:
        declared = diagnostic_kinds.kinds(path, "PreparationFailure", payload=PREPARATION_PAYLOADS)
    except (OSError, ValueError) as error:
        errors.append(str(error))
        return {}
    for retired in sorted(PREPARATION_TESTS.keys() - declared):
        errors.append(f"registered preparation diagnostic names a retired variant: {retired}")
    checks = {}
    for kind in sorted(declared):
        row = checks[kind] = {"verified": False}
        if kind not in PREPARATION_TESTS:
            errors.append(f"preparation failure {kind}: no producer and CLI comparison registered")
            continue
        comparison, cli_test = PREPARATION_TESTS[kind]
        registration = reviewed_comparisons.sources().get(comparison)
        producer = False
        if registration:
            suite, target, test = registration
            producer = (test_results(report, suite, "preparation_failure_phases", test) == ["ok"]
                        and target == "tests/preparation_failure_phases.rs"
                        and bool(report.get("reviewed_comparisons", {}).get(comparison))
                        and comparison not in report.get("failed_comparisons", {}))
        cli = cli_test in report["frontend_diagnostics"]["cli"]
        row.update(comparison=comparison, cli_test=cli_test, producer_verified=producer, cli_verified=cli, verified=producer and cli)
        if not row["verified"]:
            errors.append(f"preparation failure {kind}: producer comparison {producer}, CLI comparison {cli}")
    return checks


def location(file, line, column, start, end):
    start, end = int(start), int(end)
    if start < 0 or end < start or bool(line) != bool(column):
        raise ValueError("invalid source range")
    result = {"file": file, "byte_start": start, "byte_end": end}
    if line:
        if min(int(line), int(column)) < 1:
            raise ValueError("source positions start at one")
        result.update(line=int(line), byte_column=int(column))
    return result


def test_results(report, stage, target, name):
    return [test["status"] for test in report["suites"].get(stage, {}).get("tests", ())
            if test["target"].replace("\\", "/") == f"tests/{target}.rs" and test["test"] == name]


def collect(report, logs):
    cases = active_cases(report)
    data = {"mode": "through L5" if includes_l5(report) else "before L5", "cli": {}, "fragments": {}, "products": {}}
    expected = None
    errors = report["measurement_errors"]
    invalid = set()
    for line in logs.splitlines():
        if not line.startswith("OMEGA_FRONTEND"):
            continue
        fields = line.split("\t")
        try:
            tag = fields[0]
            if tag == "OMEGA_FRONTEND_COUNT":
                if expected is not None or len(fields) != 2:
                    raise ValueError("duplicate or malformed inventory")
                expected = int(fields[1])
                continue
            if tag == "OMEGA_FRONTEND":
                _, stage, label, command, producer, file, row, col, start, end, controls, reason, rendered, *witness = fields
                reason = unescape(reason).decode("utf-8")
                if cases.get(label) != (stage, producer):
                    raise ValueError("unexpected layer or producer for this diagnostic")
                if (command, controls) != CONTROLS[stage == "L5"]:
                    raise ValueError("diagnostic lacks the command or neighbouring-layer controls for its layer")
                if file != "case.omg" or not row or not reason:
                    raise ValueError("diagnostic lacks its file, position or cause")
                actual = unescape(rendered).decode("utf-8")
                if actual != f"{file}:{row}:{col}: {producer}: {reason} (bytes {start}..{end})":
                    raise ValueError("captured CLI diagnostic differs from the expected stop")
                diagnostic_kinds.check_occurrence(witness, file, int(row), int(col), int(start), int(end))
                group = data["cli"]
                value = {**compiler_stop("L3-resumed" if producer == "typer (after static evaluation)" else stage),
                         "producer": producer, "command": command, "controls": controls.split(","),
                         "reason": reason, "rendered": actual, "source_bytes_escaped": witness[1],
                         "occurrence_bytes_escaped": witness[3], **location(file, row, col, start, end)}
            elif tag == "OMEGA_FRONTEND_STOP":
                _, label, stage, package, file, row, col, start, end, occurrence, reason = fields
                package, file, occurrence, reason = (unescape(field).decode("utf-8") for field in (package, file, occurrence, reason))
                if stage == "L5" or not reason:
                    raise ValueError("unknown stopping layer or absent reason")
                group = data["fragments"]
                value = {"status": "refused", **compiler_stop(stage), "package": package, "reason": reason, "occurrence": occurrence, **location(file, row, col, start, end)}
                if file == "example.omg" and package == "example":
                    if not row or len(occurrence.encode("utf-8")) != int(end) - int(start):
                        raise ValueError("missing local source position or source text differs in byte length from the span")
                    chapter, fence = label.rsplit(":", 1)
                    value.update(spec_path=chapter, spec_line=int(fence) + int(row))
            elif tag == "OMEGA_FRONTEND_PREPARED":
                _, label = fields
                group, value = data["fragments"], {"status": "prepared", "stage": "L4"}
            else:
                raise ValueError("unknown diagnostic record")
            if label in group:
                raise ValueError(f"duplicate observation for {label}")
            group[label] = value
        except (ValueError, TypeError) as error:
            errors.append(f"{tag}: {error}: {line}")
            if tag == "OMEGA_FRONTEND" and len(fields) > 2:
                invalid.add(("cli", fields[2]))
            elif tag in {"OMEGA_FRONTEND_STOP", "OMEGA_FRONTEND_PREPARED"} and len(fields) > 1:
                invalid.add(("fragments", fields[1]))
    for group, label in invalid:
        data[group].pop(label, None)
    if expected != len(data["fragments"]) or not expected:
        errors.append(f"front-end fragments: received {len(data['fragments'])}, expected {expected}")
    outcomes = test_results(report, "L4", "spec_corpus", "every_spec_fragment_has_a_preparation_outcome")
    data["fragment_expectations_verified"] = outcomes == ["ok"]
    if not data["fragment_expectations_verified"]:
        errors.append(f"published fragment expectations or source ranges unverified: {outcomes or 'missing'}")
    for name in sorted(cases.keys() - data["cli"].keys()):
        errors.append(f"missing verified CLI diagnostic: {name}")
    for stage in sorted({stage for stage, _ in cases.values()} - {row["stage"] for row in data["cli"].values()}):
        errors.append(f"no verified CLI diagnostic for {stage}")
    for name, (target, test) in LAYER_WITNESSES.items():
        if name not in cases:
            continue
        results = test_results(report, cases[name][0], target, test)
        if name in data["cli"]:
            data["cli"][name]["layer_witness"] = {"target": target, "test": test, "verified": results == ["ok"]}
        if results != ["ok"]:
            errors.append(f"{name}: layer witness {target}::{test}: {results or 'missing'}")
    for stage, (target, name, product) in active_products(report).items():
        results = test_results(report, stage, target, name)
        comparison = product_comparison(stage)
        receipts = report.get("reviewed_comparisons", {})
        # record() authenticates the suite/target/test and nulls duplicate success receipts.
        if comparison in report.get("failed_comparisons", {}):
            problem = "comparison failed"
        elif results != ["ok"]:
            problem = f"test results: {results or 'missing'}"
        elif comparison not in receipts:
            problem = "comparison not observed"
        elif not receipts[comparison]:
            problem = "duplicate comparison"
        else:
            problem = None
        data["products"][stage] = {"target": target, "test": name, "product": product,
                                   "comparison": comparison, "verified": problem is None}
        if problem:
            errors.append(f"{stage} product witness {target}::{name} ({comparison}): {problem}")
    report["frontend_diagnostics"] = data


def witness_lines(report, log, cli):
    # Cargo captures each test's stdout separately. A plausible diagnostic
    # printed by another binary or test must never stand in for this witness.
    binary = "tests/frontend_diagnostics.rs" if cli else "tests/spec_corpus.rs"
    target = current = ""
    results = {}
    observed = []
    for line in log.splitlines():
        if header := re.match(r"^\s*Running (.+?) \(", line):
            target, current = header[1].replace("\\", "/"), ""
        elif outcome := re.fullmatch(r"test (.+?) \.\.\. (ok|FAILED|ignored)(?:,.*)?", line):
            results.setdefault((target, outcome[1]), []).append(outcome[2])
        elif block := re.fullmatch(r"---- (.+?) stdout ----", line):
            current = block[1]
        elif line in {"successes:", "failures:"} or line.startswith("test result:"):
            current = ""
        elif line.startswith("OMEGA_FRONTEND"):
            fields = line.split("\t", 3)
            name = fields[2] if cli and len(fields) > 2 else "every_spec_fragment_has_a_preparation_outcome"
            if target != binary or current != name or (fields[0] == "OMEGA_FRONTEND") != cli:
                report["measurement_errors"].append(f"misplaced diagnostic in {target}::{current}: {line}")
            else:
                observed.append((name, line))
    for name, line in observed:
        if cli and results.get((binary, name)) != ["ok"]:
            report["measurement_errors"].append(f"CLI diagnostic test {name}: {results.get((binary, name), 'absent')}")
        else:
            yield line


def run(report, directory, environment):
    log = directory / "frontend-cli.log"
    cmd = ["mbx", "test", "--locked", "-p", "omega", "--no-fail-fast", "--test", "frontend_diagnostics"]
    for binary, _, _ in diagnostic_kinds.FAMILIES.values():
        cmd.extend(["--test", binary])
    for binary in rule_diagnostics.BINARIES.values():
        cmd.extend(["--test", binary])
    cmd.extend(["--", "--show-output"])
    if not includes_l5(report):
        # Before-L5 mode does not measure lowering, so its CLI refusals are neither run nor required.
        skipped = [name for name, (stage, _) in CLI_CASES.items() if stage == "L5"]
        cmd += ["--exact", *[arg for name in skipped for arg in ("--skip", name)]]
    try:
        with log.open("wb") as output:
            code = subprocess.run(cmd, env=environment, stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT).returncode
    except OSError as error:
        log.write_text(str(error), encoding="utf-8")
        code = 1
    report["frontend_cli_command"] = {"command": cmd, "exit": code}
    cli_log = log.read_text(encoding="utf-8", errors="replace")
    cli_rows = witness_lines(report, cli_log, True)
    fragment_rows = witness_lines(report, (directory / "L4.log").read_text(encoding="utf-8", errors="replace"), False)
    collect(report, "\n".join([*cli_rows, *fragment_rows]))
    inventory = diagnostic_kinds.measure(report, cli_log)
    report["frontend_diagnostics"]["diagnostic_kinds"] = inventory
    report["frontend_diagnostics"]["preparation_phases"] = preparation_phases(report)
    details = ["# Published fragments that stopped before lowering", "",
               "The location identifies the failing expression in the canonical spec when the fragment supplied the source. Byte ranges are relative to the compiler input. Some fragments need a package context.", ""]
    if not report["frontend_diagnostics"]["fragment_expectations_verified"]:
        details += ["The fragment test did not verify these observations against its expected outcomes. See L4.log for differences.", ""]
    # Spec fragments are only prepared, so they cannot stop in L5.
    for stage in (stage for stage in PRODUCTS if stage != "L5"):
        details += [f"## {stage}", ""]
        rows = [row for row in report["frontend_diagnostics"]["fragments"].items() if row[1]["status"] == "refused" and row[1]["stage"] == stage]
        for label, row in sorted(rows):
            site = f"{row['spec_path']}:{row['spec_line']}:{row['byte_column']}" if "spec_line" in row else f"{label} · {row['package']}/{row['file']}"
            selected = row["occurrence"] or ("<empty span>" if "line" in row else "<source unavailable>")
            details.append(f"- {site} · {phase_label(row)} · bytes {row['byte_start']}..{row['byte_end']} · {readable(selected)} · {readable(row['reason'])}")
        if not rows:
            details.append("No fragment recorded a refusal in this layer.")
        details.append("")
    details.extend(diagnostic_kinds.details(inventory))
    details.extend(["## Preparation failure phases", "",
                    "| Phase | Producer comparison | Package command test |", "| --- | --- | --- |"])
    for kind, check in report["frontend_diagnostics"]["preparation_phases"].items():
        producer = check.get("comparison", "missing")
        command = check.get("cli_test", "missing")
        details.append(f"| {kind} | {producer} ({'verified' if check.get('producer_verified') else 'unverified'}) "
                       f"| {command} ({'verified' if check.get('cli_verified') else 'unverified'}) |")
    (directory / "frontend-diagnostics.md").write_text("\n".join(details), encoding="utf-8")


def section(report):
    data = report["frontend_diagnostics"]
    full = data.get("mode") == "through L5"
    lines = ["", "## Diagnostics through L5" if full else "## Diagnostics before L5", ""]
    for stage, rows in data.get("diagnostic_kinds", {}).items():
        verified = sum(row["state"] == "CLI verified" for row in rows.values())
        lines.append(f"{stage} exercised {verified}/{len(rows)} declared diagnostic kinds through the CLI. [Per-kind evidence](frontend-diagnostics.md#public-diagnostic-kinds)")
    lines.extend(frontend_fixture_cli.summary(report))
    phases = data.get("preparation_phases", {})
    if phases:
        verified = sum(row["verified"] for row in phases.values())
        lines.append(f"The preparation API and package CLI verified {verified}/{len(phases)} declared failure variants. "
                     + ", ".join(kind if row["verified"] else f"{kind} (unverified)" for kind, row in phases.items()) + ".")
    lines.append("")
    if not full:
        lines += ["Before-L5 mode: the lowering CLI refusals and L5 witnesses were not run.", ""]
    lines += ["| Layer | Verified CLI refusals | Product witness | Spec fragments first stopped here |", "| --- | ---: | --- | ---: |"]
    for stage, witness in data["products"].items():
        count = sum(row["stage"] == stage for row in data["cli"].values())
        stops = sum(row.get("stage") == stage and row["status"] == "refused" for row in data["fragments"].values()) if stage != "L5" else "not measured"
        proof = witness["product"] if witness["verified"] else "Unverified: " + witness["product"]
        lines.append(f"| {stage} | {count} | {proof} | {stops} |")
    prepared = sum(row["status"] == "prepared" for row in data["fragments"].values())
    if not data["fragment_expectations_verified"]:
        lines += ["", "The fragment test did not verify the observed outcomes. [See the L4 test output](L4.log)."]
    lines += ["", f"{prepared} of {len(data['fragments'])} isolated fragments produced prepared trees. [All fragment diagnostics](frontend-diagnostics.md) list the first stopping layer, spec location, selected text and reason. A fragment can require a surrounding package.",
              "", "Verified diagnostics from the package command ([CLI log](frontend-cli.log)). L0-L4 refusals use `source prepare`; their positive twin prepares and `source lower` repeats the same stop. L5 refusals use `source lower`; their positive twin lowers and the refused input prepares.", ""]
    for name, row in sorted(data["cli"].items(), key=lambda item: (item[1]["stage"], item[0])):
        witness = row.get("layer_witness")
        layer = "" if witness is None else f" · layer witness {witness['target']}::{witness['test']}" + ("" if witness["verified"] else " (unverified)")
        lines.append(f"- {row['stage']} · {name} · source {row['command']} · {row['producer']} · {row['file']}:{row['line']}:{row['byte_column']} · bytes {row['byte_start']}..{row['byte_end']} · {readable(row['reason'])}{layer}")
    return lines
