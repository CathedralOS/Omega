//! Exact provider-attachment requirements shared by Unit plan families.
use std::collections::BTreeSet;

use super::{
    CheckedProviderAttachmentRequirementPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypeShape, DataMember,
};
use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::calls::{
    parameter_binding_receiver_sources, provider_attachment_receiver_matches,
};

pub(super) fn checked_provider_attachment_requirements(
    program: &dyn ::validation::CheckingInput,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    attachment_type_identity: typed_trees::type_identity::TypeIdentityId,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    calls: &[checked_trees::FlowCallFact],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<CheckedProviderAttachmentRequirementPlan>> {
    let attachment = shapes.types.get(&attachment_type_identity)?;
    let CheckedUnitStructuralTypeShape::Record { fields, .. } = &attachment.shape else {
        return Some(Vec::new());
    };
    let provider_fields = fields
        .iter()
        .filter_map(|field| match &field.field_type {
            CheckedUnitStructuralFieldType::ProviderBacked {
                provider_type_identity,
            }
            | CheckedUnitStructuralFieldType::BindingBacked {
                provider_type_identity,
                ..
            } => Some((field, provider_type_identity)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if provider_fields.is_empty() {
        return Some(Vec::new());
    }
    // Ordinary callees own their direct provider requirements, including when
    // they borrow this receiver. Receiver loans do not forward attachment roots.
    // Select boundary operations directly: a list of unrelated operations to
    // exclude makes every new local or ordinary result call an accidental
    // provider obligation. Their executable callees retain their own closure.
    let call_operations = operations
        .iter()
        .filter(|operation| operation.call().is_some_and(|call| call.is_boundary()))
        .collect::<Vec<_>>();
    // A routed receiver carries ordinary structural arguments beside the
    // attached provider field, so non-self parameters are no longer blanket
    // rejections. Unresolved provider fields still need an attachment route,
    // and whole routed-Service parameters retain their separate custody fence.
    // An ordinary parameter of the exact attachment carrier may use its Fused
    // fields; each occurrence must retain that same attachment-field requirement.
    // An unrelated provider-bearing carrier is not another attachment surface.
    if provider_fields
        .iter()
        .any(|(field, _)| field.identity.spelling().starts_with('#'))
        || structural_parameters.iter().any(|parameter| {
            parameter_is_provider_carrier(shapes, parameter, attachment_type_identity)
        })
    {
        return None;
    }

    let attached = program.data_definition_by_symbol(machine.attached_data_symbol)?;
    // Every provider-backed field claims the boundary calls it answers: a
    // `self.<field>` receiver names its field, and the field's requirement
    // picks the trait whose signatures own each call's target. An entry may
    // carry several service fields at once — a console beside a fused
    // filesystem — so the roster resolves the provider per call rather than
    // admitting exactly one field.
    let mut providers = Vec::with_capacity(provider_fields.len());
    for (field, provider_type_identity) in &provider_fields {
        let provider_symbol = program.data_members(attached).iter().find_map(|member| {
            let source_field = member.as_field()?;
            if source_field.name.as_str() != field.identity.spelling() {
                return None;
            }
            typed_trees::service::service_requirement_declaration(
                program,
                source_field.type_reference,
            )
            .or_else(|| {
                crate::execution::terminal_unit::types::mutable_boundary_trait_symbol(
                    program,
                    source_field.type_reference,
                )
            })
        })?;
        let provider = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == provider_symbol && definition.is_boundary)?;
        providers.push((
            *field,
            *provider_type_identity,
            provider,
            program
                .trait_machine_signatures(provider)
                .iter()
                .map(|requirement| requirement.symbol)
                .collect::<Vec<_>>(),
        ));
    }

    let mut requirements = Vec::with_capacity(call_operations.len());
    for operation in call_operations {
        let boundary = operation.call().filter(|planned| planned.is_boundary());
        let coordinate = boundary?.coordinate;
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
        let call_site = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        );
        let candidates = providers
            .iter()
            .filter(|(_, _, _, provider_requirements)| {
                provider_requirements.contains(&call.target_symbol)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            // A direct call to a top-level `boundary requirement` (or a
            // boundary / admission-claim machine) resolves to that machine's
            // entry state, not to a trait signature: the call settles through
            // the requirement's own retained boundary seam and consumes no
            // attached provider field. A boundary call targeting a signature —
            // any receiver-carried provider use — still requires the field.
            // A direct call to a bodyless external leaf likewise names its
            // realization, not a field: the operation targets the leaf's
            // exact satisfied requirement and the leaf's own selected row
            // settles it. A legacy operator likewise names its own requirement;
            // an unrelated console field must not turn it into a receiver use.
            let has_independent_requirement = boundary.is_some_and(|planned| {
                let target_machine = planned.target.machine;
                program.machine(target_machine).is_some()
                    || (call.target_symbol == target_machine
                        && program.operators().iter().any(|operator| {
                            operator.is_boundary && operator.symbol == target_machine
                        }))
                    || validation::exact_external_leaf_boundary_requirement(
                        program,
                        call.target_symbol,
                    )
                    .is_some_and(|(requirement, _)| requirement == target_machine)
            });
            if has_independent_requirement {
                continue;
            }
            return None;
        }
        let call_site = call_site?;
        // The `self.<field>` receiver names the provider a call answers to:
        // a call claims exactly the field it routes through, not a
        // same-boundary sibling.
        let receiver_field =
            crate::execution::terminal_unit::calls::provider_attachment_receiver_field(
                program, &call_site,
            );
        let parameter_sources = parameter_binding_receiver_sources(
            program,
            state,
            call.statement_index,
            &call_site,
            candidates.first()?.2.symbol,
        );
        let mut covered_sources = parameter_sources
            .as_ref()
            .map(|sources| vec![false; sources.len()]);
        let mut claimed = false;
        for (field, provider_type_identity, provider, _) in candidates {
            let attached_receiver = receiver_field.as_ref().map(|name| name.as_str())
                == Some(field.identity.spelling().as_str())
                && provider_attachment_receiver_matches(
                    program,
                    machine,
                    &call_site,
                    provider.symbol,
                );
            // Provider roots bind attachment type + exact used field + boundary,
            // not the spelling `self`. The ordinary parameter must have that
            // same nominal carrier and project exactly its declared direct field.
            let mut parameter_receiver = false;
            if let Some(sources) = &parameter_sources {
                for (index, (source, place)) in sources.iter().enumerate() {
                    let matches = (|| {
                        let Some(reference) =
                            validation::unwrapped_type_reference(program, source.type_reference)
                        else {
                            return false;
                        };
                        if program.normalized_type_identity(reference) != attachment_type_identity
                            || program
                                .closed_data_definition(reference)
                                .is_none_or(|owner| owner.symbol != attached.symbol)
                        {
                            return false;
                        }
                        // Ordinary per-state plans supply their retained roster here.
                        // Composed collection has no roster; its already-planned call
                        // independently checked the same source parameter position.
                        if !structural_parameters.is_empty()
                            && !structural_parameters.iter().any(|parameter| {
                                parameter.type_identity == attachment_type_identity
                                    && program
                                        .state_parameters(state)
                                        .get(parameter.position as usize)
                                        .is_some_and(|parameter| parameter.symbol == source.symbol)
                            })
                        {
                            return false;
                        }
                        let [facts::PlaceSegment::Field { symbol }] = place.segments.as_slice()
                        else {
                            return false;
                        };
                        program.data_members(attached).iter().any(|member| matches!(member,
                    DataMember::Field(source_field) if source_field.symbol == *symbol
                        && source_field.name.as_str() == field.identity.spelling() && !source_field.relevance.is_erased()))
                    })();
                    if matches {
                        covered_sources.as_mut()?[index] = true;
                        parameter_receiver = true;
                    }
                }
            }
            if !attached_receiver && !parameter_receiver {
                continue;
            }
            if !boundary.is_some_and(|planned| {
                planned.target.machine == call.target_symbol
                    && planned.target.state == call.target_symbol
            }) {
                return None;
            }
            requirements.push(CheckedProviderAttachmentRequirementPlan {
                field_identity: field.identity.clone(),
                provider_type_identity: **provider_type_identity,
                boundary: call.target_symbol,
            });
            claimed = true;
        }
        // Every possible authority origin must have an attachment route. One
        // matching branch cannot authorize another uncovered input carrier.
        if !claimed || covered_sources.is_some_and(|sources| sources.contains(&false)) {
            return None;
        }
    }
    // Canonical roster order is the attachment's field declaration order,
    // then boundary identity — the same key the lowering consumer validates.
    // Sorting by the field's name instead would emit e.g. `beeper` before
    // `console` for `data Main { console, beeper, .. }` and trip the
    // canonical-order check downstream.
    requirements.sort_by_key(|requirement| {
        (
            fields
                .iter()
                .position(|field| field.identity == requirement.field_identity),
            requirement.boundary.arena_index(),
            requirement.boundary.generation(),
        )
    });
    requirements.dedup();
    Some(requirements)
}

/// The attachment's record fields in declaration order, so composed rosters
/// sort by the same canonical key as per-state rosters.
fn attachment_record_fields<'a>(
    shapes: &'a ShapeCollector<'_>,
    attachment_type_identity: typed_trees::type_identity::TypeIdentityId,
) -> &'a [CheckedUnitStructuralFieldPlan] {
    let Some(plan) = shapes.types.get(&attachment_type_identity) else {
        return &[];
    };
    match &plan.shape {
        CheckedUnitStructuralTypeShape::Record { fields, .. } => fields.as_slice(),
        _ => &[],
    }
}

/// Whether an ordinary argument needs a provider route this roster cannot
/// describe. Whole routed-Service parameters keep their distinct receipt fence;
/// ordinary records of the exact attachment type may contain authorized Fused
/// fields, but unrelated carriers and unresolved provider fields remain fenced.
fn parameter_is_provider_carrier(
    shapes: &ShapeCollector<'_>,
    parameter: &CheckedUnitStructuralParameterPlan,
    attachment_type_identity: typed_trees::type_identity::TypeIdentityId,
) -> bool {
    parameter.binding_carrier.is_some()
        || type_carries_unsupported_provider(
            shapes,
            parameter.type_identity,
            parameter.type_identity == attachment_type_identity,
            &mut BTreeSet::new(),
        )
}

/// Whether the shape holds a provider field unavailable through this attachment,
/// following structural field and element references so a provider buried
/// inside a nested carrier still counts. Erased fields own no runtime carrier,
/// so their types are not followed.
fn type_carries_unsupported_provider<'a>(
    shapes: &'a ShapeCollector<'_>,
    type_identity: typed_trees::type_identity::TypeIdentityId,
    allow_fused: bool,
    visited: &mut BTreeSet<typed_trees::type_identity::TypeIdentityId>,
) -> bool {
    if !visited.insert(type_identity) {
        return false;
    }
    let Some(plan) = shapes.types.get(&type_identity) else {
        return false;
    };
    if let CheckedUnitStructuralTypeShape::FixedArray {
        element_type_identity,
        ..
    } = &plan.shape
    {
        return type_carries_unsupported_provider(
            shapes,
            *element_type_identity,
            allow_fused,
            visited,
        );
    }
    shape_fields(&plan.shape).any(|field| match &field.field_type {
        CheckedUnitStructuralFieldType::ProviderBacked { .. } => true,
        CheckedUnitStructuralFieldType::BindingBacked { .. } => !allow_fused,
        CheckedUnitStructuralFieldType::Structural { type_identity } => {
            type_carries_unsupported_provider(shapes, *type_identity, allow_fused, visited)
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
        CheckedUnitStructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[]),
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

/// A fused graph spans sibling machines, so each leaf carries its own owner:
/// the call/operation facts it contributes were recorded under that machine,
/// not the dispatch one whose identity publishes the plan.
pub(super) fn checked_composed_provider_attachment_requirements(
    program: &dyn ::validation::CheckingInput,
    shapes: &ShapeCollector<'_>,
    attachment_type_identity: typed_trees::type_identity::TypeIdentityId,
    leaves: &[(
        &typed_trees::machine::Machine,
        &typed_trees::state::State,
        &[checked_trees::FlowCallFact],
        &[CheckedUnitEffectOperationPlan],
    )],
) -> Option<Vec<CheckedProviderAttachmentRequirementPlan>> {
    let mut requirements = Vec::new();
    for &(owner, state, calls, operations) in leaves.iter() {
        requirements.extend(checked_provider_attachment_requirements(
            program,
            shapes,
            owner,
            state,
            attachment_type_identity,
            &[],
            calls,
            operations,
        )?);
    }
    let fields = attachment_record_fields(shapes, attachment_type_identity);
    requirements.sort_by_key(|requirement| {
        (
            fields
                .iter()
                .position(|field| field.identity == requirement.field_identity),
            requirement.boundary.arena_index(),
            requirement.boundary.generation(),
        )
    });
    requirements.dedup();
    Some(requirements)
}
