//! A declared domain bounds the operands of exact arithmetic only on the sides
//! its predicates state. An unstated side is the carrier's own extreme, never a
//! narrower stand-in: a `u64` domain requiring only `self >= 1` admits every
//! value up to `u64::MAX`, so `x + 1` must stay an overflow obligation.
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

fn accepted(source: &str) -> bool {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled()).is_ok()
}

fn program(domain: &str, carrier: &str, addend: &str) -> String {
    format!(
        "{domain}
        data Main {{ x: {carrier} in Bounded; y: {carrier}; }}
        machine Main::main(&mut self) {{ self.y = self.x + {addend}; }}"
    )
}

#[test]
fn a_lower_bound_only_unsigned_domain_leaves_the_upper_side_unbounded() {
    assert!(
        !accepted(&program(
            "domain u64::Bounded requires self >= 1;",
            "u64",
            "1"
        )),
        "a member may be u64::MAX, so `x + 1` may overflow"
    );
}

#[test]
fn an_upper_bound_domain_keeps_exact_addition_in_range() {
    assert!(
        accepted(&program(
            "domain u64::Bounded requires self <= 100;",
            "u64",
            "1"
        )),
        "`self <= 100` bounds `x + 1` by 101"
    );
    assert!(
        !accepted(&program(
            "domain u64::Bounded requires self <= 18446744073709551615;",
            "u64",
            "1"
        )),
        "a bound at the carrier maximum leaves `x + 1` unproven"
    );
}

#[test]
fn a_signed_lower_bound_only_domain_keeps_the_carrier_maximum() {
    assert!(
        !accepted(&program(
            "domain i64::Bounded requires self >= 0;",
            "i64",
            "1"
        )),
        "a member may be i64::MAX, so `x + 1` may overflow"
    );
}
