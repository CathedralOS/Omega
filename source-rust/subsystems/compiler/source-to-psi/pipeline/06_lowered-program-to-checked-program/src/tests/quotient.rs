//! Quotients of mathematical data.
//!
//! `data Sign = Representative % same_sign ...` is the kernel's quotient of
//! the data by the relation. Two classes are equal when their
//! representatives are related, by the kernel's soundness assumption for
//! quotients, and the three rows of the equivalence the quotient selects
//! must each prove exactly the law an equivalence owes.

use super::*;
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

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
    crate::checking::judge_compilation(compilation.program)
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
        let MathematicalCoreDecision::Judged(receipt) = law.decision else {
            panic!("the kernel must judge the law: {law:?}")
        };
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
        "the proved equality is not the law its requirement declares"
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
