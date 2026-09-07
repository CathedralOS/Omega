use super::{lower_typed_trees, typed};

const PAIR: &str = r#"
data Main { observed: u64; }
machine Main::first(&mut self, floor: u64, remaining: u64, ceiling: u64)
requires floor <= remaining && remaining <= ceiling;
terminates by remaining in floor..=ceiling;
-> u64 {
    transition remaining > floor {
        true -> self.second(ceiling, remaining, floor)
        false -> remaining
    }
}
machine Main::second(&mut self, upper: u64, pending: u64, lower: u64)
requires lower <= pending && pending <= upper;
terminates by pending in lower..=upper;
-> u64 {
    transition pending > lower {
        true -> self.first(lower, pending - 1, upper)
        false -> pending
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
fn range_bearing_calls_preserve_the_rank_on_one_edge_and_decrease_the_cycle() {
    prove(PAIR);
    prove(
        &PAIR
            .replace("remaining <= ceiling", "remaining < ceiling")
            .replace("pending <= upper", "pending < upper")
            .replace("floor..=ceiling", "floor..ceiling")
            .replace("lower..=upper", "lower..upper"),
    );
}

#[test]
fn unranked_payloads_do_not_shift_numeric_call_parameters() {
    prove(
        &PAIR
            .replace("floor: u64", "enabled: bool, floor: u64")
            .replace("upper: u64", "flag: bool, upper: u64")
            .replace(
                "ceiling, remaining, floor)",
                "enabled, ceiling, remaining, floor)",
            )
            .replace(
                "lower, pending - 1, upper)",
                "flag, lower, pending - 1, upper)",
            ),
    );
}

#[test]
fn each_call_rechecks_exact_bounds_after_argument_reordering() {
    for actuals in [
        "ceiling + 1, remaining, floor",
        "ceiling, remaining, floor + 1",
        "floor, remaining, ceiling",
    ] {
        reject(&PAIR.replace("ceiling, remaining, floor)", &format!("{actuals})")));
    }
    reject(&PAIR.replace("lower..=upper", "lower..=(upper - 1)"));
    reject(&PAIR.replace("pending > lower", "pending >= lower"));
}

#[test]
fn a_preserving_call_cycle_and_a_hidden_preserving_parallel_edge_reject() {
    reject(&PAIR.replace("pending - 1", "pending"));
    reject(&PAIR.replace(
        "false -> pending",
        "false -> self.first(lower, pending, upper)",
    ));
}

#[test]
fn caller_facts_cannot_be_replaced_by_the_callees_requirements() {
    reject(&PAIR.replace(
        "floor <= remaining && remaining <= ceiling",
        "remaining <= ceiling",
    ));
    reject(&PAIR.replace(
        "self.first(lower, pending - 1, upper)",
        "self.first(lower, pending - 2, upper)",
    ));
}

#[test]
fn endpoint_mutation_invalidates_range_premises_but_disjoint_stores_do_not() {
    prove(&PAIR.replace(
        "    transition remaining > floor",
        "    self.observed = remaining; transition remaining > floor",
    ));
    for prefix in ["ceiling = 0;", "floor = 0;", "remaining = 0;"] {
        reject(&PAIR.replace(
            "    transition remaining > floor",
            &format!("    {prefix} transition remaining > floor"),
        ));
    }
}

#[test]
fn source_selected_arithmetic_cannot_authorize_a_call_range() {
    for declaration in [
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
        "operator > u64::greater(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {PAIR}"));
    }
}

#[test]
fn variable_call_step_uses_live_caller_arithmetic_premises() {
    let source = PAIR
        .replace("ceiling: u64", "ceiling: u64, step: u64")
        .replace("lower: u64", "lower: u64, amount: u64")
        .replace(
            "requires floor <= remaining",
            "requires step > 0 && floor <= remaining",
        )
        .replace(
            "requires lower <= pending",
            "requires amount > 0 && lower <= pending",
        )
        .replace(
            "ceiling, remaining, floor)",
            "ceiling, remaining, floor, step)",
        )
        .replace(
            "pending > lower",
            "pending >= amount && pending - amount >= lower",
        )
        .replace(
            "lower, pending - 1, upper)",
            "lower, pending - amount, upper, amount)",
        );
    prove(&source);
    reject(&source.replace("requires amount > 0 && ", "requires "));
    let unranged = without_ranges(&source);
    prove(&unranged);
    reject(&unranged.replace("requires amount > 0 && ", "requires "));
    let mutable_inputs = unranged
        .replace("step: u64", "mut step: u64")
        .replace("amount: u64", "mut amount: u64")
        .replace("remaining: u64", "mut remaining: u64")
        .replace("pending: u64", "mut pending: u64");
    for source in [
        mutable_inputs.clone(),
        mutable_inputs.replace(
            "    transition remaining > floor",
            "    self.observed = remaining; transition remaining > floor",
        ),
    ] {
        prove(&source);
    }
    reject(&mutable_inputs.replace(
        "    transition remaining > floor",
        "    step = 0; transition remaining > floor",
    ));
    prove(&unranged.replace(
        "    transition remaining > floor",
        "    self.observed = remaining; transition remaining > floor",
    ));
    reject(&unranged.replace(
        "    transition remaining > floor",
        "    step = 0; transition remaining > floor",
    ));
    reject(&unranged.replace(
        "pending >= amount && pending - amount >= lower",
        "pending > lower",
    ));
    reject(&unranged.replace(
        "false -> pending",
        "false -> self.first(lower, pending, upper, amount)",
    ));
}

fn without_ranges(source: &str) -> String {
    source
        .replace(" in floor..=ceiling", "")
        .replace(" in lower..=upper", "")
}

#[test]
fn unranged_natural_calls_keep_nonnegative_arrivals_and_complete_cycle_descent() {
    let source = without_ranges(PAIR);
    prove(&source);
    reject(&source.replace("pending - 1", "pending"));
    // The live guard only proves room for one decrement, not two. Removing
    // the optional range must not turn unsigned underflow into natural descent.
    reject(&source.replace("pending - 1", "pending - 2"));
    for declaration in [
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
        "operator > u64::greater(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {source}"));
    }
}

#[test]
fn unranged_natural_call_guards_preserve_boolean_wrapper_orientation() {
    let source = without_ranges(PAIR);
    for guard in [
        "pending > lower",
        "(pending > lower) == true",
        "true == (pending > lower)",
        "(pending > lower) != false",
        "false != (pending > lower)",
    ] {
        prove(&source.replace("transition pending > lower", &format!("transition {guard}")));
    }
    for guard in [
        "(pending > lower) == false",
        "false == (pending > lower)",
        "(pending > lower) != true",
        "true != (pending > lower)",
    ] {
        reject(&source.replace("transition pending > lower", &format!("transition {guard}")));
    }
}

#[test]
fn unrelated_boolean_guards_do_not_block_unranged_natural_call_progress() {
    let source = without_ranges(PAIR)
        .replace("floor: u64", "enabled: bool, floor: u64")
        .replace("upper: u64", "flag: bool, upper: u64")
        .replace("transition remaining > floor", "transition enabled")
        .replace(
            "ceiling, remaining, floor)",
            "enabled, ceiling, remaining, floor)",
        )
        .replace(
            "lower, pending - 1, upper)",
            "flag, lower, pending - 1, upper)",
        );
    prove(&source);
}
