#!/usr/bin/env python3
"""Bounded vertical-slice checker for one-sorted finitary basic matching logic.

Implements the candidate side of the bounded comparison drafted in
``wiki/drafts/matching_logic.md``: one real vertical slice with scalar
propositions, equality, quantification, a Terminal state transition, and a
refinement obligation the checker reconstructs from the canonical subject.

The fragment is the one the cited completeness result covers: a single sort,
finitary symbols, no fixpoint symbols. The tool is a certificate checker, not
a prover — the draft licenses sound checking of finite certificates in an
intentionally incomplete fragment, and every theory clause is recorded as an
axiom admission rather than discharged.

Patterns are JSON trees:

    ["top"] | ["bottom"]
    ["var", "x"]                          bound/eigenvariable occurrences
    ["sym", "name", [arg, ...]]           finitary symbol application (term)
    ["pred", "name", [arg, ...]]          atomic proposition (e.g. step)
    ["and", p, q] | ["or", p, q] | ["not", p] | ["implies", p, q]
    ["exists", "x", p] | ["forall", "x", p]
    ["equals", t, s]                      term equality
    ["in", t, "Sort"]                     sort membership (encoded refinement)
    ["def", t]                            definedness of a term

The case file declares the theory (symbols, sort refinements, axioms, the
named Terminal transition predicate) and an obligation descriptor. The
checker rebuilds the goal pattern from that descriptor — a producer cannot
supply a weaker question — then verifies the supplied certificate node by
node and reports the admissions it consumed.

Requires Python 3.9+ and no third-party packages. From the repository root:

    python3 tools/matching-logic-slice/slice_checker.py check \
        tools/matching-logic-slice/cases/reference.json

    python3 tools/matching-logic-slice/slice_checker.py record

``check`` prints a JSON verdict and exits nonzero when a positive case
rejects or a negative case accepts. ``record`` runs every pinned case and
writes the measured columns the comparison record consumes.
"""

import argparse
import hashlib
import json
from pathlib import Path
import sys
import time


CASE_SCHEMA = "omega-matching-logic-slice-case/1"
CERT_SCHEMA = "omega-matching-logic-certificate/1"
RECORD_SCHEMA = "omega-matching-logic-slice-record/1"
FRAGMENT = "one-sorted finitary basic matching logic, no fixpoint symbols"
SEMANTICS_DOC = "wiki/drafts/matching_logic.md"

RULES = (
    "axiom", "assumption", "hypothesis", "implies_intro", "modus_ponens",
    "and_intro", "and_elim_left", "and_elim_right",
    "or_intro_left", "or_intro_right", "or_elim",
    "exists_intro", "exists_elim", "forall_intro", "forall_elim",
    "eq_refl", "eq_sym", "eq_trans", "eq_subst",
    "membership_intro", "defined_closed", "defined_membership",
)


class Reject(Exception):
    pass


def fail(rule, detail):
    raise Reject({"rule": rule, "detail": detail})


# --- pattern algebra -------------------------------------------------------

def is_term(pattern):
    return (isinstance(pattern, list)
            and pattern and pattern[0] in ("var", "sym"))


def free_vars(pattern, bound=frozenset()):
    if not isinstance(pattern, list) or not pattern:
        return set()
    head = pattern[0]
    if head == "var":
        return set() if pattern[1] in bound else {pattern[1]}
    if head in ("sym", "pred"):
        out = set()
        for arg in pattern[2]:
            out |= free_vars(arg, bound)
        return out
    if head in ("exists", "forall"):
        return free_vars(pattern[2], bound | {pattern[1]})
    out = set()
    for part in pattern[1:]:
        out |= free_vars(part, bound)
    return out


