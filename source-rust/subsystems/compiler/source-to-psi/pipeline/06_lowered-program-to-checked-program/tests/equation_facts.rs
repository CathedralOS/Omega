//! The floor reasons through mathematical equations among the facts
//! (`wiki/spec/proofs/contracts.md`, "Required automatic discharge"). Each
//! positive differs from its twin in the one fact the proof depends on.
//!
//! An equation `term == other` bounds `term` by what bounds `other`: a sum of
//! two `u64` values equal to a third `u64` fits the `u64` range. Two
//! equations that meet in one term make their other terms equal, also after
//! rewriting a value through a value equality among the facts. The kernel
//! replays each step as an order substitution, an antisymmetry and a
//! transport.

#[path = "support/source_handoff.rs"]
mod source;

use source::Program;

const BOUNDED_SUM: &str = include_str!("fixtures/equation_facts/pass/bounded_sum/main.omg");
const UNBOUNDED_SUM: &str = include_str!("fixtures/equation_facts/fail/unbounded_sum/main.omg");
const CHAINED: &str = include_str!("fixtures/equation_facts/pass/chained_equality/main.omg");
const ORDERED_LINK: &str = include_str!("fixtures/equation_facts/fail/ordered_link/main.omg");
const TRANSPORTED: &str =
    include_str!("fixtures/equation_facts/pass/transported_equality/main.omg");
const UNTRANSPORTED: &str =
    include_str!("fixtures/equation_facts/fail/untransported_equality/main.omg");

/// The program text after its leading comment.
fn body(source: &str) -> &str {
    let start = source.find("machine").unwrap();
    &source[start..]
}

/// The one obligation the twin leaves unproved is the call's requirement.
fn refused_at_the_call(source: &str) {
    let rejection = Program::of(source).judged(|_| {}).rejected();
    let [unproved] = rejection.unproved.as_slice() else {
        panic!("one unproved obligation: {rejection:?}")
    };
    assert!(unproved.starts_with("CallRequirement"), "{rejection:?}");
}

#[test]
fn a_sum_equal_to_a_fixed_integer_fits_its_carrier() {
    Program::of(BOUNDED_SUM).judged(|_| {}).accepted();
}

#[test]
fn a_sum_only_bounded_below_by_a_fixed_integer_does_not_fit_its_carrier() {
    assert_eq!(
        body(BOUNDED_SUM).replace(
            "embed(x) + embed(y) == embed(z)",
            "embed(z) <= embed(x) + embed(y)"
        ),
        body(UNBOUNDED_SUM)
    );
    refused_at_the_call(UNBOUNDED_SUM);
}

#[test]
fn two_equations_meeting_in_one_term_equate_their_other_terms() {
    Program::of(CHAINED).judged(|_| {}).accepted();
}

#[test]
fn an_order_in_place_of_the_second_equation_equates_nothing() {
    assert_eq!(
        body(CHAINED).replace(
            "embed(r) + embed(t) == embed(c)",
            "embed(r) + embed(t) <= embed(c)"
        ),
        body(ORDERED_LINK)
    );
    refused_at_the_call(ORDERED_LINK);
}

#[test]
fn a_value_equality_lets_two_equations_meet() {
    Program::of(TRANSPORTED).judged(|_| {}).accepted();
}

#[test]
fn without_the_value_equality_the_equations_do_not_meet() {
    assert_eq!(
        body(TRANSPORTED).replace(", m == t", ""),
        body(UNTRANSPORTED)
    );
    refused_at_the_call(UNTRANSPORTED);
}
