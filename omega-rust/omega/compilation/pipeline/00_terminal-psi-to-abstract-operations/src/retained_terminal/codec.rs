//! Compiler-private native derivation inputs. Records are optional storage,
//! never a portable IR, installation receipt or serialized validation wrapper.
//! Same-invocation production keeps its ordinary borrowed inputs.
mod admissions;
mod entry;
mod records;
mod selection;
mod storage;
mod value;
mod vocabulary;

use super::{
    RetainedTerminalArtifact, TerminalNativeProposalRequest,
    project_terminal_native_realization_proposal,
};
use crate::provider_admission::TerminalAuthorityPermissionPolicy;
use diagnostics::Diagnostic;
use entry::EntryRecord;
use std::sync::Arc;
use value::{RecordError, RecordLimits, RecordReader, RecordValue, RecordWriter, write_values};

// Entry records no longer contain synthetic application-record service fields.
// Reject older private records before reading their differently shaped payload.
const RECORD_SCHEMA: &[u8; 8] = b"OMGNTI03";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_entry_field_record_schema_is_rejected() {
        let mut input =
            RecordReader::new(b"OMGNTI01", RecordLimits::default()).expect("bounded old schema");
        let Err(error) = DerivationInputs::read(&mut input) else {
            panic!("retired record schema accepted");
        };
        assert_eq!(error.reason, "foreign native derivation record schema");
    }
}

/// Capture only when a caller requests persistence. This does not admit or
/// execute the proposed inputs; the original producer continues normally.
pub fn retain_native_derivation_inputs(
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical_adapter_requested: bool,
    provider_proposals: &[backend_plan::TerminalTargetProviderProposal],
    request: &TerminalNativeProposalRequest<'_>,
    accepted_package_permissions: &TerminalAuthorityPermissionPolicy,
) -> Result<Vec<u8>, String> {
    capture(
        checked_entry,
        physical_adapter_requested,
        provider_proposals,
        request,
        accepted_package_permissions,
    )
    .map_err(|reason| reason.to_string())
}

fn capture(
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical_adapter_requested: bool,
    provider_proposals: &[backend_plan::TerminalTargetProviderProposal],
    request: &TerminalNativeProposalRequest<'_>,
    accepted_package_permissions: &TerminalAuthorityPermissionPolicy,
) -> Result<Vec<u8>, RecordError> {
    let TerminalNativeProposalRequest {
        evaluated_binding_identities,
        direct_boundary_leaves,
        provider_declarations,
        target_profile,
        family_realizations,
        boundaries,
        native_target,
        program_entry,
        application,
        optimizations,
        external_bindings,
        behavior_exclusions,
        package_terminal_authority_permissions,
        package_terminal_authority_permission_identities,
        callback_placements,
        x86_scalar_fma_provider,
    } = request;
    let mut output = RecordWriter::new(RecordLimits::default());
    output.extend(RECORD_SCHEMA)?;
    evaluated_binding_identities.write(&mut output)?;
    checked_entry.write(&mut output)?;
    physical_adapter_requested.write(&mut output)?;
    write_values(provider_proposals, &mut output)?;
    output.sequence(direct_boundary_leaves.len())?;
    for leaf in direct_boundary_leaves {
        leaf.write(&mut output)?;
    }
    write_values(provider_declarations, &mut output)?;
    target_profile.write(&mut output)?;
    write_values(family_realizations, &mut output)?;
    boundaries.selected_provider_plans.write(&mut output)?;
    output.sequence(boundaries.selected_provider_provenance.len())?;
    for crate::provider_admission::CompilerIntrinsicPlanProvenance {
        plan,
        semantic_schema,
        row_executions,
    } in &boundaries.selected_provider_provenance
    {
        plan.write(&mut output)?;
        semantic_schema.write(&mut output)?;
        output.sequence(row_executions.len())?;
        for execution in *row_executions {
            execution
                .filter(|execution| !execution.is_numeric())
                .write(&mut output)?;
        }
    }
    boundaries.calling_applications.write(&mut output)?;
    native_target.write(&mut output)?;
    match program_entry {
        None => 0u8.write(&mut output)?,
        Some(entry) => {
            1u8.write(&mut output)?;
            EntryRecord::write_borrowed(*entry, &mut output)?;
        }
    }
    application.write(&mut output)?;
    optimizations.write(&mut output)?;
    write_values(external_bindings, &mut output)?;
    write_values(behavior_exclusions, &mut output)?;
    package_terminal_authority_permissions.write(&mut output)?;
    package_terminal_authority_permission_identities.write(&mut output)?;
    callback_placements.write(&mut output)?;
    x86_scalar_fma_provider.write(&mut output)?;
    write_values(accepted_package_permissions.rows(), &mut output)?;
    accepted_package_permissions
        .identities()
        .write(&mut output)?;
    Ok(output.finish())
}

struct IntrinsicPlanRecord {
    plan: effects::provider_plan::ProviderPlan,
    semantic_schema: effects::provider_plan::ServiceSchema,
    row_executions: Vec<Option<effects::CompilerIntrinsicExecutionIdentity>>,
}
records::record_fields!(IntrinsicPlanRecord {
    plan,
    semantic_schema,
    row_executions
});

