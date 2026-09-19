use super::{lower_typed_trees, typed};

const WALK: &str = r#"
data Entry { value: u32; }
machine walk(entries: &[Entry], capacity: u64)
requires entries.len <= capacity;
terminates by entries -> Slice::Length in 0..=capacity;
-> u64 {
    transition entries.len > 0 {
        true -> walk(entries[1..], capacity)
        false -> 0
    }
}
"#;

fn prove(source: &str) {
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics =
        crate::checks::termination::check_machine_termination(&typed(source)).expect_err(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn length_rank_reuses_exact_entry_and_arrival_bounds() {
    prove(WALK);
    prove(
        &WALK
            .replace("entries.len <= capacity", "entries.len < capacity")
            .replace("in 0..=capacity", "in 0..capacity"),
    );
    prove(&WALK.replace("entries[1..]", "entries[0..entries.len - 1]"));
    prove(
        &WALK
            .replace(
                "entries.len <= capacity",
                "5 <= entries.len && entries.len <= capacity",
            )
            .replace("in 0..=capacity", "in 5..=capacity")
            .replace("entries.len > 0", "entries.len > 5"),
    );
}

#[test]
fn slice_rank_requires_entry_membership_and_fixed_endpoints() {
    reject(&WALK.replace("requires entries.len <= capacity;", ""));
    reject(&WALK.replace("in 0..=capacity", "in 0..capacity"));
    reject(&WALK.replace("in 0..=capacity", "in 1..=capacity"));
    reject(&WALK.replace("entries[1..], capacity", "entries[1..], capacity + 1"));
    reject(&WALK.replace("in 0..=capacity", "in 0..=entries.len"));
}

#[test]
fn every_slice_edge_proves_geometry_and_strict_descent() {
    reject(&WALK.replace("entries[1..]", "entries"));
    reject(&WALK.replace("entries[1..]", "entries[0..]"));
    reject(&WALK.replace("entries[1..]", "entries[2..]"));
    reject(&WALK.replace("entries.len > 0", "entries.len >= 0"));
    reject(&WALK.replace("false -> 0", "false -> walk(entries, capacity)"));
}

#[test]
fn variable_slice_step_uses_preserved_positive_premise() {
    let source = WALK
        .replace("capacity: u64", "capacity: u64, step: u64")
        .replace(
            "requires entries.len <= capacity",
            "requires step > 0 && entries.len <= capacity",
        )
        .replace("entries.len > 0", "entries.len >= step")
        .replace("entries[1..], capacity", "entries[step..], capacity, step");
    prove(&source);
    reject(&source.replace("requires step > 0 && ", "requires "));
    reject(&source.replace("capacity, step)", "capacity, 0)"));
}

#[test]
fn slice_descriptor_rebinding_invalidates_old_length_facts() {
    reject(&WALK.replace(
        "    transition entries.len",
        "    entries = entries[0..]; transition entries.len",
    ));
    reject(&WALK.replace(
        "    transition entries.len",
        "    capacity = 0; transition entries.len",
    ));
}

#[test]
fn source_selected_subslicing_cannot_supply_builtin_length_geometry() {
    reject(&format!(
        "boundary operator [..] Collection::window<Element>(items: &[Element], start: u64, end: u64) -> &[Element]; {WALK}"
    ));
    reject(&format!(
        "operator > u64::greater(left: u64, right: u64) -> bool; {WALK}"
    ));
}

/// Two copies of the ranked collection: a strict subslice arrival names the
/// moved copy, and the stale sibling demotes rather than sharing the role.
const DUPLICATED: &str = r#"
machine walk(entries: &[u32], capacity: u64)
requires entries.len <= capacity;
terminates by entries -> Slice::Length in 0..=capacity;
-> u64 {
    transition entries.len > 0 {
        true -> pair(entries, entries, capacity)
        false -> 0
    }
    state pair(live: &[u32], saved: &[u32], bound: u64) {
        transition live.len > 0 {
            true -> pair(live[1..], saved, bound)
            false -> 0
        }
    }
}
"#;

#[test]
fn duplicated_slice_copies_name_the_subslice_step_as_continuation() {
    prove(DUPLICATED);
    // Whichever slot receives the strict subslice is the continuation: the
    // naming follows the moved arrival, not a position.
    prove(
        &DUPLICATED
            .replace("transition live.len > 0", "transition saved.len > 0")
            .replace(
                "true -> pair(live[1..], saved, bound)",
                "true -> pair(live, saved[1..], bound)",
            ),
    );
    // The diverging window may already arrive at the state: `saved` holds
    // the pre-step snapshot and `live` is the moved copy from the start.
    prove(&DUPLICATED.replace(
        "true -> pair(entries, entries, capacity)",
        "true -> pair(entries[1..], entries, capacity)",
    ));
    // Both copies windowing leaves the continuation ambiguous -- neither is
    // the unique moved copy.
    reject(&DUPLICATED.replace(
        "true -> pair(live[1..], saved, bound)",
        "true -> pair(live[1..], saved[1..], bound)",
    ));
    // A window that drops no proved-positive front segment is not divergence
    // evidence: `live` demotes to the stale `saved` forward and the rank
    // never decreases.
    reject(&DUPLICATED.replace(
        "true -> pair(live[1..], saved, bound)",
        "true -> pair(live[0..], saved, bound)",
    ));
    // Reading the demoted copy as the next collection is rejected: `rest`
    // receives `saved`'s pre-step slice while the role named `live`.
    reject(&DUPLICATED.replace(
        "false -> 0\n        }\n    }\n}",
        "false -> finish(saved)\n        }\n    }\n    state finish(rest: &[u32]) {\n        transition rest.len > 0 {\n            true -> finish(rest[1..])\n            false -> 0\n        }\n    }\n}",
    ));
}

#[test]
fn mutable_slice_parameter_proves_only_while_the_prefix_preserves_its_path() {
    // A mutable slice parameter still denotes its arrival value while no
    // earlier statement writes its path; each edge's evaluated prefix carries
    // that write-frame evidence before this judgment reads the length.
    let source = WALK.replace("entries: &[Entry]", "mut entries: &[Entry]");
    prove(&source);
    prove(&source.replace(
        "    transition entries.len",
        "    let mut scratch: u64 = 0;\n    scratch = 1; transition entries.len",
    ));
    for prefix in ["entries = entries[0..];", "entries = entries;"] {
        reject(&source.replace(
            "    transition entries.len",
            &format!("    {prefix} transition entries.len"),
        ));
    }
}

/// The ranked collection may arrive nested inside a record carrier: `holder`
/// claims `bag`'s entry role at the unique path `bag` under `Pair`, and the
/// produced length reads `holder.bag.items` -- never a guessed carriage.
const NESTED: &str = r#"
data Bag { items: &[u32]; }
data Pair { bag: Bag; }

machine walk(bag: Bag, capacity: u64)
requires bag.items.len <= capacity;
terminates by bag.items -> Slice::Length in 0..=capacity;
-> u64 {
    transition { _ -> step(Pair { bag: bag }, capacity) }
    state step(holder: Pair, bound: u64) {
        transition holder.bag.items.len > 0 {
            true -> step(Pair { bag: Bag { items: holder.bag.items[1..] } }, bound)
            false -> 0
        }
    }
}
"#;

#[test]
fn nested_record_carriers_transport_the_produced_slice_length() {
    prove(NESTED);
    // A borrowed carrier keeps the same mapping through its `&` boundary.
    prove(
        &NESTED
            .replace(
                "step(Pair { bag: bag }, capacity)",
                "step(&Pair { bag: bag }, capacity)",
            )
            .replace("holder: Pair", "holder: &Pair")
            .replace(
                "step(Pair { bag: Bag { items: holder.bag.items[1..] } }, bound)",
                "step(&Pair { bag: Bag { items: holder.bag.items[1..] } }, bound)",
            ),
    );
    // A forward of the nested carriage is a move, not a step: the produced
    // length never decreases through `holder.bag` itself.
    reject(&NESTED.replace(
        "Pair { bag: Bag { items: holder.bag.items[1..] } }",
        "Pair { bag: holder.bag }",
    ));
    // Two `Bag` fields leave the carriage ambiguous: `holder.left` and
    // `holder.right` are equally valid readings of the same role, so no
    // coordinate forms rather than guessing which record arrived.
    reject(
        &NESTED
            .replace(
                "data Pair { bag: Bag; }",
                "data Pair { left: Bag; right: Bag; }",
            )
            .replace("bag: Bag, capacity", "bag: Bag, other: Bag, capacity")
            .replace("Pair { bag: bag }", "Pair { left: bag, right: other }")
            .replace("holder.bag.items", "holder.left.items")
            .replace(
                "Pair { bag: Bag { items: holder.left.items[1..] } }",
                "Pair { left: Bag { items: holder.left.items[1..] }, right: holder.right }",
            ),
    );
}

/// A bare slice formal may arrive packed inside a record literal: `holder`
/// claims `items`'s entry role at `Pair`'s unique slice leaf, and the
/// produced length reads `holder.bag.items` -- never a guessed carriage.
const BARE_NESTED: &str = r#"
data Bag { items: &[u32]; }
data Pair { bag: Bag; label: bool; }

machine walk(items: &[u32], capacity: u64)
requires items.len <= capacity;
terminates by items -> Slice::Length in 0..=capacity;
-> u64 {
    transition { _ -> step(Pair { bag: Bag { items: items }, label: false }, capacity) }
    state step(holder: Pair, bound: u64) {
        transition holder.bag.items.len > 0 {
            true -> step(Pair { bag: Bag { items: holder.bag.items[1..] }, label: holder.label }, bound)
            false -> 0
        }
    }
}
"#;

#[test]
fn bare_slice_role_arrives_inside_a_record_literals_unique_leaf() {
    prove(BARE_NESTED);
    // A forward of the packed carriage is a move, not a step: the produced
    // length never decreases through `holder` itself.
    reject(&BARE_NESTED.replace(
        "step(Pair { bag: Bag { items: holder.bag.items[1..] }, label: holder.label }, bound)",
        "step(holder, bound)",
    ));
    // Rebuilding the record without a strict subslice is not a step.
    reject(&BARE_NESTED.replace("holder.bag.items[1..]", "holder.bag.items"));
    // The entry premise and pinned endpoint still owe their own proofs.
    reject(&BARE_NESTED.replace("requires items.len <= capacity;\n", ""));
    reject(&BARE_NESTED.replace("in 0..=capacity", "in 0..capacity"));
    reject(&BARE_NESTED.replace(
        "label: holder.label }, bound)",
        "label: holder.label }, bound + 1)",
    ));
    // Two slice leaves leave the carriage a guess: `holder.bag` and
    // `holder.side` are equally valid readings, so no coordinate forms.
    reject(
        &BARE_NESTED
            .replace(
                "data Pair { bag: Bag; label: bool; }",
                "data Pair { bag: Bag; side: Bag; }",
            )
            .replace(
                "machine walk(items: &[u32], capacity: u64)",
                "machine walk(items: &[u32], spare: &[u32], capacity: u64)",
            )
            .replace(
                "requires items.len <= capacity;",
                "requires items.len <= capacity && spare.len <= capacity;",
            )
            .replace(
                "label: false }, capacity)",
                "side: Bag { items: spare } }, capacity)",
            )
            .replace(
                "label: holder.label }, bound)",
                "side: holder.side }, bound)",
            ),
    );
    // Two carriers contesting the same slice role keep no mapping: both
    // record slots install `items`, so neither uniquely holds its coordinate.
    reject(
        &BARE_NESTED
            .replace(
                "state step(holder: Pair, bound: u64)",
                "state step(holder: Pair, other: Pair, bound: u64)",
            )
            .replace(
                "label: false }, capacity)",
                "label: false }, Pair { bag: Bag { items: items }, label: false }, capacity)",
            )
            .replace(
                "label: holder.label }, bound)",
                "label: holder.label }, other, bound)",
            ),
    );
}

#[test]
fn named_slice_arrivals_and_entry_reentry_share_the_length_rank() {
    let source = WALK
        .replace(
            "true -> walk(entries[1..], capacity)",
            "true -> step(entries[1..], capacity)",
        )
        .replace(
            "    }\n}",
            r#"    }
    state step(pending: &[Entry], ceiling: u64) {
        transition pending.len > 0 {
            true -> walk(pending[1..], ceiling)
            false -> 0
        }
    }
}"#,
        );
    prove(&source);
    reject(&source.replace("walk(pending[1..], ceiling)", "walk(pending, ceiling)"));
    reject(&source.replace(
        "walk(pending[1..], ceiling)",
        "walk(pending[1..], ceiling + 1)",
    ));
}

/// A produced slice length still reads projected endpoints: the rank is the
/// collection's length coordinate, but `limits.cap` is an ordinary field
/// projection the same edge judgment must bind and pin across arrivals.
const PROJECTED_LIMIT: &str = r#"
data Limits { cap: u64; }
data Entry { value: u32; }

machine drain(entries: &[Entry], limits: Limits)
requires entries.len <= limits.cap;
terminates by entries -> Slice::Length in 0..=limits.cap;
-> u64 {
    transition entries.len > 0 {
        true -> drain(entries[1..], limits)
        false -> 0
    }
}
"#;

#[test]
fn slice_rank_pins_a_projected_endpoint_across_every_arrival() {
    prove(PROJECTED_LIMIT);
    // The same transport through a named state keeps the pinned endpoint.
    prove(&PROJECTED_LIMIT.replace(
        "        true -> drain(entries[1..], limits)\n        false -> 0\n    }\n}",
        r#"        true -> stage(entries[1..], limits)
        false -> 0
    }
    state stage(pending: &[Entry], bounds: Limits) {
        transition pending.len > 0 {
            true -> stage(pending[1..], bounds)
            false -> 0
        }
    }
}"#,
    ));
    // A rebuilt carrier that moves the endpoint is not a conserved limit.
    reject(&PROJECTED_LIMIT.replace(
        "true -> drain(entries[1..], limits)",
        "true -> drain(entries[1..], Limits { cap: limits.cap + 1 })",
    ));
    // Even rebuilding the same value is rejected only when correspondence
    // fails; the exact literal rebuild preserves the endpoint.
    prove(&PROJECTED_LIMIT.replace(
        "true -> drain(entries[1..], limits)",
        "true -> drain(entries[1..], Limits { cap: limits.cap })",
    ));
    // The endpoint still owes entry membership and pinning.
    reject(&PROJECTED_LIMIT.replace("requires entries.len <= limits.cap;", ""));
    reject(&PROJECTED_LIMIT.replace("in 0..=limits.cap", "in 1..=limits.cap"));
}

#[test]
fn a_terminal_slice_body_still_owes_its_authored_range() {
    prove(
        "data Entry { value: u32; } machine walk(entries: &[Entry], capacity: u64) requires entries.len <= capacity; terminates by entries -> Slice::Length in 0..=capacity; -> u64 { 0 }",
    );
    reject(
        "data Entry { value: u32; } machine walk(entries: &[Entry], capacity: u64) terminates by entries -> Slice::Length in 0..=capacity; -> u64 { 0 }",
    );
}
