//! Quotients of mathematical data.
//!
//! `data Sign = Representative % same_sign ...` is the kernel's quotient of
//! the data by the relation. Two classes are equal when their
//! representatives are related, by the kernel's soundness assumption for
//! quotients, and the three rows of the equivalence the quotient selects
//! must each prove exactly the law an equivalence owes.

use super::*;
use crate::laws::{SignatureJudgment, UnprovedLaw};

const CLASS: &str = include_str!("../../tests/fixtures/quotient/pass/equivalence_class/main.omg");
const WEAK_SYMMETRY: &str =
    include_str!("../../tests/fixtures/quotient/fail/symmetry_of_equal_values_only/main.omg");

fn proofs(source: &str) -> String {
    let start = source.find("use {omega}/").expect("the fixture's imports");
    let end = source.find("data Main").expect("the fixture's entry state");
    source[start..end].replace("use {omega}/language/core/process_exit.omg;\n", "")
}

const RELATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/relation.omg"
));

fn judged(source: &str) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[
            ("relation.omg", "MereEquivalence", RELATION),
            ("relation.omg", "Equivalence", RELATION),
            ("relation.omg", "IsMere", RELATION),
        ],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    crate::judge_compilation(compilation.program)
        .into_iter()
        .map(|verdict| verdict.expect("a judged part"))
        .collect()
}

fn signature(verdicts: &[RouteVerdict]) -> &SignatureJudgment {
    let mut laws = None;
    for verdict in verdicts {
        match verdict {
            RouteVerdict::Accepted(_) => {}
            RouteVerdict::Laws(judgment) => {
                assert!(laws.replace(&**judgment).is_none(), "one signature");
            }
            other => panic!("every part must be accepted: {other:?}"),
        }
    }
    laws.expect("the signature's judgment")
}

fn unproved(verdicts: &[RouteVerdict]) -> &UnprovedLaw {
    let mut laws = verdicts.iter().filter_map(|verdict| match verdict {
        RouteVerdict::UnprovedLaws(laws) => Some(laws),
        RouteVerdict::Laws(judgment) => panic!("the law must not be accepted: {judgment:?}"),
        _ => None,
    });
    let (Some([law]), None) = (laws.next().map(Vec::as_slice), laws.next()) else {
        panic!("expected exactly one unproved law: {verdicts:?}")
    };
    law
}

#[test]
fn related_representatives_have_one_class() {
    let verdicts = judged(&proofs(CLASS));
    let judgment = signature(&verdicts);
    let closure = |machine: &str| {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("the law `{machine}`: {judgment:?}"));
        let MathematicalCoreDecision::Judged(receipt) = law.decision;
        assert_eq!(receipt.context_depth, 0);
        eprintln!("{machine}: {} conversion steps", law.conversion_steps);
        receipt.assumption_closure
    };
    // All four laws of the relation (including `proofs_agree` via Hedberg's
    // theorem, with no UIP) are about the data alone and rest on zero assumptions.
    for row in [
        "SameSign::reflexive",
        "SameSign::symmetric",
        "SameSign::transitive",
        "SameSign::proofs_agree",
    ] {
        assert_eq!(closure(row), 0);
    }
    // The two numbers have different representatives, so their classes are
    // equal only by what the kernel assumes of every quotient: the quotient
    // type, the class of a value, and that related values have one class.
    assert_eq!(closure("related_representatives_have_one_class"), 3);
}

#[test]
fn a_symmetry_row_that_asks_for_more_is_refused() {
    let source = proofs(WEAK_SYMMETRY);
    assert_eq!(
        source,
        proofs(CLASS).replace(
            "requires same_sign(left, right) ensures same_sign(right, left) { }",
            "requires same_sign(left, right), |proof|::Identity::<Representative>(left, right) \
             ensures same_sign(right, left) { }"
        )
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "SameSign::symmetric");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.detail,
        "the proved equality is not the `symmetric` law that the quotient `Sign` needs of \
         `same_sign`"
    );
}

#[test]
fn unrelated_representatives_are_not_given_one_class() {
    // The same law with nothing saying the two numbers are related.
    let source = proofs(CLASS).replace("requires same_sign(a, b)\nensures", "ensures");
    assert_ne!(source, proofs(CLASS));
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "related_representatives_have_one_class");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `related_representatives_have_one_class`: \
         core\\Identity::<Sign>(quotient(Representative, same_sign, a), \
         quotient(Representative, same_sign, b))"
    );
}

const LIFTED: &str = include_str!("../../tests/fixtures/quotient/pass/lifted_operation/main.omg");
const SUBSTITUTED: &str =
    include_str!("../../tests/fixtures/quotient/fail/lift_selects_another_theorem/main.omg");
const LIFT: &str = "Quotient::lift::<normal, normal_respects, normal_transports>(value)";

