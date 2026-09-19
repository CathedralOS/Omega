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
                || diagnostic
                    .message
                    .contains("cannot prove the `terminates by` ranking")
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

/// A call component reads the nested carriage too: `drain`'s `bag.items`
/// rank enters `hold`'s `pair` at its `bag` field, and `step` returns it to
/// `drain`'s own `bag` formal through a rebuilt literal. The cycle's one
/// strict subslice is the descent the component owes.
const NESTED_SLICE: &str = r#"
data Bag { items: &[u32]; }
data Pair { bag: Bag; }
data Main { pad: u64; }

machine Main::main(&mut self) -> u64 {
    let bag: Bag = Bag { items: &[7, 8, 9] };
    transition {
        _ -> self.drain(bag)
    }
}

machine Main::drain(&mut self, bag: Bag)
terminates by bag.items -> Slice::Length;
-> u64 {
    transition bag.items.len > 0 {
        true -> hold(Pair { bag: bag })
        false -> 0
    }
    state hold(pair: Pair) {
        transition pair.bag.items.len > 0 {
            true -> self.step(pair)
            false -> 0
        }
    }
}

machine Main::step(&mut self, pair: Pair)
terminates by pair.bag.items -> Slice::Length;
-> u64 {
    transition pair.bag.items.len > 0 {
        true -> self.drain(Bag { items: pair.bag.items[1..] })
        false -> 0
    }
}
"#;

#[test]
fn nested_slice_carriers_transport_through_the_call_component() {
    prove(NESTED_SLICE);
    // Forwarding the nested carriage is a move, not a descent: the cycle
    // keeps the produced length and no complete pass decreases it.
    reject(&NESTED_SLICE.replace(
        "self.drain(Bag { items: pair.bag.items[1..] })",
        "self.drain(pair.bag)",
    ));
    // An ambiguous nested carrier -- `pair` offers two `Bag` paths to the
    // `bag` role -- forms no coordinate rather than picking one.
    reject(
        &NESTED_SLICE
            .replace(
                "data Pair { bag: Bag; }",
                "data Pair { left: Bag; right: Bag; }",
            )
            .replace(
                "drain(&mut self, bag: Bag)",
                "drain(&mut self, bag: Bag, other: Bag)",
            )
            .replace("self.drain(bag)", "self.drain(bag, bag)")
            .replace("Pair { bag: bag }", "Pair { left: bag, right: other }")
            .replace("pair.bag.items", "pair.left.items")
            .replace(
                "self.drain(Bag { items: pair.left.items[1..] })",
                "self.drain(Bag { items: pair.left.items[1..] }, pair.right)",
            ),
    );
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

#[test]
fn mixed_call_ranges_preserve_dependent_endpoints_in_both_directions() {
    for omitted in [" in floor..=ceiling", " in lower..=upper"] {
        let source = PAIR.replace(omitted, "");
        prove(&source);
        prove(&source.replace("floor..=ceiling", "(floor + 0)..=(ceiling + 0)"));
        prove(&source.replace(
            "    transition remaining > floor",
            "    self.observed = remaining; transition remaining > floor",
        ));
        for changed in [
            source.replace(
                "ceiling, remaining, floor)",
                "ceiling + 1, remaining, floor)",
            ),
            source.replace(
                "lower, pending - 1, upper)",
                "lower, pending - 1, upper - 1)",
            ),
            source.replace("pending - 1", "pending"),
            source.replace("pending - 1", "pending - 2"),
        ] {
            reject(&changed);
        }
    }
}

#[test]
fn mixed_call_ranges_cannot_assume_entry_or_arrival_membership() {
    let source = PAIR.replace(" in lower..=upper", "");
    reject(&source.replace("requires floor <= remaining && remaining <= ceiling;", ""));
    reject(&source.replace("requires lower <= pending && pending <= upper;", ""));
    reject(&source.replace("pending > lower", "pending >= lower"));
    for declaration in [
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
        "operator > u64::greater(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {source}"));
    }
}

