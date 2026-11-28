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
            if detail == UNPROVED_BOOLEAN),
        "{left:?}"
    );
    // A guaranteed `Bad` makes the result's `Bad` name true, which does not
    // make its `Good` name true.
    let source = stated(
        "ensures result in Outcome::Good { Outcome::Good }",
        "ensures result in Outcome::Bad { Outcome::Bad }",
    );
    let left = unproved(&source);
    assert!(
        matches!(left.as_slice(), [(ObligationKind::CallRequirement { .. }, detail)]
            if detail == UNPROVED_BOOLEAN),
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
            if detail == UNPROVED_BOOLEAN),
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

const ALTERNATIVE: &str =
    include_str!("../../tests/fixtures/clause_terms/pass/case_alternative/build.omg");
const ALTERNATIVE_BAD: &str =
    include_str!("../../tests/fixtures/clause_terms/fail/case_alternative_bad/build.omg");

/// What the prover says of a Boolean equality, such as a named case test,
/// that no hypothesis states.
const UNPROVED_BOOLEAN: &str =
    "integer order antisymmetry requires one non-address integer carrier";

/// One edit of the alternative program.
fn alternative(old: &str, new: &str) -> String {
    assert_eq!(ALTERNATIVE.matches(old).count(), 1, "{old}");
    ALTERNATIVE.replace(old, new)
}

/// The only obligation left unproved is one requirement at a call, shown
/// false or simply not proved.
fn one_call_requirement(source: &str) -> String {
    let left = unproved(source);
    match left.as_slice() {
        [(ObligationKind::CallRequirement { .. }, detail)] => detail.clone(),
        _ => panic!("one call requirement must be left: {left:?}"),
    }
}

/// `(outcome in Outcome::Good) || flag` was refused at entry, because the
/// checker could only decide a case test, and nothing decides this one
/// there. The test is now a proposition over the Boolean that names the
/// formal's entry case, so `relay` assumes the disjunction and proves
/// `take`'s requirement with `!flag`, and its caller meets the disjunction
/// with a `Good` argument but not with a `Bad` one.
#[test]
fn a_case_test_beside_an_alternative_is_a_proposition_over_the_entry_case() {
    accepted(ALTERNATIVE);
    assert_eq!(
        ALTERNATIVE_BAD.split("data Outcome").nth(1),
        alternative("relay(Outcome::Good, false)", "relay(Outcome::Bad, false)")
            .split("data Outcome")
            .nth(1),
        "the twin differs in the argument's case"
    );
    assert_eq!(one_call_requirement(ALTERNATIVE_BAD), UNPROVED_BOOLEAN);
    // Without `!flag` the disjunction alone does not give `take` its case.
    assert_eq!(
        one_call_requirement(&alternative(" requires !flag", "")),
        UNPROVED_BOOLEAN
    );
}

/// The entry name is of the value the formal held at entry: a value stored
/// over a copy has the case that was stored.
#[test]
fn an_entry_case_name_is_not_the_case_of_later_storage() {
    let source = alternative(
        "{ take(outcome); }",
        "{ let mutable held: Outcome = outcome; held = Outcome::Bad; take(held); }",
    );
    assert_eq!(one_call_requirement(&source), "the goal is false");
}

/// A test of the case decides it by structure on each branch, so a branch
/// may rest on its case whatever the entry clause says.
#[test]
fn a_branch_on_a_named_entry_case_decides_it() {
    accepted(&alternative(
        " requires !flag { take(outcome); }",
        " { transition (outcome in Outcome::Good) { true -> take(outcome), false -> skip() } }\n\
         machine skip() { }",
    ));
}

/// A callee's guarantee `(result in Outcome::Good) || flag` is assumed over
/// the Boolean that names the case its result holds after the call.
#[test]
fn a_returned_case_beside_an_alternative_is_assumed_after_the_call() {
    let pick = |flag: &str| {
        alternative(
            "machine build_entry() { relay(Outcome::Good, false); }",
            &format!(
                "machine pick(flag: bool) -> Outcome ensures (result in Outcome::Good) || flag {{ Outcome::Good }}\n\
                 machine build_entry() {{ let picked: Outcome = pick({flag}); take(picked); }}"
            ),
        )
    };
    accepted(&pick("false"));
    assert_eq!(one_call_requirement(&pick("true")), UNPROVED_BOOLEAN);
}

const OPERAND: &str = include_str!("../../tests/fixtures/clause_terms/pass/case_operand/build.omg");
const OPERAND_BAD: &str =
    include_str!("../../tests/fixtures/clause_terms/fail/case_operand_bad/build.omg");

