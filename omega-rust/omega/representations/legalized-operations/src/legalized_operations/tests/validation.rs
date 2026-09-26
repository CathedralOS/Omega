use super::super::{LegalizedCallSourceError, NativeCallOrigin, legalized_operation_plan_identity};
use super::{
    CompletionClaimSource, EntryClaim, LegalizedScalarArgument, OwnershipEvent, call_aware_plan,
    id, installed_provider_plan, structural_call_mut,
};
#[test]
fn empty_call_ownership_events_remain_bound_to_the_original_call_origin() {
    let mut installed = installed_provider_plan();
    let call = structural_call_mut(&mut installed);
    call.claim_transfers.clear();
    let NativeCallOrigin::InstalledProvider {
        completion_claim_sources,
        completion_receipts,
        ..
    } = &mut call.source
    else {
        panic!("installed fixture origin");
    };
    completion_claim_sources.clear();
    completion_receipts.clear();
    call.validate_source(&[]).unwrap();
    call.validate_source(&[OwnershipEvent::ClaimCompletion(Vec::new())])
        .unwrap();
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimTransfer(Vec::new())]),
        Err(LegalizedCallSourceError::OwnershipMismatch)
    );

    call.source = NativeCallOrigin::Authored;
    call.validate_source(&[]).unwrap();
    call.validate_source(&[OwnershipEvent::ClaimTransfer(Vec::new())])
        .unwrap();
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimCompletion(Vec::new())]),
        Err(LegalizedCallSourceError::OwnershipMismatch)
    );
}

#[test]
fn installed_call_origin_retains_projected_operands_and_structural_receipt_ordinals() {
    let mut installed = installed_provider_plan();
    let call = structural_call_mut(&mut installed);
    let path = vec![terminal_psi::StructuralPathSegment::Field("payload".into())];
    let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0] else {
        panic!("structural fixture argument");
    };
    semantic.path = path.clone();
    target.path = path.clone();
    let NativeCallOrigin::InstalledProvider {
        completion_claim_sources,
        ..
    } = &mut call.source
    else {
        panic!("installed fixture origin");
    };
    completion_claim_sources[0].entry.as_mut().unwrap().path = path;
    // Origin consistency uses structural ordinals even with a scalar prefix.
    // ABI and referent geometry remain separate independently checked contracts.
    let placement = call.call_plan.parameters[0].clone();
    call.arguments.insert(
        0,
        LegalizedScalarArgument::Scalar {
            source: id(7),
            placement,
        },
    );
    let ownership = [OwnershipEvent::ClaimCompletion(vec![id(1), id(2)])];
    call.validate_source(&ownership)
        .expect("projected structural receipt survives a scalar prefix");

    let mut wrong_projection = call.clone();
    let LegalizedScalarArgument::Structural { target, .. } = &mut wrong_projection.arguments[1]
    else {
        panic!("structural fixture argument");
    };
    target.path.clear();
    assert_eq!(
        wrong_projection.validate_source(&ownership),
        Err(LegalizedCallSourceError::ArgumentSignatureMismatch)
    );

    let mut wrong_receipt = call.clone();
    let NativeCallOrigin::InstalledProvider {
        completion_receipts,
        ..
    } = &mut wrong_receipt.source
    else {
        panic!("installed fixture origin");
    };
    completion_receipts[0].argument_index = 1;
    assert!(wrong_receipt.validate_source(&ownership).is_err());

    let mut interleaved = call.clone();
    interleaved.arguments.swap(0, 1);
    assert_eq!(
        interleaved.validate_source(&ownership),
        Err(LegalizedCallSourceError::ArgumentSignatureMismatch)
    );
}

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
    let NativeCallOrigin::InstalledProvider {
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
    let NativeCallOrigin::InstalledProvider {
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
    let NativeCallOrigin::InstalledProvider { provider, .. } = &mut call.source else {
        panic!("installed provider source");
    };
    provider.candidate = id(3);
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimCompletion(vec![id(1), id(2)])]),
        Err(LegalizedCallSourceError::ProviderIdentityMismatch)
    );
}
