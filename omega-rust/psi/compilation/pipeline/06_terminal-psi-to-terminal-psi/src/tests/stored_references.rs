//! A reference held in a record field, used through that record. Read from
//! source, as a Build program: `samples/cli/facts/build_activation_borrow`
//! keeps the `Build` it was lent in a `Configuration` and reads, writes and
//! lends it on through `configuration.builder`.
//!
//! structural_access.md, "Construction and receiving checks": "A first-class
//! `Reference { referent, access }` structural value owns a permission
//! carrier, not its referent. ... the explicit `Referent` path segment crosses
//! the borrowed boundary." A member named through a stored reference is a
//! member of the referent: the path crosses the descriptor where the field
//! holds one, exactly as it does for a reference held by a local.

use super::build_purpose::judge_build;
use super::*;

/// `accepted` for a program the checker accepts and can publish, `rejected`
/// with what it judged wrong, else the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected {
            violations,
            conflicts,
            unproved,
            ..
        }) => format!(
            "rejected: {:?}, {} conflicts, {} unproved",
            violations
                .iter()
                .map(|violation| violation.statement.as_str())
                .collect::<Vec<_>>(),
            conflicts.len(),
            unproved.len()
        ),
        other => panic!("{other:?}"),
    }
}

const CARRIER: &str = r#"
data Cell { before: u64; value: u64; after: u64; }
data Carrier<'storage> { cell: &'storage mut Cell; }
data View<'storage> { cell: &'storage Cell; }
"#;

/// A scalar field of the referent is read and stored through the record
/// that holds the reference, from the machine that built the record and
/// from a helper lent the record. This is the product of every place of
/// the sample: `carrier.cell.value = incoming` and `carrier.cell.value ==
/// 41`.
#[test]
fn a_member_of_the_referent_is_read_and_stored_through_a_stored_reference() {
    assert_eq!(
        verdict(&format!(
            r#"{CARRIER}
machine update(carrier: &mut Carrier, incoming: u64) {{ carrier.cell.value = incoming; }}
machine build_entry(cell: &mut Cell) {{
    let mut carrier: Carrier = Carrier {{ cell: cell }};
    update(&mut carrier, 41);
    let observed: bool = carrier.cell.value == 41;
    carrier.cell.value = 73;
}}
"#
        )),
        "accepted"
    );
}

/// The twin: the stored reference is shared, so nothing is stored through
/// it. ownership.md, "Borrows and aliases": `&T` grants "Shared observation
/// under its loan contract."
#[test]
fn nothing_is_stored_through_a_stored_shared_reference() {
    let refused = verdict(&format!(
        r#"{CARRIER}
machine build_entry(cell: &Cell) {{
    let mut view: View = View {{ cell: cell }};
    view.cell.value = 73;
}}
"#
    ));
    assert!(
        refused.contains("requires mutable source access"),
        "{refused}"
    );
}

/// Reading through a stored shared reference is observation, which it
/// grants.
#[test]
fn a_member_of_the_referent_is_read_through_a_stored_shared_reference() {
    assert_eq!(
        verdict(&format!(
            r#"{CARRIER}
machine build_entry(cell: &Cell) {{
    let view: View = View {{ cell: cell }};
    let observed: bool = view.cell.value == 9;
}}
"#
        )),
        "accepted"
    );
}

/// Assigning a reference to the field that holds one replaces the
/// descriptor and leaves both referents as they were. lifetimes.md,
/// "Carried-loan transport": "Assignment evaluates the replacement while old
/// loans remain active, ends the overwritten field's carried loans, then
/// installs the replacement's exact loans." Both formals state one region,
/// which is what lets the field hold either.
#[test]
fn a_reference_assigned_to_its_field_replaces_the_descriptor() {
    assert_eq!(
        verdict(&format!(
            r#"{CARRIER}
machine build_entry<'storage>(left: &'storage Cell, right: &'storage Cell) {{
    let mut view: View = View {{ cell: left }};
    view.cell = right;
    let observed: bool = view.cell.value == 9;
}}
"#
        )),
        "accepted"
    );
}

