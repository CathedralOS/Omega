use super::super::RankingRangeCallProgress;
use super::ranges::progress;
use super::{admitted, typed_source};

const DISTANCE: &str = "data Main {}
machine Main::scan_a(&mut self, index: u64 [0..=4], limit: u64 [0..=4])
requires index <= limit;
terminates by (index, limit) -> Nat::BoundedDistance in 0..=4;
-> u64 {
    transition index < limit { true -> self.scan_b(index + 1, limit) false -> index }
}
machine Main::scan_b(&mut self, index: u64 [0..=4], limit: u64 [0..=4])
requires index <= limit;
terminates by (index, limit) -> Nat::BoundedDistance in 0..=4;
-> u64 {
    transition index < limit { true -> self.scan_a(index + 1, limit) false -> index }
}";

/// Both members author the upper subject as their symbolic rank ceiling. A
/// moved actual is a changed endpoint, not a diverging copy the ceiling
/// equality can absorb.
const SYMBOLIC: &str = "data Main {}
machine Main::scan_a(&mut self, index: u64 [0..=4], limit: u64 [0..=4])
requires index <= limit;
terminates by (index, limit) -> Nat::BoundedDistance in 0..=limit;
-> u64 {
    transition index < limit { true -> self.scan_b(limit, index + 1) false -> index }
}
machine Main::scan_b(&mut self, bound: u64 [0..=4], cursor: u64 [0..=4])
requires cursor <= bound;
terminates by (cursor, bound) -> Nat::BoundedDistance in 0..=bound;
-> u64 {
    transition cursor < bound { true -> self.scan_a(cursor + 1, bound) false -> cursor }
}";

#[test]
fn a_changed_endpoint_actual_invalidates_the_premise() {
    let program = typed_source(SYMBOLIC);
    assert_eq!(admitted(&program).len(), 1);
    for changed in [
        // Moving the upper endpoint changes the destination's authored
        // ceiling even though the transported distance is preserved.
        SYMBOLIC.replace(
            "self.scan_b(limit, index + 1)",
            "self.scan_b(limit + 1, index + 1)",
        ),
        SYMBOLIC.replace(
            "self.scan_a(cursor + 1, bound)",
            "self.scan_a(cursor + 1, bound - 1)",
        ),
        // A larger step can underflow the destination's natural rank.
        DISTANCE.replace(
            "self.scan_a(index + 1, limit)",
            "self.scan_a(index + 2, limit)",
        ),
    ] {
        assert!(admitted(&typed_source(&changed)).is_empty(), "{changed}");
    }
}

#[test]
fn distance_preserving_edges_cannot_close_a_cycle() {
    // A single preserving edge beside a strict edge is a valid weak link.
    let weak = DISTANCE.replace("self.scan_b(index + 1, limit)", "self.scan_b(index, limit)");
    assert_eq!(
        progress(&typed_source(&weak), 0),
        Some(RankingRangeCallProgress::NonIncreasing)
    );
    assert_eq!(admitted(&typed_source(&weak)).len(), 1);
    // Both edges only preserve the distance, so the weak-edge graph cycles.
    let preserving = DISTANCE.replace("index + 1", "index");
    assert!(admitted(&typed_source(&preserving)).is_empty());
}

#[test]
fn an_intervening_write_to_a_rank_input_invalidates_the_premise() {
    let changed = DISTANCE
        .replace(
            "machine Main::scan_a(&mut self, index: u64 [0..=4], limit: u64 [0..=4])",
            "machine Main::scan_a(&mut self, mut index: u64 [0..=4], mut limit: u64 [0..=4])",
        )
        .replace(
            "    transition index < limit { true -> self.scan_b(index + 1, limit) false -> index }\n}\nmachine Main::scan_b",
            "    index = index; transition index < limit { true -> self.scan_b(index + 1, limit) false -> index }\n}\nmachine Main::scan_b",
        );
    assert!(admitted(&typed_source(&changed)).is_empty());
    let changed = DISTANCE
        .replace(
            "machine Main::scan_a(&mut self, index: u64 [0..=4], limit: u64 [0..=4])",
            "machine Main::scan_a(&mut self, mut index: u64 [0..=4], mut limit: u64 [0..=4])",
        )
        .replace(
            "    transition index < limit { true -> self.scan_b(index + 1, limit) false -> index }\n}\nmachine Main::scan_b",
            "    limit = limit; transition index < limit { true -> self.scan_b(index + 1, limit) false -> index }\n}\nmachine Main::scan_b",
        );
    assert!(admitted(&typed_source(&changed)).is_empty());
}

#[test]
fn distance_subjects_bind_to_the_callee_exact_formals_not_positions() {
    // The callee declares its ranked subjects under different formal names and
    // in the opposite position order. The exact name bindings still map the
    // caller's actuals onto the (lower, upper) roles.
    let permuted = "data Main {}
machine Main::scan_a(&mut self, index: u64 [0..=4], limit: u64 [0..=4])
requires index <= limit;
terminates by (index, limit) -> Nat::BoundedDistance in 0..=4;
-> u64 {
    transition index < limit { true -> self.scan_b(limit, index + 1) false -> index }
}
machine Main::scan_b(&mut self, bound: u64 [0..=4], cursor: u64 [0..=4])
requires cursor <= bound;
terminates by (cursor, bound) -> Nat::BoundedDistance in 0..=4;
-> u64 {
    transition cursor < bound { true -> self.scan_a(cursor + 1, bound) false -> cursor }
}";
    assert_eq!(admitted(&typed_source(permuted)).len(), 1);
}

#[test]
fn a_member_ranked_by_another_view_cannot_join_the_distance_order() {
    let mismatched = "data Main {}
machine Main::scan_a(&mut self, index: u64 [0..=4], limit: u64 [0..=4])
requires index <= limit;
terminates by (index, limit) -> Nat::BoundedDistance in 0..=4;
-> u64 {
    transition index < limit { true -> self.scan_b(index + 1, limit) false -> index }
}
machine Main::scan_b(&mut self, index: u64 [0..=4], limit: u64 [0..=4])
requires index <= limit;
terminates by index -> Nat::IncreasingTo(limit) in 0..=4;
-> u64 {
    transition index < limit { true -> self.scan_a(index + 1, limit) false -> index }
}";
    assert!(admitted(&typed_source(mismatched)).is_empty());
}
