"""Relate observed handoffs to the rules listed in V2's layer documents."""
from pathlib import Path
import re
import reviewed_comparisons


# These sections contain rules. Read their rows and paragraphs from the checkout
# so adding a rule exposes a new review item even before someone writes a test.
DOCUMENTS = {
    "L0": ("00_bytes-to-tokens/CONTRACT.md", ("Rules",)),
    "L1": ("01_tokens-to-syntax-trees/GRAMMAR.md", ("Rules and their fixtures",)),
    "L2": ("02_syntax-trees-to-symbol-resolved-trees/CONTRACT.md", ("Binding rules", "Handoff to typing and preparation")),
    "L3": ("03_symbol-resolved-trees-to-typed-trees/CONTRACT.md", ("Stage boundary", "Checked rules", "L3 handoff and cross-layer verification")),
    "L4": ("04_typed-trees-to-prepared-trees/CONTRACT.md", ("Typing and target closure", "Invocation admission", "Frozen values and reuse")),
}
IDENTITY_RULES = {
    "L0": ("R4", "word kind and byte span at COPY's initializer"),
    "L1": ("use, const", "single path-expression leaf at the initializer span"),
    "L2": ("Namespaces and imports", "unique declaration bound at that occurrence"),
    "L3": ("A static value evaluation request", "unique constant declaration on the typed initializer node"),
    "L4": ("Named reads connect", "frozen value 7 and the sole named input of COPY"),
}
IDENTITY_TESTS = {
    "FIRST": "alias_selects_first_equal_valued_constant",
    "OTHER": "alias_selects_other_equal_valued_constant",
}
IDENTITY_CONTROL = "each_stage_reader_distinguishes_substituted_other_product"
# These L4 promises have no L5 consumer comparison, for different reasons. Each
# stays an unverified consumer. The reason only says what kind of work would close it.
# "l5 refusal": L5 refuses the valid product; a registered L5 test pins that stop.
# "unmeasured": L5 reads the product, but no L5 test lowers a package that exercises it.
# "outside pipeline": neither the L5 contract nor lowering reads the product.
GAP_TESTS = "tests/preparation_handoff_gaps.rs"
PREPARATION_CONSUMERS = {
    "A specialized field request": ("l5 refusal", "L5 needs the field extents for the applied record.",
                                    ("nested_field_owner_arguments_stop_at_generic_data_lowering", ("specialized field owner arguments",))),
    "A local declaration after": ("l5 refusal", "Lowering the enclosing generic machine is required to inspect the two constants in Psi.",
                                  ("nested_helper_constants_stop_at_the_open_enclosing_template",
                                   ("nested helper enclosing static argument via helper(input)", "nested helper enclosing static argument via invoke::<helper>(input)"))),
    "L4 specializes the selected body's": ("unmeasured", "L4's unit test supplies a checked invocation fact and observes its target dependencies. "
                                           "The public preparation round accepts such facts, but only L4 test code can construct one today. "
                                           "No L5 test observes those dependencies in Psi from the same prepared package.", None),
    "A bounded preparation compares": ("outside pipeline", "Preparation publishes the grant and certified maxima on PreparedTrees::resource_receipt(). "
                                       "Neither the L5 contract nor lowering reads that receipt.", None),
}
# Why a product entry has no consumer comparison in this run. None of these is evidence.
CONSUMER_SCOPES = {
    "later suite": "comparison registered in a suite outside this run",
    "l5 refusal": "L5 refuses the valid product; the stop was observed in this run",
    "l5 refusal outside run": "L5 refusal pinned by a test outside this run",
    "outside pipeline": "no L5 contract paragraph or lowering code reads the product",
    "unmeasured": "no consumer comparison or pinned L5 refusal",
}
GOLDEN_TESTS = {
    "L0": ("golden_fixtures", "accept_fixtures_lex_to_their_expected_tokens", "reject_fixtures_fail_with_their_expected_error"),
    "L1": ("golden_fixtures", "accept_fixtures_parse_to_their_expected_trees", "reject_fixtures_fail_with_their_expected_error"),
    "L3": ("expected", "typed_fixtures_match_handwritten_expectations", "typed_fixtures_match_handwritten_expectations"),
}
# Witnesses with precise product assertions outside the fixture comparisons.
# Several variants may be necessary to distinguish two legal products.
ASSERTIONS = {
    "lexical-scope/binder-offsets": ("L2", "stage", "lexical_binding_uses_its_declaration_and_begins_after_its_initializer", "Lexical scope", "both references point to their distinct binder offsets"),
    "static-call/selected-machine": ("L3", "exposure", "constant_initializers_publish_references_calls_and_suffixed_values", "Calls", "initializer selects width at its call node"),
    "field-recipe/false": ("L4", "frozen_handoff", "lowering_can_distinguish_two_applications_of_the_same_field_recipe", "L3 publishes the operations", "field recipe under false returns true"),
    "field-recipe/true": ("L4", "frozen_handoff", "lowering_can_distinguish_two_applications_of_the_same_field_recipe", "L3 publishes the operations", "field recipe under true returns false"),
    "materialization/layout-plan": ("L4", "frozen_handoff", "materialization_pairs_the_frozen_value_with_its_exact_selected_layout", "Named reads connect", "materialization returns the selected layout plan"),
}
TYPED_FIXTURES = {
    "Calls": ["generic_selection"],
    "Numbers and operators": ["arithmetic"],
    "Applications": ["computed_width"],
    "Data and bindings": ["annotated_literal_binding"],
    "Domains": ["domain_alias"],
    "Evidence and effects": ["inherited_row"],
    "Collection access": ["collection_access"],
}