#[test]
fn mixed_call_range_endpoint_transport_keeps_duplicate_copies_as_alternatives() {
    let source = PAIR
        .replace(" in lower..=upper", "")
        .replace("lower: u64)", "lower: u64, spare: u64)")
        .replace(
            "requires lower <= pending",
            "requires upper == spare && lower <= pending",
        )
        .replace(
            "ceiling, remaining, floor)",
            "ceiling, remaining, floor, ceiling)",
        )
        .replace("lower, pending - 1, upper)", "lower, pending - 1, spare)");
    prove(&source);
    let changed_copy = source
        .replace(
            "requires floor <= remaining",
            "requires ceiling < 100 && floor <= remaining",
        )
        .replace(
            "requires upper == spare",
            "requires upper < 100 && upper == spare",
        )
        .replace(
            "ceiling, remaining, floor, ceiling)",
            "ceiling, remaining, floor, ceiling + 1)",
        );
    let diagnostics = lower_typed_trees(typed(&changed_copy)).expect_err(&changed_copy);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .to_lowercase()
            .contains("cannot prove requires")),
        "{changed_copy}\n{diagnostics:#?}"
    );
}

#[test]
fn mixed_call_range_pins_survive_multiple_unranged_members_and_parallel_edges() {
    let source = format!(
        "{}\n{}",
        PAIR.replace(" in lower..=upper", "").replace(
            "self.first(lower, pending - 1, upper)",
            "self.third(pending - 1, lower, upper)",
        ),
        r#"
        machine Main::third(&mut self, amount: u64, bottom: u64, top: u64)
        requires bottom <= amount && amount <= top;
        terminates by amount;
        -> u64 {
            transition { _ -> self.first(bottom, amount, top) }
        }
        "#,
    );
    prove(&source);
    reject(&source.replace(
        "self.first(bottom, amount, top)",
        "self.first(bottom, amount, top + 1)",
    ));
    reject(&source.replace(
        "false -> pending",
        "false -> self.third(pending, lower, upper)",
    ));
    let hidden_weak_cycle = source
        .replace("transition pending > lower", "transition true")
        .replace("self.third(pending - 1, lower, upper)", "self.third(pending, lower, upper)")
        .replace(
            "transition { _ -> self.first(bottom, amount, top) }",
            "transition amount > bottom { true -> self.first(bottom, amount - 1, top) false -> self.second(top, amount, bottom) }",
        );
    reject(&hidden_weak_cycle);
}

#[test]
fn mixed_call_range_endpoint_inputs_accept_checked_arithmetic_identity() {
    let source = PAIR.replace(" in lower..=upper", "");
    prove(&source.replace(
        "ceiling, remaining, floor)",
        "ceiling + 0, remaining, floor)",
    ));
    reject(&source.replace(
        "ceiling, remaining, floor)",
        "ceiling + 1, remaining, floor)",
    ));
    reject(&format!(
        "operator + u64::add(left: u64, right: u64) -> u64; {}",
        source.replace(
            "ceiling, remaining, floor)",
            "ceiling + 0, remaining, floor)"
        )
    ));
}

