//! Scalar roots retain primitive local identity through canonical publication.

#[path = "scalar_local_borrows/roster_custody.rs"]
mod roster_custody;
#[path = "scalar_local_borrows/short_circuit.rs"]
mod short_circuit;
#[path = "scalar_local_borrows/source_custody.rs"]
mod source_custody;
#[path = "scalar_local_borrows/support.rs"]
mod support;

use checked_trees::{
    CheckedScalarBindingDestination, CheckedScalarComputationKind, CheckedScalarExpression,
    CheckedUnitStructuralArgumentSourcePlan,
};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_psi::{OperationKind, StructuralAccess};

use support::{Expectations, execute, publish_original, reject, unsigned};

const SOURCE: &str = r#"
machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
machine first(left: u64, right: u64) -> u64 { left }
machine enter(before: u64, after: u64) -> u64 {
    let mut slot: u64 = 201;
    let answer: u64 = first(stamp(&mut slot, before), stamp(&mut slot, after));
    let snapshot: u64 = slot;
    slot = answer;
    snapshot
}
"#;

const TWO_LOCALS: &str = r#"
machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
machine first(left: u64, right: u64) -> u64 { left }
machine enter(before: u64, after: u64) -> u64 {
    let mut slot: u64 = 201;
    let mut spare: u64 = 101;
    let answer: u64 = first(stamp(&mut slot, before), stamp(&mut spare, after));
    let snapshot: u64 = slot;
    let other_snapshot: u64 = spare;
    slot = answer;
    spare = 13;
    snapshot
}
"#;

#[test]
fn source_debug_map_distinguishes_scalar_values_from_local_and_unit_results() {
    let checked = support::checked(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "enter").unwrap();
    let module = &lowered.semantic_module;
    let mut debug_map = lowered.debug_map.expect("source-backed local debug map");
    assert!(!debug_map.sites.is_empty());
    let bytes = terminal_codec::encode_debug_map(module, &debug_map).unwrap();
    assert_eq!(
        terminal_codec::decode_debug_map(module, &bytes).unwrap(),
        debug_map
    );
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.result,
                terminal_psi::OperationResult::Structural(_)
            ))
    );
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.result, terminal_psi::OperationResult::Unit))
    );
    let unknown =
        terminal_psi::DebugSubject::Value(semantic_vocabulary::ValueId::new(u64::MAX).unwrap());
    debug_map.sites[0].subject = unknown;
    debug_map.sites.sort_by_key(|site| site.subject);
    assert_eq!(
        terminal_codec::validate_debug_map(module, &debug_map),
        Err(terminal_codec::DebugMapError::UnknownSubject(unknown))
    );
}

#[test]
fn scalar_root_reads_current_referent_and_preserves_snapshot_after_assignment() {
    for (returned, expected, reads) in [("snapshot", 11, 1), ("slot", 7, 2), ("answer", 7, 1)] {
        execute(
            &SOURCE.replace("\n    snapshot\n", &format!("\n    {returned}\n")),
            &[unsigned(7), unsigned(11)],
            Expectations::scalar(unsigned(expected), reads),
        );
    }
}

#[test]
fn immutable_snapshot_before_nested_calls_keeps_the_initial_contents() {
    let source = SOURCE
        .replace(
            "    let answer:",
            "    let initial: u64 = slot;\n    let answer:",
        )
        .replace("\n    snapshot\n", "\n    initial\n");
    execute(
        &source,
        &[unsigned(7), unsigned(11)],
        Expectations::scalar(unsigned(201), 2),
    );
}

#[test]
fn unborrowed_mutable_peer_remains_scalar_storage_beside_the_real_local() {
    let source = SOURCE
        .replace(
            "    let answer:",
            "    let mut unborrowed: u64 = after;\n    unborrowed = before;\n    let answer:",
        )
        .replace("\n    snapshot\n", "\n    unborrowed\n");
    execute(
        &source,
        &[unsigned(7), unsigned(11)],
        Expectations::scalar(unsigned(7), 1),
    );
}

#[test]
fn boolean_scalar_local_uses_the_same_establishment_read_and_store_vocabulary() {
    let source = SOURCE.replace("u64", "bool").replace("= 201;", "= false;");
    for (returned, expected, reads) in [("snapshot", false, 1), ("slot", true, 2)] {
        execute(
            &source.replace("\n    snapshot\n", &format!("\n    {returned}\n")),
            &[
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
            ],
            Expectations::scalar(TerminalScalarValue::Boolean(expected), reads),
        );
    }
}

#[test]
fn two_scalar_locals_preserve_distinct_borrows_assignments_and_snapshots() {
    for (returned, expected, reads) in [
        ("snapshot", 7, 2),
        ("other_snapshot", 11, 2),
        ("slot", 7, 3),
        ("spare", 13, 3),
    ] {
        execute(
            &TWO_LOCALS.replace("\n    snapshot\n", &format!("\n    {returned}\n")),
            &[unsigned(7), unsigned(11)],
            Expectations {
                locals: 2,
                stores: 3,
                ..Expectations::scalar(unsigned(expected), reads)
            },
        );
    }
}

