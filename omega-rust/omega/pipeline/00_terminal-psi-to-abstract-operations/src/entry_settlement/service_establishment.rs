//! Independent native-settlement replay for selected Fused application roots.

use super::{NativeProgramEntrySettlement, NativeProgramEntrySettlementError};

pub(super) fn validate_terminal_rows(
    module: &terminal_psi::TerminalModule,
    settlement: NativeProgramEntrySettlement<'_>,
) -> Result<(), NativeProgramEntrySettlementError> {
    let rows = settlement.fused_service_establishments();
    if rows.is_empty() {
        return Ok(());
    }
    let Some(receiver_identity) = settlement.source().receiver().normalized_type_identity() else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    let entry_machines = module
        .machines
        .iter()
        .filter(|machine| machine.id == module.entry)
        .collect::<Vec<_>>();
    let [entry] = entry_machines.as_slice() else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    let Some(attachment) = entry.attachment else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    let attachment_types = module
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == attachment)
        .collect::<Vec<_>>();
    let [attachment_type] = attachment_types.as_slice() else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    let terminal_psi::StructuralTypeShape::Record { fields } = &attachment_type.shape else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    // The source receiver is semantic identity while the attachment names the
    // retained Terminal structure. Each rejoins the row independently; they
    // are not required to share one spelling across package compilation.
    if attachment_type.identity != rows[0].attachment_type_identity()
        || rows.iter().any(|row| {
            row.source_signature_identity() != settlement.source().identity()
                || row.target_slot() != settlement.source().target_slot()
                || row.receiver_type_identity() != receiver_identity
                || row.attachment_type_identity() != attachment_type.identity
        })
    {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    }
    for row in rows {
        // Each route rejoins by walking its own record-field path: every
        // intermediate segment must stay an exact `Structural` record child,
        // and the leaf must land on the erased carrier the row names.
        let mut current_fields = fields.as_slice();
        for (index, segment) in row.field_path().iter().enumerate() {
            let matching_fields = current_fields
                .iter()
                .filter(|field| field.identity == *segment)
                .collect::<Vec<_>>();
            let [field] = matching_fields.as_slice() else {
                return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
            };
            if index + 1 == row.field_path().len() {
                if !matches!(
                    &field.field_type,
                    terminal_psi::StructuralFieldType::Erased { type_identity }
                        if type_identity == row.carrier_type_identity()
                ) {
                    return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
                }
                break;
            }
            let terminal_psi::StructuralFieldType::Structural(child) = &field.field_type else {
                return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
            };
            let nested = module
                .structural_types
                .iter()
                .filter(|declaration| declaration.id == *child)
                .collect::<Vec<_>>();
            let [nested_declaration] = nested.as_slice() else {
                return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
            };
            current_fields = match &nested_declaration.shape {
                terminal_psi::StructuralTypeShape::Record { fields }
                | terminal_psi::StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => {
                    return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
                }
            };
        }
    }
    Ok(())
}

pub fn validate_for_artifact_and_selected_plans(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    settlement: NativeProgramEntrySettlement<'_>,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<(), NativeProgramEntrySettlementError> {
    artifact.validate().map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    validate_terminal_rows(&module, settlement)?;
    for row in settlement.fused_service_establishments() {
        let matching_plans = selected_provider_plans
            .plans()
            .iter()
            .filter(|plan| {
                program_entry_plan::ProgramEntryFusedServiceEstablishment::requirement_identity_for_schema(&plan.schema)
                    == row.requirement_identity()
                    && plan.schema.identity_digest() == row.service_schema_digest()
                    && plan.identity_digest() == row.selected_provider_plan_digest()
            })
            .count();
        if matching_plans != 1 {
            return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
        }
    }
    Ok(())
}