/// Each judged law of `machine`, as its conversion steps and the number of
/// assumptions it rests on.
fn judged_laws(judgment: &SignatureJudgment, machine: &str) -> Vec<(u32, u32)> {
    judgment
        .laws
        .iter()
        .filter(|law| law.machine == machine)
        .map(|law| {
            let MathematicalCoreDecision::Judged(receipt) = law.decision;
            (law.conversion_steps, receipt.assumption_closure)
        })
        .collect()
}

#[test]
fn an_operation_on_a_quotient_is_judged_by_the_theorems_it_selects() {
    let verdicts = judged(&proofs(LIFTED));
    let judgment = signature(&verdicts);
    // The congruence is about representatives only. The transport states
    // what `normal_class` requires of a class, so each of its two laws
    // rests on the quotient type and the class of a value.
    let congruence = judged_laws(judgment, "normal_respects");
    let transport = judged_laws(judgment, "normal_transports");
    eprintln!("normal_respects {congruence:?}, normal_transports {transport:?}");
    assert!(matches!(congruence.as_slice(), [(_, 0)]), "{congruence:?}");
    assert!(
        matches!(transport.as_slice(), [(_, 2), (_, 2)]),
        "{transport:?}"
    );
    // The selected transport is all the operation owes: nothing is stated
    // in its own name.
    assert!(judged_laws(judgment, "normal_class").is_empty());
}

#[test]
fn a_lift_that_selects_another_theorem_is_refused() {
    let source = proofs(SUBSTITUTED);
    assert_eq!(
        source,
        proofs(LIFTED).replace(
            LIFT,
            "Quotient::lift::<normal, same_sign_symmetric, normal_transports>(value)"
        )
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "same_sign_symmetric");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.detail,
        "the proved equality is not the congruence `normal_class` needs: from related arguments \
         and what `normal` requires of each, related results of `normal`"
    );
}

#[test]
fn a_lift_with_no_transport_theorem_owes_what_its_operation_requires() {
    let source = proofs(LIFTED).replace(LIFT, "Quotient::lift::<normal, normal_respects>(value)");
    assert_ne!(source, proofs(LIFTED));
    let verdicts = judged(&source);
    let judgment = signature(&verdicts);
    // One law in the operation's own name: what it requires of a class
    // gives what `normal` requires of a representative.
    let owed = judged_laws(judgment, "normal_class");
    assert!(matches!(owed.as_slice(), [(_, 2)]), "{owed:?}");
}

#[test]
fn a_definition_on_a_quotient_owes_both_directions() {
    let source = proofs(LIFTED).replace(LIFT, "Quotient::define::<normal, normal_respects>(value)");
    assert_ne!(source, proofs(LIFTED));
    let verdicts = judged(&source);
    let judgment = signature(&verdicts);
    let owed = judged_laws(judgment, "normal_class");
    assert!(matches!(owed.as_slice(), [(_, 2), (_, 2)]), "{owed:?}");
}

#[test]
fn a_lift_is_refused_when_its_requirement_does_not_give_the_operations() {
    // `normal` now asks for a number whose sign is zero. Nothing
    // `normal_class` requires of a class says so.
    let source = proofs(LIFTED)
        .replace(LIFT, "Quotient::lift::<normal, normal_respects>(value)")
        .replace(
            "machine normal(value: Representative) -> Representative\n\
             requires |proof|::Identity::<Representative>(value, value)",
            "machine normal(value: Representative) -> Representative\n\
             requires |proof|::Identity::<Representative>(sign(value), Representative::Zero)",
        )
        .replace(
            "    |proof|::Identity::<Representative>(left, left),\n    \
             |proof|::Identity::<Representative>(right, right)\n\
             ensures same_sign(normal(left), normal(right))",
            "    |proof|::Identity::<Representative>(sign(left), Representative::Zero),\n    \
             |proof|::Identity::<Representative>(sign(right), Representative::Zero)\n\
             ensures same_sign(normal(left), normal(right))",
        );
    assert!(
        source.matches("Representative::Zero)").count() >= 3,
        "{source}"
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "normal_class");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `normal_class`: what `normal` requires of a \
         representative"
    );
}

/// The lifted operation of `LIFTED` with an operation on representatives
/// and on classes that require nothing, so the kernel's `lift` defines it
/// and a clause may apply it with nothing to show, and a law
/// `uses_the_operation` stating `claim` of it.
fn applied(claim: &str) -> String {
    let source = proofs(LIFTED)
        .replace(
            "machine normal(value: Representative) -> Representative\n\
             requires |proof|::Identity::<Representative>(value, value)\n{",
            "machine normal(value: Representative) -> Representative\n{",
        )
        .replace(
            "requires\n    same_sign(left, right),\n    \
             |proof|::Identity::<Representative>(left, left),\n    \
             |proof|::Identity::<Representative>(right, right)\n\
             ensures same_sign(normal(left), normal(right))",
            "requires same_sign(left, right)\nensures same_sign(normal(left), normal(right))",
        )
        .replace(
            "machine normal_class(value: Sign) -> Sign\n\
             requires |proof|::Identity::<Sign>(value, value)\n{",
            "machine normal_class(value: Sign) -> Sign\n{",
        )
        .replace(LIFT, "Quotient::lift::<normal, normal_respects>(value)");
    assert!(!source.contains("requires |proof|::Identity::<Representative>(value, value)"));
    assert!(!source.contains("requires |proof|::Identity::<Sign>(value, value)"));
    assert_eq!(
        source
            .matches("Quotient::lift::<normal, normal_respects>(value)")
            .count(),
        1
    );
    format!("{source}\nmachine uses_the_operation(a: Representative)\nensures {claim}\n{{\n}}\n")
}