#[test]
fn scalar_root_write_only_borrows_update_the_original_local() {
    for (returned, expected, reads) in [("snapshot", 11, 1), ("slot", 7, 2)] {
        execute(
            &SOURCE
                .replace("&mut", "&write")
                .replace("\n    snapshot\n", &format!("\n    {returned}\n")),
            &[unsigned(7), unsigned(11)],
            Expectations {
                stamp_access: StructuralAccess::WriteOnlyBorrow,
                ..Expectations::scalar(unsigned(expected), reads)
            },
        );
    }
}

#[test]
fn scalar_root_repeated_shared_borrows_keep_the_local_and_scalar_positions() {
    let source = SOURCE
        .replace(
            "machine first",
            "machine hold(left: &u64, number: u64, right: &u64) -> u64 { number }\nmachine first",
        )
        .replace(
            "    slot = answer;",
            "    let held: u64 = hold(&slot, slot, &slot);\n    slot = answer;",
        )
        .replace("\n    snapshot\n", "\n    held\n");
    let module = execute(
        &source,
        &[unsigned(7), unsigned(11)],
        Expectations {
            machines: 4,
            ..Expectations::scalar(unsigned(11), 2)
        },
    );
    let hold = module
        .machines
        .iter()
        .find(|machine| machine.structural_parameters.len() == 2)
        .expect("shared alias callee retains both structural formals");
    assert_eq!(hold.parameters.len(), 1);
    assert_eq!(
        hold.structural_parameters
            .iter()
            .map(|parameter| parameter.position)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(
        hold.structural_parameters
            .iter()
            .all(|parameter| parameter.access == StructuralAccess::SharedBorrow)
    );
    let calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::CallStructuralScalar {
                callee,
                structural_arguments,
                ..
            } if *callee == hold.id => Some(structural_arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].len(), 2);
    assert_eq!(
        calls[0][0].place, calls[0][1].place,
        "shared actuals retain the same local referent"
    );
    assert!(
        calls[0]
            .iter()
            .all(|argument| argument.access == StructuralAccess::SharedBorrow)
    );
}

#[test]
fn scalar_local_computed_initialization_and_assignment_commit_after_rhs_calls() {
    let source = SOURCE
        .replace("= 201;", "= first(before, after);")
        .replace(
            "    let answer:",
            "    let initial: u64 = slot;\n    let answer:",
        )
        .replace(
            "    slot = answer;",
            "    slot = first(slot, stamp(&mut slot, before));",
        );
    // The assignment captures 11 before stamp writes 7, then commits 11. The
    // original immutable answer remains 7 and the snapshot remains 11.
    for (returned, expected, reads) in [
        ("initial", 7, 3),
        ("snapshot", 11, 3),
        ("slot", 11, 4),
        ("answer", 7, 3),
    ] {
        execute(
            &source.replace("\n    snapshot\n", &format!("\n    {returned}\n")),
            &[unsigned(7), unsigned(11)],
            Expectations {
                stamp_calls: 3,
                ..Expectations::scalar(unsigned(expected), reads)
            },
        );
    }
}

#[test]
fn scalar_local_read_after_nested_call_observes_the_committed_mutation() {
    let source = SOURCE
        .replace(
            "machine first",
            "machine second(left: u64, right: u64) -> u64 { right }\nmachine first",
        )
        .replace(
            "    slot = answer;",
            "    slot = second(stamp(&mut slot, before), slot);",
        )
        .replace("\n    snapshot\n", "\n    slot\n");
    execute(
        &source,
        &[unsigned(7), unsigned(11)],
        Expectations {
            machines: 4,
            stamp_calls: 3,
            ..Expectations::scalar(unsigned(7), 3)
        },
    );
}

#[test]
fn ordinary_unit_caller_uses_scalar_helper_with_activation_local_storage() {
    let source = format!(
        r#"{}
machine enter(before: u64, output: &mut u64, after: u64) {{
    let returned: u64 = calculate(before, after);
    output = returned;
}}
"#,
        SOURCE.replace("machine enter(", "machine calculate(")
    );
    execute(
        &source,
        &[unsigned(7), unsigned(11)],
        Expectations {
            result: TerminalExecutionResult::Unit,
            machines: 4,
            stores: 3,
            local_owner_is_entry: false,
            ..Expectations::scalar(unsigned(11), 1)
        },
    );
}

#[test]
fn scalar_only_caller_retains_transitive_callee_local_storage_demand() {
    let source = format!(
        r#"{}
machine enter(before: u64, after: u64) -> u64 {{
    let returned: u64 = calculate(before, after);
    returned
}}
"#,
        SOURCE.replace("machine enter(", "machine calculate(")
    );
    execute(
        &source,
        &[unsigned(7), unsigned(11)],
        Expectations {
            machines: 4,
            local_owner_is_entry: false,
            ..Expectations::scalar(unsigned(11), 1)
        },
    );
}