const STATEFUL: &str = r#"
data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining in 0..=9;
-> u32 {
    transition remaining > 0 { true -> hold(remaining) false -> remaining }
    state hold(pending: u32 [0..=9]) {
        transition pending > 1 { true -> hold(pending - 1) false -> self.step(pending) }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u32 {
    transition n > 0 { true -> self.count(n - 1) false -> n }
}
"#;

#[test]
fn component_members_keep_internal_cycles_under_the_local_ranking_rule() {
    // The whole-component judgment witnesses cross-machine call edges only;
    // every internal arrival still answers to the member's own ranking
    // judgment. A strictly decreasing internal loop beside the weak
    // `hold -> step` call edge is admitted.
    prove(STATEFUL);
    // A member that can loop internally without descent is rejected even
    // though the component's call edges are well-formed: `hold(pending)`
    // arrives with the rank unchanged, and `self.count(pending)` re-enters
    // the entry without descent.
    reject(&STATEFUL.replace("hold(pending - 1)", "hold(pending)"));
    reject(&STATEFUL.replace(
        "false -> self.step(pending)",
        "false -> self.count(pending)",
    ));
    // An unranged member is held to the same local rule: its internal loop
    // cannot borrow the component's transported entry telescope, so even the
    // decreasing arrival stays unproven, matching the standalone judgment.
    let unranged = STATEFUL.replace(" in 0..=9", "");
    reject(&unranged);
    reject(&unranged.replace("hold(pending - 1)", "hold(pending)"));
}

const DUPLICATED: &str = r#"
data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining in 0..=9;
-> u32 {
    transition remaining > 0 { true -> pair(remaining, remaining) false -> remaining }
    state pair(left: u32 [0..=9], right: u32 [0..=9]) {
        transition left > 0 { true -> self.step(right) false -> left }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u32 {
    transition n > 0 { true -> self.count(n - 1) false -> n }
}
"#;

#[test]
fn component_calls_carry_rank_through_duplicated_arrival_copies() {
    // `pair(left, right)` holds two copies of `remaining`; discovery kept both
    // claims because every arrival forwarded a bare name, so the copies are
    // equal and the call may read either carrier.
    prove(DUPLICATED);
    // `remaining - 1` names `right` as the moved copy of `remaining`; the
    // stale `left` demotes and `step(right)` transports the descended copy,
    // so the component still terminates.
    prove(&DUPLICATED.replace(
        "pair(remaining, remaining)",
        "pair(remaining, remaining - 1)",
    ));
    // The stale copy cannot stand in for the rank: once `left` demotes,
    // `step(left)` hands the callee a slot carrying no `remaining` role.
    reject(
        &DUPLICATED
            .replace(
                "pair(remaining, remaining)",
                "pair(remaining, remaining - 1)",
            )
            .replace("self.step(right)", "self.step(left)"),
    );
    reject(&DUPLICATED.replace("self.step(right)", "self.step(right + 1)"));
    // An intervening write to a carrier invalidates the copied premise.
    reject(
        &DUPLICATED
            .replace("right: u32 [0..=9]", "mut right: u32 [0..=9]")
            .replace(
                "        transition left > 0 { true -> self.step(right)",
                "        right = 0; transition left > 0 { true -> self.step(right)",
            ),
    );
}

fn reject_range(source: &str) {
    let diagnostics = lower_typed_trees(typed(source)).expect_err(source);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot prove rank range")
                || diagnostic.message.contains("machine call cycle")
        }),
        "{source}\n{diagnostics:#?}"
    );
}

fn reject_requires(source: &str) {
    let diagnostics = lower_typed_trees(typed(source)).expect_err(source);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract")),
        "{source}\n{diagnostics:#?}"
    );
}

const SUBORDINATE_REQUIRES: &str = r#"
data Main {}

machine Main::main(&mut self) -> u64 {
    transition { _ -> self.outer(6, 4) }
}

machine Main::outer(&mut self, cap: u64, remaining: u64)
requires remaining <= cap;
terminates by remaining in 0..=cap;
-> u64 {
    transition remaining > 0 {
        true -> hold(remaining, cap)
        false -> remaining
    }
    state hold(pending: u64, bound: u64) {
        transition pending > 0 {
            true -> self.inner(pending, bound)
            false -> pending
        }
    }
}

machine Main::inner(&mut self, n: u64, limit: u64)
requires n <= limit;
terminates by n;
-> u64 {
    transition n > 0 && n <= limit {
        true -> self.outer(limit, n - 1)
        false -> n
    }
}
"#;

