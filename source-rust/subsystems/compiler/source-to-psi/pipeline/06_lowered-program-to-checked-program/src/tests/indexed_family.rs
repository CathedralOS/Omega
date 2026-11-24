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

const RUNNING: &str =
    include_str!("../../tests/fixtures/indexed_family/pass/evidence_in_a_running_machine/main.omg");
const TOO_LARGE: &str = include_str!(
    "../../tests/fixtures/indexed_family/fail/evidence_of_an_element_too_large/main.omg"
);
const ONE: &str = "Vector::Cons { previous: 0, previous_total: 0, head: 7, tail: nil }";

/// The laws judged for `machine`, as how many assumptions each rests on.
fn closures(verdicts: &[RouteVerdict], machine: &str) -> Vec<u32> {
    verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::Laws(judgment) => Some(&judgment.laws),
            _ => None,
        })
        .flatten()
        .filter(|law| law.machine == machine)
        .map(|law| match law.decision {
            MathematicalCoreDecision::Judged(receipt) => receipt.assumption_closure,
            _ => panic!("the kernel must judge the law: {law:?}"),
        })
        .collect()
}

#[test]
fn a_running_machine_builds_evidence_and_uses_the_theorem() {
    let verdicts = judged(&proofs(RUNNING));
    for verdict in &verdicts {
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_) | RouteVerdict::Laws(_)),
            "every part must be accepted: {verdict:?}"
        );
    }
    // `room` builds `Nil` at (0, 0) and `Cons` at (1, 7). Each thing the
    // data requires of those two values is a law of its own: two for `Nil`,
    // three for `Cons`.
    assert_eq!(closures(&verdicts, "room").len(), 5);
    assert_eq!(closures(&verdicts, "vector_bound").len(), 1);
}

#[test]
fn evidence_of_a_list_the_data_does_not_allow_is_refused() {
    let source = proofs(TOO_LARGE);
    assert_eq!(
        source,
        proofs(RUNNING).replace(
            ONE,
            "Vector::Cons { previous: 0, previous_total: 0, head: 11, tail: nil }"
        )
    );
    let verdicts = judged(&source);
    let unproved: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::UnprovedLaws(laws) => Some(laws),
            _ => None,
        })
        .flatten()
        .collect();
    // Eleven is more than ten, and the list does not sum to seven.
    assert_eq!(unproved.len(), 2, "{unproved:?}");
    for law in unproved {
        assert_eq!(law.machine, "room");
        assert_eq!(law.cause, UnprovedCause::NoDerivation);
        assert_eq!(
            law.statement,
            "cannot prove what `Vector::Cons` requires of the value built here in `room`"
        );
    }
}

#[test]
fn evidence_is_passed_only_at_the_index_it_has() {
    // `one` is a list of one element summing to seven. Passing it as a
    // list summing to twenty is refused where the call is read.
    let source = proofs(RUNNING).replace("bounded::<1, 7>(one)", "bounded::<1, 20>(one)");
    assert_ne!(source, proofs(RUNNING));
    let verdicts = judged(&source);
    let refused = refused(&verdicts);
    assert!(
        refused
            .iter()
            .any(|detail| detail.contains("evidence passed at another index than the formal takes")),
        "{verdicts:?}"
    );
}

#[test]
fn a_child_of_a_literal_sits_at_the_index_the_case_declares() {
    // A list of two whose tail is the empty list: the case says the tail of
    // a list of two has one element.
    let source = proofs(RUNNING).replace(
        "    bounded::<1, 7>(one)\n",
        "    let two [erased]: Vector<2, 14> = Vector::Cons { previous: 1, previous_total: 7, \
         head: 7, tail: nil };\n    bounded::<1, 7>(one)\n",
    );
    assert_ne!(source, proofs(RUNNING));
    let verdicts = judged(&source);
    let refused = refused(&verdicts);
    assert!(
        refused.iter().any(|detail| detail
            .contains("child of a literal at another index than the case declares for it")),
        "{verdicts:?}"
    );
}

#[test]
fn without_the_citation_the_subtraction_is_not_justified() {
    // `bounded` no longer cites the theorem, so nothing bounds `Total` and
    // `80u64 - Total` may go below zero.
    let source = proofs(RUNNING).replace(
        "    vector_bound::<Length, Total>(evidence);\n    80u64",
        "    80u64",
    );
    assert_ne!(source, proofs(RUNNING));
    let verdicts = judged(&source);
    assert!(
        verdicts.iter().any(|verdict| matches!(
            verdict,
            RouteVerdict::Incomplete(_) | RouteVerdict::Rejected { .. }
        )),
        "{verdicts:?}"
    );
}

