#!/usr/bin/env python3
"""External proof import for the bounded matching-logic fragment.

Implements the "one small external arithmetic proof with its exact source
axiom closure and checked proof-object translation" leg drafted in
``wiki/drafts/matching_logic.md`` and detailed in
``wiki/drafts/matching_logic_external_proof_import.md``.

An external producer exports a flat step list under its own rule vocabulary
(``external-arithmetic-proof/1``). This tool translates that export into the
bounded slice checker's tree certificate (``omega-matching-logic-
certificate/1``) and re-checks the result with the real checker: the import
is a *checked* translation, not a trusted admission of the statement. Every
axiom the proof cites must appear verbatim in the declared axiom closure, and
the closure must match the cited set exactly -- an understated closure hides
a dependency, an overstated one hides nothing.

The goal is rebuilt from the case's declared obligation, so a producer cannot
substitute a weaker question. Foreign rules that would escape the fragment
(fixpoint introduction, classical principles) or that this importer does not
cover (hypothesis discharge) are rejected by name, not silently dropped.

Requires Python 3.9+ and no third-party packages. From the repository root:

    python3 tools/matching-logic-external-proof-import/external_proof_import.py \
        check tools/matching-logic-external-proof-import/cases/reference.json

    python3 tools/matching-logic-external-proof-import/external_proof_import.py \
        record

``check`` prints a JSON verdict and exits nonzero when a positive case
rejects or a negative case accepts. ``record`` runs every pinned case and
writes the evidence record the comparison consumes.
"""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import time


CASE_SCHEMA = "omega-external-proof-import-case/1"
EXPORT_SCHEMA = "external-arithmetic-proof/1"
CERT_SCHEMA = "omega-matching-logic-certificate/1"
RECORD_SCHEMA = "omega-external-proof-import-record/1"
FRAGMENT = "one-sorted finitary basic matching logic, no fixpoint symbols"
SEMANTICS_DOC = "wiki/drafts/matching_logic_external_proof_import.md"
SOURCE_DRAFT = "wiki/drafts/matching_logic.md"
SLICE_CHECKER = "tools/matching-logic-slice/slice_checker.py"

# Bounded-fragment ceiling: translations beyond this node count are refused
# rather than checked, keeping the import finitary by construction.
MAX_STEPS = 64

# Rules the importer rejects by name before translation. A fixpoint rule
# would leave the fragment entirely; a classical principle would change the
# accepted foundation. Neither may pass through silently.
FRAGMENT_ESCAPE_RULES = ("mu", "nu", "fixpoint", "least_fixpoint")
CLASSICAL_RULES = ("lem", "dne", "classical_choice", "double_negation")

# External rule spelling -> local certificate rule. ``None`` marks external
# rules with no hypothesis-free local counterpart in this bounded importer.
BRIDGE = {
    "AXIOM": "axiom",
    "MP": "modus_ponens",
    "ANDI": "and_intro",
    "ANDL": "and_elim_left",
    "ANDR": "and_elim_right",
    "ORIL": "or_intro_left",
    "ORIR": "or_intro_right",
    "EI": "exists_intro",
    "EE": "exists_elim",
    "UI": "forall_elim",
    "REFL": "eq_refl",
    "SYM": "eq_sym",
    "TRANS": "eq_trans",
    "SUBST": "eq_subst",
    "MEMBER": "membership_intro",
    "DEF_CLOSED": "defined_closed",
    "DEF_MEMBER": "defined_membership",
    # hypothesis-carrying rules are outside this bounded import
    "HYP": None,
    "IMPI": None,
    "GEN": None,
    "ORE": None,
}


class Reject(Exception):
    pass


def fail(rule, detail):
    raise Reject({"rule": rule, "detail": detail})


def load_slice_checker(root):
    """Load the sibling slice checker for the post-translation recheck."""
    path = root / SLICE_CHECKER
    specification = importlib.util.spec_from_file_location(
        "slice_checker_recheck", path)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def load_case(path):
    case = json.loads(Path(path).read_text())
    if case.get("schema") != CASE_SCHEMA:
        fail("schema", f"case schema must be {CASE_SCHEMA!r}")
    export = case.get("export")
    if not isinstance(export, dict):
        fail("schema", "case carries no 'export' object")
    if export.get("schema") != EXPORT_SCHEMA:
        fail("schema", f"export schema must be {EXPORT_SCHEMA!r}")
    return case


