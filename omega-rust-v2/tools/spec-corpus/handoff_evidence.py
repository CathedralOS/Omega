"""Relate observed handoffs to the rules listed in V2's layer documents."""
from itertools import chain
from pathlib import Path
import re
import callable_alias_evidence
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
# These L4 promises have open handoffs. A measured consumer for one case can
# coexist with an observed L5 refusal for a different case in the same paragraph.
# "l5 refusal": L5 refuses the valid product; a registered L5 test pins that stop.
# "unmeasured": L5 reads the product, but no L5 test lowers a package that exercises it.
# "outside pipeline": neither the L5 contract nor lowering reads the product.
GAP_TESTS = "tests/preparation_handoff_gaps.rs"
PREPARATION_CONSUMERS = {
    "Proofs supplied by the checker": ("unmeasured", "L4's unit test supplies a checked invocation fact and observes its target dependencies. "
                                           "The public preparation round accepts such facts, but only L4 test code can construct one today. "
                                           "No L5 test observes those dependencies in Psi from the same prepared package.", None),
    "A bounded preparation compares": ("outside pipeline", "Preparation publishes the grant and certified maxima on PreparedTrees::resource_receipt(). "
                                       "Lowering needs no admission data, so neither the L5 contract nor lowering reads that receipt. L7 publication, "
                                       "which retains the ceiling in published builds, is not started. The bound comes from the L6 checker; only L4 tests build one today.", None),
}
# Why a product entry has no consumer comparison in this run. None of these is evidence.
CONSUMER_SCOPES = {
    "later suite": "comparison registered in a suite outside this run",
    "l5 refusal": "L5 refuses the valid product; the stop was observed in this run",
    "l5 refusal outside run": "L5 refusal pinned by a test outside this run",
    "outside pipeline": "no L5 contract paragraph or lowering code reads the product",
    "unmeasured": "no consumer comparison or pinned L5 refusal for the registered input",
}
GOLDEN_TESTS = {
    "L0": ("golden_fixtures", "accept_fixtures_lex_to_their_expected_tokens", "reject_fixtures_fail_with_their_expected_error"),
    "L1": ("golden_fixtures", "accept_fixtures_parse_to_their_expected_trees", "reject_fixtures_fail_with_their_expected_error"),
    "L3": ("expected", "typed_fixtures_match_handwritten_expectations", "typed_fixtures_match_handwritten_expectations"),
}
# Witnesses with precise product assertions outside the fixture comparisons.
# Several variants may be necessary to distinguish two legal products.
ASSERTIONS = {
    "structural-callable/alias-rhs": ("L1", "callable_aliases", "structural_callable_signatures_have_one_declaration_site", "types", "parse_file accepts structural signatures declared on an alias RHS and uses of their names"),
    "lexical-scope/binder-offsets": ("L2", "stage", "lexical_binding_uses_its_declaration_and_begins_after_its_initializer", "Lexical scope", "both references point to their distinct binder offsets"),
    "static-call/selected-machine": ("L3", "exposure", "constant_initializers_publish_references_calls_and_suffixed_values", "Calls", "initializer selects width at its call node"),
    "field-recipe/false": ("L4", "frozen_handoff", "lowering_can_distinguish_two_applications_of_the_same_field_recipe", "L3 publishes the operations", "field recipe under false returns true"),
    "field-recipe/true": ("L4", "frozen_handoff", "lowering_can_distinguish_two_applications_of_the_same_field_recipe", "L3 publishes the operations", "field recipe under true returns false"),
    "materialization/layout-plan": ("L4", "frozen_handoff", "materialization_pairs_the_frozen_value_with_its_exact_selected_layout", "Named reads connect", "materialization returns the selected layout plan"),
}
TYPED_FIXTURES = {
    "Calls": ["calls/accept/generic_selection.omg"],
    "Numbers and operators": ["arithmetic/accept/arithmetic.omg"],
    "Applications": ["computed_width/accept/computed_width.omg"],
    "Data and bindings": ["literal_bindings/accept/annotated_literal_binding.omg"],
    "Domains": ["domains/accept/domain_alias.omg"],
    "Evidence and effects": ["conformances/accept/inherited_row.omg"],
    "Collection access": ["collection_access/accept/collection_access.omg"],
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
        if len(fields) >= 2 and fields[1].startswith(callable_alias_evidence.PREFIX):
            callable_alias_evidence.record(report, observations, suite, target, current_test[1] if current_test else None, fields)
            return
        key = fields[1] if len(fields) == 2 else ""
        witness = ASSERTIONS.get(key)
        if not witness or suite != witness[0] or target != f"tests/{witness[1]}.rs" or not current_test or current_test[1] != witness[2]:
            report["measurement_errors"].append(f"unregistered or misplaced assertion: {line}")
            return
        group, value = observations["assertions"], witness[2]
    elif suite in GOLDEN_TESTS:
        binary, positive, negative = GOLDEN_TESTS[suite]
        if target != f"tests/{binary}.rs":
            if suite == "L0":
                report["measurement_errors"].append(f"misplaced L0 fixture observation: {line}")
            return
        if len(fields) != 2:
            report["measurement_errors"].append(f"malformed fixture observation: {line}")
            return
        crate = DOCUMENTS[suite][0].split("/")[0]
        _, separator, path = fields[1].replace("\\", "/").rpartition(f"/{crate}/tests/fixtures/")
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
        if group is observations["goldens"]:
            group[key] = None
    else:
        group[key] = value


def named_lexer_requirements(cells, line):
    named = []
    for group, cell in zip(("accept", "reject"), cells):
        if cell in {"—", "every fixture", "tiling check on every fixture and spec example"}:
            continue  # Not a named case.
        for ref in cell.split(","):
            unit = re.fullmatch(r"`(byte_cases::[a-z][a-z0-9_]*)`", ref.strip())
            fixture = re.fullmatch(rf"`([a-z][a-z0-9_]*/{group}/[a-z][a-z0-9_]*)`", ref.strip())
            if not unit and not fixture:
                raise ValueError(f"L0 line {line}: invalid evidence reference {ref!r}")
            item = {"kind": "unit test" if unit else "fixture", "name": unit[1] if unit else f"{fixture[1]}.omg",
                    "side": "direct" if group == "accept" else "refusals", "verified": False}
            if item in named:
                raise ValueError(f"L0 line {line}: duplicate {ref}")
            named.append(item)
    return named


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
            if stage == "L1":
                for kind, cell in zip(("accept", "reject"), cells[-2:]):
                    fixtures.extend(f"{name}.omg" for name in re.findall(rf"`([a-z][a-z0-9_]*/{kind}/[a-z][a-z0-9_]*)`", cell))
            if stage == "L3":
                fixtures.extend(TYPED_FIXTURES.get(rule, ()))
            rows.append({"line": number, "section": section, "rule": rule, "description": description, "fixtures": fixtures})
            if stage == "L0":
                rows[-1]["named_requirements"] = named_lexer_requirements(cells[-2:], number)
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


def passing_fixture(stage, fixture, evidence, results):
    binary, accept, reject = GOLDEN_TESTS[stage]
    emitter = accept if "accept" in Path(fixture).parts else reject
    return (evidence["goldens"].get(f"{stage}/{fixture}") == emitter
            and results.get((stage, f"tests/{binary}.rs", emitter)) == "ok")


def check_lexer_fixtures(row, fixture_dir, evidence, results, errors):
    unconfirmed = set()
    for item in row["named_requirements"]:
        if item["kind"] != "fixture":
            continue
        fixture = item["name"]
        source = fixture_dir / fixture
        key = f"L0/{fixture}"
        missing = [p.name for p in (source, source.with_suffix(".expected")) if not p.is_file()]
        if missing:
            errors.append(f"L0 {row['rule']}: missing fixture pair member in {source.parent}: {', '.join(missing)}")
        item["verified"] = not missing and passing_fixture("L0", fixture, evidence, results)
        if item["verified"]:
            row.setdefault(item["side"], []).append(fixture)
        else:
            unconfirmed.add(key)
    return unconfirmed


def collect_rules(report):
    evidence = report.setdefault("handoff_observations", {"identity": {}, "goldens": {}, "assertions": {}})
    errors = report["measurement_errors"]
    measured = dict.fromkeys(DOCUMENTS, True)
    results = reviewed_comparisons.test_results(report)
    unconfirmed_fixtures = set()
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
        fixture_dir = path.parent / "tests/fixtures"
        prefix, property_name = IDENTITY_RULES[stage]
        matching = [row for row in rows if row["rule"].startswith(prefix)]
        if len(matching) != 1:
            errors.append(f"{stage}: expected exactly one contract entry for {prefix!r}, found {len(matching)}")
        for row in rows:
            row["direct"] = []
            row["consumer"] = []
            row["refusal_only_rule"] = ((stage == "L0" and row["rule"] in {"R13", "R17"})
                                        or (stage == "L4" and row["rule"].startswith("When preparation stops, its diagnosed entry point")))
            if stage == "L0":
                unconfirmed_fixtures.update(check_lexer_fixtures(row, fixture_dir, evidence, results, errors))
            for fixture in row.pop("fixtures"):
                source = fixture_dir / fixture
                if not source.is_file() or not source.with_suffix(".expected").is_file():
                    errors.append(f"{stage} line {row['line']}: mapped fixture missing its input or expectation: {fixture}")
                    unconfirmed_fixtures.add(f"{stage}/{fixture}")
                elif passing_fixture(stage, fixture, evidence, results):
                    if "accept" in Path(fixture).parts:
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
            for group in (("*/accept", "*/reject", "*/prepared/accept", "*/prepared/reject") if stage == "L3" else ("*/accept", "*/reject")):
                for source in sorted(fixture_dir.glob(f"{group}/*.omg")):
                    key = f"{stage}/{source.relative_to(fixture_dir).as_posix()}"
                    fixture = source.relative_to(fixture_dir).as_posix()
                    if not source.with_suffix(".expected").is_file() or not passing_fixture(stage, fixture, evidence, results):
                        unconfirmed_fixtures.add(key)
                        errors.append(f"fixture output comparison unconfirmed: {key}")
    observed = reviewed_comparisons.apply(report, outlines, unconfirmed_fixtures)
    byte_tests = {(w.rule, w.side, f"byte_cases::{w.test}") for w in observed
                  if w.owner == w.suite == "L0" and w.target == "tests/byte_cases.rs"}
    for row in outlines["L0"]:
        for requirement in row["named_requirements"]:
            if requirement["kind"] == "unit test":
                requirement["verified"] = (row["rule"], requirement["side"], requirement["name"]) in byte_tests
    callable_alias_evidence.requirements(outlines["L1"], evidence, results, errors)
    for stage in ("L0", "L1"):
        for row in outlines[stage]:
            for item in row.get("named_requirements", []):
                if not item["verified"]:
                    errors.append(f"{stage} {row['rule']}: unconfirmed named {item['kind']}: {item['name']}")
    report["handoff_rules"] = outlines


def preparation_diagnostic_consumers(report):
    """A diagnosed package and its prepare/lower commands consume L4's refusal."""
    result = report.get("preparation_reasons", {})
    if not result.get("measured"):
        return
    matches = [row for row in report["handoff_rules"]["L4"] if row["rule"].startswith("Every refusal L4 renders carries its typed cause.")]
    if len(matches) != 1:
        report["measurement_errors"].append("L4: expected one typed refusal contract for the package consumer")
        return
    owned = (witness for reason in result["reasons"].values() for witness in reason["witnesses"])
    for witness in chain(owned, result["forwarded"]):
        if witness["suite"] == "CLI" and witness["status"] == "ok":
            matches[0]["consumer"].append(
                f"CLI · {witness['test']} · {witness['cause']} at {witness['file']} "
                f"bytes {witness['byte_start']}..{witness['byte_end']} · prepare and lower returned the complete refusal")


def finish(report, directory):
    outlines = report["handoff_rules"]
    preparation_diagnostic_consumers(report)
    classify_consumers(report, outlines, reviewed_comparisons.test_results(report))
    report["unverified_contracts"] = unverified_contracts(outlines, report.get("last_layer", "L4"))
    write_details(directory, outlines, report["handoff_observations"])


def confirmed_stops(report, results, test, labels):
    """Keep each pinned L5 refusal attributed to its passing registered test."""
    if results.get(("L5", GAP_TESTS, test)) != "ok":
        return {}
    stops = {}
    for label in labels:
        case = report.get("cases", {}).get("handoff/" + label)
        if (not case or case.get("status") != "open gap" or case.get("stage") != "L5"
                or str(case.get("target", "")).replace("\\", "/") != GAP_TESTS or case.get("test") != test):
            continue
        stops[label] = case["detail"]
    return stops


def classify_consumers(report, outlines, results):
    """Record why each product entry lacks a consumer comparison. The entry stays unverified."""
    cutoff = report.get("last_layer", "L4")
    for prefix in PREPARATION_CONSUMERS:
        if sum(row["rule"].startswith(prefix) for row in outlines.get("L4", ())) != 1:
            report["measurement_errors"].append(f"L4: expected one contract entry for consumer reason {prefix!r}")
    for stage, rows in outlines.items():
        for row in rows:
            for key in ("consumer_scope", "consumer_context", "observed_stops", "unobserved_l5_cases"):
                row.pop(key, None)
            if row["refusal_only_rule"]:
                continue
            known = next((value for prefix, value in PREPARATION_CONSUMERS.items()
                          if stage == "L4" and row["rule"].startswith(prefix)), None)
            if known and known[2] and cutoff >= "L5":
                test, labels = known[2]
                stops = confirmed_stops(report, results, test, labels)
                missing = [label for label in labels if label not in stops]
                if missing:
                    row["unobserved_l5_cases"] = missing
                if stops:
                    # A different input may successfully cross this same boundary.
                    row.update(consumer_scope="l5 refusal", consumer_context=known[1], observed_stops=list(stops.values()))
                    continue
                row.update(consumer_scope="unmeasured", observed_stops=[],
                           consumer_context=f"{known[1]} No pinned L5 refusal was observed in {GAP_TESTS}::{test}.")
                continue
            if row["consumer"]:
                continue
            if any(site.startswith("consumer:") for site in row.get("outside_run", ())):
                row["consumer_scope"] = "later suite"
            elif known:
                kind, explanation, pinned = known
                row["consumer_scope"] = "l5 refusal outside run" if pinned else kind
                row["consumer_context"] = explanation
            else:
                row["consumer_scope"] = "unmeasured"


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
                elif stage < last_layer and row.get("observed_stops"):
                    missing.append("L5 product for refused input")
                if stage < last_layer and row["consumer"] and row.get("unobserved_l5_cases"):
                    missing.append("L5 outcome for registered input")
            if missing:
                # Keep the gap visible at this cutoff. A registered later check has not run.
                sides = {"direct" if side == "producer" else "refusals" if side == "refusal" else side for side in missing}
                later = [site for site in row.get("outside_run", ()) if site.partition(":")[0] in sides]
                gap = {"stage": stage, "line": row["line"], "rule": row["rule"], "missing": missing, "later_checks": later}
                if row.get("consumer_scope") and ("consumer" in missing or row.get("observed_stops") or row.get("unobserved_l5_cases")):
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
            for item in row.get("named_requirements", []):
                status = "verified" if item["verified"] else "unconfirmed"
                lines.append(f"  - Named {item['kind']} ({item['side']}): {item['name']} · {status}")
            if row.get("consumer_scope"):
                lines.append("  - Consumer status: " + CONSUMER_SCOPES[row["consumer_scope"]] + ".")
            if row.get("consumer_context"):
                lines.append("  - " + row["consumer_context"])
                lines.extend("  - Observed stop: " + detail for detail in row.get("observed_stops", ()))
                lines.extend("  - Missing L5 observation: " + label for label in row.get("unobserved_l5_cases", ()))
            for field, title in (("direct", "Producer"), ("consumer", "Consumer"), ("refusals", "Exact refusal"),
                                 ("outside_run", "Outside selected suites"), ("failed_checks", "Failed comparison"),
                                 ("failed_tests", "Enclosing test failed")):
                hits = row.get(field, [])
                if field == "consumer" and len(hits) > 1:
                    lines.append("  - Consumer checks")
                    lines.extend("    - " + hit for hit in hits)
                elif hits or (field in {"direct", "consumer"} and not row["refusal_only_rule"]):
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
    lines += ["", "Named requirements in the two scoped front-end inventories (rule references, not distinct language rules):", ""]
    for stage, kind in (("L0", "fixture"), ("L0", "unit test"), ("L1", "parse_file comparison")):
        items = [i for r in report["handoff_rules"][stage] for i in r.get("named_requirements", ()) if i["kind"] == kind]
        lines.append(f"- {stage} · {kind}: {sum(i['verified'] for i in items)}/{len(items)} confirmed.")
    lines += consumer_scope_summary(report)
    return lines


def consumer_scope_summary(report):
    """Separate deferred comparisons from L5 refusals, unmeasured consumers and products no pipeline stage reads."""
    groups = {}
    for stage, rows in report["handoff_rules"].items():
        for row in rows:
            if not row["refusal_only_rule"] and (not row["consumer"] or row.get("observed_stops") or row.get("unobserved_l5_cases")):
                groups.setdefault(row.get("consumer_scope", "unmeasured"), []).append(f"{stage} line {row['line']}")
    if not groups:
        return []
    lines = ["", "Open consumer handoffs. A successful comparison for one input can coexist with an observed L5 refusal for another.", "",
             "| Reason | Entries | Contract lines |", "| --- | ---: | --- |"]
    for scope, title in CONSUMER_SCOPES.items():
        if scope in groups:
            lines.append(f"| {title} | {len(groups[scope])} | {', '.join(groups[scope])} |")
    if report.get("last_layer", "L4") < "L5" and any(entry.startswith("L4 ") for scope, entries in groups.items() if scope != "later suite" for entry in entries):
        lines += ["", "A run ending at L4 does not require L4 consumers. The L4 entries listed under other reasons stay open in the full run."]
    return lines