/// The Product `text` judged from its `entry`, published and run in the Psi
/// interpreter: `Ok` with how it ended, or what the checker said instead.
fn run(text: &str) -> Result<terminal_interpreter::TerminalExecutionStatus, String> {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    let elaborated = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    );
    let accepted = match crate::judge(elaborated) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        Ok(RouteVerdict::FallBack(refusal)) | Err(refusal) => return Err(refusal.detail),
        Ok(other) => return Err(format!("{other:?}")),
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    Ok(instance
        .invoke(&mut psi_work::WorkMeter::with_budget(10_000))
        .unwrap())
}

/// The store lands in the caller's own cell, not in a copy the record holds:
/// the entry traps unless the cell it lent reads 73 afterwards with both
/// neighbours untouched, and unless the helper's 41 was seen in between. A
/// write confined to a snapshot, a wrong field displacement or a dropped
/// store each take the trapping arm. structural_access.md, "Referent
/// identity": "Every borrowed argument denotes the original referent
/// throughout its valid loan".
#[test]
fn a_store_through_a_stored_reference_changes_the_original_cell() {
    let program = |expected: u64| {
        format!(
            r#"{CARRIER}
machine update(carrier: &mut Carrier, incoming: u64) {{ carrier.cell.value = incoming; }}
machine entry() crashes Trap {{
    let mut cell: Cell = Cell {{ before: 11, value: 5, after: 17 }};
    let mut carrier: Carrier = Carrier {{ cell: &mut cell }};
    update(&mut carrier, 41);
    let observed: bool = carrier.cell.value == 41;
    carrier.cell.value = 73;
    transition observed && cell.value == {expected} && cell.before == 11 && cell.after == 17 {{
        true -> done()
        false -> bad()
    }}
    machine done() {{}}
    machine bad() {{ crash Trap; }}
}}
"#
        )
    };
    let complete = terminal_interpreter::TerminalExecutionStatus::Complete(
        terminal_interpreter::TerminalExecutionResult::Unit,
    );
    assert_eq!(run(&program(73)), Ok(complete.clone()));
    // The oracle is live: expecting the value the cell started with traps.
    let stale = run(&program(5)).unwrap();
    assert_ne!(stale, complete, "{stale:?}");
}

