//! Contracts that compare values with `==` state the same comparison the body
//! evaluates (`wiki/spec/proofs/contracts.md`, "Data identity in facts"), and
//! only explicit evidence moves between that comparison and
//! `|proof|::Identity` ("Equality bridges and comparison facts").
//!
//! Each program is checked with the bundled `core/equality.omg` and
//! `core/foundation.omg` beside it, as the toolchain supplies them.

#[path = "support/source_handoff.rs"]
mod source;

use source::{Program, Verdict};

const EQUALITY: &str = include_str!("../../../../../../../source/library/core/equality.omg");
const FOUNDATION: &str = include_str!("../../../../../../../source/library/core/foundation.omg");

fn checked(main: &str) -> Result<Verdict, source::EarlierRefusal> {
    Program::try_of(&[
        ("core/equality.omg", EQUALITY),
        ("core/foundation.omg", FOUNDATION),
        ("product/main.omg", main),
    ])
    .map(|program| program.judged(|_| {}))
}

#[track_caller]
fn accepted(main: &str) {
    let verdict = checked(main);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

#[track_caller]
fn call_requirement_unproved(main: &str) {
    let verdict = checked(main);
    assert!(
        format!("{verdict:?}").contains("unproved: [\"CallRequirement"),
        "{verdict:?}"
    );
}

#[track_caller]
fn refused_before_checking(main: &str, message: &str) {
    match checked(main) {
        Err(refusal) => assert!(refusal.message.contains(message), "{refusal:?}"),
        other => panic!("expected a refusal naming {message:?}, got {other:?}"),
    }
}

/// `Coin`'s selected comparison is a user row that compares one field, and
/// promises what a clause needs of an operation: no service, no crash, and
/// that it returns.
const COIN: &str = "data Coin { value: i32; year: i32; }
CoinByValue satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.value == right.value
    }
}
";

fn coin(program: &str) -> String {
    format!("{COIN}{program}")
}

#[test]
fn a_user_comparison_in_a_requirement_is_owed_by_the_same_comparison() {
    accepted(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin) requires a == b { needs(a, b); }
",
    ));
    accepted(&coin(
        "machine needs(a: Coin, b: Coin) requires a != b {}
machine forward(a: Coin, b: Coin) requires a != b { needs(a, b); }
",
    ));
}

#[test]
fn the_operator_and_the_selected_row_name_one_comparison() {
    accepted(&coin(
        "machine needs(a: Coin, b: Coin) requires CoinByValue::equals(@a, @b) {}
machine forward(a: Coin, b: Coin) requires a == b { needs(a, b); }
",
    ));
}

#[test]
fn a_comparison_of_shared_referents_is_owed_through_the_same_loans() {
    accepted(&coin(
        "machine needs(a: @Coin, b: @Coin) requires a == b {}
machine forward(a: @Coin, b: @Coin) requires a == b { needs(a, b); }
",
    ));
}

#[test]
fn a_comparison_of_other_values_does_not_prove_the_call() {
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin, c: Coin) requires a == b { needs(a, c); }
",
    ));
    // A user comparison need not be symmetric.
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin) requires a == b { needs(b, a); }
",
    ));
}

#[test]
fn equal_and_unequal_are_different_claims() {
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a != b {}
machine forward(a: Coin, b: Coin) requires a == b { needs(a, b); }
",
    ));
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin) requires a != b { needs(a, b); }
",
    ));
}

#[test]
fn a_user_comparison_is_not_read_as_its_fields() {
    // The body compares `value`, but a clause sees only the selected
    // operation: the field fact supplies no comparison result.
    call_requirement_unproved(&coin(
        "machine needs(a: Coin, b: Coin) requires a == b {}
machine forward(a: Coin, b: Coin) requires a.value == b.value { needs(a, b); }
",
    ));
}

#[test]
fn another_implementation_for_the_same_type_is_another_operation() {
    let two = "data Coin { value: i32; year: i32; }
CoinByValue satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.value == right.value
    }
}
CoinByYear satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool reaches none crashes none terminates {
        left.year == right.year
    }
}
";
    accepted(&format!(
        "{two}machine needs(a: Coin, b: Coin) requires CoinByYear::equals(@a, @b) {{}}
machine forward(a: Coin, b: Coin) requires CoinByYear::equals(@a, @b) {{ needs(a, b); }}
"
    ));
    call_requirement_unproved(&format!(
        "{two}machine needs(a: Coin, b: Coin) requires CoinByYear::equals(@a, @b) {{}}
machine forward(a: Coin, b: Coin) requires CoinByValue::equals(@a, @b) {{ needs(a, b); }}
"
    ));
}

#[test]
fn a_comparison_that_may_not_return_is_not_a_term() {
    // `Equatable::equals` promises nothing, so the row must: proof use
    // keeps the selected invocation's termination and effect rules.
    refused_before_checking(
        "data Coin { value: i32; year: i32; }
CoinByValue satisfies Equatable<Coin> {
    machine equals(left: @Coin, right: @Coin) -> bool { left.value == right.value }
}
machine needs(a: Coin, b: Coin) requires a == b {}
",
        "clause application needs an unconditional pure and total operation",
    );
}

#[test]
fn a_mutable_loan_names_no_referent_a_clause_can_keep() {
    // The machine may write through `a` before the call; a fact about the
    // referent at entry would then describe a value the call never sees.
    refused_before_checking(
        &coin(
            "machine needs(a: @Coin, b: @Coin) requires a == b {}
machine stale(a: @mutable Coin, b: @Coin) requires $a == $b { a.value = 3i32; needs(a, b); }
",
        ),
        "Identity operand has no exact input observation",
    );
    refused_before_checking(
        "use {omega}/language/core/foundation.omg as proof;
data P { x: i32; y: i32; }
machine needs(a: @P, b: @P) requires |proof|::Identity::<P>($a, $b) {}
machine stale(a: @mutable P, b: @P) requires |proof|::Identity::<P>($a, $b) { a.x = 3i32; needs(a, b); }
",
        "Identity operand has no exact input observation",
    );
}