# --- external step translation --------------------------------------------

def translate_step(step, environment):
    """Translate one flat external step into a certificate node.

    ``environment`` maps earlier step ids to their already-translated nodes;
    a premise id outside it is a forward reference or a cycle.
    """
    if not isinstance(step, dict):
        fail("malformed-step", f"step is not an object: {step!r}")
    step_id = step.get("id")
    if not isinstance(step_id, str):
        fail("malformed-step", f"step without string id: {step!r}")
    rule = step.get("rule")
    if not isinstance(rule, str):
        fail("malformed-step", f"step {step_id!r} without rule name")
    lowered = rule.lower()
    if lowered in FRAGMENT_ESCAPE_RULES:
        fail("fragment-escape",
             f"external rule {rule!r} introduces a fixpoint, which the "
             f"fragment excludes by construction")
    if lowered in CLASSICAL_RULES:
        fail("classical-rule-import",
             f"external rule {rule!r} is classical; the accepted foundation "
             f"is constructive (wiki/spec/proofs/classicality.md)")
    if rule not in BRIDGE:
        fail("unknown-external-rule", f"no bridge for external rule {rule!r}")
    local = BRIDGE[rule]
    if local is None:
        fail("unsupported-external-rule",
             f"external rule {rule!r} carries hypotheses outside the bounded "
             f"import fragment")

    def premise(key):
        ref = step.get(key)
        if not isinstance(ref, str) or ref not in environment:
            fail("premise-order",
                 f"step {step_id!r} cites {key}={ref!r}, which is not an "
                 f"earlier step")
        return environment[ref]

    def term(key):
        value = step.get(key)
        if value is None:
            fail("malformed-step",
                 f"step {step_id!r} ({rule}) missing field {key!r}")
        return value

    if rule == "AXIOM":
        return {"rule": "axiom", "name": term("name")}
    if rule == "MP":
        return {"rule": "modus_ponens", "implication": premise("of"),
                "argument": premise("by")}
    if rule == "ANDI":
        return {"rule": "and_intro", "left": premise("left"),
                "right": premise("right")}
    if rule == "ANDL":
        return {"rule": "and_elim_left", "premise": premise("of")}
    if rule == "ANDR":
        return {"rule": "and_elim_right", "premise": premise("of")}
    if rule == "ORIL":
        return {"rule": "or_intro_left", "premise": premise("of"),
                "other": term("other")}
    if rule == "ORIR":
        return {"rule": "or_intro_right", "premise": premise("of"),
                "other": term("other")}
    if rule == "EI":
        return {"rule": "exists_intro", "var": term("var"),
                "body": term("body"), "witness": term("witness"),
                "defined": premise("defined"), "premise": premise("of")}
    if rule == "EE":
        return {"rule": "exists_elim", "premise": premise("of"),
                "elimination": premise("elimination"),
                "conclusion": term("conclusion")}
    if rule == "UI":
        return {"rule": "forall_elim", "premise": premise("of"),
                "witness": term("witness"), "defined": premise("defined")}
    if rule == "REFL":
        return {"rule": "eq_refl", "term": term("term")}
    if rule == "SYM":
        return {"rule": "eq_sym", "premise": premise("of")}
    if rule == "TRANS":
        return {"rule": "eq_trans", "left": premise("left"),
                "right": premise("right")}
    if rule == "SUBST":
        return {"rule": "eq_subst", "equality": premise("equality"),
                "var": term("var"), "pattern": term("pattern"),
                "direction": step.get("direction", "right"),
                "premise": premise("of")}
    if rule == "MEMBER":
        return {"rule": "membership_intro", "term": term("term"),
                "sort": term("sort"), "premise": premise("of")}
    if rule == "DEF_CLOSED":
        return {"rule": "defined_closed", "term": term("term")}
    if rule == "DEF_MEMBER":
        return {"rule": "defined_membership", "premise": premise("of")}
    fail("unknown-external-rule", f"unhandled bridge target {rule!r}")


