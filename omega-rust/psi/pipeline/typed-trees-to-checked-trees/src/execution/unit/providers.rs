//! Exact provider-attachment requirements shared by Unit plan families.
use std::collections::BTreeSet;

use super::{
    CheckedProviderAttachmentRequirementPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypeShape, DataMember,
    TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::provider_attachment_receiver_matches;

pub(super) fn checked_provider_attachment_requirements(
    program: &TypedTrees,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    attachment_type_identity: &str,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    calls: &[checked_trees::FlowCallFact],
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
    // Ordinary callees own their direct provider requirements, including when
    // they borrow this receiver. Receiver loans do not forward attachment roots.
    // Select boundary operations directly: a list of unrelated operations to
    // exclude makes every new local or ordinary result call an accidental
    // provider obligation. Their executable callees retain their own closure.
    let call_operations = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                    | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                    | CheckedUnitEffectOperationPlan::PortWrite { .. }
            )
        })
        .collect::<Vec<_>>();
    // A routed receiver carries ordinary structural arguments beside the
    // attached provider field, so non-self parameters are no longer blanket
    // rejections. The roster below still names `self.<field>` receivers only,
    // so a parameter must not be a second provider surface: neither a fused
    // `Service` receipt nor a carrier whose own shape holds a provider-backed
    // field can cross here as an unspecialized argument.
    if field.identity.starts_with('#')
        || structural_parameters
            .iter()
            .any(|parameter| !parameter.is_self && parameter_is_provider_carrier(shapes, parameter))
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
        typed_trees::service::exact_bound_service_requirement(program, source_field.type_reference)
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
            CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
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
            // A direct call to a top-level `boundary requirement` (or a
            // boundary / admission-claim machine) resolves to that machine's
            // entry state, not to a trait signature: the call settles through
            // the requirement's own retained boundary seam and consumes no
            // attached provider field. A boundary call targeting a signature —
            // any receiver-carried provider use — still requires the field.
            let targets_machine = match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { target_machine, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    target_machine, ..
                } => program
                    .machines()
                    .iter()
                    .any(|candidate| candidate.symbol == *target_machine),
                _ => false,
            };
            if targets_machine {
                continue;
            }
            return None;
        }
        let call_site = crate::semantic_calls::find_call_site(
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
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { target_machine, .. }
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

/// Whether one ordinary argument is itself a provider surface. A fused
/// `Service` receipt parameter is a provider carrier outright, and a carrier
/// whose own shape holds a provider-backed field smuggles an unspecialized
/// provider through the argument lane: the requirement roster names the
/// attached `self.<field>` receiver only.
fn parameter_is_provider_carrier(
    shapes: &ShapeCollector<'_>,
    parameter: &CheckedUnitStructuralParameterPlan,
) -> bool {
    parameter.fused_service_erasure.is_some()
        || type_carries_provider(
            shapes,
            parameter.type_identity.as_str(),
            &mut BTreeSet::new(),
        )
}

/// Whether the shape named by `type_identity` holds a provider-backed field,
/// following structural field and element references so a provider buried
/// inside a nested carrier still counts. Erased fields own no runtime carrier,
/// so their types are not followed.
fn type_carries_provider<'a>(
    shapes: &'a ShapeCollector<'_>,
    type_identity: &'a str,
    visited: &mut BTreeSet<&'a str>,
) -> bool {
    if !visited.insert(type_identity) {
        return false;
    }
    let Some(plan) = shapes.types.get(type_identity) else {
        return false;
    };
    if let CheckedUnitStructuralTypeShape::FixedArray {
        element_type_identity,
        ..
    } = &plan.shape
    {
        return type_carries_provider(shapes, element_type_identity, visited);
    }
    shape_fields(&plan.shape).any(|field| match &field.field_type {
        CheckedUnitStructuralFieldType::ProviderBacked { .. }
        | CheckedUnitStructuralFieldType::FusedServiceBacked { .. } => true,
        CheckedUnitStructuralFieldType::Structural { type_identity } => {
            type_carries_provider(shapes, type_identity, visited)
        }
        _ => false,
    })
}

/// Every field position of a record, sum, or mixed shape in one flat view, so
/// the provider-carrier walk does not repeat the case fan-out at each level.
fn shape_fields(
    shape: &CheckedUnitStructuralTypeShape,
) -> impl Iterator<Item = &CheckedUnitStructuralFieldPlan> + '_ {
    let (fields, cases): (
        &[CheckedUnitStructuralFieldPlan],
        &[checked_trees::CheckedUnitStructuralCasePlan],
    ) = match shape {
        CheckedUnitStructuralTypeShape::Record { fields } => (fields.as_slice(), &[]),
        CheckedUnitStructuralTypeShape::Sum { cases } => (&[], cases.as_slice()),
        CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
            (fields.as_slice(), cases.as_slice())
        }
        _ => (&[], &[]),
    };
    fields
        .iter()
        .chain(cases.iter().flat_map(|case| case.fields.iter()))
}

pub(super) fn checked_composed_provider_attachment_requirements(
    program: &TypedTrees,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    attachment_type_identity: &str,
    leaves: &[(
        &typed_trees::state::State,
        &[checked_trees::FlowCallFact],
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