struct DerivationInputs {
    evaluated_binding_identities: Arc<terminal_psi::TerminalIdentityTable>,
    checked_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical_adapter_requested: bool,
    provider_proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
    leaves: Vec<backend_plan::DirectBoundaryLeaf>,
    provider_declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
    profile: Option<target::TargetProfile>,
    families: Vec<backend_plan::SelectedFamilyRealizationSource>,
    selected: effects::SelectedProviderPlanFacts,
    intrinsic_provenance: Vec<IntrinsicPlanRecord>,
    calling: Vec<calling_conventions::BoundaryCallingApplication>,
    native: Option<target::NativeTarget>,
    entry: Option<EntryRecord>,
    application: target::NativeApplicationMetadata,
    optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    external: Vec<calling_conventions::ExternalBindingRow>,
    exclusions: Vec<effects::AuthoredBehaviorExclusion>,
    permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    permission_identities: terminal_psi::TerminalIdentityTable,
    callbacks: Vec<backend_plan::NativeCallbackPlacement>,
    fma: Option<target::AdmittedX86ScalarFmaProvider>,
    accepted_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    accepted_identities: terminal_psi::TerminalIdentityTable,
}
impl DerivationInputs {
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        if input.take(RECORD_SCHEMA.len())? != RECORD_SCHEMA {
            return Err(input.error("foreign native derivation record schema"));
        }
        let evaluated_binding_identities =
            Arc::new(terminal_psi::TerminalIdentityTable::read(input)?);
        input.context.binding_identities = Some(Arc::clone(&evaluated_binding_identities));
        let checked_entry = RecordValue::read(input)?;
        let physical_adapter_requested = RecordValue::read(input)?;
        let provider_proposals = RecordValue::read(input)?;
        let leaves = RecordValue::read(input)?;
        let provider_declarations = RecordValue::read(input)?;
        let profile = RecordValue::read(input)?;
        let families = RecordValue::read(input)?;
        let selected = RecordValue::read(input)?;
        let intrinsic_provenance = RecordValue::read(input)?;
        let calling = RecordValue::read(input)?;
        let native = RecordValue::read(input)?;
        let entry = match u8::read(input)? {
            0 => None,
            1 => Some(EntryRecord::read(input)?),
            _ => return Err(input.error("invalid native entry presence tag")),
        };
        Ok(Self {
            evaluated_binding_identities,
            checked_entry,
            physical_adapter_requested,
            provider_proposals,
            leaves,
            provider_declarations,
            profile,
            families,
            selected,
            intrinsic_provenance,
            calling,
            native,
            entry,
            application: RecordValue::read(input)?,
            optimizations: RecordValue::read(input)?,
            external: RecordValue::read(input)?,
            exclusions: RecordValue::read(input)?,
            permissions: RecordValue::read(input)?,
            permission_identities: RecordValue::read(input)?,
            callbacks: RecordValue::read(input)?,
            fma: RecordValue::read(input)?,
            accepted_permissions: RecordValue::read(input)?,
            accepted_identities: RecordValue::read(input)?,
        })
    }
}

/// Recover candidates and re-run the source-free native producer. Fresh entry
/// backing, host bindings and receiving permissions are supplied by the caller
/// through the ordinary execution route after this operation.
pub fn recover_native_derivation_inputs(
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    bytes: &[u8],
) -> Result<(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy), Vec<Diagnostic>> {
    let error = |reason: String| vec![Diagnostic::error(reason)];
    let mut reader = RecordReader::new(bytes, RecordLimits::default())
        .map_err(|reason| error(reason.to_string()))?;
    let inputs = DerivationInputs::read(&mut reader).map_err(|reason| error(reason.to_string()))?;
    reader
        .finish()
        .map_err(|reason| error(reason.to_string()))?;
    let DerivationInputs {
        evaluated_binding_identities,
        checked_entry,
        physical_adapter_requested,
        provider_proposals,
        leaves,
        provider_declarations,
        profile,
        families,
        selected,
        intrinsic_provenance,
        calling,
        native,
        entry,
        application,
        optimizations,
        external,
        exclusions,
        permissions,
        permission_identities,
        callbacks,
        fma,
        accepted_permissions,
        accepted_identities,
    } = inputs;
    crate::provider_admission::terminal_authority_permissions::validate_package_terminal_authority_permission_custody(
        accepted_permissions.iter(), &accepted_identities)?;
    let accepted = crate::provider_admission::terminal_authority_permission_policy_with_rows(
        accepted_permissions,
        accepted_identities,
    )
    .map_err(|reason| {
        error(format!(
            "recovered accepted package permission candidates: {reason:?}"
        ))
    })?;
    let entry_adapter_requested = entry
        .as_ref()
        .is_some_and(|entry| entry.physical_adapter_requested);
    let request = TerminalNativeProposalRequest {
        evaluated_binding_identities: &evaluated_binding_identities,
        direct_boundary_leaves: leaves.iter().collect(),
        provider_declarations: &provider_declarations,
        target_profile: profile,
        family_realizations: &families,
        boundaries: crate::BoundaryApplicationSettlement {
            selected_provider_plans: &selected,
            selected_provider_provenance: intrinsic_provenance
                .iter()
                .map(
                    |row| crate::provider_admission::CompilerIntrinsicPlanProvenance {
                        plan: &row.plan,
                        semantic_schema: &row.semantic_schema,
                        row_executions: &row.row_executions,
                    },
                )
                .collect(),
            calling_applications: calling,
        },
        native_target: native,
        program_entry: entry.as_ref().map(EntryRecord::borrowed),
        application,
        optimizations,
        external_bindings: &external,
        behavior_exclusions: &exclusions,
        package_terminal_authority_permissions: permissions,
        package_terminal_authority_permission_identities: permission_identities,
        callback_placements: callbacks,
        x86_scalar_fma_provider: fma,
    };
    let mut proposal =
        project_terminal_native_realization_proposal(&artifact, checked_entry, request)?;
    if physical_adapter_requested || entry_adapter_requested {
        proposal
            .bind_physical_adapter(&artifact)
            .map_err(|reason| error(reason.to_owned()))?;
    }
    Ok((
        RetainedTerminalArtifact::new_with_native_realization_proposal(
            artifact,
            proposal.with_target_provider_proposals(provider_proposals),
        ),
        accepted,
    ))
}
