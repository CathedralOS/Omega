//! Authored structural call and terminal callee production through selection.

use crate::tests::fixtures::microsoft_environment::microsoft_selection_environment;
use crate::tests::fixtures::structural_call::structural_call_fixture;
use crate::{legalize_target_operations, select_instructions};
use isa_x86_64::X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR;
use selected_instructions::SelectedInstructionId;
use semantic_vocabulary::ObligationId;
use terminal_psi::{CrashCause, CrashRouteBucket, CrashRouteGuard};

#[test]
fn structural_call_and_terminal_callee_are_produced_and_replayed() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("one whole-root call and its structural callee legalize");
    assert!(legalized.plan().scalar_functions.is_empty());
    assert_eq!(legalized.plan().structural_unit_functions.len(), 2);
    let legalized_call = legalized.plan().structural_unit_functions[0]
        .call
        .as_ref()
        .unwrap();
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
    assert!(legalized.plan().structural_unit_functions[1].call.is_none());
    assert_eq!(
        legalized.plan().structural_unit_functions[0].recipe,
        legalized_operations::StructuralUnitLegalizationRecipe::AuthoredCallThenReturnUnitV1
    );
    assert_eq!(
        legalized.plan().structural_unit_functions[1].recipe,
        legalized_operations::StructuralUnitLegalizationRecipe::ReturnUnitV1
    );
    assert_eq!(legalized.receipt().function_count(), 2);

    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog)
        .expect("bounded Microsoft structural Unit calls select atomically");
    assert!(selected.plan().functions.is_empty());
    assert_eq!(selected.plan().structural_unit_functions.len(), 2);
    let caller = &selected.plan().structural_unit_functions[0];
    let call = caller.call.as_ref().unwrap();
    assert_eq!(call.id, SelectedInstructionId(0));
    assert_eq!(
        call.source,
        legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit
    );
    assert_eq!(
        call.constraint,
        X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR
    );
    assert!(call.arguments.len() == 2 && !call.implicit_uses.is_empty());
    assert_eq!(
        call.requirement_obligations,
        legalized_call.requirement_obligations
    );
    assert_eq!(call.crash_continuations, legalized_call.crash_continuations);
    assert_eq!(caller.terminator.instruction.id, SelectedInstructionId(1));
    assert!(caller.terminator.instruction.operands.is_empty());
    assert!(selected.plan().structural_unit_functions[1].call.is_none());
    assert_eq!(selected.receipt().function_count(), 2);
    assert_eq!(selected.receipt().block_count(), 2);
    assert_eq!(selected.receipt().virtual_register_count(), 0);
    assert_eq!(selected.receipt().instruction_count(), 3);
}
