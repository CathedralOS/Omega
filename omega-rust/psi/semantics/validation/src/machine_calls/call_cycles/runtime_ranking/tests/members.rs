//! Member-chain scalar subjects inside runtime call components resolve to
//! the exact projected coordinate of the record carrier formal. Each call
//! edge walks the actual's forward, prefix member, or rebuilt literal and
//! binds the destination chain against the callee's own declaration; the
//! record carrier itself is an identity role, not a scalar that may drift.

use super::super::RankingRangeCallProgress;
use super::super::projection::RankOrder;
use super::ranges::progress;
use super::{RankProjection, admitted, typed_source};

const MEMBER: &str = "data Bag { count: u64 [0..=9]; }
data Main {}
machine Main::scan_a(&mut self, bag: Bag)
terminates by bag.count -> Nat::Descending in 0..=9;
-> u64 {
    transition bag.count > 0 {
        true -> self.scan_b(Bag { count: bag.count - 1 })
        false -> bag.count
    }
}
machine Main::scan_b(&mut self, bag: Bag)
terminates by bag.count -> Nat::Descending in 0..=9;
-> u64 {
    transition bag.count > 0 {
        true -> self.scan_a(Bag { count: bag.count - 1 })
        false -> bag.count
    }
}";

#[test]
fn member_scalar_component_admits_strict_rebuilt_arrivals() {
    let program = typed_source(MEMBER);
    for machine in program.machines() {
        let rank = RankProjection::resolve(&program, machine).expect("member projection");
        assert!(matches!(rank.order, RankOrder::Natural(_)));
        assert!(rank.record_subject.is_valid());
    }
    assert_eq!(
        progress(&program, 0),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(admitted(&program).len(), 1);
}

#[test]
fn member_scalar_component_classifies_forwards_and_rejects_stalls() {
    // Forwarding the whole record preserves its projected coordinate: a
    // valid weak link beside the strict rebuilt edge.
    let weak = MEMBER.replace(
        "self.scan_b(Bag { count: bag.count - 1 })",
        "self.scan_b(bag)",
    );
    assert_eq!(
        progress(&typed_source(&weak), 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert_eq!(admitted(&typed_source(&weak)).len(), 1);
    // Rebuilding with the unchanged or a deeper-cut leaf cannot descend.
    for actual in [
        "Bag { count: bag.count }",
        "Bag { count: bag.count - 2 }",
        "Bag { count: bag.count + 1 }",
    ] {
        let changed = MEMBER.replace("Bag { count: bag.count - 1 }", actual);
        assert!(admitted(&typed_source(&changed)).is_empty(), "{actual}");
    }
}

#[test]
fn member_scalar_component_rejects_foreign_carriers_and_moved_endpoints() {
    // A sibling record's `count` is a different coordinate: the rebuilt
    // literal reading `spare.count` never lands on the ranked slot.
    let foreign = MEMBER
        .replace(
            "machine Main::scan_b(&mut self, bag: Bag)",
            "machine Main::scan_b(&mut self, bag: Bag, spare: Bag)",
        )
        .replace(
            "self.scan_a(Bag { count: bag.count - 1 })",
            "self.scan_a(Bag { count: spare.count - 1 })",
        );
    assert!(admitted(&typed_source(&foreign)).is_empty());
    // A respelled member chain in the authored range is not the pinned
    // literal endpoint either.
    let respelled = MEMBER.replace(" in 0..=9;", " in 0..=bag.count;");
    assert!(admitted(&typed_source(&respelled)).is_empty());
}

#[test]
fn member_scalar_component_rejects_prefix_stores_into_the_carrier() {
    let written = MEMBER
        .replace(
            "machine Main::scan_b(&mut self, bag: Bag)",
            "machine Main::scan_b(&mut self, mut bag: Bag)",
        )
        .replace(
            "    transition bag.count > 0 {\n        true -> self.scan_a",
            "    bag.count = 5;\n    transition bag.count > 0 {\n        true -> self.scan_a",
        );
    assert_ne!(written, MEMBER);
    assert!(admitted(&typed_source(&written)).is_empty());
}

const NESTED: &str = "data Inner { count: u64 [0..=9]; }
data Outer { tag: u64; inner: Inner; }
data Main {}
machine Main::scan_a(&mut self, outer: Outer)
terminates by outer.inner.count -> Nat::Descending in 0..=9;
-> u64 {
    transition outer.inner.count > 0 {
        true -> self.scan_b(outer)
        false -> outer.inner.count
    }
}
machine Main::scan_b(&mut self, outer: Outer)
terminates by outer.inner.count -> Nat::Descending in 0..=9;
-> u64 {
    transition outer.inner.count > 0 {
        true -> self.scan_a(Outer {
            tag: outer.tag,
            inner: Inner { count: outer.inner.count - 1 }
        })
        false -> outer.inner.count
    }
}";

#[test]
fn nested_member_scalar_component_walks_the_chain_through_the_literal() {
    let program = typed_source(NESTED);
    for machine in program.machines() {
        assert!(RankProjection::resolve(&program, machine).is_some());
    }
    assert_eq!(admitted(&program).len(), 1);
    // Stalling the nested leaf leaves only a weak cycle.
    let stalled = NESTED.replace("outer.inner.count - 1", "outer.inner.count");
    assert!(admitted(&typed_source(&stalled)).is_empty());
    // A same-spelled leaf under the wrong prefix is a foreign coordinate.
    let wrong = NESTED.replace("outer.inner.count - 1", "outer.tag");
    assert!(admitted(&typed_source(&wrong)).is_empty());
}

const INCREASING: &str = "data Bag { cursor: u64; limit: u64 [0..=9]; }
data Main {}
machine Main::scan_a(&mut self, bag: Bag)
requires bag.cursor <= bag.limit;
terminates by bag.cursor -> Nat::IncreasingTo(bag.limit) in 0..=9;
-> u64 {
    transition bag.cursor < bag.limit {
        true -> self.scan_b(bag)
        false -> bag.cursor
    }
}
machine Main::scan_b(&mut self, bag: Bag)
requires bag.cursor <= bag.limit;
terminates by bag.cursor -> Nat::IncreasingTo(bag.limit) in 0..=9;
-> u64 {
    transition bag.cursor < bag.limit {
        true -> self.scan_a(Bag { cursor: bag.cursor + 1, limit: bag.limit })
        false -> bag.cursor
    }
}";

#[test]
fn member_increasing_component_pins_the_projected_bound() {
    let program = typed_source(INCREASING);
    assert_eq!(
        progress(&program, 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(admitted(&program).len(), 1);
    // Rebuilding the bound field moves the pinned view endpoint: the strict
    // edge's destination no longer shares the caller's ceiling.
    let moved = INCREASING.replace("limit: bag.limit", "limit: 9");
    assert!(admitted(&typed_source(&moved)).is_empty());
    // Stalling the cursor leaves a weak-only component.
    let stalled = INCREASING.replace("cursor: bag.cursor + 1", "cursor: bag.cursor");
    assert!(admitted(&typed_source(&stalled)).is_empty());
}