#[test]
fn subordinate_call_reads_the_caller_members_proven_range_invariant() {
    prove(SUBORDINATE_REQUIRES);
    // The invariant supplies exactly the comparisons it proves: a strictly
    // stronger requirement, or one the carried facts do not establish,
    // still has no site evidence.
    reject_requires(&SUBORDINATE_REQUIRES.replace("requires n <= limit;", "requires n < limit;"));
    reject_requires(&SUBORDINATE_REQUIRES.replace(
        "requires n <= limit;",
        "requires n <= limit && limit <= 42;",
    ));
    // An actual that is not the carried endpoint leaves the ceiling
    // unpinned at the component edge even though `pending <= pending`
    // remains provable.
    reject(
        &SUBORDINATE_REQUIRES.replace("self.inner(pending, bound)", "self.inner(pending, pending)"),
    );
    // An arrival the member cannot pin fails its own state-edge judgment
    // rather than feeding a stale premise into the requires proof, and an
    // intervening write to a required carrier does the same.
    reject(
        &SUBORDINATE_REQUIRES
            .replace(
                "transition remaining > 0 {",
                "transition remaining > 0 && cap > 0 {",
            )
            .replace("hold(remaining, cap)", "hold(remaining, cap - 1)"),
    );
    reject(
        &SUBORDINATE_REQUIRES
            .replace("pending: u64, bound: u64)", "pending: u64, mut bound: u64)")
            .replace(
                "        transition pending > 0 {",
                "        bound = bound; transition pending > 0 {",
            ),
    );
}

const MIXED_STATEFUL: &str = r#"
data Main {}
machine Main::outer(&mut self, cap: u64, remaining: u64)
requires remaining <= cap;
terminates by remaining in 0..=cap;
-> u64 {
    transition remaining > 0 { true -> hold(remaining, cap) false -> remaining }
    state hold(pending: u64, bound: u64) {
        transition pending > 0 {
            true -> self.inner(pending, bound)
            false -> pending
        }
    }
}
machine Main::inner(&mut self, n: u64, limit: u64)
terminates by n;
-> u64 {
    transition n > 0 && n <= limit { true -> self.outer(limit, n - 1) false -> n }
}
"#;

#[test]
fn mixed_component_conserves_endpoints_through_internal_state_calls() {
    // `hold(pending, bound)` carries the ranked input and the authored
    // ceiling. The site reads `outer`'s own range invariant -- `0 <= pending
    // <= bound`, re-established at the arrival by the member's state-edge
    // judgment -- so the guard need not respell membership; the unranged
    // member must still transport that exact endpoint back.
    prove(MIXED_STATEFUL);
    prove(&MIXED_STATEFUL.replace("pending > 0", "pending > 0 && pending <= bound"));
    // An actual that is not the carried endpoint cannot pin the authored
    // ceiling, even when it is spelled from the same carrier.
    reject(&MIXED_STATEFUL.replace(
        "self.inner(pending, bound)",
        "self.inner(pending, bound + 1)",
    ));
    reject(&MIXED_STATEFUL.replace("self.inner(pending, bound)", "self.inner(pending, pending)"));
    // A store into a subject or endpoint carrier before the call invalidates
    // the arrival invariant, whatever value it stores.
    reject(
        &MIXED_STATEFUL
            .replace("pending: u64, bound: u64)", "pending: u64, mut bound: u64)")
            .replace(
                "        transition pending > 0",
                "        bound = bound; transition pending > 0",
            ),
    );
    reject(
        &MIXED_STATEFUL
            .replace("pending: u64, bound: u64)", "mut pending: u64, bound: u64)")
            .replace(
                "        transition pending > 0",
                "        pending = 0; transition pending > 0",
            ),
    );
    // The invariant is consumed only as the member's own proof: an arrival
    // that cannot establish membership or pin the ceiling fails -- the
    // component judgment now reports the moved endpoint first, and the
    // member's state-edge judgment cannot pin it either -- and a missing
    // entry premise fails the component's entry obligation.
    reject_range(
        &MIXED_STATEFUL
            .replace(
                "transition remaining > 0 {",
                "transition remaining > 0 && cap > 0 {",
            )
            .replace("hold(remaining, cap)", "hold(remaining, cap - 1)"),
    );
    reject(&MIXED_STATEFUL.replace("requires remaining <= cap;\n", ""));
    // A callee's public `requires` is ordinary contract application at the
    // site, and the member's own proven range invariant is the site's
    // established evidence for it: `pending <= bound` discharges
    // `n <= limit` without a respelled guard.
    let public =
        MIXED_STATEFUL.replace("terminates by n;", "requires n <= limit;\nterminates by n;");
    prove(&public);
    prove(&public.replace("pending > 0", "pending > 0 && pending <= bound"));
}

