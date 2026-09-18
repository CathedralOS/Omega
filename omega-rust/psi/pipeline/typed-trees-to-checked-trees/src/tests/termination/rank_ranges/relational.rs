use super::{lower_typed_trees, typed};

fn prove_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn prove(source: &str) {
    prove_termination(source);
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("complete checking: {source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
        .expect_err("the range must be proved, not assumed");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

const DEPENDENT: &str = "machine walk(lo: u64, hi: u64, n: u64) requires lo <= n && n <= hi; terminates by n -> Nat::Descending in lo..=hi; -> u64";

#[test]
fn requires_establishes_entry_and_exact_backedge_reproves_fixed_endpoints() {
    let source =
        format!("{DEPENDENT} {{ transition n > lo {{ true -> walk(lo, hi, n - 1) false -> n }} }}");
    prove(&source);
    // A redundant scalar conjunct must preserve the same relational proof.
    prove(&source.replace("transition n > lo", "transition n > lo && n > 0"));
    for arguments in ["lo + 1, hi, n - 1", "lo, hi + 1, n - 1", "lo, hi, n"] {
        reject(&format!(
            "{DEPENDENT} {{ transition n > lo {{ true -> walk({arguments}) false -> n }} }}"
        ));
    }
    reject(&format!(
        "{DEPENDENT} {{ transition n >= lo {{ true -> walk(lo, hi, n - 1) false -> n }} }}"
    ));
}

#[test]
fn nonzero_floor_uses_requires_without_a_declared_parameter_range() {
    let source = "machine walk(n: u64) requires 5 <= n && n <= 10; terminates by n -> Nat::Descending in 5..=10; -> u64 { transition n > 5 { true -> walk(n - 1) false -> n } }";
    prove(source);
    reject(&source.replace("n > 5", "n >= 5"));
    reject(&source.replace("in 5..=10", "in 6..=10"));
    reject(&source.replace("in 5..=10", "in 5..10"));
}

#[test]
fn acyclic_and_terminal_entry_cannot_borrow_a_backedge_guard() {
    prove(&format!("{DEPENDENT} {{ n }}"));
    reject(
        "machine walk(n: u64) terminates by n in 5..=10; -> u64 { transition n > 5 && n <= 10 { true -> walk(n - 1) false -> n } }",
    );
    reject("machine walk(n: u64) terminates by n in 5..=10; -> u64 { n }");
}

#[test]
fn bounded_distance_subject_may_shrink_but_view_argument_stays_pinned() {
    let source = "machine shrink(lower: u64, upper: u64) requires lower <= upper && upper <= 10; terminates by (lower, upper) -> Nat::BoundedDistance in 0..=10; -> u64 { transition lower < upper { true -> shrink(lower, upper - 1) false -> lower } }";
    prove(source);
    // Explicit and derived nonzero bounds agree at complete checking.
    prove(&source.replace(
        "transition lower < upper",
        "transition lower < upper && upper > 0",
    ));
    reject(
        "machine shrink(lower: u64, upper: u64) requires lower <= upper && upper <= 10; terminates by (lower, upper) -> Nat::BoundedDistance in 0..=10; -> u64 { transition lower < upper { true -> shrink(lower, upper) false -> lower } }",
    );
    reject(
        "machine climb(index: u64, limit: u64 [0..=10]) requires index <= limit; terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1); -> u64 { transition index < limit { true -> climb(index, limit - 1) false -> index } }",
    );
}

#[test]
fn increasing_view_relational_tier_requires_natural_rank_formation_at_entry() {
    let source = "machine climb(index: u64, limit: u64 [0..=10]) requires index <= limit; terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1); -> u64 { transition index < limit { true -> climb(index + 1, limit) false -> index } }";
    prove(source);
    reject(&source.replace("requires index <= limit;", ""));
    reject(&source.replace("in 0..=(limit + 1)", "in 1..=(limit + 1)"));
    // The original clamped tier needs no index<=limit entry premise.
    prove(
        &source
            .replace("requires index <= limit;", "")
            .replace("in 0..=(limit + 1)", "in 0..=limit"),
    );
}

#[test]
fn computed_endpoint_may_land_under_requires_facts_not_declarations() {
    // `limit` has no declared bound, so `limit + 1` cannot form on storage
    // bounds alone; the requires fact bounds the leaf, and the edge
    // judgment's installed hypotheses still land the endpoint inside u64.
    let source = "machine climb(index: u64, limit: u64) requires index <= limit && limit <= 10; terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1); -> u64 { transition index < limit { true -> climb(index + 1, limit) false -> index } }";
    prove(source);
    // Without the requires bound on `limit`, nothing lands `limit + 1`.
    reject(&source.replace(
        "requires index <= limit && limit <= 10;",
        "requires index <= limit;",
    ));
    // The requires fact cannot rescue an endpoint that overflows anyway.
    reject(&source.replace(
        "in 0..=(limit + 1)",
        "in 0..=(limit + 18446744073709551615u64)",
    ));
}

#[test]
fn unrelated_payloads_and_immutable_locals_preserve_exact_parameter_ordinals() {
    let source = "data Payload { value: u64; } machine climb(flag: bool, limit: u64 [0..=10], payload: Payload, index: u64) requires index <= limit; terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1); -> u64 { let unrelated: u64 = 7; transition index < limit { true -> climb(flag, limit, payload, index + 1) false -> index } }";
    prove(source);
    lower_typed_trees(typed(source)).expect("the unrelated payload fixture checks completely");
    reject(&source.replace(
        "climb(flag, limit, payload, index + 1)",
        "climb(flag, limit + 1, payload, index + 1)",
    ));
}

#[test]
fn intervening_write_cannot_reuse_entry_hypotheses() {
    reject(&format!(
        "{DEPENDENT} {{ n = 1; transition n > lo {{ true -> walk(lo, hi, n - 1) false -> n }} }}"
    ));
}

#[test]
fn a_failed_guard_on_a_mutable_parameter_still_discharges_the_next_edge() {
    // Consecutive transitions evaluate no intervening statement, so a failed
    // guard on a mutable parameter still holds at the next edge. Here it
    // contradicts the second guard and vacuously discharges an edge that
    // cannot decrease.
    prove(
        "machine walk(mut remaining: u32 [0..=5]) terminates by remaining -> Nat::Descending in 0..=5; -> u32 { transition remaining > 0 { true -> walk(remaining - 1) } transition remaining > 0 { true -> walk(remaining) false -> remaining } }",
    );
    // A store into the ranked path before the dispatch invalidates the
    // mutable arrival premise: the preserved-prefix evidence fails, so the
    // descending edge can no longer bound its argument from the declared
    // parameter range.
    reject(
        "machine walk(mut remaining: u32 [0..=5]) terminates by remaining -> Nat::Descending in 0..=5; -> u32 { remaining = 1; transition remaining > 0 { true -> walk(remaining - 1) } transition remaining > 0 { true -> walk(remaining) false -> remaining } }",
    );
}

#[test]
fn disjoint_receiver_store_preserves_rank_range_and_pinned_endpoints() {
    let source = "data Cursor { visited: u64; } machine Cursor::walk(&mut self, index: u64, limit: u64 [0..=10]) requires index <= limit; terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1); -> u64 { self.visited = index; transition index < limit { true -> walk(index + 1, limit) false -> index } }";
    prove(source);
    for replacement in ["index = 0", "limit = 0"] {
        reject(&source.replace("self.visited = index", replacement));
    }
    reject(&source.replace("walk(index + 1, limit)", "walk(index + 1, limit + 1)"));
    reject(&source.replace("walk(index + 1, limit)", "walk(index, limit)"));
    prove(&source.replace("self.visited = index", "self.visited = index * 1"));
    reject(&format!(
        "operator * u64::multiply(left: u64, right: u64) -> u64; {}",
        source.replace("self.visited = index", "self.visited = index * 1")
    ));
    reject(&source.replace(
        "self.visited = index",
        "let alias: &mut u64 = &mut self.visited; alias = index",
    ));
    reject(&format!(
        "boundary machine unknown() -> u64; {}",
        source.replace("self.visited = index", "self.visited = unknown()")
    ));
}

#[test]
fn selected_arithmetic_and_every_evaluated_prefix_keep_builtin_custody() {
    let declaration =
        "machine walk(n: u64) requires 5 <= n && n <= 10; terminates by n in 5..=10; -> u64";
    for operator in [
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
        "operator > u64::greater(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!(
            "{operator} {declaration} {{ transition n > 5 {{ true -> walk(n - 1) false -> n }} }}"
        ));
    }
    reject(&format!(
        "operator == u64::equal(left: u64, right: u64) -> bool; {declaration} {{ transition {{ n == 7 && true -> n n > 5 -> walk(n - 1) _ -> n }} }}"
    ));
    reject(&format!(
        "operator + u64::add(left: u64, right: u64) -> u64; {declaration} {{ let unrelated: u64 = n + 0; transition n > 5 {{ true -> walk(n - 1) false -> n }} }}"
    ));
}

#[test]
fn foreign_same_spelled_endpoint_custody_cannot_bind_local_parameters() {
    let source =
        format!("{DEPENDENT} {{ transition n > lo {{ true -> walk(lo, hi, n - 1) false -> n }} }}");
    let mut program = typed(&format!("{source} {}", source.replace("walk", "other")));
    program.ranking_expression_custody[0].rank_range =
        program.ranking_expression_custody[1].rank_range;
    let diagnostics = crate::checks::termination::check_machine_termination(&program)
        .expect_err("foreign endpoints");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range"))
    );
}
