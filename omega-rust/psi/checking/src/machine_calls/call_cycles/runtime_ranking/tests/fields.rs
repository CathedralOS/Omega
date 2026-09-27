//! Declared field views inside runtime call components produce the measure's
//! exact `u64` coordinate of the record subject. Members share the order only
//! through the same declared measure; each call edge re-resolves the chain
//! against the formals' own declarations and walks the actual's forward,
//! borrow, prefix member, or rebuilt literal.

use super::{RankProjection, admitted, ranges::progress, typed_source};
use crate::machine_calls::call_cycles::runtime_ranking::projection::RankOrder;
use crate::proof_contracts::contract_entailment::RankingRangeCallProgress;

const FIELD: &str = "data Countdown { remaining: u64 [0..=5]; }
measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.remaining }
data Main {}
machine Main::scan_a(&mut self, countdown: Countdown)
terminates by countdown -> Countdown::Remaining in 0..=5;
-> u64 {
    transition countdown.remaining > 0 {
        true -> self.scan_b(countdown)
        false -> countdown.remaining
    }
}
machine Main::scan_b(&mut self, pending: Countdown)
terminates by pending -> Countdown::Remaining in 0..=5;
-> u64 {
    transition pending.remaining > 0 {
        true -> self.scan_a(Countdown { remaining: pending.remaining - 1 })
        false -> pending.remaining
    }
}";

