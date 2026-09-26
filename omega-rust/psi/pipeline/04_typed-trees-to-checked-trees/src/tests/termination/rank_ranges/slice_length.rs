use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

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
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
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
