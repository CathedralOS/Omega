//! Declared computation views inside runtime call components produce their
//! rank from the body; strict monotonicity is the only reason the subject's
//! descent stands for the rank's descent, and formation is proved per site.

use super::{RankProjection, admitted, typed_source};
use crate::machine_calls::call_cycles::runtime_ranking::projection::RankOrder;

const DOUBLED: &str = "data Countdown {}
measure Countdown::Doubled(value: u8) -> u8 { value * 2 }
data Main {}
machine Main::scan_a(&mut self, remaining: u8 [0..=100])
terminates by remaining -> Countdown::Doubled in 0..=200;
-> u8 {
    transition remaining > 0 { true -> self.scan_b(remaining) false -> remaining }
}
machine Main::scan_b(&mut self, pending: u8 [0..=100])
terminates by pending -> Countdown::Doubled in 0..=200;
-> u8 {
    transition pending > 0 { true -> self.scan_a(pending - 1) false -> pending }
}";

#[test]
fn computed_members_project_the_body_over_the_subject() {
    let program = typed_source(DOUBLED);
    for machine in program.machines() {
        let rank = RankProjection::resolve(&program, machine).expect("computed projection");
        assert!(matches!(rank.order, RankOrder::DeclaredComputation { .. }));
        assert!(rank.range.is_valid());
    }
    assert_eq!(admitted(&program).len(), 1);
    // The optional range is transported, not required; the produced rank
    // must still form inside `u8` from the members' own bounds.
    let unranged = DOUBLED.replace(" in 0..=200;", ";");
    assert_ne!(unranged, DOUBLED);
    assert_eq!(admitted(&typed_source(&unranged)).len(), 1);
    // The shared range judgment still owns descent on the produced rank.
    let stalled = DOUBLED.replace("self.scan_a(pending - 1)", "self.scan_a(pending)");
    assert!(admitted(&typed_source(&stalled)).is_empty());
    let raised = DOUBLED.replace("self.scan_a(pending - 1)", "self.scan_a(pending + 1)");
    assert!(admitted(&typed_source(&raised)).is_empty());
}

#[test]
fn computed_members_share_an_order_only_through_the_same_measure() {
    let mixed = DOUBLED
        .replace(
            "measure Countdown::Doubled(value: u8) -> u8 { value * 2 }",
            "measure Countdown::Doubled(value: u8) -> u8 { value * 2 }\nmeasure Countdown::Remaining(value: u8) -> u8 { value }",
        )
        .replace(
            "terminates by pending -> Countdown::Doubled in 0..=200;",
            "terminates by pending -> Countdown::Remaining in 0..=100;",
        );
    assert_ne!(mixed, DOUBLED);
    let program = typed_source(&mixed);
    assert!(matches!(
        RankProjection::resolve(&program, &program.machines()[1]).map(|rank| rank.order),
        Some(RankOrder::DeclaredIdentity { .. })
    ));
    assert!(admitted(&program).is_empty());
}

#[test]
fn computed_members_prove_formation_inside_the_carrier() {
    // `[0..=200] * 2` fits the authored `0..=400` but not `u8`: the member's
    // initial rank is unproven.
    let overflowing = DOUBLED
        .replace("pending: u8 [0..=100]", "pending: u8 [0..=200]")
        .replace(
            "terminates by pending -> Countdown::Doubled in 0..=200;",
            "terminates by pending -> Countdown::Doubled in 0..=400;",
        );
    assert_ne!(overflowing, DOUBLED);
    assert!(admitted(&typed_source(&overflowing)).is_empty());
    // Without a range, an unconstrained subject still cannot form `value * 2`
    // at its call site.
    let unbounded = DOUBLED
        .replace(" in 0..=200;", ";")
        .replace("pending: u8 [0..=100]", "pending: u8");
    assert_ne!(unbounded, DOUBLED);
    assert!(admitted(&typed_source(&unbounded)).is_empty());
}