def subst(pattern, var, term):
    """Capture-avoiding substitution of a term for a variable."""
    if not isinstance(pattern, list) or not pattern:
        return pattern
    head = pattern[0]
    if head == "var":
        return term if pattern[1] == var else pattern
    if head in ("sym", "pred"):
        return [head, pattern[1],
                [subst(a, var, term) for a in pattern[2]]]
    if head in ("exists", "forall"):
        binder, body = pattern[1], pattern[2]
        if binder == var:
            return pattern
        if binder in free_vars(term):
            fresh = binder + "'"
            while fresh in free_vars(body) or fresh in free_vars(term):
                fresh += "'"
            body = subst(body, binder, ["var", fresh])
            binder = fresh
        return [head, binder, subst(body, var, term)]
    return [head] + [subst(p, var, term) for p in pattern[1:]]


def check_pattern(pattern, symbols, sorts):
    """Structural validation: arities, declared symbols/sorts, term-ness."""
    if not isinstance(pattern, list) or not pattern:
        fail("pattern", f"malformed pattern {pattern!r}")
    head = pattern[0]
    if head == "var":
        if len(pattern) != 2 or not isinstance(pattern[1], str):
            fail("pattern", f"malformed variable {pattern!r}")
        return
    if head == "sym":
        name, args = pattern[1], pattern[2]
        if symbols.get(name) != len(args):
            fail("pattern",
                 f"symbol {name} expects {symbols.get(name)} args, "
                 f"got {len(args)}")
        for arg in args:
            if not is_term(arg):
                fail("pattern", f"non-term argument {arg!r} in {name}")
            check_pattern(arg, symbols, sorts)
        return
    if head == "pred":
        for arg in pattern[2]:
            if not is_term(arg):
                fail("pattern", f"non-term argument {arg!r} in {pattern[1]}")
            check_pattern(arg, symbols, sorts)
        return
    if head in ("and", "or", "implies"):
        check_pattern(pattern[1], symbols, sorts)
        check_pattern(pattern[2], symbols, sorts)
        return
    if head == "not":
        check_pattern(pattern[1], symbols, sorts)
        return
    if head in ("exists", "forall"):
        if not isinstance(pattern[1], str):
            fail("pattern", f"malformed binder {pattern!r}")
        check_pattern(pattern[2], symbols, sorts)
        return
    if head in ("equals",):
        for part in pattern[1], pattern[2]:
            if not is_term(part):
                fail("pattern", f"equality side is not a term: {part!r}")
            check_pattern(part, symbols, sorts)
        return
    if head == "in":
        if not is_term(pattern[1]):
            fail("pattern", f"membership subject is not a term: {pattern!r}")
        check_pattern(pattern[1], symbols, sorts)
        if pattern[2] not in sorts:
            fail("pattern", f"undeclared sort {pattern[2]!r}")
        return
    if head == "def":
        if not is_term(pattern[1]):
            fail("pattern", f"definedness subject is not a term: {pattern!r}")
        check_pattern(pattern[1], symbols, sorts)
        return
    if head in ("top", "bottom"):
        return
    fail("pattern", f"unknown pattern head {head!r}")


# --- case loading and goal reconstruction ----------------------------------

def load_case(path):
    case = json.loads(Path(path).read_text())
    if case.get("schema") != CASE_SCHEMA:
        fail("schema", f"case schema must be {CASE_SCHEMA!r}")
    return case


def build_goal(case):
    """Reconstruct the refinement obligation from the canonical subject.

    The slice's obligation is the one the draft names: after the declared
    Terminal state transition fires from the pinned pre-state, there exists
    a successor state on which the reconstructed refinement holds. The
    producer never sees the goal; it proves what this derivation emits.
    """
    obligation = case.get("obligation", {})
    if obligation.get("kind") != "refinement_after_transition":
        fail("obligation",
             f"unknown obligation kind {obligation.get('kind')!r}")
    pre = obligation["pre_state"]
    pred = obligation["transition"]
    post_var = obligation["post_variable"]
    refinement = obligation["refinement"]
    goal = ["exists", post_var,
            ["and",
             ["pred", pred, [["sym", pre, []], ["var", post_var]]],
             refinement]]
    check_pattern(goal, case.get("symbols", {}),
                  set(case.get("refinements", {})))
    return goal


# --- certificate checking ---------------------------------------------------

