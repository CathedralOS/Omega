//! Ordered claim-completion settlement metadata through legalization and selection.

use crate::tests::fixtures::claim_completion::claim_completion_settlement_fixture;
use crate::tests::fixtures::microsoft_environment::microsoft_selection_environment;
use crate::{
    legalize_target_operations, select_instructions, selected_instruction_plan_identity,
    validate_legalized_operations, validate_selected_instructions,
};
use legalized_operations::legalized_operation_plan_identity;
use selected_instructions::SelectedInstructionId;
use semantic_vocabulary::ClaimId;

#[test]
fn claim_completion_settlement_is_ordered_metadata_without_instruction_ids() {
    let (abstract_plan, target, unit) = claim_completion_settlement_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("two-Extent claim-completion settlement legalizes and replays");
    let caller = &legalized.plan().scalar_functions[0];
    let settlements = crate::tests::fixtures::ordinary_graph::settlements(caller);
    assert!(
        caller.blocks[0]
            .instructions
            .iter()
            .all(|row| row.result.is_none())
    );
    assert_eq!(settlements.len(), 2);
    assert_eq!(settlements[0].completion_claim_sources.len(), 2);
    assert_eq!(settlements[0].completion_receipts.len(), 1);
    assert_eq!(
        settlements[0].ownership,
        [optimization_unit::OwnershipEvent::ClaimCompletion(vec![
            ClaimId::new(1).unwrap()
        ])]
    );
    assert_eq!(
        settlements[1].ownership,
        [optimization_unit::OwnershipEvent::ClaimCompletion(vec![
            ClaimId::new(2).unwrap()
        ])]
    );

    let legalized_identity = legalized.receipt().identity();
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].blocks[0]
        .instructions
        .swap(0, 1);
    assert_ne!(
        legalized_operation_plan_identity(&corrupted),
        legalized_identity
    );
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted).is_err());

    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog)
        .expect("metadata settlement selects with only the return instruction");
    let selected_caller = &selected.plan().functions[0];
    assert!(selected_caller.calls.is_empty());
    assert_eq!(
        selected_caller
            .boundary_settlements
            .iter()
            .map(|row| &row.settlement)
            .collect::<Vec<_>>(),
        settlements
    );
    assert_eq!(
        match &selected_caller.blocks[0].terminator {
            selected_instructions::SelectedTerminator::Return { instruction, .. } => instruction.id,
            _ => panic!("return fixture"),
        },
        SelectedInstructionId(0)
    );

    let selected_identity = selected.receipt().identity();
    let mut corrupted = selected.plan().clone();
    corrupted.functions[0].boundary_settlements[0]
        .settlement
        .provider_execution = target_operations::ProviderExecutionBinding::from_execution_record(
        target_operations::ProviderPlanReportIdentity::new(23).unwrap(),
        29,
        31,
        37,
        41,
    )
    .unwrap();
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted,)
            .is_err()
    );
}
