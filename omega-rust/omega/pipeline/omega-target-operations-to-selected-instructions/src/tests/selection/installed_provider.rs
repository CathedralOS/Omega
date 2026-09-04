//! Installed-provider selected-call source custody, identity, and tamper rejection.

use crate::tests::fixtures::installed_provider::installed_provider_legalization_fixture;
use crate::tests::fixtures::microsoft_environment::microsoft_selection_environment;
use crate::{
    SelectedInstructionError, legalize_target_operations, select_instructions,
    selected_instruction_plan_identity, validate_selected_instructions,
};
use psi_core::ClaimId;

#[test]
fn installed_provider_call_selection_retains_and_hashes_exact_source_custody() {
    let (abstract_plan, target, unit) = installed_provider_legalization_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("installed provider call legalizes");
    let legalized_source = legalized.plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("legalized installed call")
        .source
        .clone();
    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog)
        .expect("installed provider call selects through the shared physical ABI");
    let selected_call = selected.plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("selected installed call");
    assert_eq!(selected_call.source, legalized_source);
    assert_eq!(
        selected_call.ownership,
        [omega_optimization_unit::OwnershipEvent::ClaimCompletion(
            vec![ClaimId::new(1).unwrap(), ClaimId::new(2).unwrap()]
        )]
    );
    assert_eq!(selected_call.claim_transfers.len(), 2);

    let selected_identity = selected.receipt().identity();
    let mut corrupted = selected.plan().clone();
    let call = corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("selected installed call");
    let omega_legalized_operations::LegalizedCallUnitSource::InstalledProvider { provider, .. } =
        &mut call.source
    else {
        panic!("installed provider source")
    };
    provider.candidate_identity.push_str("::substituted");
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(matches!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted,),
        Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
    ));

    let mut wrong_kind = selected.plan().clone();
    wrong_kind.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("selected installed call")
        .source = omega_legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit;
    assert_ne!(
        selected_instruction_plan_identity(&wrong_kind),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, wrong_kind,)
            .is_err()
    );

    let mut receipt_tamper = selected.plan().clone();
    let call = receipt_tamper.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("selected installed call");
    let omega_legalized_operations::LegalizedCallUnitSource::InstalledProvider {
        completion_receipts,
        ..
    } = &mut call.source
    else {
        panic!("installed provider source")
    };
    completion_receipts[0].argument_index = 1;
    assert_ne!(
        selected_instruction_plan_identity(&receipt_tamper),
        selected_identity
    );
    assert!(
        validate_selected_instructions(
            &legalized,
            &constraints,
            &physical,
            &catalog,
            receipt_tamper,
        )
        .is_err()
    );
}
