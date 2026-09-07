use super::*;

#[test]
fn structural_call_source_validation_preserves_installed_completion_custody() {
    let mut authored = call_aware_plan();
    structural_call_mut(&mut authored)
        .validate_source(&[OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])])
        .expect("authored call source");

    let mut installed = installed_provider_plan();
    let call = structural_call_mut(&mut installed);
    call.validate_source(&[OwnershipEvent::ClaimCompletion(vec![id(1), id(2)])])
        .expect("installed provider source");
    assert_ne!(
        legalized_operation_plan_identity(&authored),
        legalized_operation_plan_identity(&installed)
    );

    let mut with_unrelated_source = installed.clone();
    let call = structural_call_mut(&mut with_unrelated_source);
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
    call.validate_source(&[OwnershipEvent::ClaimCompletion(vec![id(1), id(2)])])
        .expect("unrelated retained caller source is permitted");
    assert_ne!(
        legalized_operation_plan_identity(&installed),
        legalized_operation_plan_identity(&with_unrelated_source)
    );

    let mut duplicate_source = installed.clone();
    let call = structural_call_mut(&mut duplicate_source);
    let LegalizedCallUnitSource::InstalledProvider {
        completion_claim_sources,
        ..
    } = &mut call.source
    else {
        panic!("installed provider source");
    };
    completion_claim_sources.push(completion_claim_sources[0].clone());
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimCompletion(vec![id(1), id(2)])]),
        Err(LegalizedCallSourceError::CompletionEvidenceMismatch)
    );

    let mut wrong_ownership = installed.clone();
    let call = structural_call_mut(&mut wrong_ownership);
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])]),
        Err(LegalizedCallSourceError::OwnershipMismatch)
    );

    let mut wrong_provider = installed;
    let call = structural_call_mut(&mut wrong_provider);
    let LegalizedCallUnitSource::InstalledProvider { provider, .. } = &mut call.source else {
        panic!("installed provider source");
    };
    provider.candidate = id(3);
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimCompletion(vec![id(1), id(2)])]),
        Err(LegalizedCallSourceError::ProviderIdentityMismatch)
    );
}