const AT_A_CLASS: &str = "|proof|::Identity::<Sign>(normal_class(a as Sign), normal(a) as Sign)";

#[test]
fn an_operation_lifted_to_a_quotient_computes_at_a_class() {
    // `normal_class` is the kernel's `lift` of `normal`, so at the class of
    // `a` it is the class of `normal(a)`. That holds by `lift`'s point
    // computation, a theorem derived from the quotient's `beta`, so the law
    // rests on `beta` beside the quotient's type, its classes and soundness.
    let verdicts = judged(&applied(AT_A_CLASS));
    let judgment = signature(&verdicts);
    let law = judged_laws(judgment, "uses_the_operation");
    eprintln!("uses_the_operation {law:?}");
    // Seven: the quotient type, the class of a value, that the quotient is
    // a set, soundness, the eliminator and its point computation, which
    // `lift` and its derived computation rest on, and the statement of
    // `normal_respects`, which is proved as a law of its own.
    assert!(matches!(law.as_slice(), [(_, 7)]), "{law:?}");
}

#[test]
fn an_operation_lifted_to_a_quotient_gives_only_its_operation_s_class() {
    // The twin claims every class goes to the class of zero, which `normal`
    // does not do for a number other than zero.
    let claim = "|proof|::Identity::<Sign>(normal_class(a as Sign), Representative::Zero as Sign)";
    let verdicts = judged(&applied(claim));
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "uses_the_operation");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

#[test]
fn an_operation_lifted_to_data_or_with_a_number_as_it_is_computes_at_a_class() {
    // `sign_of` returns plain data, a set by Hedberg's theorem; `pick_class`
    // passes `other` to `pick` as it is.
    let law = |claim: &str| {
        format!(
            "{}{PICK}\nmachine uses_the_operation(a: Representative, b: Representative)\n\
             ensures {claim}\n{{\n}}\n",
            proofs(CLASS)
        )
    };
    for claim in [
        "|proof|::Identity::<Representative>(sign_of(a as Sign), sign(a))",
        "|proof|::Identity::<Sign>(pick_class(b, a as Sign), pick(a, b) as Sign)",
    ] {
        let verdicts = judged(&law(claim));
        let judgment = signature(&verdicts);
        assert!(
            matches!(judged_laws(judgment, "uses_the_operation").as_slice(), [_]),
            "{claim}: {judgment:?}"
        );
    }
    // Each twin names the other argument where the operation took this one.
    for claim in [
        "|proof|::Identity::<Representative>(sign_of(a as Sign), sign(b))",
        "|proof|::Identity::<Sign>(pick_class(b, a as Sign), pick(b, a) as Sign)",
    ] {
        let verdicts = judged(&law(claim));
        let law = unproved(&verdicts);
        assert_eq!(law.machine, "uses_the_operation", "{claim}");
        assert_eq!(law.cause, UnprovedCause::NoDerivation, "{claim}");
    }
}

#[test]
fn an_operation_whose_representatives_must_meet_a_requirement_is_not_a_term() {
    // `lift` defines a function only from a total operation on
    // representatives; `normal` here requires something of its argument.
    let source = format!(
        "{}\nmachine uses_the_operation(a: Representative)\n\
         ensures {AT_A_CLASS}\n{{\n}}\n",
        proofs(LIFTED)
    );
    let verdicts = judged(&source);
    let refused: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        matches!(refused.as_slice(), [detail]
            if detail.contains("whose operation on representatives requires something of them")),
        "{verdicts:?}"
    );
}

/// An operation `pick_class` on classes that also takes a number as it is,
/// lifted from `pick`, and `sign_of`, which returns a number and not a
/// class.
const PICK: &str = r"
machine pick(value: Representative, other: Representative) -> Representative {
    sign(value)
}

machine pick_respects(left: Representative, right: Representative, other: Representative)
requires same_sign(left, right)
ensures same_sign(pick(left, other), pick(right, other))
{
}

machine pick_class(other: Representative, value: Sign) -> Sign {
    Quotient::lift::<pick, pick_respects>(value, other)
}

machine sign_respects(left: Representative, right: Representative)
requires same_sign(left, right)
ensures |proof|::Identity::<Representative>(sign(left), sign(right))
{
}

machine sign_of(value: Sign) -> Representative {
    Quotient::define::<sign, sign_respects>(value)
}
";