def check_certificate(case, certificate):
    symbols = case.get("symbols", {})
    sorts = set(case.get("refinements", {}))
    refinements = case.get("refinements", {})
    axioms = case.get("axioms", {})
    assumptions = case.get("assumptions", {})
    admissions = []

    def derive(node, scope):
        """Return the pattern the node proves; raise Reject otherwise."""
        if not isinstance(node, dict):
            fail("certificate", f"malformed node {node!r}")
        rule = node.get("rule")
        if rule not in RULES:
            fail("certificate", f"unknown rule {rule!r}")

        if rule == "axiom":
            name = node.get("name")
            if name not in axioms:
                fail("axiom", f"undeclared axiom {name!r}")
            pattern = axioms[name]
            for var, term in (node.get("subst") or {}).items():
                if not is_term(term):
                    fail("axiom",
                         f"substitution for {var} is not a term: {term!r}")
                pattern = subst(pattern, var, term)
            admissions.append(f"axiom:{name}")
            return pattern

        if rule == "assumption":
            name = node.get("name")
            if name not in assumptions:
                fail("assumption", f"undeclared assumption {name!r}")
            admissions.append(f"assumption:{name}")
            return assumptions[name]

        if rule == "hypothesis":
            index = node.get("index")
            if index is None or index >= len(scope):
                fail("hypothesis", f"no open hypothesis at index {index!r}")
            return scope[index]

        if rule == "implies_intro":
            assumed = node.get("assumes")
            check_pattern(assumed, symbols, sorts)
            body = derive(node.get("premise"), scope + [assumed])
            return ["implies", assumed, body]

        if rule == "modus_ponens":
            imp = derive(node.get("implication"), scope)
            arg = derive(node.get("argument"), scope)
            if not (isinstance(imp, list) and imp and imp[0] == "implies"):
                fail("modus_ponens",
                     f"implication premise derived {imp!r}")
            if imp[1] != arg:
                fail("modus_ponens",
                     f"antecedent {imp[1]!r} != derived {arg!r}")
            return imp[2]

        if rule == "and_intro":
            return ["and", derive(node.get("left"), scope),
                    derive(node.get("right"), scope)]

        if rule in ("and_elim_left", "and_elim_right"):
            conj = derive(node.get("premise"), scope)
            if not (isinstance(conj, list) and conj and conj[0] == "and"):
                fail(rule, f"premise is not a conjunction: {conj!r}")
            return conj[1] if rule == "and_elim_left" else conj[2]

        if rule in ("or_intro_left", "or_intro_right"):
            side = derive(node.get("premise"), scope)
            other = node.get("other")
            check_pattern(other, symbols, sorts)
            return (["or", side, other] if rule == "or_intro_left"
                    else ["or", other, side])

        if rule == "or_elim":
            disj = derive(node.get("disjunction"), scope)
            if not (isinstance(disj, list) and disj and disj[0] == "or"):
                fail("or_elim", f"premise is not a disjunction: {disj!r}")
            left = derive(node.get("left"), scope)
            right = derive(node.get("right"), scope)
            if left != ["implies", disj[1], node.get("conclusion")]:
                fail("or_elim",
                     f"left case must derive {disj[1]!r} -> conclusion")
            if right != ["implies", disj[2], node.get("conclusion")]:
                fail("or_elim",
                     f"right case must derive {disj[2]!r} -> conclusion")
            check_pattern(node["conclusion"], symbols, sorts)
            return node["conclusion"]

        if rule == "exists_intro":
            var = node.get("var")
            body = node.get("body")
            witness = node.get("witness")
            if not isinstance(var, str) or not is_term(witness):
                fail("exists_intro", "needs a binder var and a term witness")
            check_pattern(["exists", var, body], symbols, sorts)
            defined = derive(node.get("defined"), scope)
            if defined != ["def", witness]:
                fail("exists_intro",
                     f"witness needs definedness, got {defined!r}")
            premise = derive(node.get("premise"), scope)
            if premise != subst(body, var, witness):
                fail("exists_intro",
                     f"premise {premise!r} is not {body!r}[{var}:={witness!r}]")
            return ["exists", var, body]

        if rule == "exists_elim":
            ex = derive(node.get("premise"), scope)
            if not (isinstance(ex, list) and ex and ex[0] == "exists"):
                fail("exists_elim", f"premise is not existential: {ex!r}")
            var, body = ex[1], ex[2]
            imp = derive(node.get("elimination"), scope)
            conclusion = node.get("conclusion")
            check_pattern(conclusion, symbols, sorts)
            if var in free_vars(conclusion):
                fail("exists_elim",
                     f"eigenvariable {var} escapes into {conclusion!r}")
            if imp != ["forall", var, ["implies", body, conclusion]]:
                fail("exists_elim",
                     f"elimination must derive forall {var}. {body!r} -> "
                     f"{conclusion!r}")
            return conclusion

        if rule == "forall_intro":
            var = node.get("var")
            if not isinstance(var, str):
                fail("forall_intro", "needs a binder var")
            for open_hyp in scope:
                if var in free_vars(open_hyp):
                    fail("forall_intro",
                         f"eigenvariable {var} free in open hypothesis "
                         f"{open_hyp!r}")
            body = derive(node.get("premise"), scope)
            return ["forall", var, body]

        if rule == "forall_elim":
            prem = derive(node.get("premise"), scope)
            if not (isinstance(prem, list) and prem and prem[0] == "forall"):
                fail("forall_elim", f"premise is not universal: {prem!r}")
            witness = node.get("witness")
            if not is_term(witness):
                fail("forall_elim", f"witness is not a term: {witness!r}")
            defined = derive(node.get("defined"), scope)
            if defined != ["def", witness]:
                fail("forall_elim",
                     f"witness needs definedness, got {defined!r}")
            return subst(prem[2], prem[1], witness)

        if rule == "eq_refl":
            term = node.get("term")
            if not is_term(term):
                fail("eq_refl", f"not a term: {term!r}")
            check_pattern(term, symbols, sorts)
            return ["equals", term, term]

        if rule == "eq_sym":
            prem = derive(node.get("premise"), scope)
            if not (isinstance(prem, list) and prem and prem[0] == "equals"):
                fail("eq_sym", f"premise is not an equality: {prem!r}")
            return ["equals", prem[2], prem[1]]

        if rule == "eq_trans":
            left = derive(node.get("left"), scope)
            right = derive(node.get("right"), scope)
            if not (isinstance(left, list) and left and left[0] == "equals"
                    and isinstance(right, list) and right
                    and right[0] == "equals"):
                fail("eq_trans", "premises must be equalities")
            if left[2] != right[1]:
                fail("eq_trans",
                     f"shared term mismatch: {left[2]!r} vs {right[1]!r}")
            return ["equals", left[1], right[2]]

        if rule == "eq_subst":
            eq = derive(node.get("equality"), scope)
            if not (isinstance(eq, list) and eq and eq[0] == "equals"):
                fail("eq_subst", f"premise is not an equality: {eq!r}")
            var = node.get("var")
            body = node.get("pattern")
            check_pattern(body, symbols, sorts)
            derived = derive(node.get("premise"), scope)
            direction = node.get("direction", "right")
            source, target = (eq[2], eq[1]) if direction == "right" \
                else (eq[1], eq[2])
            if derived != subst(body, var, source):
                fail("eq_subst",
                     f"premise {derived!r} is not {body!r} with "
                     f"{source!r} for {var}")
            return subst(body, var, target)

        if rule == "membership_intro":
            term = node.get("term")
            sort = node.get("sort")
            if sort not in refinements:
                fail("membership_intro", f"undeclared sort {sort!r}")
            if not is_term(term):
                fail("membership_intro", f"not a term: {term!r}")
            body = refinements[sort]
            derived = derive(node.get("premise"), scope)
            if derived != subst(body, "x", term):
                fail("membership_intro",
                     f"premise {derived!r} is not the {sort} refinement "
                     f"for {term!r}")
            return ["in", term, sort]

        if rule == "defined_closed":
            term = node.get("term")
            if not is_term(term):
                fail("defined_closed", f"not a term: {term!r}")
            if free_vars(term):
                fail("defined_closed",
                     f"open term needs evidence: {term!r}")
            check_pattern(term, symbols, sorts)
            return ["def", term]

        if rule == "defined_membership":
            prem = derive(node.get("premise"), scope)
            if not (isinstance(prem, list) and prem and prem[0] == "in"):
                fail("defined_membership",
                     f"premise is not membership: {prem!r}")
            return ["def", prem[1]]

        fail("certificate", f"unhandled rule {rule!r}")

    if certificate.get("schema") != CERT_SCHEMA:
        fail("schema", f"certificate schema must be {CERT_SCHEMA!r}")
    conclusion = derive(certificate.get("derivation"), [])
    return conclusion, admissions


