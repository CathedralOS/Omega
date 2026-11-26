//! A hypothesis used on part of a sum, stated with a named proposition.
//!
//! A law's `requires` equality is a rewrite rule of its proof. Matching it
//! against whole subterms finds it only where the goal contains its left
//! side as written. Under a conformance whose laws let addition regroup and
//! reorder, the prover also uses it on some addends of a longer sum.

use super::*;
use crate::laws::{SignatureJudgment, UnprovedLaw};

const KEPT: &str =
    include_str!("../../tests/fixtures/sum_hypothesis/pass/proposition_under_addition/main.omg");
const UNFOUNDED: &str = include_str!(
    "../../tests/fixtures/sum_hypothesis/fail/proposition_without_its_hypothesis/main.omg"
);

fn proofs(source: &str) -> String {
    let start = source.find("data Peano").expect("the fixture's data");
    let end = source.find("data Main").expect("the fixture's entry state");
    format!(
        "use {{omega}}/language/core/foundation.omg as proof;\n{}",
        &source[start..end]
    )
}

fn judged(source: &str) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[],
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
fn a_hypothesis_about_two_addends_proves_a_sum_of_four() {
    let verdicts = judged(&proofs(KEPT));
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "add_pair_keeps_difference")
        .expect("the law");
    let MathematicalCoreDecision::Judged(receipt) = law.decision;
    // The proof stands on the two laws of the conformance and nothing else:
    // the hypothesis is the law's own premise, bound in its statement.
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 2));
    eprintln!(
        "add_pair_keeps_difference: {} conversion steps",
        law.conversion_steps
    );
}

#[test]
fn without_the_hypothesis_the_sum_stays_unproved() {
    let source = proofs(UNFOUNDED);
    assert_eq!(
        source,
        proofs(KEPT).replace("requires\n    same_difference(a, a2)\n", "")
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "add_pair_keeps_difference");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    // The clause is printed as the proposition the law states.
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `add_pair_keeps_difference`: \
         same_difference(add_pair(a, b), add_pair(a2, b))"
    );
}