#[test]
fn an_argument_that_is_no_class_is_shared_by_both_calls() {
    // `pick` takes a number and a second number it ignores. Its lift takes
    // them in the other order, the second as it is, so the congruence has
    // two representatives of the class and one `other`. `sign_of` returns a
    // number, not a class, so its congruence ends in an equality.
    let source = format!("{}{PICK}", proofs(CLASS));
    let verdicts = judged(&source);
    let judgment = signature(&verdicts);
    assert!(
        matches!(judged_laws(judgment, "pick_respects").as_slice(), [(_, 0)]),
        "{judgment:?}"
    );
    assert!(
        matches!(judged_laws(judgment, "sign_respects").as_slice(), [(_, 0)]),
        "{judgment:?}"
    );
    // Neither operation requires anything, so neither owes a law.
    assert!(judged_laws(judgment, "pick_class").is_empty());
    assert!(judged_laws(judgment, "sign_of").is_empty());

    // The same lift with `other` as a second pair of representatives is
    // refused where the theorem is read: `other` is no class.
    let paired = source.replace(
        "machine pick_respects(left: Representative, right: Representative, other: Representative)",
        "machine pick_respects(left: Representative, right: Representative, other: Representative, again: Representative)",
    );
    assert_ne!(paired, source);
    let verdicts = judged(&paired);
    let refused: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        matches!(refused.as_slice(), [detail]
            if detail.contains("theorem of a quotient operation over other than two representatives")),
        "{verdicts:?}"
    );
}

/// The fallback refusals of `verdicts`.
fn refusals(verdicts: &[RouteVerdict]) -> Vec<&str> {
    verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
            _ => None,
        })
        .collect()
}

const PICKED: &str = "Quotient::lift::<pick, pick_respects>(value, other)";

/// `CLASS` and `PICK` with `pick_class` lifting `pick` by `lift`, and a law
/// `uses_the_operation` stating `claim` of two numbers `a` and `b`.
fn picked(lift: &str, claim: &str) -> String {
    let source = format!("{}{PICK}", proofs(CLASS));
    assert_eq!(source.matches(PICKED).count(), 1);
    format!(
        "{}\nmachine uses_the_operation(a: Representative, b: Representative)\n\
         ensures {claim}\n{{\n}}\n",
        source.replace(PICKED, lift)
    )
}

#[test]
fn an_operation_lifted_with_a_computed_or_omitted_argument_computes_at_a_class() {
    // `pick_class` passes `sign(other)` to `pick`, or a constant where it
    // leaves `other` out. The congruence is stated of `pick`'s parameters,
    // so `pick_respects` serves both, and at the class of `a` the operation
    // is the class of `pick` at what it passes.
    for (lift, claim) in [
        (
            "Quotient::lift::<pick, pick_respects>(value, sign(other))",
            "|proof|::Identity::<Sign>(pick_class(b, a as Sign), pick(a, sign(b)) as Sign)",
        ),
        (
            "Quotient::lift::<pick, pick_respects>(value, Representative::Zero)",
            "|proof|::Identity::<Sign>(pick_class(b, a as Sign), \
             pick(a, Representative::Zero) as Sign)",
        ),
    ] {
        let verdicts = judged(&picked(lift, claim));
        let judgment = signature(&verdicts);
        assert!(
            matches!(judged_laws(judgment, "uses_the_operation").as_slice(), [_]),
            "{lift}: {verdicts:?}"
        );
        assert!(
            matches!(judged_laws(judgment, "pick_respects").as_slice(), [(_, 0)]),
            "{lift}: {judgment:?}"
        );
        assert!(judged_laws(judgment, "pick_class").is_empty(), "{lift}");
    }
}

#[test]
fn an_operation_lifted_with_a_computed_or_omitted_argument_gives_only_its_class() {
    // Each twin claims the operation takes the class of `b` where it took
    // the class of `a`.
    for (lift, claim) in [
        (
            "Quotient::lift::<pick, pick_respects>(value, sign(other))",
            "|proof|::Identity::<Sign>(pick_class(b, a as Sign), pick(b, sign(a)) as Sign)",
        ),
        (
            "Quotient::lift::<pick, pick_respects>(value, Representative::Zero)",
            "|proof|::Identity::<Sign>(pick_class(b, a as Sign), \
             pick(b, Representative::Zero) as Sign)",
        ),
    ] {
        let verdicts = judged(&picked(lift, claim));
        let law = unproved(&verdicts);
        assert_eq!(law.machine, "uses_the_operation", "{lift}");
        assert_eq!(law.cause, UnprovedCause::NoDerivation, "{lift}");
    }
}

/// `both` takes two numbers, and `both_class` passes it one class twice.
/// Its congruence relates two pairs of representatives.
const BOTH: &str = r"
machine both(value: Representative, other: Representative) -> Representative {
    sign(other)
}

machine both_respects(
    left: Representative,
    right: Representative,
    other_left: Representative,
    other_right: Representative
)
requires same_sign(left, right), same_sign(other_left, other_right)
ensures same_sign(both(left, other_left), both(right, other_right))
{
}