def check_case(path):
    case = load_case(path)
    certificate = case.get("certificate")
    expect = case.get("expect", "accept")
    goal = build_goal(case)
    started = time.perf_counter()
    try:
        conclusion, admissions = check_certificate(case, certificate)
    except Reject as problem:
        elapsed = (time.perf_counter() - started) * 1000
        return {
            "case": Path(path).stem, "expect": expect, "goal": goal,
            "verdict": "reject", "diagnostic": problem.args[0],
            "elapsed_ms": round(elapsed, 3),
        }
    elapsed = (time.perf_counter() - started) * 1000
    if conclusion != goal:
        return {
            "case": Path(path).stem, "expect": expect, "goal": goal,
            "verdict": "reject",
            "diagnostic": {
                "rule": "goal",
                "detail": f"certificate proves {conclusion!r}, goal is "
                          f"{goal!r}",
            },
            "elapsed_ms": round(elapsed, 3),
        }
    certificate_bytes = len(json.dumps(certificate).encode())
    return {
        "case": Path(path).stem, "expect": expect, "goal": goal,
        "verdict": "accept", "admissions": admissions,
        "certificate_bytes": certificate_bytes,
        "elapsed_ms": round(elapsed, 3),
    }


def source_lines(path):
    return sum(1 for line in Path(path).read_text().splitlines())