def record(report, suite, target, current_test, line):
    observations = report.setdefault("handoff_observations", {"identity": {}, "goldens": {}, "assertions": {}})
    target = target.replace("\\", "/")
    fields = line.split("\t")
    if fields[0] == "OMEGA_IDENTITY_CONTROL":
        if (len(fields) != 4 or fields[1] not in DOCUMENTS or fields[2:] != ["FIRST", "OTHER"]
                or suite != "L4" or target != "tests/constant_identity_handoff.rs"
                or not current_test or current_test[1] != IDENTITY_CONTROL):
            report["measurement_errors"].append(f"unexpected identity substitution: {line}")
            return
        group, key, value = observations.setdefault("controls", {}), fields[1], fields[3]
    elif fields[0] == "OMEGA_CONSTANT_IDENTITY":
        if len(fields) != 4:
            report["measurement_errors"].append(f"malformed identity observation: {line}")
            return
        _, stage, twin, actual = fields
        if (suite != "L4" or target != "tests/constant_identity_handoff.rs" or stage not in DOCUMENTS
                or twin not in IDENTITY_TESTS or not current_test or current_test[1] != IDENTITY_TESTS[twin]):
            report["measurement_errors"].append(f"unexpected source of identity observation: {line}")
            return
        group, key, value = observations["identity"], f"{stage}/{twin}", actual
    elif fields[0] == "OMEGA_HANDOFF_ASSERT":
        key = fields[1] if len(fields) == 2 else ""
        witness = ASSERTIONS.get(key)
        if not witness or suite != witness[0] or target != f"tests/{witness[1]}.rs" or not current_test or current_test[1] != witness[2]:
            report["measurement_errors"].append(f"unregistered or misplaced assertion: {line}")
            return
        group, value = observations["assertions"], witness[2]
    elif suite in GOLDEN_TESTS:
        binary, positive, negative = GOLDEN_TESTS[suite]
        if target != f"tests/{binary}.rs":
            return
        if len(fields) != 2:
            report["measurement_errors"].append(f"malformed fixture observation: {line}")
            return
        crate = DOCUMENTS[suite][0].split("/")[0]
        _, separator, path = fields[1].replace("\\", "/").rpartition(f"/{crate}/fixtures/")
        if not separator or not path.endswith(".omg") or ".." in path.split("/"):
            report["measurement_errors"].append(f"unexpected fixture path: {line}")
            return
        test = positive if "/accept/" in "/" + path else negative
        if not current_test or current_test[1] != test:
            report["measurement_errors"].append(f"unexpected test for fixture: {line}")
            return
        group, key, value = observations["goldens"], f"{suite}/{path}", test
    else:
        return
    if key in group:
        report["measurement_errors"].append(f"duplicate handoff observation: {key}")
    else:
        group[key] = value


