use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

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
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics =
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).expect_err(source);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("machine call cycle")
                || diagnostic.message.contains("cannot prove rank range")
        ),
        "{source}\n{diagnostics:#?}"
    );
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
fn unranged_increasing_calls_keep_selected_meaning() {
    for declaration in [
        "operator + u64::add(left: u64, right: u64) -> u64;",
        "operator < u64::less(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {}", without_ranges(PAIR)));
    }
}

#[test]
fn mixed_increasing_call_ranges_keep_range_endpoints_and_view_limits_pinned() {
    for omitted in [" in 0..=capacity", " in 0..=ceiling"] {
        let source = PAIR.replace(omitted, "");
        prove(&source);
        reject(&source.replace(
            "self.second(capacity, cursor, limit)",
            "self.second(capacity + 1, cursor, limit)",
        ));
        reject(&source.replace(
            "self.first(bound, position + 1, ceiling)",
            "self.first(bound, position + 1, ceiling + 1)",
        ));
        reject(&source.replace(
            "self.first(bound, position + 1, ceiling)",
            "self.first(bound - 1, position, ceiling)",
        ));
        reject(&source.replace("position + 1", "position"));
    }
}