machine both_class(value: Sign) -> Sign {
    Quotient::lift::<both, both_respects>(value, value)
}
";

#[test]
fn an_operation_lifted_with_a_repeated_class_computes_at_a_class() {
    let law = |claim: &str| {
        format!(
            "{}{BOTH}\nmachine uses_the_operation(a: Representative, b: Representative)\n\
             ensures {claim}\n{{\n}}\n",
            proofs(CLASS)
        )
    };
    let verdicts = judged(&law(
        "|proof|::Identity::<Sign>(both_class(a as Sign), both(a, a) as Sign)",
    ));
    let judgment = signature(&verdicts);
    assert!(
        matches!(judged_laws(judgment, "both_respects").as_slice(), [(_, 0)]),
        "{verdicts:?}"
    );
    assert!(
        matches!(judged_laws(judgment, "uses_the_operation").as_slice(), [_]),
        "{judgment:?}"
    );
    // The twin passes the class of `b` to the second parameter, which the
    // operation passes the class of `a`.
    let verdicts = judged(&law(
        "|proof|::Identity::<Sign>(both_class(a as Sign), both(a, b) as Sign)",
    ));
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "uses_the_operation");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

#[test]
fn a_repeated_class_needs_a_congruence_for_each_pair() {
    // The congruence relates only the first pair, so nothing relates the
    // representatives the second parameter takes, and it is not proved.
    let source = format!("{}{BOTH}", proofs(CLASS)).replace(
        "requires same_sign(left, right), same_sign(other_left, other_right)",
        "requires same_sign(left, right)",
    );
    assert!(source.contains("requires same_sign(left, right)\nensures same_sign(both("));
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "both_respects");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

#[test]
fn a_lift_that_computes_an_argument_owes_its_transport_itself() {
    // A transport theorem is stated over the congruence's binders, which a
    // computed argument has no formal for. The same lift with `other` as it
    // is is `an_operation_on_a_quotient_is_judged_by_the_theorems_it_selects`.
    let source = picked(
        "Quotient::lift::<pick, pick_respects, sign_respects>(value, sign(other))",
        "|proof|::Identity::<Representative>(a, a)",
    );
    let verdicts = judged(&source);
    assert!(
        matches!(refusals(&verdicts).as_slice(), [detail]
            if detail.contains("transport theorem of a quotient operation that computes, repeats or leaves out an argument")),
        "{verdicts:?}"
    );
}

#[test]
fn a_computed_argument_is_never_a_class() {
    // `pick_class` passes the class `same_class` computes where `pick`
    // takes a number: `lift` takes a class only as one of the machine's
    // formals, so that the class it eliminates is one it was given.
    let source = picked(
        "Quotient::lift::<pick, pick_respects>(same_class(value), other)",
        "|proof|::Identity::<Representative>(a, a)",
    )
    .replace(
        "machine pick_class(",
        "machine same_class(value: Sign) -> Sign {\n    value\n}\n\nmachine pick_class(",
    );
    assert!(
        source.contains("machine same_class(value: Sign)"),
        "{source}"
    );
    let verdicts = judged(&source);
    assert!(
        matches!(refusals(&verdicts).as_slice(), [detail]
            if detail.contains("a class is passed only as a formal")),
        "{:?}",
        refusals(&verdicts)
    );
}

const BOUND: &str =
    include_str!("../../tests/fixtures/indexed_family/pass/vector_sum_bound/main.omg");

#[test]
fn a_program_may_hold_a_quotient_and_data_indexed_by_values() {
    // Both rest on declarations of the kernel's own. The quotient's come
    // first in the signature, and the family names its five from where they
    // start, so the vector's theorem is judged beside the quotient's laws.
    let start = BOUND.find("data Vector").expect("the fixture's data");
    let end = BOUND.find("data Main").expect("the fixture's entry state");
    let source = format!("{}{}", proofs(CLASS), &BOUND[start..end]);
    let alone = judged(&proofs(CLASS));
    let verdicts = judged(&source);
    let judgment = signature(&verdicts);
    assert_eq!(judgment.laws.len(), signature(&alone).laws.len() + 1);
    assert!(
        matches!(judged_laws(judgment, "vector_bound").as_slice(), [_]),
        "{judgment:?}"
    );

    // The bound one element can break is still refused in its case.
    let tight = source.replace("10 * embed(Length)", "9 * embed(Length)");
    assert_ne!(tight, source);
    let verdicts = judged(&tight);
    let unproved: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::UnprovedLaws(laws) => Some(laws),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(
        matches!(unproved.as_slice(), [law] if law.machine == "vector_bound"),
        "{verdicts:?}"
    );
}

