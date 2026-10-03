//! Materialize literals and view ranges at their selected call and authored
//! argument position. Owned literal formals receive fresh bounded backing;
//! borrowed formals retain views. A range (`view[a..b]`, `self.items[a..b]`)
//! narrows its source at its own position through the one call-argument
//! range staging both Unit routes share (`byte_subslices`), so a terminal
//! call in a guarded transition arm stages it inside the arm, where only the
//! selected arm evaluates it.
use super::super::{
    CheckedUnitEffectOperationPlan, PlaceId, ValueDeclaration, lookup_type_id, unsupported,
};
use super::{CheckedTrees, LoweringError, catalogs};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::terminal_identities::lookup_domain_id_for_carrier;
use crate::unit::attached_unit::byte_subslices;

/// Completed operands from the shared authored-position schedule. Their slots
/// have already followed any control splits; evaluating them again at the call
/// would repeat side effects and move reads across structural snapshots.
pub(super) struct StagedArguments<'a> {
    pub scalars: &'a [ValueDeclaration],
    pub ranges: &'a [(usize, PlaceId)],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    staged: Option<StagedArguments<'_>>,
    catalogs: &mut catalogs::ComposedCatalogs,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(Option<Vec<ValueDeclaration>>, Vec<PlaceId>), LoweringError> {
    let state_plan = state;
    let state = state.state;
    let arguments = byte_subslices::arguments(operation);
    if !arguments.iter().any(|argument| {
        argument.byte_sequence_literal().is_some() || byte_subslices::range(argument).is_some()
    }) {
        if let Some(staged) = staged {
            return Ok((Some(staged.scalars.to_vec()), Vec::new()));
        }
        let mut calls = catalogs.scalar_calls.emission_context();
        let values = evaluation.arguments(
            checked, machine, state, operation, values, next_value, next_block, next_edge,
            operations, &mut calls,
        )?;
        catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
        return Ok((values, Vec::new()));
    }
    let Some(call) = operation.call() else {
        return unsupported("literal call escaped the composed Unit operation family");
    };
    let target_parameters = super::super::call_parameters::structural_parameters(checked, call)?;
    let (scalar_count, coordinate) = (call.scalar_arguments.len(), call.coordinate);
    // A literal occurrence's qualification evidence is its target parameter's
    // declared domain set; clone the roster so the call machinery can borrow
    // the catalogs mutably below.
    let parameter_qualifications = target_parameters
        .iter()
        .map(|parameter| parameter.qualifications.clone())
        .collect::<Vec<_>>();
    let authored =
        crate::emission::call_source_custody::authored::locate_source(checked, state, coordinate)?;
    let signature = crate::emission::call_source_custody::authored::target_signature(
        checked,
        machine,
        authored.source_target,
    )?;
    let positions = super::super::call_parameters::structural_positions(checked, call, &signature)?
        .into_iter()
        .map(|position| position as usize)
        .collect::<Vec<_>>();
    let scalar_positions = signature
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            checked
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    if positions.len() != arguments.len()
        || scalar_positions.len() != scalar_count
        || positions.iter().any(|position| {
            *position >= signature.parameters.len() || scalar_positions.contains(position)
        })
        || positions.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return unsupported("literal call lost its authored parameter partition");
    }
    let source_value_count = values.len();
    let mut scalar_slots = Vec::new();
    let mut byte_places = Vec::new();
    let mut structural_ordinal = 0;
    for argument_position in 0..signature.parameters.len() {
        if positions.get(structural_ordinal) == Some(&argument_position) {
            let argument = &arguments[structural_ordinal];
            if argument.byte_sequence_literal().is_some() {
                byte_places.push(establish(
                    argument,
                    parameter_qualifications.get(structural_ordinal).ok_or(
                        LoweringError::Unsupported("literal call target parameter is absent"),
                    )?,
                    catalogs,
                    next_value,
                    operations,
                )?);
            } else if byte_subslices::range(argument).is_some() {
                let place = if let Some(staged) = &staged {
                    staged
                        .ranges
                        .iter()
                        .find_map(|(ordinal, place)| {
                            (*ordinal == structural_ordinal).then_some(*place)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "scheduled call lost its staged range operand",
                        ))?
                } else {
                    stage_range(
                        checked,
                        state_plan,
                        operation,
                        structural_ordinal,
                        source_value_count,
                        catalogs,
                        evaluation,
                        values,
                        next_value,
                        operations,
                    )?
                };
                byte_places.push(place);
            }
            structural_ordinal += 1;
        } else if staged.is_none()
            && scalar_positions.get(scalar_slots.len()) == Some(&argument_position)
        {
            let mut calls = catalogs.scalar_calls.emission_context();
            let value = evaluation.argument_at(
                checked,
                machine,
                state,
                operation,
                scalar_slots.len(),
                source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                &mut calls,
            )?;
            catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
            scalar_slots.push(values.len());
            values.push(value);
        }
        // Existing custody may erase an unused receiver or a compile-only
        // structural parameter. Do not renumber the remaining formal positions.
    }
    if let Some(staged) = staged {
        if staged.scalars.len() != scalar_count {
            return unsupported("scheduled call lost its completed scalar operands");
        }
        return Ok((Some(staged.scalars.to_vec()), byte_places));
    }
    // A later operand can split control and rebind earlier private slots. Read
    // the completed values only after every operand, then retire those slots.
    let scalars = scalar_slots
        .into_iter()
        .map(|slot| {
            values.get(slot).copied().ok_or(LoweringError::Unsupported(
                "literal call lost an evaluated scalar operand",
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.truncate(source_value_count);
    Ok((Some(scalars), byte_places))
}

/// Narrow the call's `ordinal`th structural argument's source at its
/// position. The endpoints read the namespace the call's authored operands
/// read, before any of this call's own operands evaluated.
#[allow(clippy::too_many_arguments)]
pub(super) fn stage_range(
    checked: &CheckedTrees,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    ordinal: usize,
    source_value_count: usize,
    catalogs: &mut catalogs::ComposedCatalogs,
    evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<PlaceId, LoweringError> {
    let bindings = evaluation
        .scalar_bindings
        .clone()
        .unwrap_or_else(|| {
            crate::expression_preparation::bindings::ScalarBindings::new(
                values
                    .iter()
                    .take(source_value_count)
                    .map(|value| value.scalar_type),
            )
        })
        .with_structural_parameters(&evaluation.structural_parameters)
        .with_resolved_structural_observations(
            &evaluation.structural_fields,
            &evaluation.structural_cases,
        )
        .with_primitive_storage(&evaluation.primitive_storage)
        .with_structural_locals(&evaluation.structural_locals)
        .with_view_locals(&evaluation.view_locals)
        .with_element_views(&evaluation.element_views);
    let context = byte_subslices::StagingContext {
        state: state.state,
        structural_types: &catalogs.structural_types,
        type_ids: &catalogs.type_ids,
        bindings: &bindings,
    };
    byte_subslices::emit(
        checked,
        &context,
        operation,
        ordinal,
        values,
        &mut catalogs.temporary_places,
        &mut catalogs.next_place,
        next_value,
        operations,
    )
}

fn establish(
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    parameter_qualifications: &[language_semantics::SemanticDomainId],
    catalogs: &mut catalogs::ComposedCatalogs,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<PlaceId, LoweringError> {
    let bytes = argument
        .byte_sequence_literal()
        .ok_or(LoweringError::Unsupported(
            "literal call argument has no payload",
        ))?;
    let structural_type = lookup_type_id(&catalogs.type_ids, &argument.type_identity)?;
    if !argument.path.is_empty() {
        return unsupported("literal call argument is not a whole byte carrier");
    }
    let qualifications = parameter_qualifications
        .iter()
        .map(|domain| lookup_domain_id_for_carrier(&catalogs.domain_ids, *domain, structural_type))
        .collect::<Result<Vec<_>, _>>()?;
    let declaration = crate::emission::structural_byte_sequence_store::emit_call_literal(
        bytes,
        structural_type,
        argument.access,
        qualifications,
        &catalogs.structural_types,
        &mut catalogs.temporary_places,
        &mut catalogs.next_place,
        next_value,
        &mut catalogs.scalar_calls.next_call_obligation,
        operations,
    )?;
    Ok(declaration.id)
}
