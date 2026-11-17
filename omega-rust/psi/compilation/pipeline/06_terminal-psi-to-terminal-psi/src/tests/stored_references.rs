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
pub(super) fn run(text: &str) -> Result<terminal_interpreter::TerminalExecutionStatus, String> {
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
            calling: None,
            providers: None,
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
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
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

/// A callee lent or given the record stores through the reference it
/// carries, so the call ends what the caller knew of the cell that
/// reference reaches, just as the same store written in the caller does
/// above. dependent_values.md, "Mutation frames": a fact survives a call
/// only when its storage is disjoint from the callee's frame, aliases
/// included. The twin whose callee stores nothing proves the same `need`.
#[test]
fn a_callee_store_through_a_lent_record_ends_facts_about_the_original_cell() {
    let program = |formal: &str, actual: &str, store: &str| {
        format!(
            r#"{CARRIER}
machine need(value: u64) requires value == 5 {{ }}
machine update(carrier: {formal}, incoming: u64) {{ {store} }}
machine entry() {{
    let mut cell: Cell = Cell {{ before: 11, value: 5, after: 17 }};
    let mut carrier: Carrier = Carrier {{ cell: &mut cell }};
    update({actual}, 73);
    need(cell.value);
}}
"#
        )
    };
    for (formal, actual) in [("&mut Carrier", "&mut carrier"), ("Carrier", "carrier")] {
        assert_eq!(
            run(&program(formal, actual, "")),
            Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )),
            "{formal}"
        );
        let stale = run(&program(formal, actual, "carrier.cell.value = incoming;")).unwrap_err();
        assert!(stale.contains("unproved"), "{formal}: {stale}");
    }
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

