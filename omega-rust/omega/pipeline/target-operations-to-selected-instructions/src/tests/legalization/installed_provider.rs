//! Installed-provider source and completion custody retained by legalization.

use crate::legalize_target_operations;
use crate::tests::fixtures::installed_provider::installed_provider_legalization_fixture;
use semantic_vocabulary::{BoundaryMachineId, ClaimId, MachineId};

#[test]
fn installed_provider_call_legalization_retains_source_and_completion_custody() {
    let (abstract_plan, target, unit) = installed_provider_legalization_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("installed provider call derives and independently replays");
    let call = legalized.plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("installed provider call");
    assert_eq!(
        legalized.plan().structural_unit_functions[0].recipe,
        legalized_operations::StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1
    );
    let legalized_operations::LegalizedCallUnitSource::InstalledProvider {
        boundary,
        provider,
        completion_claim_sources,
        completion_receipts,
    } = &call.source
    else {
        panic!("installed provider source kind");
    };
    assert_eq!(*boundary, BoundaryMachineId::new(1).unwrap());
    assert_eq!(provider.candidate, MachineId::new(2).unwrap());
    assert_eq!(completion_claim_sources.len(), 2);
    assert_eq!(completion_receipts.len(), 2);
    assert_eq!(
        call.ownership,
        [optimization_unit::OwnershipEvent::ClaimCompletion(vec![
            ClaimId::new(1).unwrap(),
            ClaimId::new(2).unwrap(),
        ])]
    );
    call.validate_source()
        .expect("retained installed source remains internally valid");
}