def document_rules(stage, source):
    """Keep location and complete prose for review. A paragraph can contain several promises."""
    sections = DOCUMENTS[stage][1]
    found = set()
    section = None
    rows = []
    paragraph = []
    first = 0

    def flush():
        if paragraph:
            description = " ".join(paragraph)
            rows.append({"line": first, "section": section, "rule": description, "fixtures": []})
            paragraph.clear()

    for number, line in enumerate(source.splitlines(), 1):
        if line.startswith("#"):
            flush()
            section = line.removeprefix("## ") if line.startswith("## ") else None
            if section in sections:
                found.add(section)
            continue
        if section not in sections:
            continue
        if line.startswith("|"):
            flush()
            # A vertical bar inside inline code belongs to the example, not to the table.
            cells = [cell.strip() for cell in re.split(r"\|(?=(?:[^`]*`[^`]*`)*[^`]*$)", line.strip("|"))]
            if cells[0] in {"#", "Rule", "Subject"} or cells[0].startswith("---"):
                continue
            expected_columns = {"L0": 5, "L1": 3, "L2": 3, "L3": 3}
            if len(cells) != expected_columns.get(stage):
                raise ValueError(f"{stage} line {number}: unrecognized contract table row")
            rule = cells[0]
            description = cells[1] if stage != "L1" else rule
            fixtures = []
            if stage in {"L0", "L1"}:
                for kind, cell in zip(("accept", "reject"), cells[-2:]):
                    fixtures.extend(f"{kind}/{name}.omg" for name in re.findall(r"`([a-z][a-z0-9_]*)`", cell))
            if stage == "L3":
                fixtures.extend(f"accept/{name}.omg" for name in TYPED_FIXTURES.get(rule, ()))
            rows.append({"line": number, "section": section, "rule": rule, "description": description, "fixtures": fixtures})
        elif stage in {"L2", "L3", "L4"} and line.strip():
            if not paragraph:
                first = number
            paragraph.append(line.strip())
        else:
            flush()
    flush()
    missing = [name for name in sections if name not in found or not any(row["section"] == name for row in rows)]
    if missing:
        raise ValueError(f"{stage}: no contract entries extracted for {missing}")
    return rows