def count_nodes(node):
    """Count certificate nodes; a flat export may reuse steps, so the tree
    can be larger than the step list."""
    if not isinstance(node, dict):
        return 0
    total = 1
    for key, value in node.items():
        if key in ("premise", "left", "right", "implication", "argument",
                   "defined", "equality", "disjunction", "elimination"):
            total += count_nodes(value)
    return total


def cited_axioms(steps):
    """Every AXIOM step in the export is a source citation, whether or not
    the declared conclusion's proof cone reaches it."""
    cited = set()
    for step in steps:
        if isinstance(step, dict) and step.get("rule") == "AXIOM":
            name = step.get("name")
            if isinstance(name, str):
                cited.add(name)
    return cited


def translate_export(export):
    steps = export.get("steps")
    if not isinstance(steps, list) or not steps:
        fail("schema", "export carries no step list")
    if len(steps) > MAX_STEPS:
        fail("certificate-bound",
             f"export has {len(steps)} steps, bound is {MAX_STEPS}")
    environment = {}
    for step in steps:
        node = translate_step(step, environment)
        step_id = step["id"]
        if step_id in environment:
            fail("malformed-step", f"duplicate step id {step_id!r}")
        environment[step_id] = node
    conclusion_ref = export.get("conclusion")
    if not isinstance(conclusion_ref, str) or conclusion_ref not in environment:
        fail("schema",
             f"export conclusion {conclusion_ref!r} is not a step id")
    derivation = environment[conclusion_ref]
    certificate = {"schema": CERT_SCHEMA, "derivation": derivation}
    return certificate, count_nodes(derivation)


# --- goal reconstruction ---------------------------------------------------

def build_goal(case, checker):
    """Rebuild the declared goal from the case's canonical subject.

    ``refinement_after_transition`` reuses the slice checker's obligation;
    ``statement`` declares an exact pattern the certificate must conclude.
    """
    obligation = case.get("obligation", {})
    kind = obligation.get("kind")
    if kind == "refinement_after_transition":
        return checker.build_goal(case)
    if kind == "statement":
        goal = obligation.get("pattern")
        checker.check_pattern(goal, case.get("symbols", {}),
                              set(case.get("refinements", {})))
        return goal
    fail("obligation", f"unknown obligation kind {kind!r}")


# --- checking --------------------------------------------------------------

def check_case(path, root=None):
    if root is None:
        root = Path(__file__).resolve().parents[2]
    return check_case_data(load_case(path), Path(path).stem, root)


def check_case_data(case, name, root):
    checker = load_slice_checker(root)
    expect = case.get("expect", "accept")
    started = time.perf_counter()

    def finish(**fields):
        elapsed = (time.perf_counter() - started) * 1000
        fields.setdefault("case", name)
        fields.setdefault("expect", expect)
        fields["elapsed_ms"] = round(elapsed, 3)
        return fields

    try:
        goal = build_goal(case, checker)
    except (Reject, checker.Reject) as problem:
        return finish(verdict="reject",
                      diagnostic={"rule": "obligation",
                                  "detail": str(problem)})
    try:
        certificate, tree_nodes = translate_export(case["export"])
    except Reject as problem:
        return finish(verdict="reject", goal=goal,
                      diagnostic=problem.args[0])

    # Exact source axiom closure: cited axioms must equal the declared
    # closure, and every cited axiom must be a declared case axiom.
    cited = cited_axioms(case["export"]["steps"])
    declared_closure = set(case.get("axiom_closure", []))
    declared_axioms = set(case.get("axioms", {}))
    if not cited <= declared_axioms:
        return finish(
            verdict="reject", goal=goal,
            diagnostic={"rule": "undeclared-axiom",
                        "detail": f"cited outside case axioms: "
                                  f"{sorted(cited - declared_axioms)}"})
    if not cited <= declared_closure:
        return finish(
            verdict="reject", goal=goal,
            diagnostic={"rule": "axiom-outside-closure",
                        "detail": f"cited outside declared closure: "
                                  f"{sorted(cited - declared_closure)}"})
    if cited != declared_closure:
        return finish(
            verdict="reject", goal=goal,
            diagnostic={"rule": "closure-overstated",
                        "detail": f"declared but never cited: "
                                  f"{sorted(declared_closure - cited)}"})

    # Checked proof-object translation: replay the translated certificate
    # with the real bounded-slice checker, not this importer's own judgment.
    try:
        conclusion, admissions = checker.check_certificate(case, certificate)
    except checker.Reject as problem:
        return finish(verdict="reject", goal=goal,
                      diagnostic={"rule": "translation-recheck",
                                  "detail": problem.args[0]})
    if conclusion != goal:
        return finish(
            verdict="reject", goal=goal,
            diagnostic={"rule": "goal",
                        "detail": f"translated proof derives {conclusion!r}, "
                                  f"goal is {goal!r}"})

    export_bytes = len(json.dumps(case["export"]).encode())
    certificate_bytes = len(json.dumps(certificate).encode())
    used_rules = sorted({
        step.get("rule") for step in case["export"]["steps"]
        if isinstance(step, dict)})
    bridge = {rule: BRIDGE[rule] for rule in used_rules if rule in BRIDGE}
    return finish(
        verdict="accept", goal=goal, admissions=admissions,
        axiom_closure=sorted(cited),
        observation_profile={
            "export_bytes": export_bytes,
            "certificate_bytes": certificate_bytes,
            "export_steps": len(case["export"]["steps"]),
            "certificate_nodes": tree_nodes,
        },
        bridge_graph=bridge)


