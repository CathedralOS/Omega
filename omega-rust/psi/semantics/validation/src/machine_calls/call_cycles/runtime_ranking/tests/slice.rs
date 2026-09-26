use super::super::RankingRangeCallProgress;
use super::super::projection::RankOrder;
use super::ranges::progress;
use super::{RankProjection, admitted, typed_source};

const SLICES: &str = "data Main {}
machine Main::scan_a(&mut self, items: &[u64], capacity: u64)
requires items.len <= capacity;
terminates by items -> Slice::Length in 0..=capacity;
-> u64 {
    transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }
}
machine Main::scan_b(&mut self, items: &[u64], capacity: u64)
requires items.len <= capacity;
terminates by items -> Slice::Length in 0..=capacity;
-> u64 {
    transition items.len > 0 { true -> self.scan_a(items[1..], capacity) false -> 0 }
}";

#[test]
fn slice_length_component_admits_exact_tail_arrival() {
    let program = typed_source(SLICES);
    for machine in program.machines() {
        assert!(
            RankProjection::resolve(&program, machine).is_some(),
            "{:#?}",
            machine.termination_plan
        );
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
fn unranged_slice_component_still_requires_length_formation() {
    let source = SLICES.replace(" in 0..=capacity", "");
    let program = typed_source(&source);
    assert_eq!(
        progress(&program, 0),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(admitted(&program).len(), 1);
    // Without the site guard the subslice's `len - 1` endpoint cannot prove
    // natural-rank formation at the destination.
    let unguarded = source.replace(
        "transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }",
        "transition { _ -> self.scan_b(items[1..], capacity) }",
    );
    assert!(admitted(&typed_source(&unguarded)).is_empty());
}

#[test]
fn slice_length_edges_classify_preserving_calls_and_reject_weak_cycles() {
    // Forwarding the same collection preserves its length: a valid weak link
    // beside the strict subslice edge.
    let weak = SLICES.replace(
        "self.scan_b(items[1..], capacity)",
        "self.scan_b(items, capacity)",
    );
    assert_eq!(
        progress(&typed_source(&weak), 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert_eq!(admitted(&typed_source(&weak)).len(), 1);
    // A rewindowed slice that loses no element also only preserves; with both
    // edges weak the component has no complete strict cycle.
    let preserving = SLICES.replace("items[1..]", "items[0..]");
    assert_eq!(
        progress(&typed_source(&preserving), 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert!(admitted(&typed_source(&preserving)).is_empty());
}

#[test]
fn a_slice_actual_without_ranked_ancestry_cannot_carry_the_component() {
    let changed = SLICES
        .replace(
            "machine Main::scan_a(&mut self, items: &[u64], capacity: u64)",
            "machine Main::scan_a(&mut self, items: &[u64], spare: &[u64], capacity: u64)",
        )
        .replace(
            "self.scan_b(items[1..], capacity)",
            "self.scan_b(spare[1..], spare, capacity)",
        )
        .replace(
            "machine Main::scan_b(&mut self, items: &[u64], capacity: u64)",
            "machine Main::scan_b(&mut self, items: &[u64], spare: &[u64], capacity: u64)",
        )
        .replace(
            "self.scan_a(items[1..], capacity)",
            "self.scan_a(items[1..], spare, capacity)",
        );
    assert!(admitted(&typed_source(&changed)).is_empty());
}

#[test]
fn a_changed_slice_endpoint_actual_invalidates_the_premise() {
    for changed in [
        // Moving the authored ceiling is a changed endpoint, not a diverging
        // copy the range equality can absorb.
        SLICES.replace(
            "self.scan_b(items[1..], capacity)",
            "self.scan_b(items[1..], capacity + 1)",
        ),
        SLICES.replace(
            "self.scan_a(items[1..], capacity)",
            "self.scan_a(items[1..], capacity - 1)",
        ),
        // A deeper cut can underflow the destination's authored floor.
        SLICES.replace("items[1..]", "items[2..]"),
    ] {
        assert!(admitted(&typed_source(&changed)).is_empty(), "{changed}");
    }
}

#[test]
fn an_intervening_write_to_the_slice_invalidates_the_premise() {
    // Mutability alone is a storage capability, not a value change: with no
    // prefix write the mutable slice still denotes its arrival sequence.
    let mutable_only = SLICES.replace(
        "machine Main::scan_a(&mut self, items: &[u64], capacity: u64)",
        "machine Main::scan_a(&mut self, mut items: &[u64], capacity: u64)",
    );
    assert_eq!(admitted(&typed_source(&mutable_only)).len(), 1);
    let changed = SLICES
        .replace(
            "machine Main::scan_a(&mut self, items: &[u64], capacity: u64)",
            "machine Main::scan_a(&mut self, mut items: &[u64], capacity: u64)",
        )
        .replace(
            "    transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }\n}\nmachine Main::scan_b",
            "    items = items; transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }\n}\nmachine Main::scan_b",
        );
    assert!(admitted(&typed_source(&changed)).is_empty());
    // Rebinding the capacity endpoint between the guard and the call is an
    // intervening write even when the stored expression keeps its spelling.
    let changed = SLICES
        .replace(
            "machine Main::scan_a(&mut self, items: &[u64], capacity: u64)",
            "machine Main::scan_a(&mut self, items: &[u64], mut capacity: u64)",
        )
        .replace(
            "    transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }\n}\nmachine Main::scan_b",
            "    capacity = capacity; transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }\n}\nmachine Main::scan_b",
        );
    assert!(admitted(&typed_source(&changed)).is_empty());
}

#[test]
fn a_disjoint_write_preserves_the_slice_premise() {
    let source = SLICES
        .replace("data Main {}", "data Main { visited: u64; }")
        .replace(
            "    transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }\n}\nmachine Main::scan_b",
            "    self.visited = capacity; transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }\n}\nmachine Main::scan_b",
        );
    assert_eq!(admitted(&typed_source(&source)).len(), 1);
}

#[test]
fn slice_subjects_bind_to_the_callee_exact_formals_not_positions() {
    // The callee declares its slice formal under a different name and after
    // the scalar endpoint. Exact name bindings still map the caller's actuals
    // onto the (collection, bound) roles.
    let permuted = "data Main {}
machine Main::scan_a(&mut self, items: &[u64], capacity: u64)
requires items.len <= capacity;
terminates by items -> Slice::Length in 0..=capacity;
-> u64 {
    transition items.len > 0 { true -> self.scan_b(capacity, items[1..]) false -> 0 }
}
machine Main::scan_b(&mut self, bound: u64, entries: &[u64])
requires entries.len <= bound;
terminates by entries -> Slice::Length in 0..=bound;
-> u64 {
    transition entries.len > 0 { true -> self.scan_a(entries[1..], bound) false -> 0 }
}";
    assert_eq!(admitted(&typed_source(permuted)).len(), 1);
}

#[test]
fn mixed_slice_component_conserves_a_symbolic_endpoint() {
    let mixed = "data Main {}
machine Main::scan_a(&mut self, items: &[u64], capacity: u64)
requires items.len <= capacity;
terminates by items -> Slice::Length in 0..=capacity;
-> u64 {
    transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }
}
machine Main::scan_b(&mut self, items: &[u64], capacity: u64)
requires items.len <= capacity;
terminates by items -> Slice::Length;
-> u64 {
    transition items.len > 0 { true -> self.scan_a(items[1..], capacity) false -> 0 }
}";
    assert_eq!(admitted(&typed_source(mixed)).len(), 1);
    // A diverging copy of the conserved endpoint cannot carry the authored
    // range: `capacity + 1` equals no caller input, so the unranged member
    // cannot pin the ceiling.
    let diverged = mixed.replace(
        "self.scan_b(items[1..], capacity)",
        "self.scan_b(items[1..], capacity + 1)",
    );
    assert!(admitted(&typed_source(&diverged)).is_empty());
    let dropped = mixed.replace(
        "self.scan_b(items[1..], capacity)",
        "self.scan_b(items[1..], items.len)",
    );
    assert!(admitted(&typed_source(&dropped)).is_empty());
}

#[test]
fn slice_component_reads_duplicated_collection_copies() {
    // `pair(first, second)` holds two copies of the same collection: every
    // arrival forwarded a bare name, so the produced length coordinate is one
    // shared value and either copy may carry it into the next call.
    let source = "data Main {}
        machine Main::scan(&mut self, items: &[u64], capacity: u64)
        requires items.len <= capacity;
        terminates by items -> Slice::Length in 0..=capacity;
        -> u64 {
            transition items.len > 0 { true -> pair(items, items, capacity) false -> 0 }
            state pair(first: &[u64], second: &[u64], bound: u64) {
                transition first.len > 0 && first.len <= bound {
                    true -> self.step(second, bound)
                    false -> 0
                }
            }
        }
        machine Main::step(&mut self, rest: &[u64], capacity: u64)
        requires rest.len <= capacity;
        terminates by rest -> Slice::Length in 0..=capacity;
        -> u64 {
            transition rest.len > 0 { true -> self.scan(rest[1..], capacity) false -> 0 }
        }";
    assert_eq!(admitted(&typed_source(source)).len(), 1);
    // A windowed second copy is the moved copy of the collection: `second`
    // names the continuation the rank reads -- `rest.len` is `items.len - 1`
    // at the site -- while the stale `first` snapshot demotes.
    let diverged = source.replace(
        "pair(items, items, capacity)",
        "pair(items, items[1..], capacity)",
    );
    assert_eq!(admitted(&typed_source(&diverged)).len(), 1);
    // Transporting the demoted copy instead leaves the callee's slice
    // unranked: `first` forwards the pre-step value and carries no role.
    let stale = diverged.replace("self.step(second, bound)", "self.step(first, bound)");
    assert!(admitted(&typed_source(&stale)).is_empty());
    // When both copies arrive through the same window, no unique moved copy
    // exists and the rank has no carrier at all.
    let ambiguous = source.replace(
        "pair(items, items, capacity)",
        "pair(items[1..], items[1..], capacity)",
    );
    assert!(admitted(&typed_source(&ambiguous)).is_empty());
}

#[test]
fn a_ranged_slice_member_calls_from_a_subordinate_state_under_its_own_invariant() {
    // `hold -> step(pending, bound)` forwards the carried collection and the
    // authored ceiling. The site consumes `scan`'s own range invariant
    // `pending.len <= bound` through the telescoped length atom, so the guard
    // need not respell it; the unranged member transports the endpoint back.
    let source = "data Main {}
        machine Main::scan(&mut self, items: &[u64], capacity: u64)
        requires items.len <= capacity;
        terminates by items -> Slice::Length in 0..=capacity;
        -> u64 {
            transition items.len > 0 { true -> hold(items, capacity) false -> 0 }
            state hold(pending: &[u64], bound: u64) {
                transition pending.len > 0 {
                    true -> self.step(pending, bound)
                    false -> 0
                }
            }
        }
        machine Main::step(&mut self, rest: &[u64], capacity: u64)
        terminates by rest -> Slice::Length;
        -> u64 {
            transition rest.len > 0 && rest.len <= capacity {
                true -> self.scan(rest[1..], capacity)
                false -> 0
            }
        }";
    assert_eq!(admitted(&typed_source(source)).len(), 1);
    // A changed endpoint and a prefix store into a carrier keep rejecting.
    let moved = source.replace("self.step(pending, bound)", "self.step(pending, bound + 1)");
    assert!(admitted(&typed_source(&moved)).is_empty());
    let dropped = source.replace(
        "self.step(pending, bound)",
        "self.step(pending, pending.len)",
    );
    assert!(admitted(&typed_source(&dropped)).is_empty());
    let written = source
        .replace(
            "pending: &[u64], bound: u64)",
            "pending: &[u64], mut bound: u64)",
        )
        .replace(
            "                transition pending.len > 0 {",
            "                bound = bound; transition pending.len > 0 {",
        );
    assert_ne!(written, source);
    assert!(admitted(&typed_source(&written)).is_empty());
    let missing_entry = source.replace("requires items.len <= capacity;", "");
    assert!(admitted(&typed_source(&missing_entry)).is_empty());
}

const PROJECTED: &str = "data Bag { items: &[u64]; }
data Main {}
machine Main::scan_a(&mut self, bag: Bag, capacity: u64)
requires bag.items.len <= capacity;
terminates by bag.items -> Slice::Length in 0..=capacity;
-> u64 {
    transition bag.items.len > 0 {
        true -> self.scan_b(Bag { items: bag.items[1..] }, capacity)
        false -> 0
    }
}
machine Main::scan_b(&mut self, bag: Bag, capacity: u64)
requires bag.items.len <= capacity;
terminates by bag.items -> Slice::Length in 0..=capacity;
-> u64 {
    transition bag.items.len > 0 {
        true -> self.scan_a(Bag { items: bag.items[1..] }, capacity)
        false -> 0
    }
}";

#[test]
fn projected_slice_member_component_admits_exact_tail_arrival() {
    let program = typed_source(PROJECTED);
    for machine in program.machines() {
        let rank = RankProjection::resolve(&program, machine).expect("slice projection");
        assert!(matches!(rank.order, RankOrder::SliceLength));
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

/// A scalar-ranked member whose authored endpoint spells `slice.len`: the
/// produced length is a non-polynomial input, pinned and conserved under the
/// same exact discipline as an integer formal, never normalized through the
/// collection's own value.
const LEN_ENDPOINT: &str = "data Main {}
machine Main::outer(&mut self, remaining: u64, items: &[u64])
requires items.len >= remaining;
terminates by remaining in 0..=items.len;
-> u64 {
    transition remaining > 0 && items.len > 0 {
        true -> self.inner(remaining - 1, items)
        false -> remaining
    }
}
machine Main::inner(&mut self, n: u64, bag: &[u64])
requires bag.len >= n;
terminates by n in 0..=bag.len;
-> u64 {
    transition n > 0 { true -> self.outer(n, bag) false -> n }
}";

#[test]
fn scalar_members_pin_a_produced_len_endpoint() {
    let program = typed_source(LEN_ENDPOINT);
    assert_eq!(
        progress(&program, 0),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert_eq!(admitted(&program).len(), 1);
    // A subslice actual shortens the produced length: `items[1..].len` is
    // `items.len - 1`, a changed endpoint, not a transported pin.
    for changed in [
        LEN_ENDPOINT.replace(
            "self.inner(remaining - 1, items)",
            "self.inner(remaining - 1, items[1..])",
        ),
        LEN_ENDPOINT.replace("self.outer(n, bag)", "self.outer(n, bag[1..])"),
    ] {
        assert!(admitted(&typed_source(&changed)).is_empty(), "{changed}");
    }
    // A foreign slice actual cannot pin the endpoint even though it shares
    // the authored type; only the conserved carrier may arrive.
    let foreign = LEN_ENDPOINT
        .replace(
            "machine Main::outer(&mut self, remaining: u64, items: &[u64])",
            "machine Main::outer(&mut self, remaining: u64, items: &[u64], spare: &[u64])",
        )
        .replace(
            "machine Main::inner(&mut self, n: u64, bag: &[u64])",
            "machine Main::inner(&mut self, n: u64, bag: &[u64], spare: &[u64])",
        )
        .replace(
            "self.inner(remaining - 1, items)",
            "self.inner(remaining - 1, spare, items)",
        )
        .replace("self.outer(n, bag)", "self.outer(n, bag, spare)");
    assert!(admitted(&typed_source(&foreign)).is_empty());
    // An intervening write to the endpoint-bearing formal invalidates the
    // premise before the call.
    let written = LEN_ENDPOINT
        .replace(
            "machine Main::outer(&mut self, remaining: u64, items: &[u64])",
            "machine Main::outer(&mut self, remaining: u64, mut items: &[u64])",
        )
        .replace(
            "    transition remaining > 0 && items.len > 0 {",
            "    items = items[1..]; transition remaining > 0 && items.len > 0 {",
        );
    assert_ne!(written, LEN_ENDPOINT);
    assert!(admitted(&typed_source(&written)).is_empty());
}

#[test]
fn an_unranged_member_conserves_a_produced_len_endpoint() {
    let mixed = "data Main {}
machine Main::outer(&mut self, remaining: u64, items: &[u64])
requires items.len >= remaining;
terminates by remaining in 0..=items.len;
-> u64 {
    transition remaining > 0 { true -> self.inner(remaining - 1, items) false -> remaining }
}
machine Main::inner(&mut self, n: u64, bag: &[u64])
requires bag.len >= n;
terminates by n;
-> u64 {
    transition n > 0 { true -> self.outer(n, bag) false -> n }
}";
    assert_eq!(admitted(&typed_source(mixed)).len(), 1);
    // The unranged member rewindowing the collection is a changed endpoint:
    // `bag[1..].len` equals no caller-carried `items.len`.
    let diverged = mixed.replace("self.outer(n, bag)", "self.outer(n, bag[1..])");
    assert!(admitted(&typed_source(&diverged)).is_empty());
}

/// A member-chain `.len` endpoint (`hold.bag.len`) resolves the projected
/// slice coordinate of the record formal's unique leaf and transports that
/// coordinate through the component exactly like a field projection does.
const PROJECTED_LEN_ENDPOINT: &str = "data Holder { bag: &[u64]; }
data Main {}
machine Main::outer(&mut self, remaining: u64 [0..=4], cap: u64 [4..=8], hold: Holder)
requires hold.bag.len >= cap;
terminates by remaining in 0..=hold.bag.len;
-> u64 {
    transition remaining > 0 { true -> self.inner(remaining, cap, hold) false -> remaining }
}
machine Main::inner(&mut self, n: u64 [0..=4], cap: u64 [4..=8], wrap: Holder)
requires wrap.bag.len >= cap;
terminates by n in 0..=wrap.bag.len;
-> u64 {
    transition n > 0 { true -> self.outer(n - 1, cap, wrap) false -> n }
}";

#[test]
fn a_member_chain_len_endpoint_transports_through_record_carriers() {
    assert_eq!(admitted(&typed_source(PROJECTED_LEN_ENDPOINT)).len(), 1);
    // Rebuilding the carrier with a rewindowed leaf transports a different
    // produced length, so the destination's pinned endpoint is unproven.
    let rewindowed = PROJECTED_LEN_ENDPOINT.replace(
        "self.outer(n - 1, cap, wrap)",
        "self.outer(n - 1, cap, Holder { bag: wrap.bag[1..] })",
    );
    assert!(admitted(&typed_source(&rewindowed)).is_empty());
    // A foreign record of the same declaration carries no equality to the
    // endpoint's coordinate: `rest.bag.len` is not `wrap.bag.len`.
    let foreign = PROJECTED_LEN_ENDPOINT
        .replace(
            "machine Main::outer(&mut self, remaining: u64 [0..=4], cap: u64 [4..=8], hold: Holder)",
            "machine Main::outer(&mut self, remaining: u64 [0..=4], cap: u64 [4..=8], hold: Holder, spare: Holder)",
        )
        .replace(
            "machine Main::inner(&mut self, n: u64 [0..=4], cap: u64 [4..=8], wrap: Holder)",
            "machine Main::inner(&mut self, n: u64 [0..=4], cap: u64 [4..=8], wrap: Holder, rest: Holder)",
        )
        .replace(
            "self.inner(remaining, cap, hold)",
            "self.inner(remaining, cap, hold, spare)",
        )
        .replace(
            "self.outer(n - 1, cap, wrap)",
            "self.outer(n - 1, cap, rest, wrap)",
        );
    assert!(admitted(&typed_source(&foreign)).is_empty());
    // Two admissible carriage readings keep no coordinate: the rebuilt
    // `Outer` duplicates the leaf into `left` and `right`, so `wrap.bag.len`
    // has no unique transported value at the destination.
    let ambiguous = PROJECTED_LEN_ENDPOINT
        .replace(
            "data Holder { bag: &[u64]; }",
            "data Holder { bag: &[u64]; }\ndata Outer { left: Holder; right: Holder; }",
        )
        .replace(
            "machine Main::outer(&mut self, remaining: u64 [0..=4], cap: u64 [4..=8], hold: Holder)",
            "machine Main::outer(&mut self, remaining: u64 [0..=4], cap: u64 [4..=8], nest: Outer)",
        )
        .replace(
            "requires hold.bag.len >= cap;\nterminates by remaining in 0..=hold.bag.len;",
            "requires nest.left.bag.len >= cap;\nterminates by remaining in 0..=cap;",
        )
        .replace(
            "self.inner(remaining, cap, hold)",
            "self.inner(remaining, cap, nest.left)",
        )
        .replace(
            "self.outer(n - 1, cap, wrap)",
            "self.outer(n - 1, cap, Outer { left: wrap, right: wrap })",
        );
    assert!(admitted(&typed_source(&ambiguous)).is_empty());
}

#[test]
fn a_member_ranked_by_another_view_cannot_join_the_slice_order() {
    let mismatched = "data Main {}
machine Main::scan_a(&mut self, items: &[u64], capacity: u64)
requires items.len <= capacity;
terminates by items -> Slice::Length in 0..=capacity;
-> u64 {
    transition items.len > 0 { true -> self.scan_b(items[1..], capacity) false -> 0 }
}
machine Main::scan_b(&mut self, items: &[u64], remaining: u64)
requires remaining <= 4;
terminates by remaining in 0..=4;
-> u64 {
    transition remaining > 0 { true -> self.scan_a(items, remaining - 1) false -> 0 }
}";
    assert!(admitted(&typed_source(mismatched)).is_empty());
}
