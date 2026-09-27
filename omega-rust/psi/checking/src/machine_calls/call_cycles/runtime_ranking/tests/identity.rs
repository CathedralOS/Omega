//! Declared identity views inside runtime call components share validation's
//! classification with the checked stage; the produced rank is the subject.

use super::{RankProjection, admitted, typed_source};
use crate::machine_calls::call_cycles::runtime_ranking::projection::RankOrder;

const IDENTITY: &str = "data Countdown {}
measure Countdown::Remaining(value: u32) -> u32 { value }
data Main {}
machine Main::scan_a(&mut self, remaining: u32 [0..=5])
terminates by remaining -> Countdown::Remaining in 0..=5;
-> u32 {
    transition remaining > 0 { true -> self.scan_b(remaining) false -> remaining }
}
machine Main::scan_b(&mut self, pending: u32 [0..=5])
terminates by pending -> Countdown::Remaining in 0..=5;
-> u32 {
    transition pending > 0 { true -> self.scan_a(pending - 1) false -> pending }
}";

#[test]
fn declared_identity_members_project_the_subject_as_their_rank() {
    let program = typed_source(IDENTITY);
    for machine in program.machines() {
        let rank = RankProjection::resolve(&program, machine).expect("identity projection");
        assert!(matches!(rank.order, RankOrder::DeclaredIdentity { .. }));
        assert!(rank.range.is_valid());
    }
    assert_eq!(admitted(&program).len(), 1);
    // The optional range is transported, not required.
    let unranged = IDENTITY.replace(" in 0..=5;", ";");
    assert_ne!(unranged, IDENTITY);
    assert_eq!(admitted(&typed_source(&unranged)).len(), 1);
    // The shared range judgment still owns descent: a forwarded rank leaves
    // the cycle without a strict decrease.
    let stalled = IDENTITY.replace("self.scan_a(pending - 1)", "self.scan_a(pending)");
    assert!(admitted(&typed_source(&stalled)).is_empty());
    let raised = IDENTITY.replace("self.scan_a(pending - 1)", "self.scan_a(pending + 1)");
    assert!(admitted(&typed_source(&raised)).is_empty());
}

#[test]
fn declared_identity_views_keep_their_private_witness_identity() {
    // A builtin view member and a declared identity member produce equal
    // ranks, but the authored view is not relabeled `Nat::Descending`.
    let mixed = IDENTITY.replace(
        "terminates by pending -> Countdown::Remaining in 0..=5;",
        "terminates by pending -> Nat::Descending in 0..=5;",
    );
    assert_ne!(mixed, IDENTITY);
    let program = typed_source(&mixed);
    assert!(matches!(
        RankProjection::resolve(&program, &program.machines()[1]).map(|rank| rank.order),
        Some(RankOrder::Natural(_))
    ));
    assert!(admitted(&program).is_empty());
    // Two declared measures with identical bodies are two orders.
    let second = IDENTITY
        .replace(
            "measure Countdown::Remaining(value: u32) -> u32 { value }",
            "measure Countdown::Remaining(value: u32) -> u32 { value }\nmeasure Countdown::Pending(value: u32) -> u32 { value }",
        )
        .replace(
            "terminates by pending -> Countdown::Remaining in 0..=5;",
            "terminates by pending -> Countdown::Pending in 0..=5;",
        );
    assert_ne!(second, IDENTITY);
    assert!(admitted(&typed_source(&second)).is_empty());
}

#[test]
fn declared_identity_views_reject_uncovered_domains_and_other_bodies() {
    // The measure's declared refinement must contain the subject's enforced
    // bounds: `[1..=5]` does not cover a `[0..=5]` subject.
    let domain = IDENTITY.replace(
        "measure Countdown::Remaining(value: u32) -> u32 { value }",
        "measure Countdown::Remaining(value: u32 [1..=5]) -> u32 { value }",
    );
    assert_ne!(domain, IDENTITY);
    let program = typed_source(&domain);
    assert!(RankProjection::resolve(&program, &program.machines()[0]).is_none());
    assert!(admitted(&program).is_empty());
    // A covering refinement is admitted.
    let covered = IDENTITY.replace(
        "measure Countdown::Remaining(value: u32) -> u32 { value }",
        "measure Countdown::Remaining(value: u32 [0..=5]) -> u32 { value }",
    );
    assert_eq!(admitted(&typed_source(&covered)).len(), 1);
    // A widening carrier is not an identity of this subject.
    let widened = IDENTITY.replace(
        "measure Countdown::Remaining(value: u32) -> u32 { value }",
        "measure Countdown::Remaining(value: u64) -> u64 { value }",
    );
    assert!(admitted(&typed_source(&widened)).is_empty());
}
