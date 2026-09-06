//! Exact establishment custody for direct Fused fields of one selected
//! `ProgramEntry` receiver.

use super::*;

pub fn derive_fused_program_entry_establishments(
    checked: &CheckedTrees,
    source: &program_entry_plan::SelectedProgramEntrySourceSignature,
    selected: &[SelectedProviderReviewProvenance],
) -> Result<Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>, Vec<Diagnostic>> {
    let Some(receiver_identity) = source.receiver().normalized_type_identity() else {
        return Ok(Vec::new());
    };
    let matching_machines = checked
        .machines()
        .iter()
        .filter(|machine| machine.symbol == source.machine_symbol())
        .collect::<Vec<_>>();
    let [machine] = matching_machines.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry establishment rejoins {} source machines; expected one",
            matching_machines.len(),
        ))]);
    };
    let matching_states = checked
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == source.state_symbol())
        .collect::<Vec<_>>();
    let [state] = matching_states.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry establishment rejoins {} entry states; expected one",
            matching_states.len(),
        ))]);
    };
    let receiver_parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_self)
        .collect::<Vec<_>>();
    let [receiver_parameter] = receiver_parameters.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected provisioned ProgramEntry rejoins {} self receivers; expected one",
            receiver_parameters.len(),
        ))]);
    };
    let attached_symbol = machine.attached_data_symbol;
    if !attached_symbol.is_valid() {
        return Err(vec![Diagnostic::error(
            "selected provisioned ProgramEntry has no attached receiver data",
        )]);
    }
    let owners = checked
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == attached_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry receiver rejoins {} attached data declarations; expected one",
            owners.len(),
        ))]);
    };
    if checked
        .normalized_type_identity(receiver_parameter.type_reference)
        .as_str()
        != receiver_identity
    {
        return Err(vec![Diagnostic::error(
            "selected ProgramEntry receiver identity drifted before Fused establishment",
        )]);
    }
    let mut diagnostics = Vec::new();
    let mut service_fields = Vec::new();
    for member in checked.data_members(owner) {
        let DataMember::Field(field) = member else {
            continue;
        };
        match typed_trees::service::classify_exact_bound_service_carrier(
            checked,
            field.type_reference,
        ) {
            Ok(Some(carrier)) => service_fields.push((field, carrier)),
            Ok(None) => {}
            Err(reason) => diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry field `{}::{}` has invalid Service establishment shape: {reason}",
                owner.name, field.name,
            ))),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    if service_fields.is_empty() {
        return Ok(Vec::new());
    }

    let mut attachment_identities = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .filter(|plan| plan.machine == machine.symbol && plan.state == state.symbol)
        .filter_map(|plan| plan.attachment_type_identity.clone())
        .chain(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter()
                .filter(|plan| {
                    plan.machine == machine.symbol
                        && plan.states.iter().any(|plan| plan.state == state.symbol)
                })
                .map(|plan| plan.attachment_type_identity.clone()),
        )
        .collect::<Vec<_>>();
    attachment_identities.sort();
    attachment_identities.dedup();
    let [attachment_type_identity] = attachment_identities.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry establishment rejoins {} Terminal attachment identities; expected one",
            attachment_identities.len(),
        ))]);
    };

    let structural_types = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .filter(|plan| plan.identity == *attachment_type_identity)
        .collect::<Vec<_>>();
    let [structural_type] = structural_types.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry receiver `{receiver_identity}` with attachment `{attachment_type_identity}` rejoins {} Terminal structural types; expected one (available: {})",
            structural_types.len(),
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .map(|plan| plan.identity.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ))]);
    };
    let CheckedUnitStructuralTypeShape::Record { fields } = &structural_type.shape else {
        return Err(vec![Diagnostic::error(
            "selected ProgramEntry receiver is not one exact Terminal record",
        )]);
    };

    let mut rows = Vec::new();
    for (field, carrier) in service_fields {
        let field_identity = data_field_identity(field);
        let matching_fields = fields
            .iter()
            .filter(|candidate| candidate.identity == field_identity)
            .collect::<Vec<_>>();
        let [checked_field] = matching_fields.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` rejoins {} Terminal fields; expected one",
                owner.name,
                field.name,
                matching_fields.len(),
            )));
            continue;
        };
        let CheckedUnitStructuralFieldType::FusedServiceBacked {
            provider_type_identity,
            erasure,
        } = &checked_field.field_type
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` lacks exact Fused Terminal custody",
                owner.name, field.name,
            )));
            continue;
        };
        let carrier_type_identity = checked
            .normalized_type_identity(field.type_reference)
            .into_string();
        if provider_type_identity != &carrier_type_identity
            || erasure.requirement != carrier.requirement
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` substituted its carrier or requirement",
                owner.name, field.name,
            )));
            continue;
        }
        let Some(authorization) = checked.fused_service_erasure(carrier.requirement) else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` lacks Fused erasure authority",
                owner.name, field.name,
            )));
            continue;
        };
        if authorization.provider_plan_digest != erasure.provider_plan_digest {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` substituted its selected plan digest",
                owner.name, field.name,
            )));
            continue;
        }
        let Some(requirement) = checked
            .traits()
            .iter()
            .find(|definition| definition.is_boundary && definition.symbol == carrier.requirement)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` lost its boundary requirement",
                owner.name, field.name,
            )));
            continue;
        };
        let Some(schema) = provider_planning::service_schema::from_typed(checked, requirement)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` cannot reconstruct its schema",
                owner.name, field.name,
            )));
            continue;
        };
        let matching_plans = selected
            .iter()
            .filter(|candidate| {
                candidate.plan.schema == schema
                    && candidate.plan.identity_digest().as_bytes()
                        == &authorization.provider_plan_digest
                    && candidate.selected_by.composition_mode() == Ok(CompositionMode::Fused)
            })
            .collect::<Vec<_>>();
        let [selected_plan] = matching_plans.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` rejoins {} exact Fused provider plans; expected one",
                owner.name,
                field.name,
                matching_plans.len(),
            )));
            continue;
        };
        let Some(bound_domain) = checked
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == carrier.bound_domain)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Service field `{}::{}` lost its Bound domain",
                owner.name, field.name,
            )));
            continue;
        };
        let carrier_base_identity = unconstrained_type_identity(checked, field.type_reference);
        let row = program_entry_plan::ProgramEntryFusedServiceEstablishment::new(
            source.identity(),
            source.target_slot(),
            receiver_identity.to_owned(),
            attachment_type_identity.clone(),
            field_identity,
            carrier_type_identity,
            carrier_base_identity,
            bound_domain.name.as_str().to_owned(),
            program_entry_plan::ProgramEntryFusedServiceEstablishment::requirement_identity_for_schema(&schema),
            schema.identity_digest(),
            selected_plan.plan.identity_digest(),
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        rows.push(row);
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    rows.sort_by(|left, right| left.field_identity().cmp(right.field_identity()));
    if rows
        .windows(2)
        .any(|pair| pair[0].field_identity() == pair[1].field_identity())
    {
        return Err(vec![Diagnostic::error(
            "selected ProgramEntry repeats one Fused Service establishment field",
        )]);
    }
    Ok(rows)
}

fn unconstrained_type_identity(
    checked: &CheckedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> String {
    while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
        checked.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    checked
        .normalized_type_identity(type_reference)
        .into_string()
}
