//! Scalar roots retain primitive local identity through canonical publication.

#[path = "scalar_local_borrows/roster_custody.rs"]
mod roster_custody;
#[path = "scalar_local_borrows/source_custody.rs"]
mod source_custody;
#[path = "scalar_local_borrows/support.rs"]
mod support;

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_psi::OperationKind;

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
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .unwrap();
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
