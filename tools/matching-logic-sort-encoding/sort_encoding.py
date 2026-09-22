#!/usr/bin/env python3
"""Encode a typed subject schema into one-sorted finitary basic matching logic.

Implements the bounded typed-to-one-sorted encoding leg drafted in
``wiki/drafts/reference/matching_logic_sort_encoding.md``: Omega's typed carriers are
emitted as membership/refinement patterns over a single sort, with explicit
disjointness, definedness, junk-model, revision, and loan clauses.

Every emitted clause is an axiom admission — the encoding relocates trust to
the clause set rather than discharging it — so ``check`` reports the clause
inventory together with the consistency diagnostics the comparison's evidence
requires. Nothing here is admitted authority over Omega proofs; the output is
comparison input, not a checked translation.

Requires Python 3.9+ and no third-party packages. From the repository root:

    python3 tools/matching-logic-sort-encoding/sort_encoding.py encode \
        tools/matching-logic-sort-encoding/cases/reference.json

    python3 tools/matching-logic-sort-encoding/sort_encoding.py check \
        tools/matching-logic-sort-encoding/cases/reference.json

``encode`` prints the clause inventory and evidence record as JSON.
``check`` prints the same report plus diagnostics and exits nonzero when any
consistency rule fails.
"""

import argparse
import hashlib
import json
from pathlib import Path
import sys


SCHEMA = "omega-sort-encoding-record/1"
CASE_SCHEMA = "omega-sort-encoding-case/1"
FRAGMENT = "one-sorted finitary basic matching logic, no fixpoint symbols"
SEMANTICS_DOC = "wiki/drafts/reference/matching_logic_sort_encoding.md"

EXCLUSIVE_LOAN_KINDS = ("mut", "write")


def diagnostic(rule, detail):
    return {"rule": rule, "detail": detail}


def clause(kind, text, obligation=None):
    row = {"kind": kind, "pattern": text}
    if obligation:
        row["obligation"] = obligation
    return row


def integer_refinement(name, low, high, exclusive_high=False):
    bound = f"x < {high}" if exclusive_high else f"x <= {high}"
    return f"{name}(x) == Integer(x) & {low} <= x & {bound}"


def encode_memberships(case):
    """Emit the membership/refinement clauses for each declared type family."""
    types = case.get("types", {})
    clauses = []
    memberships = set()
    declared = set()

    integers = types.get("integers") or {}
    ranged = types.get("ranged", [])
    # Slice lengths are Nat members regardless of the pointee type.
    uses_integer = integers or ranged or types.get("slices")
    if uses_integer:
        clauses.append(clause(
            "membership", "Integer(x)",
            "closure of arithmetic over members; the only shared base "
            "membership under the range-refinement reading"))
        memberships.add("Integer")
        declared.add("Integer")
        clauses.append(clause(
            "refinement", "Nat(x) == Integer(x) & 0 <= x",
            "totality of constructors; signed Int members below 0 are "
            "not Nat members (Int is not Nat's nonneg half)"))
        memberships.add("Nat")
        declared.add("Nat")
        for width in integers.get("signed_widths", []):
            name = f"Int{width}"
            clauses.append(clause(
                "refinement",
                integer_refinement(name, f"-2^{width - 1}",
                                   f"2^{width - 1}", exclusive_high=True),
                f"signed {width}-bit range; not disjoint from Nat by "
                "membership but by refinement bounds"))
            memberships.add(name)
            declared.add(name)
        for width in integers.get("unsigned_widths", []):
            name = f"UInt{width}"
            clauses.append(clause(
                "refinement",
                integer_refinement(name, 0, f"2^{width}",
                                   exclusive_high=True),
                f"unsigned {width}-bit range"))
            memberships.add(name)
            declared.add(name)
        for entry in ranged:
            clauses.append(clause(
                "refinement",
                integer_refinement(entry["name"], entry["low"],
                                   entry["high"]),
                "bound predicates on the same one-sorted integers; "
                "revised at each bound change"))
            memberships.add(entry["name"])
            declared.add(entry["name"])

    if types.get("addresses"):
        clauses.append(clause(
            "membership", "Addr(x)",
            "interpreted under the target capsule's address model; "
            "membership grants no storage or access authority"))
        memberships.add("Addr")
        declared.add("Addr")

    slices = types.get("slices", [])
    sums = types.get("sums", [])
    if slices or sums:
        clauses.append(clause(
            "pair", "x = <b, l>",
            "the pair constructor must be injective: "
            "<b1,l1> = <b2,l2> implies b1 = b2 and l1 = l2"))

    for entry in slices:
        element = entry["element"]
        membership = type_membership(element)
        memberships.add(membership)
        memberships.add(entry["name"])
        memberships.add("Addr")
        memberships.add("Nat")
        declared.add(entry["name"])
        clauses.append(clause(
            "membership",
            f"{entry['name']}(x) <-> exists b l. x = <b, l> & Addr(b) & "
            f"Nat(l) & forall i. (0 <= i & i < l) -> {membership}(b + i)",
            "bounded quantifier over the pointee region; the definedness "
            "guard is i < l on b + i"))

    for entry in sums:
        arms = entry["arms"]
        memberships.add(entry["name"])
        declared.add(entry["name"])
        for arm in arms:
            memberships.add(f"Tag_{entry['name']}_{arm['tag']}")
            if arm.get("payload"):
                memberships.add(type_membership(arm["payload"]))
        disjunction = " | ".join(
            f"(Tag_{entry['name']}_{arm['tag']}(x) & "
            f"{type_membership(arm['payload'])}(payload(x)))"
            if arm.get("payload")
            else f"Tag_{entry['name']}_{arm['tag']}(x)"
            for arm in arms)
        clauses.append(clause(
            "membership", f"{entry['name']}(x) <-> {disjunction}",
            "pairwise tag disjointness, exhaustiveness, payload "
            "membership per arm"))
        for left in range(len(arms)):
            for right in range(left + 1, len(arms)):
                clauses.append(clause(
                    "disjointness",
                    f"Tag_{entry['name']}_{arms[left]['tag']}(x) -> "
                    f"not Tag_{entry['name']}_{arms[right]['tag']}(x)"))

    for loan in case.get("loans", []):
        memberships.add(f"Loan_{loan['kind']}")
    return clauses, sorted(memberships), declared