def source_lines(path):
    return sum(1 for line in Path(path).read_text().splitlines())


def build_record(root):
    cases_dir = root / "tools" / "matching-logic-external-proof-import" / "cases"
    results = [check_case(p, root) for p in sorted(cases_dir.glob("*.json"))]
    positive = [r for r in results if r["expect"] == "accept"]
    negative = [r for r in results if r["expect"] == "reject"]
    divergences = [
        r["case"] for r in results
        if r["verdict"] != ("accept" if r["expect"] == "accept" else "reject")
    ]
    tool = root / "tools" / "matching-logic-external-proof-import" / \
        "external_proof_import.py"
    draft = root / SOURCE_DRAFT
    return {
        "schema": RECORD_SCHEMA,
        "fragment": FRAGMENT,
        "rule_version": EXPORT_SCHEMA,
        "semantics_version":
            hashlib.sha256(draft.read_bytes()).hexdigest()[:16],
        "semantics_doc": SOURCE_DRAFT,
        "design_doc": SEMANTICS_DOC,
        "recorded_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "importer": {
            "path": "tools/matching-logic-external-proof-import/"
                    "external_proof_import.py",
            "physical_lines": source_lines(tool),
            "sha256": hashlib.sha256(tool.read_bytes()).hexdigest(),
            "bridge": BRIDGE,
        },
        "translation": "checked: every accepted case replays the translated "
                       "certificate through tools/matching-logic-slice/"
                       "slice_checker.py",
        "cases": results,
        "summary": {
            "total": len(results),
            "positive": len(positive),
            "negative": len(negative),
            "divergences": divergences,
        },
    }


def main(argv=None):
    parser = argparse.ArgumentParser(prog="external_proof_import")
    parser.add_argument("command", choices=("check", "record"))
    parser.add_argument("case", nargs="?")
    parser.add_argument("--out", default=None)
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parents[2]

    if args.command == "record":
        record = build_record(root)
        text = json.dumps(record, indent=2, sort_keys=True) + "\n"
        out = args.out or root / "tools" / \
            "matching-logic-external-proof-import" / "record.json"
        Path(out).write_text(text)
        print(text, end="")
        return 1 if record["summary"]["divergences"] else 0

    if not args.case:
        parser.error("check needs a case path")
    try:
        result = check_case(args.case, root)
    except Reject as problem:
        print(json.dumps({"verdict": "reject", "diagnostic": problem.args[0]},
                         indent=2, sort_keys=True))
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    expected = "accept" if result["expect"] == "accept" else "reject"
    return 0 if result["verdict"] == expected else 1


if __name__ == "__main__":
    sys.exit(main())
