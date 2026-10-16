//! Construction of the immutable semantic roots after admission succeeds.

use calling_conventions::ValuePlacement;
use effects::provider_plan::ServiceEntryClaim;

use crate::{ProgramEntryVisibleParameterSignature, ProgramStorageEntryDiagnostic};

use super::validation::ValidatedSemanticEntryInputs;
use super::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    OptimizedProgramStorageSemanticRoot,
};

pub(super) fn construct(
    validated: ValidatedSemanticEntryInputs,
    schema: &effects::provider_plan::ServiceSchema,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<OptimizedProgramStorageSemanticEntryContract, ProgramStorageEntryDiagnostic> {
    let ValidatedSemanticEntryInputs {
        target,
        slot,
        source,
        boundary,
        method,
        requirement_identity,
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
            source.identities(),
            &schema.identities,
            identities,
        )?,
        semantic_root(
            storage_source,
            &method.entry_claims[1],
            &boundary.plan().call.parameters[1],
            source.identities(),
            &schema.identities,
            identities,
        )?,
    ];

    Ok(OptimizedProgramStorageSemanticEntryContract {
        target,
        target_slot: slot,
        requirement_identity: identities
            .index_of(&requirement_identity)
            .ok_or_else(|| missing_identity("requirement"))?,
        source_signature_identity: source.identity(),
        source_signature: source,
        semantic_boundary_entry_plan: boundary.plan().clone(),
        semantic_calling_plan_report_fingerprint: boundary.contract_report_fingerprint(),
        semantic_calling_application_report_fingerprint: semantic_application_report_fingerprint,
        semantic_calling_application_commitment: semantic_application_commitment,
        roots,
        physical_contract,
        physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1,
    })
}

fn semantic_root(
    source: &ProgramEntryVisibleParameterSignature,
    claim: &ServiceEntryClaim,
    placement: &ValuePlacement,
    plan_identities: &terminal_psi::TerminalIdentityTable,
    claim_identities: &effects::provider_plan::ProviderIdentityTable,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<OptimizedProgramStorageSemanticRoot, ProgramStorageEntryDiagnostic> {
    Ok(OptimizedProgramStorageSemanticRoot {
        role: source.role(),
        parameter_index: source.visible_parameter_index(),
        carrier_identity: identities
            .index_of(claim_identities.text(claim.carrier_identity))
            .ok_or_else(|| missing_identity("root carrier"))?,
        parameter_type_identity: plan_identities
            .text(source.normalized_type_identity())
            .and_then(|text| identities.index_of(text))
            .ok_or_else(|| missing_identity("root parameter"))?,
        domain: claim.domain.clone(),
        effective_carry: claim.effective_carry,
        shape: source.value_shape(),
        placement: placement.clone(),
    })
}

fn missing_identity(role: &str) -> ProgramStorageEntryDiagnostic {
    ProgramStorageEntryDiagnostic(format!(
        "selected semantic ProgramStorage {role} identity is missing from Terminal Psi"
    ))
}
