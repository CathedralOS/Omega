//! Construction of the immutable semantic roots after admission succeeds.

use calling_conventions::ValuePlacement;
use effects::provider_plan::ServiceEntryClaim;

use crate::ProgramEntrySourceVisibleParameterSignature;

use super::validation::ValidatedSemanticEntryInputs;
use super::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    OptimizedProgramStorageSemanticRoot,
};

pub(super) fn construct(
    validated: ValidatedSemanticEntryInputs,
) -> OptimizedProgramStorageSemanticEntryContract {
    let ValidatedSemanticEntryInputs {
        target,
        slot,
        source,
        boundary,
        method,
        semantic_application_report_fingerprint,
        semantic_application_commitment,
        physical_contract,
    } = validated;
    let [image_source, storage_source] = source.visible_parameters() else {
        unreachable!("semantic entry admission retained exactly two visible roots")
    };
    let roots = [
        semantic_root(
            image_source,
            &method.entry_claims[0],
            &boundary.plan().call.parameters[0],
        ),
        semantic_root(
            storage_source,
            &method.entry_claims[1],
            &boundary.plan().call.parameters[1],
        ),
    ];

    OptimizedProgramStorageSemanticEntryContract {
        target,
        target_slot: slot,
        requirement_identity: method.requirement_identity,
        source_signature_identity: source.identity(),
        source_signature: source,
        semantic_boundary_entry_plan: boundary.plan().clone(),
        semantic_calling_plan_report_fingerprint: boundary.contract_report_fingerprint(),
        semantic_calling_application_report_fingerprint: semantic_application_report_fingerprint,
        semantic_calling_application_commitment: semantic_application_commitment,
        roots,
        physical_contract,
        physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1,
    }
}

fn semantic_root(
    source: &ProgramEntrySourceVisibleParameterSignature,
    claim: &ServiceEntryClaim,
    placement: &ValuePlacement,
) -> OptimizedProgramStorageSemanticRoot {
    OptimizedProgramStorageSemanticRoot {
        role: source.role(),
        parameter_index: source.visible_parameter_index(),
        carrier_identity: claim.carrier_identity.clone(),
        parameter_type_identity: source.normalized_type_identity().to_owned(),
        domain: claim.domain.clone(),
        effective_carry: claim.effective_carry,
        shape: source.value_shape(),
        placement: placement.clone(),
    }
}