def finish(report, directory):
    evidence = report.get("handoff_observations", {"identity": {}, "goldens": {}, "assertions": {}})
    errors = report["measurement_errors"]
    measured = dict.fromkeys(DOCUMENTS, True)
    results = {(stage, test["target"].replace("\\", "/"), test["test"]): test["status"]
               for stage, suite in report["suites"].items() for test in suite["tests"]}
    control = results.get(("L4", "tests/constant_identity_handoff.rs", IDENTITY_CONTROL))
    substitutions = evidence.get("controls", {})
    report["verified_identity_substitutions"] = [s for s in DOCUMENTS if control == "ok" and substitutions.get(s) == "OTHER"]
    if control != "FAILED":
        for stage in DOCUMENTS:
            if stage not in report["verified_identity_substitutions"]:
                errors.append(f"{stage}: unverified substitution of equal-valued declaration (test {control or 'absent'})")
    for twin, test in IDENTITY_TESTS.items():
        status = results.get(("L4", "tests/constant_identity_handoff.rs", test))
        if status not in {"ok", "FAILED"}:
            errors.append(f"identity test {test}: {status or 'absent'}")
        divergence = None
        for stage in DOCUMENTS:
            reading = evidence["identity"].get(f"{stage}/{twin}")
            if divergence:
                measured[stage] = False
                if reading is not None:
                    errors.append(f"{stage}: unexpected identity observation after {divergence} stopped COPY = {twin}")
                continue
            if reading is None:
                errors.append(f"{stage}: missing identity observation for COPY = {twin}")
            elif reading != twin:
                divergence = stage
                report["cases"][f"handoff/constant identity/{stage}/{twin}"] = {
                    "cohort": "handoff", "stage": stage, "status": "inspection failed",
                    "detail": f"COPY = {twin}: expected {twin}, observed {reading!r}",
                    "later_layers_not_reached": [later for later in DOCUMENTS if later > stage],
                }
            if reading != twin or status not in {"ok", "FAILED"}:
                measured[stage] = False
    outlines = {}
    for stage, (filename, _) in DOCUMENTS.items():
        path = Path("layers") / filename
        try:
            rows = document_rules(stage, path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as error:
            errors.append(str(error))
            rows = []
        prefix, property_name = IDENTITY_RULES[stage]
        matching = [row for row in rows if row["rule"].startswith(prefix)]
        if len(matching) != 1:
            errors.append(f"{stage}: expected exactly one contract entry for {prefix!r}, found {len(matching)}")
        for row in rows:
            row["direct"] = []
            row["consumer"] = []
            row["refusal_only_rule"] = ((stage == "L0" and row["rule"] in {"R13", "R17"})
                                        or (stage == "L4" and row["rule"].startswith("When preparation stops, its diagnosed entry point")))
            if stage == "L0" and row["rule"] == "R16" and "L1/accept/right_shift.omg" in evidence["goldens"]:
                row["consumer"].append("L1 · right_shift.omg checks shift grouping and nested generic closers")
            for fixture in row.pop("fixtures"):
                if stage in {"L1", "L3"} and not (path.parent / "fixtures" / fixture).is_file():
                    errors.append(f"{stage} line {row['line']}: mapped fixture missing: {fixture}")
                if (path.parent / "fixtures" / fixture).is_file() and f"{stage}/{fixture}" in evidence["goldens"]:
                    if fixture.startswith("accept/"):
                        row["direct"].append(fixture)
                    else:
                        row.setdefault("refusals", []).append(fixture)
            if row in matching:
                if measured[stage]:
                    row["direct"].append(property_name)
                next_stage = f"L{int(stage[1:]) + 1}"
                if measured[stage] and measured.get(next_stage):
                    row["consumer"].append(f"{next_stage} · {IDENTITY_RULES[next_stage][1]}")
        for key, (owner, binary, test, rule_prefix, claim) in ASSERTIONS.items():
            if stage != owner:
                continue
            entries = [row for row in rows if row["rule"].startswith(rule_prefix)]
            if len(entries) != 1:
                errors.append(f"{stage}: expected one contract entry for {key}, found {len(entries)}")
            elif key not in evidence["assertions"]:
                errors.append(f"missing product assertion: {stage}/{binary}/{test} · {claim}")
            elif results.get((stage, f"tests/{binary}.rs", test)) != "ok":
                entries[0].setdefault("failed_tests", []).append(f"{binary}::{test} · {claim}")
            else:
                entries[0]["direct"].append(f"{claim} ({binary}::{test})")
        outlines[stage] = rows
        if stage in GOLDEN_TESTS:
            fixture_dir = path.parent / "fixtures"
            for group in (("accept", "reject", "prepared/accept", "prepared/reject") if stage == "L3" else ("accept", "reject")):
                for source in sorted((fixture_dir / group).glob("*.omg")):
                    key = f"{stage}/{source.relative_to(fixture_dir).as_posix()}"
                    if key not in evidence["goldens"]:
                        errors.append(f"fixture output comparison unconfirmed: {key}")
    reviewed_comparisons.apply(report, outlines)
    classify_consumers(report, outlines, results)
    report["handoff_rules"] = outlines
    report["unverified_contracts"] = unverified_contracts(outlines, report.get("last_layer", "L4"))
    write_details(directory, outlines, evidence)


def confirmed_stops(report, results, test, labels):
    """Details of the pinned L5 refusals, only when every label came from its passing registered test."""
    if results.get(("L5", GAP_TESTS, test)) != "ok":
        return []
    stops = []
    for label in labels:
        case = report.get("cases", {}).get("handoff/" + label)
        if (not case or case.get("status") != "open gap" or case.get("stage") != "L5"
                or str(case.get("target", "")).replace("\\", "/") != GAP_TESTS or case.get("test") != test):
            return []
        stops.append(case["detail"])
    return stops


def classify_consumers(report, outlines, results):
    """Record why each product entry lacks a consumer comparison. The entry stays unverified."""
    cutoff = report.get("last_layer", "L4")
    for prefix in PREPARATION_CONSUMERS:
        if sum(row["rule"].startswith(prefix) for row in outlines.get("L4", ())) != 1:
            report["measurement_errors"].append(f"L4: expected one contract entry for consumer reason {prefix!r}")
    for stage, rows in outlines.items():
        for row in rows:
            row.pop("consumer_scope", None)
            if row["refusal_only_rule"] or row["consumer"]:
                continue
            known = [value for prefix, value in PREPARATION_CONSUMERS.items() if stage == "L4" and row["rule"].startswith(prefix)]
            if any(site.startswith("consumer:") for site in row.get("outside_run", ())):
                row["consumer_scope"] = "later suite"
            elif not known:
                row["consumer_scope"] = "unmeasured"
            else:
                kind, explanation, pinned = known[0]
                row["consumer_context"] = explanation
                row["consumer_scope"] = kind
                if kind == "l5 refusal":
                    test, labels = pinned
                    if cutoff < "L5":
                        row["consumer_scope"] = "l5 refusal outside run"
                    else:
                        row["observed_stops"] = confirmed_stops(report, results, test, labels)
                        if not row["observed_stops"]:
                            # The registered stop did not run as pinned. Claim no reason at all.
                            row["consumer_scope"] = "unmeasured"
                            row["consumer_context"] += f" The pinned L5 refusal in {GAP_TESTS}::{test} was not observed."


def unverified_contracts(outlines, last_layer):
    gaps = []
    for stage, rows in outlines.items():
        for row in rows:
            if row["refusal_only_rule"]:
                missing = [] if row.get("refusals") else ["refusal"]
            else:
                missing = [] if row["direct"] else ["producer"]
                if stage < last_layer and not row["consumer"]:
                    missing.append("consumer")
            if missing:
                # Keep the gap visible at this cutoff. A registered later check has not run.
                sides = {"direct" if side == "producer" else "refusals" if side == "refusal" else side for side in missing}
                later = [site for site in row.get("outside_run", ()) if site.partition(":")[0] in sides]
                gap = {"stage": stage, "line": row["line"], "rule": row["rule"], "missing": missing, "later_checks": later}
                if "consumer" in missing and row.get("consumer_scope"):
                    gap["consumer_scope"] = row["consumer_scope"]
                gaps.append(gap)
    return gaps


def write_details(directory, outlines, evidence):
    lines = ["# Handoff evidence before lowering", "",
             "The two programs differ in which constant COPY names. FIRST and OTHER both equal 7u8. Each column records the selected spelling or declaration in the actual product.", "",
             "| Input | L0 token | L1 path | L2 binding | L3 typed reference | L4 dependency |",
             "| --- | --- | --- | --- | --- | --- |"]
    for twin in IDENTITY_TESTS:
        cells = [evidence["identity"].get(f"{stage}/{twin}", "unobserved") for stage in DOCUMENTS]
        lines.append("| " + " | ".join([twin, *cells]) + " |")
    lines += ["", "Each entry below comes from a rule table or contract paragraph. A snapshot checks its fixture's exact output. A consumer entry checks one specific use of the producer's information. An empty entry needs review and may already have an assertion elsewhere. A broad rule can have additional unmeasured promises.", ""]
    for stage, rows in outlines.items():
        filename, sections = DOCUMENTS[stage]
        lines.extend([f"## {stage}", "", f"Source: layers/{filename}. Sections inventoried: {', '.join(sections)}.", ""])
        for row in rows:
            label = row["rule"].split(". ")[0]
            if label.startswith("R") and label[1:].isdigit():
                label += " · " + row["description"]
            lines.append(f"- Line {row['line']} · {label}")
            if row["refusal_only_rule"]:
                lines.append("  - This rule specifies the diagnostic for a refused input.")
            if row.get("consumer_scope"):
                lines.append("  - Consumer status: " + CONSUMER_SCOPES[row["consumer_scope"]] + ".")
            if row.get("consumer_context"):
                lines.append("  - " + row["consumer_context"])
                lines.extend("  - Observed stop: " + detail for detail in row.get("observed_stops", ()))
            for field, title in (("direct", "Producer"), ("consumer", "Consumer"), ("refusals", "Exact refusal"),
                                 ("outside_run", "Outside selected suites"), ("failed_checks", "Failed comparison"),
                                 ("failed_tests", "Enclosing test failed")):
                hits = row.get(field, [])
                if hits or (field in {"direct", "consumer"} and not row["refusal_only_rule"]):
                    lines.append(f"  - {title}: " + (", ".join(hits) if hits else "no mapped observation"))
        lines.append("")
    (directory / "handoffs.md").write_text("\n".join(lines), encoding="utf-8")


def summary(report):
    rows = [row for layer in report["handoff_rules"].values() for row in layer]
    product_rows = [row for row in rows if not row["refusal_only_rule"]]
    missing = sum(not row["direct"] or not row["consumer"] for row in product_rows)
    refusal_rows = [row for row in rows if row["refusal_only_rule"]]
    observed_refusals = sum(bool(row.get("refusals")) for row in refusal_rows)
    counts = report["reviewed_registration_counts"]
    in_scope = counts["registered"] - counts["outside_run"]
    lines = ["", "## Coverage of handoff rules", "",
             f"Stage readers distinguished the other equal-valued declaration at {len(report['verified_identity_substitutions'])} of {len(DOCUMENTS)} product boundaries.", "",
             f"The run observed {counts['observed']} of {in_scope} registered comparisons in scope. {counts['outside_run']} registrations belong to suites outside this run. One assertion can support several contract entries.", "",
             f"{missing} of {len(product_rows)} product entries lack a producer or consumer comparison in the selected suites. {observed_refusals} of {len(refusal_rows)} refusal-only rules have an observed diagnostic. Individual comparisons cover only the properties they inspect. [Handoff evidence](handoffs.md)", "",
             "| Layer | Documented entries | With producer evidence | With consumer evidence | Product entries with neither | Refusal-only rules |",
             "| --- | ---: | ---: | ---: | ---: | ---: |"]
    for stage, rows in report["handoff_rules"].items():
        lines.append(f"| {stage} | {len(rows)} | {sum(bool(r['direct']) for r in rows)} | {sum(bool(r['consumer']) for r in rows)} | {sum(not r['direct'] and not r['consumer'] and not r['refusal_only_rule'] for r in rows)} | {sum(r['refusal_only_rule'] for r in rows)} |")
    lines += consumer_scope_summary(report)
    return lines


def consumer_scope_summary(report):
    """Separate deferred comparisons from L5 refusals, unmeasured consumers and products no pipeline stage reads."""
    groups = {}
    for stage, rows in report["handoff_rules"].items():
        for row in rows:
            if not row["refusal_only_rule"] and not row["consumer"]:
                groups.setdefault(row.get("consumer_scope", "unmeasured"), []).append(f"{stage} line {row['line']}")
    if not groups:
        return []
    lines = ["", "Product entries without a consumer comparison in this run, by reason. None of them counts as consumer evidence.", "",
             "| Reason | Entries | Contract lines |", "| --- | ---: | --- |"]
    for scope, title in CONSUMER_SCOPES.items():
        if scope in groups:
            lines.append(f"| {title} | {len(groups[scope])} | {', '.join(groups[scope])} |")
    if report.get("last_layer", "L4") < "L5" and any(entry.startswith("L4 ") for scope, entries in groups.items() if scope != "later suite" for entry in entries):
        lines += ["", "A run ending at L4 does not require L4 consumers. The L4 entries listed under other reasons stay open in the full run."]
    return lines
