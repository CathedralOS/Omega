//! Term forms a clause writes, read by stage 05's one clause reader
//! (`read/clause_terms.rs`), so a form served in one position is served in
//! every other.

use super::build_purpose::judge_build;
use super::*;

/// `!b` was read only as a whole clause, so `requires a == !b` was refused
/// as a term with no carrier value. It is the Boolean negation of `b` as an
/// operand too: a caller that knows `a == !b` meets the requirement, and one
/// that knows only `a == b` does not.
#[test]
fn a_negated_boolean_is_an_operand_of_a_comparison() {
    const NEGATED: &str =
        include_str!("../../tests/fixtures/clause_terms/pass/negated_operand/build.omg");
    const UNRELATED: &str =
        include_str!("../../tests/fixtures/clause_terms/fail/negated_operand_unrelated/build.omg");
    match judge_build(NEGATED) {
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
        }
        other => panic!("`a == !b` must meet the same requirement: {other:?}"),
    }
    let unproved = match judge_build(UNRELATED) {
        Ok(RouteVerdict::Incomplete(unproved)) | Ok(RouteVerdict::Rejected { unproved, .. }) => {
            unproved
        }
        other => panic!("`a == b` must leave `a == !b` unproved: {other:?}"),
    };
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(
        matches!(unproved[0].kind, ObligationKind::CallRequirement { .. }),
        "{unproved:?}",
    );
}

/// `a < b` was read only as a whole clause, so `requires flag == (a < b)`
/// was refused. A comparison is the Boolean value it computes as an operand
/// too: a caller that knows `flag == (a < b)` meets the requirement, and one
/// that knows only `flag == (a <= b)` does not.
#[test]
fn a_comparison_is_an_operand_of_a_boolean_equality() {
    const SAME: &str =
        include_str!("../../tests/fixtures/clause_terms/pass/comparison_operand/build.omg");
    const WEAKER: &str =
        include_str!("../../tests/fixtures/clause_terms/fail/comparison_operand_weaker/build.omg");
    match judge_build(SAME) {
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
        }
        other => panic!("`flag == (a < b)` must meet the same requirement: {other:?}"),
    }
    let unproved = match judge_build(WEAKER) {
        Ok(RouteVerdict::Incomplete(unproved)) | Ok(RouteVerdict::Rejected { unproved, .. }) => {
            unproved
        }
        other => panic!("`flag == (a <= b)` must leave `flag == (a < b)` unproved: {other:?}"),
    };
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(
        matches!(unproved[0].kind, ObligationKind::CallRequirement { .. }),
        "{unproved:?}",
    );
}

const STATED: &str = include_str!("../../tests/fixtures/clause_terms/pass/stated_case/build.omg");
const NOT_RETURNED: &str =
    include_str!("../../tests/fixtures/clause_terms/fail/stated_case_not_returned/build.omg");

/// What is left unproved, as each obligation's kind and reason.
fn unproved(source: &str) -> Vec<(ObligationKind, String)> {
    match judge_build(source) {
        Ok(RouteVerdict::Incomplete(unproved)) | Ok(RouteVerdict::Rejected { unproved, .. }) => {
            unproved
                .into_iter()
                .map(|obligation| (obligation.kind, obligation.detail))
                .collect()
        }
        other => panic!("the program must leave an obligation unproved: {other:?}"),
    }
}

fn accepted(source: &str) {
    match judge_build(source) {
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
        }
        other => panic!("the program must be accepted: {other:?}"),
    }
}

/// One edit of the passing program.
fn stated(old: &str, new: &str) -> String {
    assert_eq!(STATED.matches(old).count(), 1, "{old}");
    STATED.replace(old, new)
}