def build_record(root):
    cases_dir = root / "tools" / "matching-logic-slice" / "cases"
    results = [check_case(p) for p in sorted(cases_dir.glob("*.json"))]
    positive = [r for r in results if r["expect"] == "accept"]
    negative = [r for r in results if r["expect"] == "reject"]
    divergences = [
        r["case"] for r in results
        if r["verdict"] != ("accept" if r["expect"] == "accept" else "reject")
    ]
    tool = root / "tools" / "matching-logic-slice" / "slice_checker.py"
    return {
        "schema": RECORD_SCHEMA,
        "fragment": FRAGMENT,
        "semantics_doc": SEMANTICS_DOC,
        "recorded_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "checker": {
            "path": "tools/matching-logic-slice/slice_checker.py",
            "physical_lines": source_lines(tool),
            "sha256": hashlib.sha256(tool.read_bytes()).hexdigest(),
            "rules": list(RULES),
        },
        "theory": "per-case axiom inventory; every clause is an admission",
        "cases": results,
        "summary": {
            "total": len(results),
            "positive": len(positive),
            "negative": len(negative),
            "divergences": divergences,
        },
    }


def main(argv=None):
    parser = argparse.ArgumentParser(prog="slice_checker")
    parser.add_argument("command", choices=("check", "record"))
    parser.add_argument("case", nargs="?")
    parser.add_argument("--out", default=None)
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parents[2]

    if args.command == "record":
        record = build_record(root)
        text = json.dumps(record, indent=2, sort_keys=True) + "\n"
        out = args.out or root / "tools" / "matching-logic-slice" / "record.json"
        Path(out).write_text(text)
        print(text, end="")
        return 1 if record["summary"]["divergences"] else 0

    if not args.case:
        parser.error("check needs a case path")
    result = check_case(args.case)
    print(json.dumps(result, indent=2, sort_keys=True))
    expected = "accept" if result["expect"] == "accept" else "reject"
    return 0 if result["verdict"] == expected else 1


if __name__ == "__main__":
    sys.exit(main())
