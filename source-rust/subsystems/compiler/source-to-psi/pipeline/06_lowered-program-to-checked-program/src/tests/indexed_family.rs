//! Data indexed by values, and theorems proved by induction over it.
//!
//! `Vector<Length, Total>` has no layout: a value of it is evidence that a
//! list of that length and sum exists. Stage 05 declares it as the kernel's
//! indexed family over an integer vocabulary, and a theorem machine over
//! erased evidence of it as a statement for every index and every value.
//! Stage 06 proves each case of the data with the integer search and
//! assembles the cases with the family's induction.

use super::*;
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

const BOUND: &str =
    include_str!("../../tests/fixtures/indexed_family/pass/vector_sum_bound/main.omg");
const TOO_TIGHT: &str =
    include_str!("../../tests/fixtures/indexed_family/fail/vector_sum_bound_too_tight/main.omg");
const CITATION: &str = "        vector_bound::<previous, previous_total>(tail);\n";

fn proofs(source: &str) -> String {
    let start = source.find("data Vector").expect("the fixture's data");
    let end = source.find("data Main").expect("the fixture's entry state");
    source[start..end].to_owned()
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

fn refused(verdicts: &[RouteVerdict]) -> Vec<&str> {
    verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_sum_of_a_vector_is_bounded_by_induction_over_it() {
    let verdicts = judged(&proofs(BOUND));
    let judgment = signature(&verdicts);
    let [law] = judgment.laws.as_slice() else {
        panic!("one law: {judgment:?}")
    };
    assert_eq!(law.machine, "vector_bound");
    let MathematicalCoreDecision::Judged(receipt) = law.decision else {
        panic!("the kernel must judge the theorem: {law:?}")
    };
    assert_eq!(receipt.context_depth, 0);
    // The theorem rests on the integer vocabulary and the laws of integers
    // its two cases used, and on nothing about the data: the family, its
    // constructors and the induction are definitions.
    assert!(receipt.assumption_closure > 0, "{receipt:?}");
    // The certificate the kernel judged is closed and checks on its own.
    let mut checked = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    eprintln!(
        "vector_bound: {} conversion steps, {} assumptions",
        law.conversion_steps, receipt.assumption_closure
    );
}

#[test]
fn a_bound_one_element_can_break_is_refused() {
    let source = proofs(TOO_TIGHT);
    assert_eq!(
        source,
        proofs(BOUND).replace(
            "ensures embed(Total) <= 10 * embed(Length)",
            "ensures embed(Total) <= 9 * embed(Length)"
        )
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "vector_bound");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    // The empty list satisfies the tighter bound; the longer one does not.
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `vector_bound` in the case `Vector::Cons`: \
         embed(Total) <= 9 * embed(Length)"
    );
    assert_eq!(
        law.detail,
        "no proof of this case from what the data requires of it and the theorem at the \
         children the body cites"
    );
}

#[test]
fn the_step_needs_the_theorem_at_the_tail() {
    // The same theorem whose step does not cite it at the tail has no
    // induction hypothesis, and the longer list's bound does not follow.
    let source = proofs(BOUND).replace(CITATION, "");
    assert_ne!(source, proofs(BOUND));
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "vector_bound");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

#[test]
fn a_step_may_not_require_more_than_the_data_gives() {
    // The data lets an element be ten. A step that requires nine asks for
    // something no case of the data promises.
    let source = proofs(BOUND).replace(
        "    requires head <= 10 && embed(previous) + 1 == embed(Length)\n             && embed(previous_total)",
        "    requires head <= 9 && embed(previous) + 1 == embed(Length)\n             && embed(previous_total)",
    );
    assert_ne!(source, proofs(BOUND));
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "vector_bound");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.detail,
        "what the step of this case requires does not follow from what the data requires of \
         the case"
    );
}

#[test]
fn the_theorem_is_cited_at_the_index_the_child_has() {
    // The tail is a vector of `previous` elements summing to
    // `previous_total`. Citing the theorem with the two exchanged is a
    // statement about another vector.
    let source = proofs(BOUND).replace(
        CITATION,
        "        vector_bound::<previous_total, previous>(tail);\n",
    );
    assert_ne!(source, proofs(BOUND));
    let verdicts = judged(&source);
    let refused = refused(&verdicts);
    assert!(
        matches!(refused.as_slice(), [detail]
            if detail.contains("theorem cited at another index than the case declares for that child")),
        "{verdicts:?}"
    );
}

/// The data and theorem of `samples/cli/facts/source_indexed_binary_tree`:
/// a case with two children, each at its own index, and a step that cites
/// the theorem at both.
const TREE: &str = r"
data Tree<Nodes: u64, Total: u64>
    requires ((self in Tree::Empty) && (Nodes == 0 && Total == 0)) || ((self in Tree::Fork) && (self.head <= 10 && embed(self.left_nodes) + embed(self.right_nodes) + 1 == embed(Nodes)
              && embed(self.left_total) + embed(self.right_total) + embed(self.head) == embed(Total)))
{
    case Empty;
    case Fork(left_nodes: u64, left_total: u64, right_nodes: u64, right_total: u64, head: u64,
              left [erased]: Tree<left_nodes, left_total>, right [erased]: Tree<right_nodes, right_total>);
}
machine tree_bound<Nodes: u64, Total: u64>(evidence [erased]: Tree<Nodes, Total>)
terminates by Nodes
ensures embed(Total) <= 10 * embed(Nodes)
{
    transition evidence {
        Tree::Empty { .. } -> done(),
        Tree::Fork { left_nodes, left_total, right_nodes, right_total, head, left, right, .. }
            -> step(left_nodes, left_total, right_nodes, right_total, head, left, right)
    }
    machine done() requires Nodes == 0 && Total == 0 {}
    machine step(left_nodes: u64, left_total: u64, right_nodes: u64, right_total: u64, head: u64,
               left [erased]: Tree<left_nodes, left_total>, right [erased]: Tree<right_nodes, right_total>)
    requires head <= 10 && embed(left_nodes) + embed(right_nodes) + 1 == embed(Nodes)
             && embed(left_total) + embed(right_total) + embed(head) == embed(Total)
    {
        tree_bound::<left_nodes, left_total>(left);
        tree_bound::<right_nodes, right_total>(right);
    }
}
";

#[test]
fn a_tree_with_two_children_needs_the_theorem_at_both() {
    let verdicts = judged(TREE);
    let judgment = signature(&verdicts);
    let [law] = judgment.laws.as_slice() else {
        panic!("one law: {judgment:?}")
    };
    assert_eq!(law.machine, "tree_bound");
    assert!(
        matches!(law.decision, MathematicalCoreDecision::Judged(_)),
        "{law:?}"
    );
    eprintln!("tree_bound: {} conversion steps", law.conversion_steps);

    // Without the theorem at the right child the bound does not follow.
    let one_sided = TREE.replace(
        "        tree_bound::<right_nodes, right_total>(right);\n",
        "",
    );
    assert_ne!(one_sided, TREE);
    let verdicts = judged(&one_sided);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "tree_bound");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert!(
        law.statement.contains("in the case `Tree::Fork`"),
        "{law:?}"
    );
}