/// Rebinding a stored shared reference selects the other cell and changes
/// neither: the entry traps unless the view reads the second cell's 9 after
/// the assignment and both cells still hold what they were given. The two
/// cells are fields of one lent record, so both references are of its one
/// region.
#[test]
fn a_rebound_stored_reference_reads_its_new_referent() {
    let status = run(&format!(
        r#"{CARRIER}
data Two {{ left: Cell; right: Cell; }}
machine observe(two: &Two) -> bool {{
    let mut view: View = View {{ cell: &two.left }};
    let first: bool = view.cell.value == 5;
    view.cell = &two.right;
    first && view.cell.value == 9
}}
machine entry() crashes Trap {{
    let two: Two = Two {{
        left: Cell {{ before: 11, value: 5, after: 17 }},
        right: Cell {{ before: 23, value: 9, after: 29 }}
    }};
    transition observe(&two) && two.left.value == 5 && two.right.value == 9
        && two.left.before == 11 && two.left.after == 17
        && two.right.before == 23 && two.right.after == 29 {{
        true -> done()
        false -> bad()
    }}
    machine done() {{}}
    machine bad() {{ crash Trap; }}
}}
"#
    ));
    assert_eq!(
        status,
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
}

/// Holding the record shared grants no exclusive access through the
/// exclusive reference inside it. loans.md, "Reborrow lineage and access":
/// a `Read` parent rejects a mutable child, and the parent of the stored
/// reference here is the shared loan of its record. A store through it and
/// an exclusive loan of its referent are both rejected.
#[test]
fn an_exclusive_reference_reached_through_a_shared_record_grants_no_write() {
    for body in ["carrier.cell.value = 1;", "set(carrier.cell);"] {
        let verdict = verdict(&format!(
            r#"{CARRIER}
machine set(cell: &mut Cell) {{ cell.value = 1; }}
machine poke(carrier: &Carrier) {{ {body} }}
machine build_entry(cell: &mut Cell) {{
    let carrier: Carrier = Carrier {{ cell: cell }};
    poke(&carrier);
}}
"#
        ));
        assert!(verdict.starts_with("rejected"), "{body}: {verdict}");
    }
}

/// The stored exclusive reference suspends its parent while it is still
/// used: writing the lent cell by its own name between two uses of the
/// record is a conflict (loans.md: a mutable child suspends that branch of
/// its parent until the child's lineage closes). The same store after the
/// record's last use is accepted.
#[test]
fn a_stored_exclusive_reference_excludes_its_parent_while_it_is_used() {
    let program = |first: &str, second: &str| {
        format!(
            r#"{CARRIER}
machine build_entry(cell: &mut Cell) {{
    let mut carrier: Carrier = Carrier {{ cell: cell }};
    {first}
    {second}
}}
"#
        )
    };
    let through = "carrier.cell.value = 1;";
    let direct = "cell.value = 5;";
    assert_eq!(verdict(&program(through, direct)), "accepted");
    let overlapped = verdict(&program(direct, through));
    assert!(
        overlapped.starts_with("rejected") && overlapped.contains("1 conflicts"),
        "{overlapped}"
    );
}

/// A store through the stored reference ends what was known of the cell it
/// reaches: a callee that needs the value the cell was built with is no
/// longer owed nothing. The twin without the store proves the same call.
#[test]
fn a_store_through_a_stored_reference_ends_facts_about_the_original_cell() {
    let program = |store: &str| {
        format!(
            r#"{CARRIER}
machine need(value: u64) requires value == 5 {{ }}
machine entry() {{
    let mut cell: Cell = Cell {{ before: 11, value: 5, after: 17 }};
    let mut carrier: Carrier = Carrier {{ cell: &mut cell }};
    {store}
    need(cell.value);
}}
"#
        )
    };
    assert_eq!(
        run(&program("")),
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
    let stale = run(&program("carrier.cell.value = 73;")).unwrap_err();
    assert!(stale.contains("unproved"), "{stale}");
}

/// The same replacement of an exclusive reference: the record gives up the
/// first cell and writes the second. The entry traps unless the first cell
/// still reads 5 and the second reads the 9 stored after the assignment, so
/// a store that followed the old descriptor, or a replacement that copied
/// one cell over the other, takes the trapping arm.
#[test]
fn a_rebound_exclusive_reference_writes_only_its_new_referent() {
    let status = run(&format!(
        r#"{CARRIER}
data Two {{ left: Cell; right: Cell; }}
machine swap<'storage>(left: &'storage mut Cell, right: &'storage mut Cell) {{
    let mut carrier: Carrier = Carrier {{ cell: left }};
    carrier.cell = right;
    carrier.cell.value = 9;
}}
machine entry() crashes Trap {{
    let mut two: Two = Two {{
        left: Cell {{ before: 11, value: 5, after: 17 }},
        right: Cell {{ before: 23, value: 7, after: 29 }}
    }};
    swap(&mut two.left, &mut two.right);
    transition two.left.value == 5 && two.right.value == 9
        && two.left.before == 11 && two.left.after == 17
        && two.right.before == 23 && two.right.after == 29 {{
        true -> done()
        false -> bad()
    }}
    machine done() {{}}
    machine bad() {{ crash Trap; }}
}}
"#
    ));
    assert_eq!(
        status,
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
}