def type_membership(name):
    if name == "nat":
        return "Nat"
    if name.startswith("u") and name[1:].isdigit():
        return f"UInt{name[1:]}"
    if name.startswith("i") and name[1:].isdigit():
        return f"Int{name[1:]}"
    return name


def encode_operations(case):
    clauses = []
    for operation in case.get("operations", []):
        guard = operation.get("definedness")
        if not guard:
            continue
        clauses.append(clause(
            "definedness",
            f"{operation['name']}: precondition [{guard}]",
            "a partial operation is a definedness precondition, not a "
            "typing rule; the unguarded form is unprovable or vacuous"))
    return clauses


def encode_loans(case):
    clauses = []
    kinds = {loan["kind"] for loan in case.get("loans", [])}
    for kind in sorted(kinds):
        clauses.append(clause(
            "membership", f"Loan_{kind}(x)",
            "ownership is a loan class over the same carrier, not a "
            "value shape"))
    for kind in EXCLUSIVE_LOAN_KINDS:
        if kind in kinds:
            clauses.append(clause(
                "disjointness",
                f"Loan_{kind}(x) over region r -> no other live Loan over r",
                "exclusive loans may not duplicate; shared loans may"))
    if any(loan.get("parent") for loan in case.get("loans", [])):
        clauses.append(clause(
            "lineage", "ChildOf(x, y)",
            "a reborrow inherits the parent's region membership"))
    return clauses


