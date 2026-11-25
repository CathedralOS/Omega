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
    let start = source.find("use omega").expect("the fixture's imports");
    let end = source.find("data Main").expect("the fixture's entry state");
    source[start..end].replace("use omega\\language\\core\\ProcessExit;\n", "")
}

fn judged(source: &str) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[(
            "relation.omg",
            "Equivalence",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/relation.omg"
            )),
        )],
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
    // The three laws of the relation are about the data alone.
    for row in [
        "SameSign::reflexive",
        "SameSign::symmetric",
        "SameSign::transitive",
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
            "requires same_sign(left, right), core\\Identity::<Representative>(left, right) \
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
             requires core\\Identity::<Representative>(value, value)",
            "machine normal(value: Representative) -> Representative\n\
             requires core\\Identity::<Representative>(sign(value), Representative::Zero)",
        )
        .replace(
            "    core\\Identity::<Representative>(left, left),\n    \
             core\\Identity::<Representative>(right, right)\n\
             ensures same_sign(normal(left), normal(right))",
            "    core\\Identity::<Representative>(sign(left), Representative::Zero),\n    \
             core\\Identity::<Representative>(sign(right), Representative::Zero)\n\
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

#[test]
fn an_operation_lifted_to_a_quotient_is_not_a_term() {
    let source = format!(
        "{}\nmachine uses_the_operation(a: Representative)\n\
         requires core\\Identity::<Sign>(a as Sign, a as Sign)\n\
         ensures core\\Identity::<Sign>(normal_class(a as Sign), normal_class(a as Sign))\n{{\n}}\n",
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
            if detail.contains("term that applies an operation lifted to a quotient")),
        "{verdicts:?}"
    );
}

#[test]
fn an_argument_that_is_no_class_is_shared_by_both_calls() {
    // `pick` takes a number and a second number it ignores. Its lift takes
    // them in the other order, the second as it is, so the congruence has
    // two representatives of the class and one `other`. `sign_of` returns a
    // number, not a class, so its congruence ends in an equality.
    let source = format!(
        "{}{}",
        proofs(CLASS),
        r"
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
ensures core\Identity::<Representative>(sign(left), sign(right))
{
}

machine sign_of(value: Sign) -> Representative {
    Quotient::define::<sign, sign_respects>(value)
}
"
    );
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