/// `x in S::C` was read only in a data or domain predicate. In a machine
/// clause it states the case a formal holds at entry or the result holds at
/// return: the callee proves a guaranteed case where it returns, the caller
/// may rest on it, and a required case is owed at the call.
#[test]
fn a_machine_clause_states_the_case_of_a_formal_or_its_result() {
    accepted(STATED);
    assert_eq!(
        NOT_RETURNED,
        STATED
            .replace(
                "{ Outcome::Good }\nmachine take",
                "{ Outcome::Bad }\nmachine take"
            )
            .replacen(
                STATED.split("data Outcome").next().unwrap(),
                NOT_RETURNED.split("data Outcome").next().unwrap(),
                1
            ),
        "the twin differs in the returned case and its comment"
    );
    let left = unproved(NOT_RETURNED);
    assert!(
        matches!(left.as_slice(), [(ObligationKind::NormalGuarantee { .. }, detail)]
            if detail == "the goal is false"),
        "{left:?}"
    );
}

#[test]
fn a_required_case_is_owed_at_the_call() {
    // Nothing says which case `pick` returns once it guarantees none.
    let left = unproved(&stated(" ensures result in Outcome::Good", ""));
    assert!(
        matches!(left.as_slice(), [(ObligationKind::CallRequirement { .. }, detail)]
            if detail == "the case a requires clause tests is not established by structure at this call"),
        "{left:?}"
    );
    // A guaranteed `Bad` shows the requirement false.
    let source = stated(
        "ensures result in Outcome::Good { Outcome::Good }",
        "ensures result in Outcome::Bad { Outcome::Bad }",
    );
    let left = unproved(&source);
    assert!(
        matches!(left.as_slice(), [(ObligationKind::CallRequirement { .. }, detail)]
            if detail == "the goal is false"),
        "{left:?}"
    );
}

#[test]
fn a_required_case_holds_at_entry_until_the_value_is_replaced() {
    // `relay` requires `Good` and passes its formal on.
    let relay = |requirement: &str, body: &str| {
        stated(
            "machine build_entry() { let picked: Outcome = pick(); take(picked); }",
            &format!(
                "machine relay(outcome: Outcome){requirement} {{ {body} }}\n\
                 machine build_entry() {{ let picked: Outcome = pick(); relay(picked); }}"
            ),
        )
    };
    accepted(&relay(
        " requires outcome in Outcome::Good",
        "take(outcome);",
    ));
    // Without its own requirement `relay` knows nothing of its formal.
    let left = unproved(&relay("", "take(outcome);"));
    assert!(
        matches!(left.as_slice(), [(ObligationKind::CallRequirement { .. }, detail)]
            if detail == "the case a requires clause tests is not established by structure at this call"),
        "{left:?}"
    );
    // The entry case is of the entry value: a value stored over it has the
    // case that was stored.
    let left = unproved(&relay(
        " requires outcome in Outcome::Good",
        "let mutable held: Outcome = outcome; held = Outcome::Bad; take(held);",
    ));
    assert!(
        matches!(left.as_slice(), [(ObligationKind::CallRequirement { .. }, detail)]
            if detail == "the goal is false"),
        "{left:?}"
    );
}

#[test]
fn a_case_test_beside_an_alternative_states_nothing_and_is_refused() {
    // `(outcome in Good) || flag` may hold with the test false, so nothing
    // may be assumed of the case at entry, and the clause is refused by name.
    let source = stated(
        "machine take(outcome: Outcome) requires outcome in Outcome::Good { }\n\
         machine build_entry() { let picked: Outcome = pick(); take(picked); }",
        "machine take(outcome: Outcome, flag: bool) requires (outcome in Outcome::Good) || flag { }\n\
         machine build_entry() { let picked: Outcome = pick(); take(picked, false); }",
    );
    match judge_build(&source) {
        Ok(RouteVerdict::FallBack(refusal)) => assert!(
            refusal.detail.contains("a case test in an entry clause"),
            "{refusal:?}"
        ),
        other => panic!("the clause must be refused by name: {other:?}"),
    }
}