const SLICE_STATEFUL: &str = r#"
data Entry { value: u32; }
data Main {}
machine Main::scan(&mut self, entries: &[Entry], capacity: u64)
requires entries.len <= capacity;
terminates by entries -> Slice::Length in 0..=capacity;
-> u64 {
    transition entries.len > 0 { true -> hold(entries, capacity) false -> 0 }
    state hold(pending: &[Entry], bound: u64) {
        transition pending.len > 0 {
            true -> self.step(pending, bound)
            false -> 0
        }
    }
}
machine Main::step(&mut self, rest: &[Entry], capacity: u64)
terminates by rest -> Slice::Length;
-> u64 {
    transition rest.len > 0 && rest.len <= capacity {
        true -> self.scan(rest[1..], capacity)
        false -> 0
    }
}
"#;

#[test]
fn ranged_slice_member_calls_from_a_subordinate_state_under_its_own_invariant() {
    // `hold -> step(pending, bound)` forwards the carried collection and the
    // authored ceiling; the site reads `scan`'s range invariant
    // `pending.len <= bound` from the arrival rather than a respelled guard.
    prove(SLICE_STATEFUL);
    prove(&SLICE_STATEFUL.replace("pending.len > 0", "pending.len > 0 && pending.len <= bound"));
    // A changed endpoint, an intervening carrier write, an arrival that does
    // not pin the ceiling, and a missing entry premise keep rejecting.
    reject(&SLICE_STATEFUL.replace("self.step(pending, bound)", "self.step(pending, bound + 1)"));
    reject(&SLICE_STATEFUL.replace(
        "self.step(pending, bound)",
        "self.step(pending, pending.len)",
    ));
    reject(
        &SLICE_STATEFUL
            .replace(
                "pending: &[Entry], bound: u64)",
                "pending: &[Entry], mut bound: u64)",
            )
            .replace(
                "        transition pending.len > 0",
                "        bound = bound; transition pending.len > 0",
            ),
    );
    reject_range(
        &SLICE_STATEFUL
            .replace(
                "transition entries.len > 0 {",
                "transition entries.len > 0 && capacity > 0 {",
            )
            .replace("hold(entries, capacity)", "hold(entries, capacity - 1)"),
    );
    reject(&SLICE_STATEFUL.replace("requires entries.len <= capacity;\n", ""));
    // A ranged callee's public `requires` is discharged by the caller
    // member's own proven invariant: the carried collection's produced
    // length is the rank, so `pending.len <= bound` is established site
    // evidence for `rest.len <= capacity`.
    let public = SLICE_STATEFUL
        .replace(
            "terminates by rest -> Slice::Length;",
            "requires rest.len <= capacity;\nterminates by rest -> Slice::Length in 0..=capacity;",
        )
        .replace("rest.len > 0 && rest.len <= capacity", "rest.len > 0");
    prove(&public);
    prove(&public.replace("pending.len > 0", "pending.len > 0 && pending.len <= bound"));
}

const FIELD_SUBORDINATE_REQUIRES: &str = r#"
data Countdown {
    remaining: u64 [0..=9];
    limit: u64 [0..=9];
}

measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.remaining }

data Main {}

machine Main::main(&mut self) -> u64 {
    transition { _ -> self.outer(Countdown { remaining: 4, limit: 6 }) }
}

