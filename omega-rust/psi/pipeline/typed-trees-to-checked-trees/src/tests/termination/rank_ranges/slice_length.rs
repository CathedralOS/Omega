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

#[test]
fn a_terminal_slice_body_still_owes_its_authored_range() {
    prove(
        "data Entry { value: u32; } machine walk(entries: &[Entry], capacity: u64) requires entries.len <= capacity; terminates by entries -> Slice::Length in 0..=capacity; -> u64 { 0 }",
    );
    reject(
        "data Entry { value: u32; } machine walk(entries: &[Entry], capacity: u64) terminates by entries -> Slice::Length in 0..=capacity; -> u64 { 0 }",
    );
}
