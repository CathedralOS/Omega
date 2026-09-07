//! Authored structural call and terminal callee production through selection.

use crate::tests::fixtures::microsoft_environment::microsoft_selection_environment;
use crate::tests::fixtures::ordinary_graph::call;
use crate::tests::fixtures::structural_call::structural_call_fixture;
use crate::{legalize_target_operations, select_instructions};
use isa_x86_64::X86_64_MICROSOFT_CALL_UNIT;
use selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use semantic_vocabulary::ObligationId;
use terminal_psi::{CrashCause, CrashRouteBucket, CrashRouteGuard};

#[test]
fn structural_call_and_terminal_callee_are_produced_and_replayed() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("one whole-root call and its structural callee legalize");
    assert_eq!(legalized.plan().scalar_functions.len(), 2);
    let legalized_call = call(&legalized.plan().scalar_functions[0]);
    assert_eq!(
        legalized_call.requirement_obligations,
        [ObligationId::new(1).unwrap()]
    );
    assert_eq!(
        legalized_call.crash_continuations,
        [CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        }]
    );
    assert!(
        legalized.plan().scalar_functions[1].blocks[0]
            .instructions
            .is_empty()
    );
    assert_eq!(legalized.receipt().function_count(), 2);

    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog)
        .expect("bounded Microsoft structural Unit calls select as ordinary instructions");
    assert_eq!(selected.plan().functions.len(), 2);
    let caller = &selected.plan().functions[0];
    assert!(caller.structural.is_some());
    assert_eq!(caller.calls.len(), 1);
    assert_eq!(caller.calls[0].call, *legalized_call);
    let instruction = caller
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|row| row.id == caller.calls[0].instruction)
        .unwrap();
    assert!(matches!(
        instruction.kind,
        SelectedInstructionKind::CallUnit { .. }
    ));
    assert_eq!(instruction.constraint, X86_64_MICROSOFT_CALL_UNIT);
    assert_eq!(instruction.operands.len(), 2);
    assert_eq!(caller.outgoing_arguments.len(), 2);
    assert_eq!(caller.memory_accesses.len(), 10);
    let SelectedTerminator::Return { instruction, .. } = &caller.blocks[0].terminator else {
        panic!("Unit return");
    };
    assert!(instruction.operands.is_empty());
    assert!(selected.plan().functions[1].calls.is_empty());
    assert_eq!(selected.receipt().function_count(), 2);
    assert_eq!(selected.receipt().block_count(), 2);
    assert!(selected.receipt().virtual_register_count() > 0);
    assert!(selected.receipt().instruction_count() > caller.memory_accesses.len());
}