#[test]
fn level_binders_u_and_v_work_in_mathematical_definitions() {
    // Both `u: core\Level` and `v: core\Level` resolve in `core\Type<u>` and
    // `core\Type<v>` without hitting builtin-type-name refusal.
    let source = r#"
use {omega}/language/core/foundation.omg as proof;
let keep_u<u: |proof|::Level>(P: |proof|::Type<u>): |proof|::Type<u> = P;
let keep_v<v: |proof|::Level>(P: |proof|::Type<v>): |proof|::Type<v> = P;

machine check_u(x: i32)
ensures keep_u::<0>(|proof|::Identity::<i32>(x, x))
{
}

machine check_v(x: i32)
ensures keep_v::<0>(|proof|::Identity::<i32>(x, x))
{
}
"#;
    let verdicts = judged(source);
    for verdict in &verdicts {
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_) | RouteVerdict::Laws(_)),
            "expected acceptance for u/v level binders: {verdict:?}"
        );
    }

    // Negative twin: a non-Level binder `T` in `core\Type<T>` is refused.
    let bad = r#"
use {omega}/language/core/foundation.omg as proof;
let bad_level<T>(P: |proof|::Type<T>): |proof|::Type<T> = P;

machine check_bad(x: i32)
ensures bad_level::<i32>(|proof|::Identity::<i32>(x, x))
{
}
"#;
    let verdicts = judged(bad);
    assert!(
        verdicts
            .iter()
            .any(|verdict| matches!(verdict, RouteVerdict::FallBack(_))),
        "expected refusal for non-Level binder in core\\Type<T>: {verdicts:?}"
    );
}

#[test]
fn quotient_refuses_conformance_proved_for_another_relation() {
    let source = format!(
        "{}\n\
         proposition other_rel(a: Representative, b: Representative) =\n\
             |proof|::Identity::<Representative>(a, b);\n\
         OtherRel satisfies |relation|::MereEquivalence<Representative, other_rel> {{\n\
             machine reflexive(value: Representative) ensures other_rel(value, value) {{ }}\n\
             machine symmetric(left: Representative, right: Representative) requires other_rel(left, right) ensures other_rel(right, left) {{ }}\n\
             machine transitive(left: Representative, middle: Representative, right: Representative) requires other_rel(left, middle), other_rel(middle, right) ensures other_rel(left, right) {{ }}\n\
             machine proofs_agree(a: Representative, b: Representative) ensures |relation|::IsMere(other_rel(a, b)) {{ }}\n\
         }}\n",
        proofs(CLASS).replace("as SameSign;", "as OtherRel;")
    );
    let verdicts = judged(&source);
    let refused: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        refused
            .iter()
            .any(|detail| detail.contains("proved for a different relation")),
        "expected refusal for wrong-relation conformance: {verdicts:?}"
    );
}

#[test]
fn quotient_refuses_plain_equivalence_without_mere_relation() {
    let source = proofs(CLASS)
        .replace(
            "SameSign satisfies |relation|::MereEquivalence<Representative, same_sign>",
            "SameSign satisfies |relation|::Equivalence<Representative, same_sign>",
        )
        .replace(
            "    machine proofs_agree(a: Representative, b: Representative) ensures |relation|::IsMere(same_sign(a, b)) { }\n",
            "",
        )
        .replace(
            "|relation|::MereEquivalence<Representative, same_sign>",
            "|relation|::Equivalence<Representative, same_sign>",
        );
    let verdicts = judged(&source);
    let refused: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        refused
            .iter()
            .any(|detail| detail.contains("MereEquivalence")),
        "expected refusal for plain Equivalence without MereRelation: {verdicts:?}"
    );
}

#[test]
fn quotient_effectivity_uses_selected_mere_equivalence_laws() {
    let source = format!(
        "{}\n\
         machine equal_classes_are_related(a: Representative, b: Representative)\n\
         requires |proof|::Identity::<Sign>(a as Sign, b as Sign)\n\
         ensures same_sign(a, b)\n\
         {{\n\
         }}\n",
        proofs(CLASS)
    );
    let verdicts = judged(&source);
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "equal_classes_are_related")
        .unwrap_or_else(|| panic!("expected effectivity law: {judgment:?}"));
    let MathematicalCoreDecision::Judged(receipt) = law.decision;
    // Effectivity cites the quotient, projection, effectivity assumption,
    // and the four selected `MereEquivalence` law statements.
    assert!(
        receipt.assumption_closure >= 3,
        "effectivity must report its assumption closure: {receipt:?}"
    );

    // Negative twin: without the class-equality premise, `same_sign(a, b)` is refused.
    let unproved_source = source.replace(
        "requires |proof|::Identity::<Sign>(a as Sign, b as Sign)\n",
        "",
    );
    let verdicts = judged(&unproved_source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "equal_classes_are_related");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

/// The executable quotient of the `quotient_closed_literal` sample family,
/// over a record that stores an `i32`.
const STORED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../samples/cli/facts/quotient_closed_literal/parameter/main.omg"
));