machine Main::outer(&mut self, countdown: Countdown)
requires countdown.remaining <= countdown.limit;
terminates by countdown -> Countdown::Remaining in 0..=countdown.limit;
-> u64 {
    transition countdown.remaining > 0 {
        true -> hold(countdown)
        false -> countdown.remaining
    }
    state hold(pending: Countdown) {
        transition pending.remaining > 0 {
            true -> self.inner(pending)
            false -> pending.remaining
        }
    }
}

machine Main::inner(&mut self, current: Countdown)
requires current.remaining <= current.limit;
terminates by current -> Countdown::Remaining in 0..=current.limit;
-> u64 {
    transition current.remaining > 0 {
        true -> self.outer(Countdown { remaining: current.remaining - 1, limit: current.limit })
        false -> current.remaining
    }
}
"#;

#[test]
fn field_measure_member_reads_its_proven_invariant_at_a_subordinate_call() {
    // `hold(countdown)` carries the ranked record whole: the site reads
    // `outer`'s proven invariant `pending.remaining <= pending.limit` through
    // the telescoped field coordinate, discharging `inner`'s public requires
    // without a respelled guard.
    prove(FIELD_SUBORDINATE_REQUIRES);
    prove(&FIELD_SUBORDINATE_REQUIRES.replace(
        "pending.remaining > 0",
        "pending.remaining > 0 && pending.remaining <= pending.limit",
    ));
    // The invariant supplies exactly the membership it proves: a strictly
    // stronger requires, or a conjunct the carried facts do not establish,
    // still has no site evidence.
    reject_requires(&FIELD_SUBORDINATE_REQUIRES.replace(
        "requires current.remaining <= current.limit;",
        "requires current.remaining < current.limit;",
    ));
    reject_requires(&FIELD_SUBORDINATE_REQUIRES.replace(
        "requires current.remaining <= current.limit;",
        "requires current.remaining <= current.limit && current.limit <= 5;",
    ));
    // A decreasing internal arrival keeps the endpoint pinned, so the
    // re-established invariant still discharges the same requires.
    prove(&FIELD_SUBORDINATE_REQUIRES.replace(
        "true -> hold(countdown)",
        "true -> hold(Countdown { remaining: countdown.remaining - 1, limit: countdown.limit })",
    ));
    // An intervening write to the record carrier invalidates the premise the
    // invariant was read from.
    reject(
        &FIELD_SUBORDINATE_REQUIRES
            .replace("state hold(pending: Countdown)", "state hold(mut pending: Countdown)")
            .replace(
                "        transition pending.remaining > 0",
                "        pending = Countdown { remaining: 0, limit: 0 }; transition pending.remaining > 0",
            ),
    );
    // An arrival whose rebuilt record moves the authored endpoint fails the
    // member's own state-edge judgment rather than feeding a stale premise:
    // `pending.limit` there is `countdown.remaining - 1`, not the pinned
    // `countdown.limit`.
    reject(&FIELD_SUBORDINATE_REQUIRES.replace(
        "true -> hold(countdown)",
        "true -> hold(Countdown { remaining: countdown.remaining, limit: countdown.remaining - 1 })",
    ));
}

const PROJECTED_ENDPOINT_REQUIRES: &str = r#"
data Limits {
    cap: u64 [0..=9];
}

data Main {}

machine Main::main(&mut self) -> u64 {
    transition { _ -> self.outer(4, Limits { cap: 6 }) }
}

machine Main::outer(&mut self, remaining: u64, limits: Limits)
requires remaining <= limits.cap;
terminates by remaining in 0..=limits.cap;
-> u64 {
    transition remaining > 0 {
        true -> hold(remaining, limits)
        false -> remaining
    }
    state hold(pending: u64, bounds: Limits) {
        transition pending > 0 {
            true -> self.inner(pending, bounds)
            false -> pending
        }
    }
}

