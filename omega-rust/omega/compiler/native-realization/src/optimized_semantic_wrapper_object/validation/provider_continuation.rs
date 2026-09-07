use super::super::error::{
    InstalledProgramStorageContinuationEvidenceError,
    OptimizedProgramStorageSemanticWrapperObjectError,
};
use super::super::shared::*;

/// Replay the checked-provider half of the ProgramStorage join whenever the
/// canonical child owns an installation. Synthetic encoding fixtures retain
/// their existing no-installation route, while a real installed child cannot
/// reach wrapper composition unless its selected call, provider body, claim
/// completions, and opaque installation are one exact continuation.
pub(crate) fn validate_retained_installed_provider_continuation(
    source: &StagedValidatedOptimizedObjectArtifact,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let Some(installation) = source.provider_installation() else {
        return Ok(());
    };
    validate_installed_program_storage_continuation_evidence(
        installation,
        source.selected_plan(),
        source.artifact().semantic_entry,
    )
    .map_err(OptimizedProgramStorageSemanticWrapperObjectError::InstalledProviderContinuation)
}

/// Independently compare one immutable admitted installation with a selected
/// ProgramStorage continuation. This function is intentionally
/// diagnostic-only: it accepts borrowed evidence and returns no carrier,
/// receipt, encoding, object, installation, or publication authority.
pub fn validate_installed_program_storage_continuation_evidence(
    installation: &AdmittedProviderInstallation,
    selected: &SelectedInstructionPlan,
    semantic_entry: MachineId,
) -> Result<(), InstalledProgramStorageContinuationEvidenceError> {
    use InstalledProgramStorageContinuationEvidenceError as Error;

    if installation.psi() != selected.psi || selected.entry != semantic_entry {
        return Err(Error::RootMismatch);
    }
    if !selected.functions.is_empty() || selected.structural_unit_functions.len() != 2 {
        return Err(Error::FunctionRosterMismatch);
    }
    let Some(entry) = selected
        .structural_unit_functions
        .iter()
        .find(|function| function.machine == selected.entry)
    else {
        return Err(Error::FunctionRosterMismatch);
    };
    let Some(call) = entry.call.as_ref() else {
        return Err(Error::EntryCallMissing);
    };
    let SelectedStructuralUnitCallSource::InstalledProvider {
        boundary,
        provider,
        completion_claim_sources,
        completion_receipts,
    } = &call.source
    else {
        return Err(Error::SourceKindMismatch);
    };
    let [installed_candidate] = installation.installed_candidates() else {
        return Err(Error::InstallationRosterMismatch);
    };
    let [installed_call] = installation.installed_unit_calls() else {
        return Err(Error::InstallationRosterMismatch);
    };
    let semantic_arguments = call
        .arguments
        .iter()
        .map(|argument| argument.semantic.clone())
        .collect::<Vec<_>>();
    let entry_claims = entry
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .collect::<Vec<_>>();
    let completed_entry_claims = completion_receipts
        .iter()
        .map(|receipt| receipt.claim)
        .collect::<Vec<_>>();
    if installed_candidate != provider || installed_call.provider() != provider {
        return Err(Error::ProviderMismatch);
    }
    if !structural_signature_matches(entry, &provider.signature) {
        return Err(Error::StructuralContractMismatch);
    }
    if installed_call.caller() != entry.machine
        || installed_call.psi_operation() != call.operation
        || installed_call.boundary() != *boundary
        || installed_call.structural_arguments() != semantic_arguments
        || installed_call.completion_claim_sources() != completion_claim_sources
        || installed_call.completion_receipts() != completion_receipts
        || call.callee != provider.candidate
        || !entry.boundary_settlements.is_empty()
    {
        return Err(Error::CallEvidenceMismatch);
    }
    if entry_claims != completed_entry_claims {
        return Err(Error::EntryClaimMismatch);
    }
    if !entry_claims_match_parameters(entry) {
        return Err(Error::EntryClaimMismatch);
    }
    let Some(provider_function) = selected
        .structural_unit_functions
        .iter()
        .find(|function| function.machine == provider.candidate)
    else {
        return Err(Error::ProviderFunctionMismatch);
    };
    if !structural_signature_matches(provider_function, &provider.signature)
        || !entry_claims_match_parameters(provider_function)
    {
        return Err(Error::StructuralContractMismatch);
    }
    let provider_claims = provider_function
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .collect::<Vec<_>>();
    let settled_provider_claims = provider_function
        .boundary_settlements
        .iter()
        .map(
            |settlement| match settlement.completion_receipts.as_slice() {
                [receipt] => Some(receipt.claim),
                _ => None,
            },
        )
        .collect::<Option<Vec<_>>>();
    let settlement_sources_match =
        provider_function
            .boundary_settlements
            .iter()
            .all(|settlement| {
                settlement.completion_claim_sources.len() == provider_function.entry_claims.len()
                    && settlement
                        .completion_claim_sources
                        .iter()
                        .zip(&provider_function.entry_claims)
                        .all(|(source, claim)| {
                            source.claim == claim.claim && source.entry.as_ref() == Some(claim)
                        })
            });
    if provider_function.call.is_some()
        || provider_function.boundary_settlements.len() != 2
        || settled_provider_claims.as_deref() != Some(provider_claims.as_slice())
        || provider_claims.len() != completion_receipts.len()
        || !settlement_sources_match
    {
        return Err(Error::ProviderSettlementMismatch);
    }
    Ok(())
}

fn structural_signature_matches(
    function: &selected_instructions::SelectedStructuralUnitFunction,
    signature: &terminal_psi::ProviderSignature,
) -> bool {
    function.abi.parameters.len() == signature.parameters.len()
        && function
            .abi
            .parameters
            .iter()
            .zip(&signature.parameters)
            .all(|(actual, expected)| {
                let actual = &actual.semantic;
                actual.position == expected.position
                    && actual.is_self == expected.is_self
                    && actual.structural_type == expected.structural_type
                    && actual.multiplicity == expected.multiplicity
                    && actual.access == expected.access
                    && actual.qualifications == expected.qualifications
            })
}

fn entry_claims_match_parameters(
    function: &selected_instructions::SelectedStructuralUnitFunction,
) -> bool {
    function.entry_claims.len() == function.abi.parameters.len()
        && function
            .entry_claims
            .iter()
            .zip(&function.abi.parameters)
            .all(|(claim, parameter)| {
                claim.input == parameter.semantic.place && claim.path.is_empty()
            })
}