def check_case(case, memberships, declared):
    diagnostics = []
    types = case.get("types", {})
    for operation in case.get("operations", []):
        if not operation.get("definedness"):
            diagnostics.append(diagnostic(
                "missing-definedness-precondition",
                f"operation {operation['name']!r} is partial but carries "
                "no definedness precondition; the encoded obligation is "
                "unprovable or vacuously satisfiable"))

    inhabitants = case.get("inhabitants", {})
    for membership in memberships:
        if inhabitants.get(membership, 0) < 1:
            diagnostics.append(diagnostic(
                "vacuous-membership",
                f"membership class {membership} has no inhabitant in the "
                "intended model; universals over it are vacuously true"))

    for revision in case.get("revisions", []):
        if revision["tightened_upper"] > revision["prior_upper"]:
            diagnostics.append(diagnostic(
                "revision-not-refinement",
                f"revision of {revision['element']!r} raises the bound "
                f"{revision['prior_upper']} -> {revision['tightened_upper']}; "
                "a revision retains the weaker assertion, it does not "
                "substitute a new one"))
        lower = revision.get("tightened_lower")
        if lower is not None and lower < revision["prior_lower"]:
            diagnostics.append(diagnostic(
                "revision-not-refinement",
                f"revision of {revision['element']!r} lowers the bound "
                f"{revision['prior_lower']} -> {lower}"))

    live_by_region = {}
    elements = {loan["element"] for loan in case.get("loans", [])}
    regions = set(case.get("regions", []))
    for loan in case.get("loans", []):
        region = loan["region"]
        live_by_region.setdefault(region, []).append(loan)
        parent = loan.get("parent")
        if parent is not None and parent not in elements and (
                parent not in regions):
            diagnostics.append(diagnostic(
                "reborrow-without-lineage",
                f"loan {loan['element']!r} names parent {parent!r} which is "
                "neither a live loan element nor a declared region; "
                "ChildOf(x, y) needs a parent carrying region membership"))
    for region, loans in live_by_region.items():
        exclusive = [loan for loan in loans
                     if loan["kind"] in EXCLUSIVE_LOAN_KINDS]
        if len(exclusive) > 1 or (exclusive and len(loans) > 1):
            diagnostics.append(diagnostic(
                "exclusive-loan-duplicated",
                f"region {region!r} carries "
                f"{len(loans)} live loans including exclusive kind(s) "
                f"{sorted(loan['kind'] for loan in exclusive)}"))

    if (types.get("slices") or types.get("sums")) and not (
            case.get("pair_constructor_injective")):
        diagnostics.append(diagnostic(
            "pair-constructor-not-injective",
            "slices/sums are encoded through the pair constructor but the "
            "case does not assert injectivity"))

    for entry in types.get("slices", []):
        if type_membership(entry["element"]) not in declared:
            diagnostics.append(diagnostic(
                "unknown-slice-element-membership",
                f"slice {entry['name']!r} element {entry['element']!r} "
                "has no declared membership for the pointee quantifier"))

    for entry in types.get("sums", []):
        tags = [arm["tag"] for arm in entry["arms"]]
        if len(set(tags)) != len(tags):
            diagnostics.append(diagnostic(
                "sum-tags-not-disjoint",
                f"sum {entry['name']!r} repeats a tag"))
        for arm in entry["arms"]:
            payload = arm.get("payload")
            if payload and type_membership(payload) not in declared:
                diagnostics.append(diagnostic(
                    "unknown-payload-membership",
                    f"sum {entry['name']!r} arm {arm['tag']!r} has payload "
                    f"{payload!r} outside every declared membership"))

    for fixpoint in case.get("fixpoints", []):
        if not fixpoint.get("certificate"):
            diagnostics.append(diagnostic(
                "unguarded-fixpoint",
                f"fixpoint symbol {fixpoint['name']!r} appears without a "
                "bounded certificate; the fragment admits none"))

    return diagnostics


def evidence(case, clauses, memberships, declared, diagnostics,
             semantics_digest):
    return {
        "schema": SCHEMA,
        "fragment": FRAGMENT,
        "rule_version": SCHEMA,
        "semantics_version": semantics_digest,
        "subject": {
            "name": case["subject"],
            "case_digest": hashlib.sha256(
                json.dumps(case, sort_keys=True).encode()).hexdigest(),
        },
        "target_capsule": case.get("target", {}),
        "observation_profile": [
            "clause inventory emission",
            "definedness precondition coverage",
            "intended-model inhabitedness (junk guard)",
            "revision refinement",
            "loan disjointness and reborrow lineage",
            "pair-constructor injectivity",
            "sum tag disjointness and payload membership",
            "fixpoint admission",
        ],
        "bridge_graph": memberships,
        "declared_type_memberships": sorted(declared),
        "admissions": [
            {"clause": row["pattern"], "kind": row["kind"]}
            for row in clauses],
        "diagnostics": diagnostics,
    }


def report(case, semantics_digest):
    clauses, memberships, declared = encode_memberships(case)
    clauses += encode_operations(case)
    clauses += encode_loans(case)
    diagnostics = check_case(case, memberships, declared)
    return evidence(case, clauses, memberships, declared, diagnostics,
                    semantics_digest)


def semantics_digest(root):
    path = Path(root) / SEMANTICS_DOC
    if not path.is_file():
        return "unavailable"
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def load_case(path):
    case = json.loads(Path(path).read_text(encoding="utf-8"))
    if case.get("schema") != CASE_SCHEMA:
        raise ValueError(
            f"case schema must be {CASE_SCHEMA!r}: {path}")
    return case


def repository_root():
    return Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("encode", "check"))
    parser.add_argument("case", help="subject case JSON file")
    args = parser.parse_args()
    try:
        case = load_case(args.case)
        result = report(case, semantics_digest(repository_root()))
    except (OSError, ValueError, KeyError) as error:
        print(f"sort_encoding: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2))
    if args.command == "check" and result["diagnostics"]:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