/// An exclusive reference held by a local is weakened into a shared field:
/// a shared reborrow through it. The record then observes the cell the
/// local was lent, and the twin that asks a shared reference for an
/// exclusive field is rejected for the access it lacks.
#[test]
fn an_exclusive_reference_is_weakened_into_a_shared_field() {
    let status = run(&format!(
        r#"{CARRIER}
machine entry() crashes Trap {{
    let mut cell: Cell = Cell {{ before: 11, value: 5, after: 17 }};
    let exclusive: &mut Cell = &mut cell;
    let view: View = View {{ cell: exclusive }};
    transition view.cell.value == 5 && view.cell.before == 11 && view.cell.after == 17 {{
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
    let strengthened = verdict(&format!(
        r#"{CARRIER}
machine build_entry(cell: &Cell) {{
    let shared: &Cell = cell;
    let carrier: Carrier = Carrier {{ cell: shared }};
}}
"#
    ));
    assert!(
        strengthened.contains("requires mutable source access"),
        "{strengthened}"
    );
}

/// A whole data value is stored through the stored reference: a case of a
/// sum field of the referent, as a Build writes `configuration.builder.
/// subsystem = Subsystem::Unspecified { .. }`. The entry traps unless the
/// cell it lent holds the new case with its payload and its scalar
/// neighbour is untouched.
const STORED_MODE: &str = include_str!("../../tests/fixtures/stored_reference_end/main.omg");

#[test]
fn a_data_value_is_stored_through_a_stored_reference() {
    let status = run(STORED_MODE);
    assert_eq!(
        status,
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
}

#[test]
fn a_plain_local_reference_ends_before_its_noncopyable_owner_is_moved() {
    let source = STORED_MODE
        .replace(
            "machine start(holder: &mut Holder) { holder.engine.mode",
            "machine start(engine: &mut Engine) { engine.mode",
        )
        .replace(
            "let mut holder: Holder = Holder { engine: &mut engine };",
            "let lent: &mut Engine = &mut engine;",
        )
        .replace("start(&mut holder);", "start(lent);");
    assert_eq!(
        run(&source),
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
}

#[test]
fn releasing_a_dead_reference_keeps_its_scalar_sibling() {
    let source = STORED_MODE
        .replace(
            "engine: &'storage mut Engine;",
            "engine: &'storage mut Engine; mark: u64;",
        )
        .replace(
            "Holder { engine: &mut engine }",
            "Holder { engine: &mut engine, mark: 11 }",
        )
        .replace(
            "check(level, engine.count)",
            "check(level, engine.count, holder.mark)",
        )
        .replace("count: u64) crashes", "count: u64, mark: u64) crashes")
        .replace("count == 3", "count == 3 && mark == 11");
    assert_eq!(
        run(&source),
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
}

#[test]
fn an_owned_move_before_the_stored_references_last_use_is_rejected() {
    let source = STORED_MODE
        .replace(
            "start(&mut holder);",
            "let moved: Mode = engine.mode; start(&mut holder);",
        )
        .replace("transition engine.mode", "transition moved");
    let result = run(&source).unwrap_err();
    assert!(
        result.contains("ConflictingUse") && result.contains("ObserveWhileExclusive"),
        "{result}"
    );
}

#[test]
fn a_missing_stored_reference_disposition_is_refused() {
    let typed = super::boundary_calls::typed(STORED_MODE);
    let entry = super::boundary_calls::entry(&typed);
    let mut elaborated = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
            calling: None,
            providers: None,
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let mut removed = 0;
    for machine in &mut elaborated.module.machines {
        for block in &mut machine.blocks {
            block.operations.retain(|operation| {
                if matches!(
                    operation.kind,
                    terminal_psi::OperationKind::ReleaseReference { .. }
                ) {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
        }
    }
    assert!(removed > 0);
    let refused = crate::judge(Ok(elaborated)).unwrap_err();
    assert!(
        refused.detail.contains("no explicit last-use disposition"),
        "{refused:?}"
    );
}

/// Data that itself holds a view, behind a reference a record holds, as
/// `Configuration` holds the `Build`. The view is what makes the referent's
/// storage something a helper's summary describes: `update` returns with
/// `holder.named.name` holding what it held on entry.
const NAMED: &str = r#"
data Named<'name> { name: &'name [u8]; value: u64; }
data Holder<'storage, 'name> { named: &'storage mut Named<'name>; }
machine update(holder: &mut Holder, incoming: u64) { holder.named.value = incoming; }
"#;

/// A helper that was lent the record and kept every view behind its
/// reference where it was does not keep the reference's loan alive: once the
/// record is no longer used, the lent storage is the lender's again.
/// ownership.md, "Borrows and aliases": "A loan held by a local closes after
/// the last use of that local and of every loan derived from it, not at the
/// end of the enclosing block". The entry traps unless the helper's 41 was
/// stored first and the direct 7 after it.
#[test]
fn a_helper_that_keeps_the_views_it_was_lent_does_not_extend_the_loan() {
    let program = |expected: u64| {
        format!(
            r#"{NAMED}
machine journey(named: &mut Named) -> bool {{
    let mut holder: Holder = Holder {{ named: named }};
    update(&mut holder, 41);
    let observed: bool = holder.named.value == 41;
    named.value = 7;
    observed
}}
machine entry() crashes Trap {{
    let mut named: Named = Named {{ name: "abc", value: 5 }};
    transition journey(&mut named) && named.value == {expected} {{
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
    assert_eq!(run(&program(7)), Ok(complete.clone()));
    // The oracle is live: expecting the helper's value traps.
    let stale = run(&program(41)).unwrap();
    assert_ne!(stale, complete, "{stale:?}");
}

/// The twin: the same write while the record is still used afterwards is a
/// conflict (loans.md: a mutable child suspends that branch of its parent
/// until the child's lineage closes).
#[test]
fn a_write_between_two_uses_of_the_lent_record_is_still_a_conflict() {
    let verdict = verdict(&format!(
        r#"{NAMED}
machine build_entry(named: &mut Named) {{
    let mut holder: Holder = Holder {{ named: named }};
    update(&mut holder, 41);
    named.value = 7;
    update(&mut holder, 73);
}}
"#
    ));
    assert!(
        verdict.starts_with("rejected") && verdict.contains("1 conflicts"),
        "{verdict}"
    );
}

/// The same record made by a helper and taken by value by another.
const HANDED_ON: &str = r#"
machine wrap<'storage, 'name>(named: &'storage mut Named<'name>) -> Holder<'storage, 'name> {
    Holder { named: named }
}
machine consume(holder: Holder) {
    let mut owned: Holder = holder;
    update(&mut owned, 41);
}
"#;

/// A record that holds a reference is passed straight on as an owned
/// argument, `consume(wrap(named))`, and the machine that takes it lends it
/// to a helper (`samples/cli/facts/build_activation_borrow/parameter`:
/// `configure_parameter(wrap(builder))`). lifetimes.md, "Carried-loan
/// transport": "Moving a borrow-carrying value transfers its exact contained
/// loan paths and access." The entry traps unless the helper's store landed
/// in the record it lent, so a store confined to a copy takes the trapping
/// arm.
#[test]
fn a_record_holding_a_reference_is_passed_on_as_an_owned_argument() {
    let program = |expected: u64| {
        format!(
            r#"{NAMED}{HANDED_ON}
machine journey(named: &mut Named) {{ consume(wrap(named)); }}
machine entry() crashes Trap {{
    let mut named: Named = Named {{ name: "abc", value: 5 }};
    journey(&mut named);
    transition named.value == {expected} {{
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
    assert_eq!(run(&program(41)), Ok(complete.clone()));
    // The oracle is live: expecting the value the record started with traps.
    let stale = run(&program(5)).unwrap();
    assert_ne!(stale, complete, "{stale:?}");
}

/// The twin: the reference the record holds still suspends the lent storage
/// until the record is handed on. Writing that storage by its own name
/// before then is a conflict (loans.md: a mutable child suspends that branch
/// of its parent until the child's lineage closes); the same write after the
/// record was taken is accepted.
#[test]
fn a_record_passed_on_as_an_owned_argument_excludes_its_parent_until_then() {
    let program = |before: &str, after: &str| {
        format!(
            r#"{NAMED}{HANDED_ON}
machine build_entry(named: &mut Named) {{
    let holder: Holder = wrap(named);
    {before}
    consume(holder);
    {after}
}}
"#
        )
    };
    let direct = "named.value = 7;";
    assert_eq!(verdict(&program("", direct)), "accepted");
    let overlapped = verdict(&program(direct, ""));
    assert!(
        overlapped.starts_with("rejected") && overlapped.contains("1 conflicts"),
        "{overlapped}"
    );
}

/// `lent` holds `engine` exclusively until `start(lent)`. Copying
/// `engine.mode` while it does is refused (ownership.md: an exclusive child
/// suspends its parent branch), and a `match` reads the same storage.
const LENT_MODE: &str = "data Mode { case Idle; case Busy(level: u64); }
data Engine { mode: Mode; count: u64; }
machine start(engine: &mut Engine) { engine.mode = Mode::Busy { level: 7 }; }
machine entry() crashes Trap {
    let mut engine: Engine = Engine { mode: Mode::Idle, count: 3 };
    let lent: &mut Engine = &mut engine;
    BEFORE
    start(lent);
    AFTER
    transition engine.mode {
        Mode::Busy { level } -> check(level, engine.count)
        Mode::Idle -> bad()
    }
    machine check(level: u64, count: u64) crashes Trap {
        transition level == 7 && count == 3 {
            true -> done()
            false -> bad()
        }
    }
    machine done() {}
    machine bad() { crash Trap; }
}";

#[test]
fn a_match_on_storage_lent_to_a_live_exclusive_reference_is_rejected() {
    let observe =
        "let busy: bool = match engine.mode { Mode::Busy { level } -> true  Mode::Idle -> false };";
    let after = LENT_MODE.replace("BEFORE", "").replace("AFTER", observe);
    assert_eq!(
        run(&after),
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
    let before = LENT_MODE.replace("BEFORE", observe).replace("AFTER", "");
    let result = run(&before).unwrap_err();
    assert!(
        result.contains("ConflictingUse") && result.contains("ObserveWhileExclusive"),
        "{result}"
    );
}
