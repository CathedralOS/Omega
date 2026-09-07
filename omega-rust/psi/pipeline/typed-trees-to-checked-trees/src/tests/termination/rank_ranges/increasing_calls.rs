use super::{lower_typed_trees, typed};

const PAIR: &str = r#"
data Main { observed: u64; }
machine Main::first(&mut self, limit: u64, cursor: u64, capacity: u64)
requires cursor <= limit && limit <= capacity;
terminates by cursor -> Nat::IncreasingTo(limit) in 0..=capacity;
-> u64 {
    transition cursor < limit {
        true -> self.second(capacity, cursor, limit)
        false -> cursor
    }
}
machine Main::second(&mut self, ceiling: u64, position: u64, bound: u64)
requires position <= bound && bound <= ceiling;
terminates by position -> Nat::IncreasingTo(bound) in 0..=ceiling;
-> u64 {
    transition position < bound {
        true -> self.first(bound, position + 1, ceiling)
        false -> position
    }
}
"#;

fn prove(source: &str) {
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics = lower_typed_trees(typed(source)).expect_err(source);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("machine call cycle")
                || diagnostic.message.contains("cannot prove rank range")
        ),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn increasing_call_ranges_check_the_produced_rank_with_reordered_formals() {
    prove(PAIR);
    prove(
        &PAIR
            .replace("limit <= capacity", "limit < capacity")
            .replace("bound <= ceiling", "bound < ceiling")
            .replace("0..=capacity", "0..capacity")
            .replace("0..=ceiling", "0..ceiling"),
    );
}

#[test]
fn increasing_calls_preserve_a_nonzero_floor_on_the_produced_distance() {
    let source = PAIR
        .replace("cursor <= limit", "cursor < limit")
        .replace("position <= bound", "position < bound")
        .replace(
            "transition position < bound",
            "transition position + 1 < bound",
        )
        .replace("0..=capacity", "1..=capacity")
        .replace("0..=ceiling", "1..=ceiling");
    prove(&source);
    reject(&source.replace(
        "transition position + 1 < bound",
        "transition position < bound",
    ));
}

#[test]
fn moving_the_view_bound_cannot_masquerade_as_rank_descent() {
    // The range itself is constant, so only the view-bound obligation exposes
    // this otherwise in-range decrease of the produced distance.
    let source = PAIR
        .replace("0..=capacity", "0..=100")
        .replace("0..=ceiling", "0..=100")
        .replace("limit <= capacity", "limit <= 100")
        .replace("bound <= ceiling", "bound <= 100")
        .replace(
            "self.first(bound, position + 1, ceiling)",
            "self.first(bound - 1, position, ceiling)",
        );
    reject(&source);
    reject(&PAIR.replace(
        "self.second(capacity, cursor, limit)",
        "self.second(capacity, cursor, limit + 1)",
    ));
}

#[test]
fn increasing_call_ranges_reject_moved_endpoints_and_unproved_arrivals() {
    reject(&PAIR.replace(
        "self.second(capacity, cursor, limit)",
        "self.second(capacity + 1, cursor, limit)",
    ));
    reject(&PAIR.replace("position < bound", "position <= bound"));
    reject(&PAIR.replace(" && limit <= capacity", ""));
    reject(&PAIR.replace("0..=ceiling", "1..=ceiling"));
}

#[test]
fn increasing_components_reject_preserving_cycles_and_hidden_weak_calls() {
    reject(&PAIR.replace("position + 1", "position"));
    reject(&PAIR.replace(
        "false -> position",
        "false -> self.first(bound, position, ceiling)",
    ));
}

#[test]
fn bound_prefix_writes_invalidate_facts_while_disjoint_stores_preserve_them() {
    prove(&PAIR.replace(
        "    transition cursor < limit",
        "    self.observed = cursor; transition cursor < limit",
    ));
    for prefix in ["limit = 0;", "capacity = 0;", "cursor = 0;"] {
        reject(&PAIR.replace(
            "    transition cursor < limit",
            &format!("    {prefix} transition cursor < limit"),
        ));
    }
}

#[test]
fn increasing_call_steps_need_live_positive_arithmetic_premises() {
    let source = PAIR
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
        .replace("position < bound", "amount <= bound - position")
        .replace(
            "self.first(bound, position + 1, ceiling)",
            "self.first(bound, position + amount, ceiling, amount)",
        );
    prove(&source);
    reject(&source.replace("requires amount > 0 && ", "requires "));
    let unranged = without_ranges(&source);
    prove(&unranged);
    reject(&unranged.replace("requires amount > 0 && ", "requires "));
}

#[test]
fn source_selected_addition_and_comparison_do_not_supply_increasing_progress() {
    for declaration in [
        "operator + u64::add(left: u64, right: u64) -> u64;",
        "operator < u64::less(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {PAIR}"));
    }
}

fn without_ranges(source: &str) -> String {
    source
        .replace(" in 0..=capacity", "")
        .replace(" in 0..=ceiling", "")
}

#[test]
fn increasing_call_components_need_no_optional_rank_range() {
    let source = without_ranges(PAIR);
    prove(&source);
    prove(&source.replace(
        "    transition cursor < limit",
        "    self.observed = cursor; transition cursor < limit",
    ));
    reject(&source.replace("position + 1", "position"));
    reject(&source.replace(
        "false -> position",
        "false -> self.first(bound, position, ceiling)",
    ));
    reject(&source.replace(
        "self.first(bound, position + 1, ceiling)",
        "self.first(bound - 1, position, ceiling)",
    ));
    reject(&source.replace(
        "    transition cursor < limit",
        "    limit = 0; transition cursor < limit",
    ));
}

#[test]
fn unranged_increasing_calls_forward_clamped_zero_without_counting_plateau_as_descent() {
    let source = without_ranges(PAIR)
        .replace("cursor <= limit && ", "")
        .replace("position <= bound && ", "")
        .replace(
            "transition cursor < limit {\n        true -> self.second(capacity, cursor, limit)\n        false -> cursor\n    }",
            "transition { _ -> self.second(capacity, cursor, limit) }",
        );
    // Entry may already be at or beyond the bound. Forwarding preserves rank
    // zero; only the second member's below-bound arm must strictly decrease.
    prove(&source);
    reject(&source.replace(
        "transition position < bound",
        "transition position < 18446744073709551615u64",
    ));
    reject(&source.replace(
        "false -> position",
        "false -> self.first(bound, position, ceiling)",
    ));
}

#[test]
fn unranged_increasing_calls_keep_selected_meaning_and_uniform_range_policy() {
    for declaration in [
        "operator + u64::add(left: u64, right: u64) -> u64;",
        "operator < u64::less(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {}", without_ranges(PAIR)));
    }
    reject(&PAIR.replace(" in 0..=capacity", ""));
    reject(&PAIR.replace(" in 0..=ceiling", ""));
}
