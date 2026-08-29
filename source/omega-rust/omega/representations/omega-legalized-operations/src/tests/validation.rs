use super::*;

#[test]
fn structural_call_source_validation_preserves_installed_completion_custody() {
    let authored = call_aware_plan();
    authored.structural_unit_functions[0]
        .call
        .as_ref()
        .expect("authored call")
        .validate_source()
        .expect("authored call source");

    let installed = installed_provider_plan();
    let call = installed.structural_unit_functions[0]
        .call
        .as_ref()
        .expect("installed call");
    call.validate_source().expect("installed provider source");
    assert_ne!(
        legalized_operation_plan_identity(&authored),
        legalized_operation_plan_identity(&installed)
    );

    let mut with_unrelated_source = installed.clone();
    let call = with_unrelated_source.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("installed call");
    let LegalizedCallUnitSource::InstalledProvider {
        completion_claim_sources,
        ..
    } = &mut call.source
    else {
        panic!("installed provider source");
    };
    completion_claim_sources.push(CompletionClaimSource {
        claim: id(3),
        entry: Some(EntryClaim {
            claim: id(3),
            input: id(1),
            path: Vec::new(),
        }),
        content: None,
    });
    call.validate_source()
        .expect("unrelated retained caller source is permitted");
    assert_ne!(
        legalized_operation_plan_identity(&installed),
        legalized_operation_plan_identity(&with_unrelated_source)
    );

    let mut duplicate_source = installed.clone();
    let call = duplicate_source.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("installed call");
    let LegalizedCallUnitSource::InstalledProvider {
        completion_claim_sources,
        ..
    } = &mut call.source
    else {
        panic!("installed provider source");
    };
    completion_claim_sources.push(completion_claim_sources[0].clone());
    assert_eq!(
        call.validate_source(),
        Err(LegalizedCallSourceError::CompletionEvidenceMismatch)
    );

    let mut wrong_ownership = installed.clone();
    let call = wrong_ownership.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("installed call");
    call.ownership = vec![OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])];
    assert_eq!(
        call.validate_source(),
        Err(LegalizedCallSourceError::OwnershipMismatch)
    );

    let mut wrong_provider = installed;
    let call = wrong_provider.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("installed call");
    let LegalizedCallUnitSource::InstalledProvider { provider, .. } = &mut call.source else {
        panic!("installed provider source");
    };
    provider.candidate = id(3);
    assert_eq!(
        call.validate_source(),
        Err(LegalizedCallSourceError::ProviderIdentityMismatch)
    );
}
