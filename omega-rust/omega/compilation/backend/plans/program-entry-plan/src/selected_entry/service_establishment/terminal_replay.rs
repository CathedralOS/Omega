//! Replay established Fused services against a Terminal receiver and selected plans.

use super::ProgramEntryFusedServiceEstablishment;
use crate::SelectedProgramEntrySourceSignature;

/// Check the exact receiver-field routes, source signature, target slot,
/// erased carriers and selected provider commitments for each establishment.
///
/// The caller first validates the canonical Terminal artifact and its selected
/// entry receipt. This check does not grant provider or execution authority.
pub fn validate_terminal_fused_service_establishments(
    module: &terminal_psi::TerminalModule,
    source: &SelectedProgramEntrySourceSignature,
    eligibility: Option<&terminal_psi::CheckedProgramEntryReceiverEligibility>,
    rows: &[ProgramEntryFusedServiceEstablishment],
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
) -> Result<(), &'static str> {
    if rows.is_empty() {
        return Ok(());
    }
    let Some(receiver_identity) = source.receiver().normalized_type_identity() else {
        return Err("Terminal proposal gives a free ProgramEntry a Fused root establishment");
    };
    let entry_machines = module
        .machines
        .iter()
        .filter(|machine| machine.id == module.entry)
        .collect::<Vec<_>>();
    let [entry_machine] = entry_machines.as_slice() else {
        return Err("Terminal proposal has no unique entry machine for root establishment");
    };
    let eligibility =
        eligibility.ok_or("Terminal proposal root establishment lost its checked context")?;
    if source.receiver().normalized_type_identity()
        != Some(eligibility.source_receiver_type_identity())
    {
        return Err("Terminal proposal root establishment substituted its source context");
    }
    let attachment = eligibility.terminal_receiver_type();
    match eligibility.projection() {
        terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
            terminal_self,
            source_position,
        } => {
            let mut parameters = entry_machine
                .structural_parameters
                .iter()
                .filter(|parameter| parameter.place == terminal_self);
            let parameter = parameters
                .next()
                .ok_or("Terminal proposal root establishment lost its context parameter")?;
            if parameters.next().is_some()
                || source.receiver().source_position() != Some(source_position)
                || parameter.structural_type != attachment
                || parameter.access != terminal_psi::StructuralAccess::MutableBorrow
                || parameter.is_self != entry_machine.attachment.is_some()
                || entry_machine
                    .attachment
                    .is_some_and(|identity| identity != attachment)
            {
                return Err(
                    "Terminal proposal root establishment substituted its context parameter",
                );
            }
        }
        terminal_psi::CheckedProgramEntryReceiverProjection::Erased { source_position } => {
            if source.receiver().source_position() != Some(source_position)
                || entry_machine.attachment != Some(attachment)
                || entry_machine
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.is_self)
            {
                return Err("Terminal proposal root establishment substituted its erased context");
            }
        }
    }
    let attachment_types = module
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == attachment)
        .collect::<Vec<_>>();
    let [attachment_type] = attachment_types.as_slice() else {
        return Err("Terminal proposal root establishment has no unique attachment type");
    };
    let terminal_psi::StructuralTypeShape::Record { fields } = &attachment_type.shape else {
        return Err("Terminal proposal root establishment attachment is not a record");
    };
    if attachment_type.identity != eligibility.owned_receiver_type_identity()
        || attachment_type.identity != rows[0].attachment_type_identity()
        || rows
            .iter()
            .any(|row| row.attachment_type_identity() != attachment_type.identity)
    {
        return Err("Terminal proposal root establishment substituted its receiver type");
    }
    let mut field_routes = std::collections::BTreeSet::new();
    for row in rows {
        if row.source_signature_identity() != source.identity()
            || row.target_slot() != source.target_slot()
            || row.receiver_type_identity() != receiver_identity
            || !field_routes.insert(row.field_path().to_vec())
        {
            return Err("Terminal proposal root establishment drifted from ProgramEntry custody");
        }
        // Replay the route through the record-field tree: each
        // intermediate segment is a nested record field; the leaf is the
        // erased service field rejoining its carrier.
        let mut current_fields = fields.as_slice();
        let route_length = row.field_path().len();
        for (position, segment) in row.field_path().iter().enumerate() {
            let matching_fields = current_fields
                .iter()
                .filter(|field| field.identity == *segment)
                .collect::<Vec<_>>();
            let [field] = matching_fields.as_slice() else {
                return Err("Terminal proposal root establishment has no unique receiver field");
            };
            if position + 1 == route_length {
                if !matches!(
                    &field.field_type,
                    terminal_psi::StructuralFieldType::Erased { type_identity }
                        if type_identity == row.carrier_type_identity()
                ) {
                    return Err(
                        "Terminal proposal root establishment substituted its erased carrier",
                    );
                }
            } else {
                let terminal_psi::StructuralFieldType::Structural(child) = field.field_type else {
                    return Err(
                        "Terminal proposal root establishment has no unique receiver field",
                    );
                };
                let nested_types = module
                    .structural_types
                    .iter()
                    .filter(|declaration| declaration.id == child)
                    .collect::<Vec<_>>();
                let [nested_type] = nested_types.as_slice() else {
                    return Err(
                        "Terminal proposal root establishment has no unique nested record type",
                    );
                };
                current_fields = match &nested_type.shape {
                    terminal_psi::StructuralTypeShape::Record { fields }
                    | terminal_psi::StructuralTypeShape::Mixed { fields, .. } => fields,
                    _ => {
                        return Err(
                            "Terminal proposal root establishment route leaves its record tree",
                        );
                    }
                };
            }
        }
        let matching_plans = selected_provider_plans
            .iter()
            .filter(|plan| {
                ProgramEntryFusedServiceEstablishment::requirement_identity_for_schema(&plan.schema)
                    == row.requirement_identity()
                    && plan.schema.identity_digest() == row.service_schema_digest()
                    && plan.identity_digest() == row.selected_provider_plan_digest()
            })
            .count();
        if matching_plans != 1 {
            return Err(
                "Terminal proposal root establishment has no unique selected provider plan",
            );
        }
    }
    Ok(())
}