/// `flag == (outcome in Outcome::Good)` was refused while the signature was
/// read, because the reader took the case test for a scalar operand and its
/// subject for a record named whole. As an operand of `==` the test is the
/// proposition it states, and the clause is that both sides hold together:
/// `relay` proves `take`'s requirement from it and `flag`, and its caller
/// meets it with a `Good` argument but not with a `Bad` one.
#[test]
fn a_case_test_is_a_boolean_operand_of_an_equality() {
    accepted(OPERAND);
    assert_eq!(
        OPERAND_BAD.split("data Outcome").nth(1),
        OPERAND
            .replace("relay(Outcome::Good, true)", "relay(Outcome::Bad, true)")
            .split("data Outcome")
            .nth(1),
        "the twin differs in the argument's case"
    );
    assert_eq!(one_call_requirement(OPERAND_BAD), "the goal is false");
    // `!=` states the opposite: a false `flag` then says the case holds.
    let unequal = OPERAND
        .replace("flag == (outcome", "flag != (outcome")
        .replace("requires flag {", "requires !flag {")
        .replace("relay(Outcome::Good, true)", "relay(Outcome::Good, false)");
    accepted(&unequal);
    assert_eq!(
        one_call_requirement(
            &unequal.replace("relay(Outcome::Good, false)", "relay(Outcome::Bad, false)")
        ),
        UNPROVED_BOOLEAN
    );
}

/// A case test has no carrier value, so it is refused by name where a
/// clause needs one, such as an operand of arithmetic.
#[test]
fn a_case_test_is_no_scalar_value() {
    let source = OPERAND.replace(
        "requires flag == (outcome in Outcome::Good)",
        "requires (outcome in Outcome::Good) < flag",
    );
    match judge_build(&source) {
        Ok(RouteVerdict::FallBack(refusal)) => assert!(
            refusal
                .detail
                .contains("case test as a value outside a Boolean comparison"),
            "{refusal:?}"
        ),
        other => panic!("the clause must be refused by name: {other:?}"),
    }
}

/// A membership test computes the Boolean that names the case, so a branch
/// on it decides the name on each edge: on the false edge of
/// `outcome in Outcome::Good`, `(outcome in Outcome::Good) || flag` leaves
/// `flag`, and `need` may be called. `(outcome in Outcome::Bad) || flag`
/// leaves nothing on that edge to give `need` its `flag`.
#[test]
fn a_membership_test_decides_the_named_case_on_each_edge() {
    let branch = |case: &str| {
        alternative(
            " requires (outcome in Outcome::Good) || flag requires !flag { take(outcome); }",
            &format!(
                " requires (outcome in Outcome::{case}) || flag \
                 {{ transition (outcome in Outcome::Good) \
                 {{ true -> take(outcome), false -> need(flag) }} }}\n\
                 machine need(flag: bool) requires flag {{ }}"
            ),
        )
        .replace("relay(Outcome::Good, false)", "relay(Outcome::Bad, true)")
    };
    accepted(&branch("Good"));
    assert_eq!(one_call_requirement(&branch("Bad")), UNPROVED_BOOLEAN);
}

/// A sum holds exactly one of its cases, so a clause that rules one case
/// out of a two-case sum states the other: `!(outcome in Outcome::Bad)`
/// gives `take` its `Good`, at entry and after a call. A `Bad` argument does
/// not meet that clause, and a result guaranteed not `Good` is not `Good`.
#[test]
fn a_named_case_rules_out_the_other_cases() {
    let entry = alternative(
        "requires (outcome in Outcome::Good) || flag requires !flag",
        "requires !(outcome in Outcome::Bad)",
    );
    accepted(&entry);
    assert_eq!(
        one_call_requirement(
            &entry.replace("relay(Outcome::Good, false)", "relay(Outcome::Bad, false)")
        ),
        "the goal is false"
    );
    let returned = |case: &str| {
        alternative(
            "machine build_entry() { relay(Outcome::Good, false); }",
            &format!(
                "machine pick() -> Outcome ensures !(result in Outcome::{case}) {{ Outcome::Good }}\n\
                 machine build_entry() {{ let picked: Outcome = pick(); take(picked); }}"
            ),
        )
    };
    accepted(&returned("Bad"));
    let left = unproved(&returned("Good"));
    assert!(
        matches!(
            left.as_slice(),
            [
                (ObligationKind::NormalGuarantee { .. }, _),
                (ObligationKind::CallRequirement { .. }, _)
            ] | [
                (ObligationKind::CallRequirement { .. }, _),
                (ObligationKind::NormalGuarantee { .. }, _)
            ]
        ),
        "{left:?}"
    );
}

/// A guarantee may project a payload under the test of its case, as
/// `core/representation.omg` writes `(result in Result::Err) || ..`. The
/// caller assumes it over the result's case names, and the dispatch arm
/// that selects the payload's case decides the name, so the payload's
/// guaranteed bound is known there. A weaker guaranteed bound is not.
#[test]
fn a_guarded_payload_guarantee_is_known_in_the_arm_that_selects_its_case() {
    let program = |relation: &str| {
        format!(
            "data Reading {{ case Missing; case Present(count: u32); }}\n\
             machine sense(flag: bool) -> Reading ensures (result in Reading::Missing) || result.count {relation} 0 {{\n\
             transition flag {{ true -> Reading::Present {{ count: 1 }}, false -> Reading::Missing }} }}\n\
             machine positive(count: u32) requires count > 0 {{ }}\n\
             machine skip() {{ }}\n\
             machine build_entry() {{ let reading: Reading = sense(true);\n\
             transition reading {{ Reading::Present {{ count }} -> positive(count), Reading::Missing -> skip() }} }}\n"
        )
    };
    accepted(&program(">"));
    assert_eq!(
        one_call_requirement(&program(">=")),
        "the bounds do not decide the comparison"
    );
}