#[test]
fn a_caller_owes_what_the_cited_theorem_guarantees() {
    // `relay` passes its own evidence on to `bounded`, which cites the
    // theorem. The theorem's guarantee is one of `bounded`'s entry
    // requirements, so `relay` owes it at the call. `relay` cannot prove it
    // for every `Length` and `Total`, so it becomes `relay`'s requirement
    // too, whether `relay` cites the theorem itself or not, and `room`,
    // which passes numbers, proves it by arithmetic.
    let relay = |citation: &str| {
        format!(
            "{}\nmachine relay<Length: u64, Total: u64>(evidence [erased]: Vector<Length, Total>) -> u64\n\
             requires Length <= 8\n{{\n{citation}    bounded::<Length, Total>(evidence)\n}}\n",
            proofs(RUNNING).replace("    bounded::<1, 7>(one)\n", "    relay::<1, 7>(one)\n")
        )
    };
    for citation in ["    vector_bound::<Length, Total>(evidence);\n", ""] {
        let source = relay(citation);
        assert!(source.contains("relay::<1, 7>(one)"));
        let verdicts = judged(&source);
        for verdict in &verdicts {
            assert!(
                matches!(verdict, RouteVerdict::Accepted(_) | RouteVerdict::Laws(_)),
                "every part must be accepted: {verdict:?}"
            );
        }
    }
}

/// The running fixture with a type beside the indices: each longer list
/// also holds a marker of the type the list is at.
fn with_marker(source: &str) -> String {
    let replaced = [
        (
            "data Vector<Length: u64, Total: u64>",
            "data Vector<T [copy], Length: u64, Total: u64>",
        ),
        (
            "head: u64,\n              tail [erased]: Vector<previous, previous_total>);",
            "head: u64, marker: T,\n              tail [erased]: Vector<T, previous, previous_total>);",
        ),
        (
            "machine vector_bound<Length: u64, Total: u64>(evidence [erased]: Vector<Length, Total>)",
            "machine vector_bound<T [copy], Length: u64, Total: u64>(evidence [erased]: Vector<T, Length, Total>)",
        ),
        (
            "tail [erased]: Vector<previous, previous_total>)\n    requires",
            "tail [erased]: Vector<T, previous, previous_total>)\n    requires",
        ),
        (
            "vector_bound::<previous, previous_total>(tail);",
            "vector_bound::<T, previous, previous_total>(tail);",
        ),
        (
            "machine bounded<Length: u64, Total: u64>(evidence [erased]: Vector<Length, Total>) -> u64",
            "machine bounded<T [copy], Length: u64, Total: u64>(evidence [erased]: Vector<T, Length, Total>) -> u64",
        ),
        (
            "vector_bound::<Length, Total>(evidence);",
            "vector_bound::<T, Length, Total>(evidence);",
        ),
        (
            "let nil [erased]: Vector<0, 0>",
            "let nil [erased]: Vector<bool, 0, 0>",
        ),
        (
            "let one [erased]: Vector<1, 7> = Vector::Cons { previous: 0, previous_total: 0, head: 7, tail: nil };",
            "let one [erased]: Vector<bool, 1, 7> = Vector::Cons { previous: 0, previous_total: 0, head: 7, marker: true, tail: nil };",
        ),
        ("bounded::<1, 7>(one)", "bounded::<bool, 1, 7>(one)"),
    ];
    let mut source = source.to_owned();
    for (old, new) in replaced {
        assert_eq!(source.matches(old).count(), 1, "{old}");
        source = source.replace(old, new);
    }
    source
}

#[test]
fn a_type_beside_the_indices_changes_nothing_that_is_proved() {
    // The theorem says nothing of `T` and no clause of the data can, so it
    // is the same theorem and `room` owes the same five requirements.
    let verdicts = judged(&with_marker(&proofs(RUNNING)));
    for verdict in &verdicts {
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_) | RouteVerdict::Laws(_)),
            "every part must be accepted: {verdict:?}"
        );
    }
    assert_eq!(closures(&verdicts, "room").len(), 5);
    assert_eq!(
        closures(&verdicts, "vector_bound"),
        closures(&judged(&proofs(RUNNING)), "vector_bound")
    );
}

#[test]
fn a_list_with_a_marker_still_owes_what_its_case_requires() {
    let source =
        with_marker(&proofs(RUNNING)).replace("head: 7, marker: true", "head: 11, marker: true");
    let verdicts = judged(&source);
    let unproved: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::UnprovedLaws(laws) => Some(laws),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(unproved.len(), 2, "{verdicts:?}");
}

const RETAIN: &str = "let retain<Length: u64, Total: u64>(value: Vector<Length, Total>): \
                      Vector<Length, Total> = value;\n";

#[test]
fn a_definition_hands_evidence_on_at_the_index_it_has() {
    // `retain` returns the evidence it takes, so `kept` is evidence of the
    // same list. Naming it at another total is refused.
    let handed = |at: &str| {
        format!("{RETAIN}{}", proofs(RUNNING)).replace(
            "    bounded::<1, 7>(one)\n",
            &format!(
                "    let kept [erased]: Vector<{at}> = retain::<{at}>(one);\n    \
                 bounded::<{at}>(kept)\n"
            ),
        )
    };
    let verdicts = judged(&handed("1, 7"));
    for verdict in &verdicts {
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_) | RouteVerdict::Laws(_)),
            "every part must be accepted: {verdict:?}"
        );
    }
    let verdicts = judged(&handed("1, 20"));
    assert!(
        refused(&verdicts)
            .iter()
            .any(|detail| detail
                .contains("evidence handed on at another index than the local declares")),
        "{verdicts:?}"
    );
}