/// The carrier, relation and quotient of `STORED`, and a law `one_class`
/// that `requires` and `ensures` as given.
fn stored(requires: &str, ensures: &str) -> String {
    let start = STORED
        .find("data Representative")
        .expect("the sample's carrier");
    let end = STORED
        .find("machine representative(")
        .expect("the sample's operation");
    format!(
        "use {{omega}}/language/core/relation.omg;\n\
         use {{omega}}/language/core/foundation.omg as proof;\n{}\n\
         machine one_class(a: Representative, b: Representative)\n\
         {requires}ensures {ensures}\n{{\n}}\n",
        &STORED[start..end]
    )
}

const ONE_CLASS: &str =
    "|proof|::Identity::<EquivalenceClass>(a as EquivalenceClass, b as EquivalenceClass)";

/// The unproved obligations of a rejected closure, by their account.
fn rejected(verdicts: &[RouteVerdict]) -> Vec<String> {
    verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::Rejected { unproved, .. } => Some(unproved),
            _ => None,
        })
        .flatten()
        .map(|obligation| format!("{}: {}", obligation.statement, obligation.detail))
        .collect()
}

#[test]
fn related_stored_representatives_have_one_class() {
    // `related_representatives_have_one_class` for a quotient of data that
    // stores an `i32`: its relation is the Identity of the two records, and
    // the quotient's soundness at them gives the Identity of the classes.
    let verdicts = judged(&stored("requires equivalent(a, b)\n", ONE_CLASS));
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{:?} {:?}",
        refusals(&verdicts),
        rejected(&verdicts)
    );
}

#[test]
fn unrelated_stored_representatives_are_not_given_one_class() {
    // With nothing relating them, two records have two classes as far as
    // the checker knows; so too when only `a` is related to itself.
    for requires in ["", "requires equivalent(a, a)\n"] {
        let verdicts = judged(&stored(requires, ONE_CLASS));
        let rejected = rejected(&verdicts);
        assert!(
            matches!(rejected.as_slice(), [statement] if statement.contains("one_class")),
            "{requires:?}: {rejected:?} {:?}",
            refusals(&verdicts)
        );
    }
}

/// A quotient of records that store two numbers, by equality of the first
/// alone, so two records with one class may differ.
const TAGGED: &str = r"use {omega}/language/core/relation.omg;
use {omega}/language/core/foundation.omg as proof;

data Key [copy] { value: i32; }
data Tagged [copy] { key: Key; tag: i32; }

proposition same_value(a: Tagged, b: Tagged) = |proof|::Identity::<Key>(a.key, b.key);

SameValue satisfies |relation|::Equivalence<Tagged, same_value> {
    machine reflexive(value: Tagged) ensures same_value(value, value) { }
    machine symmetric(left: Tagged, right: Tagged) requires same_value(left, right) ensures same_value(right, left) { }
    machine transitive(left: Tagged, middle: Tagged, right: Tagged) requires same_value(left, middle), same_value(middle, right) ensures same_value(left, right) { }
}

data Value [copy] = Tagged % same_value
requires same_value satisfies
    |relation|::Equivalence<Tagged, same_value>
    as SameValue;
";

fn tagged(requires: &str) -> String {
    format!(
        "{TAGGED}\nmachine one_class(a: Tagged, b: Tagged)\n{requires}\
         ensures |proof|::Identity::<Value>(a as Value, b as Value)\n{{\n}}\n"
    )
}

#[test]
fn stored_representatives_related_but_unequal_have_one_class() {
    // `a` and `b` may differ in `tag`: only the quotient's soundness at
    // them, from `same_value(a, b)`, makes their classes one.
    let verdicts = judged(&tagged("requires same_value(a, b)\n"));
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{:?} {:?}",
        refusals(&verdicts),
        rejected(&verdicts)
    );
    // The twin relates `a` to itself only.
    let verdicts = judged(&tagged("requires same_value(a, a)\n"));
    let rejected = rejected(&verdicts);
    assert!(
        matches!(rejected.as_slice(), [statement] if statement.contains("one_class")),
        "{rejected:?} {:?}",
        refusals(&verdicts)
    );
}

/// A quotient declared once for every element type, as the owner ruling
/// writes one (OWNER_QUESTIONS.md, "Generic quotients name the applied
/// relation and its proof"): lists of `T` with one class per length.
/// `quotient` is the declaration, `law` the program's claim about it.
fn counted(quotient: &str, law: &str) -> String {
    format!(
        r"use {{omega}}/language/core/relation.omg;
use {{omega}}/language/core/foundation.omg as proof;

data Count {{
    case Zero;
    case Next(previous: Count);
}}

data Tagged<T> {{
    case Nil;
    case More(item: T, rest: Tagged<T>);
}}

machine count<T>(t: Tagged<T>) -> Count
terminates by t
{{
    transition t {{
        Tagged::Nil -> Count::Zero,
        Tagged::More {{ item, rest }} -> Count::Next {{ previous: count::<T>(rest) }}
    }}
}}

proposition same_count<T>(a: Tagged<T>, b: Tagged<T>) =
    |proof|::Identity::<Count>(count::<T>(a), count::<T>(b));

SameCount<T> satisfies |relation|::MereEquivalence<Tagged<T>, same_count<T> > {{
    machine reflexive(value: Tagged<T>) ensures same_count::<T>(value, value) {{ }}
    machine symmetric(left: Tagged<T>, right: Tagged<T>) requires same_count::<T>(left, right) ensures same_count::<T>(right, left) {{ }}
    machine transitive(left: Tagged<T>, middle: Tagged<T>, right: Tagged<T>) requires same_count::<T>(left, middle), same_count::<T>(middle, right) ensures same_count::<T>(left, right) {{ }}
    machine proofs_agree(a: Tagged<T>, b: Tagged<T>) ensures |relation|::IsMere(same_count::<T>(a, b)) {{ }}
}}

{quotient}

{law}
"
    )
}

