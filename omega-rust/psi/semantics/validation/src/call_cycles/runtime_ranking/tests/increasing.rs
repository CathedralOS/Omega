use super::ranges::progress;
use super::*;

const INCREASING: &str = "data Main {}
machine Main::first(&mut self, limit: u64, cursor: u64, capacity: u64)
requires cursor <= limit && limit <= capacity;
terminates by cursor -> Nat::IncreasingTo(limit) in 0..=capacity;
-> u64 {
    transition cursor < limit { true -> self.second(capacity, cursor, limit) false -> cursor }
}
machine Main::second(&mut self, ceiling: u64, position: u64, bound: u64)
requires position <= bound && bound <= ceiling;
terminates by position -> Nat::IncreasingTo(bound) in 0..=ceiling;
-> u64 {
    transition position < bound { true -> self.first(bound, position + 1, ceiling) false -> position }
}";

#[test]
fn increasing_call_compares_produced_distance_in_each_exact_telescope() {
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
}

#[test]
fn a_constant_rank_range_cannot_hide_a_moving_view_bound() {
    let source = INCREASING
        .replace("0..=capacity", "0..=100")
        .replace("0..=ceiling", "0..=100")
        .replace("limit <= capacity", "limit <= 100")
        .replace("bound <= ceiling", "bound <= 100");
    let program = typed_source(&source);
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
    let moved = source.replace(
        "self.first(bound, position + 1, ceiling)",
        "self.first(bound - 1, position, ceiling)",
    );
    let program = typed_source(&moved);
    assert!(progress(&program, 1).is_none());
    assert!(admitted(&program).is_empty());
}

#[test]
fn increasing_calls_reject_unproved_arrivals_and_weak_cycles() {
    for source in [
        INCREASING.replace("position + 1", "position"),
        INCREASING.replace("&& limit <= capacity", ""),
        INCREASING.replace(
            "self.second(capacity, cursor, limit)",
            "self.second(capacity - 1, cursor, limit)",
        ),
    ] {
        assert!(admitted(&typed_source(&source)).is_empty(), "{source}");
    }
}

#[test]
fn positive_source_rank_can_strictly_drop_to_the_zero_plateau() {
    let program = typed_source(&INCREASING.replace("position + 1", "position + 2"));
    // This is a ranking judgment, not executable addition formation or the
    // destination's separate authored arrival-requirement judgment.
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
}

#[test]
fn increasing_calls_keep_distinct_authored_orders_and_selected_meanings() {
    let mixed = INCREASING.replace("position -> Nat::IncreasingTo(bound)", "position");
    assert!(admitted(&typed_source(&mixed)).is_empty());
    let authored = format!("{INCREASING}\noperator + u64::add(left: u64, right: u64) -> u64;");
    let program = typed_source(&authored);
    assert!(progress(&program, 1).is_none());
    assert!(admitted(&program).is_empty());
}

#[test]
fn increasing_bound_custody_cannot_be_missing_or_foreign() {
    let program = typed_source(INCREASING);
    let mut missing = program.clone();
    for custody in &mut missing.ranking_expression_custody {
        custody.view_arguments.clear();
    }
    assert!(admitted(&missing).is_empty());
    let mut foreign = program;
    let first = foreign.ranking_expression_custody[0].view_arguments[0];
    let second = foreign.ranking_expression_custody[1].view_arguments[0];
    foreign.ranking_expression_custody[0].view_arguments[0] = second;
    foreign.ranking_expression_custody[1].view_arguments[0] = first;
    assert!(progress(&foreign, 0).is_none());
    assert!(admitted(&foreign).is_empty());
}

#[test]
fn variable_increasing_step_requires_a_proved_strict_edge() {
    let source = "data Main {}
        machine Main::first(&mut self, n: u64, step: u64, cap: u64)
        requires step > 0 && n <= cap;
        terminates by n -> Nat::IncreasingTo(cap) in 0..=cap;
        -> u64 { transition n + step <= cap { true -> self.second(n, step, cap) false -> n } }
        machine Main::second(&mut self, n: u64, step: u64, cap: u64)
        requires step > 0 && n <= cap;
        terminates by n -> Nat::IncreasingTo(cap) in 0..=cap;
        -> u64 { transition n + step <= cap { true -> self.first(n + step, step, cap) false -> n } }";
    let program = typed_source(source);
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict)
    );
    assert_eq!(admitted(&program).len(), 1);
    assert!(admitted(&typed_source(&source.replace("step > 0 && ", ""))).is_empty());
    assert!(
        admitted(&typed_source(
            &source
                .replace("step: u64", "step: i64")
                .replace("step > 0 && ", "")
        ))
        .is_empty()
    );
}

#[test]
fn variable_increasing_step_keeps_a_distinct_rank_ceiling() {
    let source = INCREASING
        .replace("capacity: u64", "capacity: u64, step: u64")
        .replace("bound: u64", "bound: u64, amount: u64")
        .replace(
            "requires cursor <= limit",
            "requires step > 0 && cursor <= limit",
        )
        .replace(
            "requires position <= bound",
            "requires amount > 0 && position <= bound",
        )
        .replace(
            "self.second(capacity, cursor, limit)",
            "self.second(capacity, cursor, limit, step)",
        )
        .replace("position < bound", "position + amount <= bound")
        .replace(
            "self.first(bound, position + 1, ceiling)",
            "self.first(bound, position + amount, ceiling, amount)",
        );
    let program = typed_source(&source);
    assert_eq!(
        progress(&program, 0),
        Some(RankingRangeCallProgress::NonIncreasing),
        "forwarding edge"
    );
    assert_eq!(
        progress(&program, 1),
        Some(RankingRangeCallProgress::Strict),
        "advancing edge"
    );
    assert_eq!(admitted(&program).len(), 1);
}