machine Main::inner(&mut self, n: u64, current: Limits)
requires n <= current.cap;
terminates by n;
-> u64 {
    transition n > 0 && n <= current.cap {
        true -> self.outer(n - 1, current)
        false -> n
    }
}
"#;

#[test]
fn scalar_member_with_projected_endpoint_reads_its_invariant_at_a_subordinate_call() {
    // `outer`'s rank is the scalar `remaining`; its authored ceiling is the
    // entry-spelled projection `limits.cap`. The member's own state-edge
    // judgment re-proves `0 <= pending <= bounds.cap` on every `hold`
    // arrival through the `pending -> remaining, bounds -> limits`
    // telescope, so `inner`'s public `requires n <= current.cap` is
    // discharged without a respelled guard.
    prove(PROJECTED_ENDPOINT_REQUIRES);
    prove(&PROJECTED_ENDPOINT_REQUIRES.replace(
        "transition pending > 0 {",
        "transition pending > 0 && pending <= bounds.cap {",
    ));
    // A decreasing internal arrival keeps the endpoint pinned, so the
    // re-established invariant still discharges the same requires.
    prove(&PROJECTED_ENDPOINT_REQUIRES.replace(
        "true -> hold(remaining, limits)",
        "true -> hold(remaining - 1, limits)",
    ));
    // The invariant supplies exactly the membership it proves: a strictly
    // stronger requires, or a conjunct the carried facts do not establish,
    // still has no site evidence.
    reject_requires(
        &PROJECTED_ENDPOINT_REQUIRES
            .replace("requires n <= current.cap;", "requires n < current.cap;"),
    );
    reject_requires(&PROJECTED_ENDPOINT_REQUIRES.replace(
        "requires n <= current.cap;",
        "requires n <= current.cap && current.cap <= 5;",
    ));
    // An actual that is not the carried record leaves the ceiling unpinned:
    // the rebuilt literal's `cap` is `pending`, not the proven `bounds.cap`.
    reject(&PROJECTED_ENDPOINT_REQUIRES.replace(
        "true -> self.inner(pending, bounds)",
        "true -> self.inner(pending, Limits { cap: pending })",
    ));
    // An intervening write to the record carrier invalidates the premise the
    // invariant was read from.
    reject(
        &PROJECTED_ENDPOINT_REQUIRES
            .replace(
                "state hold(pending: u64, bounds: Limits)",
                "state hold(pending: u64, mut bounds: Limits)",
            )
            .replace(
                "        transition pending > 0 {",
                "        bounds = bounds; transition pending > 0 {",
            ),
    );
    // An arrival whose rebuilt record moves the authored endpoint fails the
    // member's own state-edge judgment rather than feeding a stale premise:
    // `bounds.cap` there is `remaining - 1`, not the pinned `limits.cap`.
    reject(&PROJECTED_ENDPOINT_REQUIRES.replace(
        "true -> hold(remaining, limits)",
        "true -> hold(remaining, Limits { cap: remaining - 1 })",
    ));
}

#[test]
fn mixed_endpoint_arithmetic_equality_uses_only_live_caller_premises() {
    let source = PAIR
        .replace(" in lower..=upper", "")
        .replace("ceiling: u64)", "ceiling: u64, shift: u64)")
        .replace(
            "ceiling, remaining, floor)",
            "ceiling + shift, remaining, floor)",
        )
        .replace(
            "lower, pending - 1, upper)",
            "lower, pending - 1, upper, 0)",
        );
    let required = source.replace(
        "requires floor <= remaining",
        "requires shift == 0 && floor <= remaining",
    );
    prove(&required);
    let guarded = source.replace(
        "transition remaining > floor",
        "transition remaining > floor && shift == 0",
    );
    prove(&guarded);
    reject(&guarded.replace("shift == 0", "shift != 0"));
    reject(&source);
    reject(&required.replace("shift: u64", "mut shift: u64").replace(
        "    transition remaining > floor",
        "    shift = 1; transition remaining > floor",
    ));
}
