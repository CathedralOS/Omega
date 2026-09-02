//! Exact provider-attachment requirements shared by Unit plan families.

use super::*;

pub(super) fn checked_provider_attachment_requirements(
    program: &TypedTrees,
    shapes: &ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    attachment_type_identity: &str,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    calls: &[psi_checked_trees::FlowCallFact],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<CheckedProviderAttachmentRequirementPlan>> {
    let attachment = shapes.types.get(attachment_type_identity)?;
    let CheckedUnitStructuralTypeShape::Record { fields } = &attachment.shape else {
        return Some(Vec::new());
    };
    let provider_fields = fields
        .iter()
        .filter_map(|field| match &field.field_type {
            CheckedUnitStructuralFieldType::ProviderBacked {
                provider_type_identity,
            }
            | CheckedUnitStructuralFieldType::FusedServiceBacked {
                provider_type_identity,
                ..
            } => Some((field, provider_type_identity)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(field, provider_type_identity)] = provider_fields.as_slice() else {
        return provider_fields.is_empty().then(Vec::new);
    };
    let call_operations = operations
        .iter()
        .filter(|operation| {
            !matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
                    | CheckedUnitEffectOperationPlan::ReturnUnit { .. }
            )
        })
        .collect::<Vec<_>>();
    if field.identity.starts_with('#')
        || !structural_parameters.is_empty()
        || call_operations.is_empty()
    {
        return None;
    }

    let attached_name = machine.attached_data.as_ref()?;
    let attached = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)?;
    let provider_symbol = program.data_members(attached).iter().find_map(|member| {
        let DataMember::Field(source_field) = member else {
            return None;
        };
        if source_field.name.as_str() != field.identity {
            return None;
        }
        psi_typed_trees::service::exact_bound_service_requirement(
            program,
            source_field.type_reference,
        )
        .or_else(|| {
            match program
                .type_reference_table
                .type_reference(source_field.type_reference)
            {
                TypeReferenceNode::Named { symbol, .. }
                | TypeReferenceNode::DynamicTrait { symbol, .. } => Some(*symbol),
                _ => None,
            }
        })
    })?;
    let provider = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == provider_symbol && definition.is_boundary)?;
    let provider_requirements = program
        .trait_machine_signatures(provider)
        .iter()
        .map(|requirement| requirement.symbol)
        .collect::<Vec<_>>();

    let mut requirements = Vec::with_capacity(call_operations.len());
    for operation in call_operations {
        let coordinate = match operation {
            CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::PortWrite { coordinate, .. } => coordinate,
            _ => return None,
        };
        let matching_calls = calls
            .iter()
            .filter(|call| {
                u32::try_from(call.statement_index).ok() == Some(coordinate.statement_index)
                    && u32::try_from(call.call_ordinal).ok() == Some(coordinate.call_ordinal)
            })
            .collect::<Vec<_>>();
        let [call] = matching_calls.as_slice() else {
            return None;
        };
        if !provider_requirements.contains(&call.target_symbol) {
            return None;
        }
        let call_site = crate::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        if !provider_attachment_receiver_matches(program, machine, &call_site, provider.symbol) {
            return None;
        }
        if !matches!(operation,
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { target_machine, .. }
                if *target_machine == call.target_symbol)
        {
            return None;
        }
        requirements.push(CheckedProviderAttachmentRequirementPlan {
            field_identity: field.identity.clone(),
            provider_type_identity: provider_type_identity.to_string(),
            boundary: call.target_symbol,
        });
    }
    requirements.sort_by_key(|requirement| {
        (
            requirement.boundary.arena_index(),
            requirement.boundary.generation(),
        )
    });
    requirements.dedup_by_key(|requirement| requirement.boundary);
    Some(requirements)
}

pub(super) fn checked_composed_provider_attachment_requirements(
    program: &TypedTrees,
    shapes: &ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    attachment_type_identity: &str,
    leaves: &[(
        &psi_typed_trees::state::State,
        &[psi_checked_trees::FlowCallFact],
        &[CheckedUnitEffectOperationPlan],
    )],
) -> Option<Vec<CheckedProviderAttachmentRequirementPlan>> {
    let mut requirements = Vec::new();
    for (state, calls, operations) in leaves.iter().copied() {
        requirements.extend(checked_provider_attachment_requirements(
            program,
            shapes,
            machine,
            state,
            attachment_type_identity,
            &[],
            calls,
            operations,
        )?);
    }
    requirements.sort_by_key(|requirement| {
        (
            requirement.field_identity.clone(),
            requirement.boundary.arena_index(),
            requirement.boundary.generation(),
        )
    });
    requirements.dedup();
    Some(requirements)
}