const COUNTED: &str = "data Counted<T> = Tagged<T> % same_count<T>
requires same_count<T> satisfies |relation|::MereEquivalence<Tagged<T>, same_count<T>>
    as SameCount<T>;";

const SAME_COUNT_ONE_CLASS: &str = "machine one_class_per_count(a: Tagged<u64>, b: Tagged<u64>)
requires same_count::<u64>(a, b)
ensures |proof|::Identity::<Counted<u64> >(a as Counted<u64>, b as Counted<u64>)
{
}";

#[test]
fn related_representatives_have_one_class_in_an_application_of_a_generic_quotient() {
    // `Counted<u64>` is the quotient of `Tagged<u64>` by `same_count` at
    // `u64`, whose equivalence is `SameCount` at `u64`: four rows generated
    // from `SameCount`'s, each judged, and the class law beside them.
    let verdicts = judged(&counted(COUNTED, SAME_COUNT_ONE_CLASS));
    let judgment = signature(&verdicts);
    for machine in [
        "one_class_per_count",
        "reflexive",
        "symmetric",
        "transitive",
    ] {
        assert!(
            judgment.laws.iter().any(|law| law.machine.contains(machine)
                && matches!(law.decision, MathematicalCoreDecision::Judged(_))),
            "{machine}: {judgment:?}"
        );
    }
}

#[test]
fn unrelated_representatives_have_no_one_class_in_an_application_of_a_generic_quotient() {
    let law = SAME_COUNT_ONE_CLASS.replace("requires same_count::<u64>(a, b)\n", "");
    assert_ne!(law, SAME_COUNT_ONE_CLASS);
    let verdicts = judged(&counted(COUNTED, &law));
    assert_eq!(unproved(&verdicts).machine, "one_class_per_count");
}

/// The refusals of `Counted<u64>` declared as `quotient`.
fn counted_refusals(quotient: &str) -> Vec<String> {
    let verdicts = judged(&counted(quotient, SAME_COUNT_ONE_CLASS));
    refusals(&verdicts).into_iter().map(str::to_owned).collect()
}

#[test]
fn a_generic_quotient_whose_conformance_does_not_close_is_refused() {
    for conformance in ["as SameCount;", "as SameCount<T, T>;"] {
        let refused = counted_refusals(&COUNTED.replace("as SameCount<T>;", conformance));
        assert!(
            refused
                .iter()
                .any(|detail| detail.contains("does not close at its arguments")),
            "{conformance}: {refused:?}"
        );
    }
}

#[test]
fn a_generic_quotient_whose_conformance_is_for_another_carrier_is_refused() {
    let refused = counted_refusals(&COUNTED.replace("as SameCount<T>;", "as SameCount<u8>;"));
    assert!(
        refused
            .iter()
            .any(|detail| detail.contains("proved for a different carrier")),
        "{refused:?}"
    );
}

#[test]
fn a_generic_quotient_whose_conformance_is_for_another_relation_is_refused() {
    let other = "proposition any_count<T>(a: Tagged<T>, b: Tagged<T>) =
    |proof|::Identity::<Count>(Count::Zero, Count::Zero);

AnyCount<T> satisfies |relation|::MereEquivalence<Tagged<T>, any_count<T> > {
    machine reflexive(value: Tagged<T>) ensures any_count::<T>(value, value) { }
    machine symmetric(left: Tagged<T>, right: Tagged<T>) requires any_count::<T>(left, right) ensures any_count::<T>(right, left) { }
    machine transitive(left: Tagged<T>, middle: Tagged<T>, right: Tagged<T>) requires any_count::<T>(left, middle), any_count::<T>(middle, right) ensures any_count::<T>(left, right) { }
    machine proofs_agree(a: Tagged<T>, b: Tagged<T>) ensures |relation|::IsMere(any_count::<T>(a, b)) { }
}
";
    let quotient = format!(
        "{other}\n{}",
        COUNTED.replace("as SameCount<T>;", "as AnyCount<T>;")
    );
    let refused = counted_refusals(&quotient);
    assert!(
        refused
            .iter()
            .any(|detail| detail.contains("proved for a different relation")),
        "{refused:?}"
    );
}