#[test]
fn declared_field_view_members_share_the_produced_coordinate() {
    let program = typed_source(FIELD);
    for machine in program.machines() {
        let rank = RankProjection::resolve(&program, machine).expect("field projection");
        assert!(matches!(rank.order, RankOrder::CustomStructView { .. }));
    }
    // The forward edge preserves the coordinate; the rebuilt literal strictly
    // decreases it.
    assert_eq!(
        progress(&program, 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(admitted(&program).len(), 1);
    // The optional range is transported, not required; a one-sided range is a
    // mixed component whose literal endpoints conserve trivially.
    let unranged = FIELD.replace(" in 0..=5;", ";");
    assert_ne!(unranged, FIELD);
    assert_eq!(admitted(&typed_source(&unranged)).len(), 1);
    let mixed = FIELD.replacen(" in 0..=5;", ";", 1);
    assert_ne!(mixed, FIELD);
    assert_eq!(admitted(&typed_source(&mixed)).len(), 1);
}

#[test]
fn field_view_call_from_a_subordinate_state_reads_the_carried_record() {
    // `hold` carries `pending`'s record: the call's strict edge resolves the
    // rebuilt literal against the site formal's entry role, and the entry
    // invariant the member re-proves on arrival supplies its range facts.
    let subordinate = FIELD.replace(
        "machine Main::scan_b(&mut self, pending: Countdown)
terminates by pending -> Countdown::Remaining in 0..=5;
-> u64 {
    transition pending.remaining > 0 {
        true -> self.scan_a(Countdown { remaining: pending.remaining - 1 })
        false -> pending.remaining
    }
}",
        "machine Main::scan_b(&mut self, pending: Countdown)
terminates by pending -> Countdown::Remaining in 0..=5;
-> u64 {
    transition pending.remaining > 0 {
        true -> hold(pending)
        false -> pending.remaining
    }
    state hold(carried: Countdown) {
        transition carried.remaining > 0 {
            true -> self.scan_a(Countdown { remaining: carried.remaining - 1 })
            false -> carried.remaining
        }
    }
}",
    );
    assert_ne!(subordinate, FIELD);
    assert_eq!(admitted(&typed_source(&subordinate)).len(), 1);
    // The decrease lives on `scan_b`'s internal `hold` arrival, which the
    // member's own state-edge judgment owns: `carried` carries `pending`'s
    // role, so the forwarded call edge preserves the site's coordinate and
    // the all-weak call cycle cannot terminate.
    let unmapped = FIELD.replace(
        "machine Main::scan_b(&mut self, pending: Countdown)
terminates by pending -> Countdown::Remaining in 0..=5;
-> u64 {
    transition pending.remaining > 0 {
        true -> self.scan_a(Countdown { remaining: pending.remaining - 1 })
        false -> pending.remaining
    }
}",
        "machine Main::scan_b(&mut self, pending: Countdown)
terminates by pending -> Countdown::Remaining in 0..=5;
-> u64 {
    transition pending.remaining > 0 {
        true -> hold(Countdown { remaining: pending.remaining - 1 })
        false -> pending.remaining
    }
    state hold(carried: Countdown) {
        transition carried.remaining > 0 {
            true -> self.scan_a(carried)
            false -> carried.remaining
        }
    }
}",
    );
    assert_ne!(unmapped, FIELD);
    assert!(admitted(&typed_source(&unmapped)).is_empty());
}

#[test]
fn field_view_cycle_needs_one_strict_edge() {
    for body in [
        FIELD.replace(
            "Countdown { remaining: pending.remaining - 1 }",
            "Countdown { remaining: pending.remaining }",
        ),
        FIELD.replace(
            "Countdown { remaining: pending.remaining - 1 }",
            "Countdown { remaining: 5 }",
        ),
    ] {
        assert!(admitted(&typed_source(&body)).is_empty());
    }
}

#[test]
fn field_view_members_share_the_order_only_through_the_same_measure() {
    // Two declared measures with identical bodies are two orders.
    let second = FIELD
        .replace(
            "measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.remaining }",
            "measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.remaining }\nmeasure Countdown::Pending(pending: Countdown) -> u64 { pending.remaining }",
        )
        .replace(
            "terminates by pending -> Countdown::Remaining in 0..=5;",
            "terminates by pending -> Countdown::Pending in 0..=5;",
        );
    assert_ne!(second, FIELD);
    assert!(admitted(&typed_source(&second)).is_empty());
    // A builtin natural view on the same field value produces equal ranks but
    // is not the authored order.
    let mixed = FIELD
        .replace("self.scan_b(countdown)", "self.scan_b(countdown.remaining)")
        .replace(
            "machine Main::scan_b(&mut self, pending: Countdown)\nterminates by pending -> Countdown::Remaining in 0..=5;\n-> u64 {\n    transition pending.remaining > 0 {\n        true -> self.scan_a(Countdown { remaining: pending.remaining - 1 })\n        false -> pending.remaining\n    }\n}",
            "machine Main::scan_b(&mut self, pending: u64 [0..=5])\nterminates by pending in 0..=5;\n-> u64 {\n    transition pending > 0 {\n        true -> self.scan_a(Countdown { remaining: pending - 1 })\n        false -> pending\n    }\n}",
        );
    assert_ne!(mixed, FIELD);
    assert!(admitted(&typed_source(&mixed)).is_empty());
}

#[test]
fn field_view_call_rejects_a_prefix_store_into_the_ranked_record() {
    // The ranked field is a premise carrier: writing through `pending` before
    // the transition invalidates the arrival the edge judgment reads.
    let written = FIELD
        .replace(
            "machine Main::scan_b(&mut self, pending: Countdown)",
            "machine Main::scan_b(&mut self, mut pending: Countdown)",
        )
        .replace(
            "    transition pending.remaining > 0 {\n        true -> self.scan_a",
            "    pending.remaining = 5;\n    transition pending.remaining > 0 {\n        true -> self.scan_a",
        );
    assert_ne!(written, FIELD);
    assert!(admitted(&typed_source(&written)).is_empty());
    // A whole-record store into the premise carrier rejects the same way.
    let rebound = FIELD
        .replace(
            "machine Main::scan_b(&mut self, pending: Countdown)",
            "machine Main::scan_b(&mut self, mut pending: Countdown)",
        )
        .replace(
            "    transition pending.remaining > 0 {\n        true -> self.scan_a",
            "    pending = pending;\n    transition pending.remaining > 0 {\n        true -> self.scan_a",
        );
    assert_ne!(rebound, FIELD);
    assert!(admitted(&typed_source(&rebound)).is_empty());
}

#[test]
fn field_view_call_rejects_an_unrelated_actual_carrier() {
    // A record of another declaration with the same field spelling is not
    // this coordinate's carrier.
    let foreign = FIELD.replace(
        "self.scan_b(countdown)",
        "self.scan_b(Foreign { remaining: countdown.remaining })",
    );
    let foreign = format!(
        "data Foreign {{ remaining: u64 [0..=5]; }}\n{}",
        foreign.replacen(
            "machine Main::scan_b(&mut self, pending: Countdown)",
            "machine Main::scan_b(&mut self, pending: Foreign)",
            1
        )
    );
    assert!(admitted(&typed_source(&foreign)).is_empty());
}

const NESTED: &str = "data Inner { remaining: u64 [0..=5]; }
data Countdown { tag: u64; inner: Inner; }
measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.inner.remaining }
data Main {}
machine Main::scan_a(&mut self, countdown: Countdown)
terminates by countdown -> Countdown::Remaining in 0..=5;
-> u64 {
    transition countdown.inner.remaining > 0 {
        true -> self.scan_b(countdown)
        false -> countdown.inner.remaining
    }
}
machine Main::scan_b(&mut self, pending: Countdown)
terminates by pending -> Countdown::Remaining in 0..=5;
-> u64 {
    transition pending.inner.remaining > 0 {
        true -> self.scan_a(Countdown {
            tag: pending.tag,
            inner: Inner { remaining: pending.inner.remaining - 1 }
        })
        false -> pending.inner.remaining
    }
}";

#[test]
fn nested_field_view_walks_the_declared_chain_through_a_literal() {
    let program = typed_source(NESTED);
    assert_eq!(admitted(&program).len(), 1);
    // The strict edge cannot land on the outer field: the chain resolves the
    // exact declared leaf, so stalling `inner.remaining` rejects.
    let stalled = NESTED.replace(
        "Inner { remaining: pending.inner.remaining - 1 }",
        "Inner { remaining: pending.inner.remaining }",
    );
    assert!(admitted(&typed_source(&stalled)).is_empty());
    // A same-named leaf under a different owner is a different coordinate.
    let wrong = NESTED.replace("pending.inner.remaining - 1", "pending.tag");
    assert!(admitted(&typed_source(&wrong)).is_empty());
}

#[test]
fn borrowed_record_subject_keeps_the_same_coordinate() {
    let program = typed_source(
        "data Card { power: u64 [0..=5]; }
        measure Card::PowerOrder(card: Card) -> u64 { card.power }
        data Main {}
        machine Main::scan_a(&mut self, card: &Card)
        terminates by card -> Card::PowerOrder in 0..=5;
        -> u64 {
            transition card.power > 0 { true -> self.scan_b(card) false -> card.power }
        }
        machine Main::scan_b(&mut self, pending: &Card)
        terminates by pending -> Card::PowerOrder in 0..=5;
        -> u64 {
            transition pending.power > 0 {
                true -> self.scan_a(&Card { power: pending.power - 1 })
                false -> pending.power
            }
        }",
    );
    for machine in program.machines() {
        let rank = RankProjection::resolve(&program, machine).expect("borrowed field view");
        assert!(matches!(rank.order, RankOrder::CustomStructView { .. }));
    }
    assert_eq!(admitted(&program).len(), 1);
}

const PINNED: &str = "data Countdown { remaining: u64 [0..=5]; limit: u64 [0..=5]; }
measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.remaining }
data Main {}
machine Main::scan_a(&mut self, countdown: Countdown)
requires countdown.remaining <= countdown.limit;
terminates by countdown -> Countdown::Remaining in 0..=countdown.limit;
-> u64 {
    transition countdown.remaining > 0 {
        true -> self.scan_b(countdown)
        false -> countdown.remaining
    }
}
machine Main::scan_b(&mut self, pending: Countdown)
requires pending.remaining <= pending.limit;
terminates by pending -> Countdown::Remaining in 0..=pending.limit;
-> u64 {
    transition pending.remaining > 0 {
        true -> self.scan_a(Countdown {
            remaining: pending.remaining - 1,
            limit: pending.limit
        })
        false -> pending.remaining
    }
}";

#[test]
fn field_endpoints_stay_pinned_through_the_actual() {
    let program = typed_source(PINNED);
    assert_eq!(admitted(&program).len(), 1);
    // Rebuilding the literal with a different limit moves the endpoint the
    // callee's range reads: `pending.limit` no longer names the arrival's
    // ceiling.
    let moved = PINNED.replace("limit: pending.limit", "limit: 5");
    assert_ne!(moved, PINNED);
    assert!(admitted(&typed_source(&moved)).is_empty());
    // A store into the endpoint's carrier invalidates the premise before the
    // call.
    let written = PINNED
        .replace(
            "machine Main::scan_b(&mut self, pending: Countdown)",
            "machine Main::scan_b(&mut self, mut pending: Countdown)",
        )
        .replace(
            "    transition pending.remaining > 0 {\n        true -> self.scan_a",
            "    pending.limit = 5;\n    transition pending.remaining > 0 {\n        true -> self.scan_a",
        );
    assert_ne!(written, PINNED);
    assert!(admitted(&typed_source(&written)).is_empty());
}
