//! Materialize immutable literals at their selected call and authored argument position.

use super::*;

pub(super) fn evaluate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    evaluation: &mut crate::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(Option<Vec<ValueDeclaration>>, Vec<PlaceId>), LoweringError> {
    let arguments = crate::attached_unit::byte_subslices::arguments(operation);
    if !arguments
        .iter()
        .any(|argument| argument.byte_sequence_literal().is_some())
    {
        let mut calls = catalogs.scalar_calls.emission_context();
        let values = evaluation.arguments(
            checked, machine, state, operation, values, next_value, next_block, next_edge,
            operations, &mut calls,
        )?;
        catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
        return Ok((values, Vec::new()));
    }
    let (target_parameters, scalar_count, coordinate) = match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine,
            scalar_arguments,
            ..
        } => {
            let target = catalogs
                .lowered_boundaries
                .iter()
                .find(|target| target.source == *target_machine)
                .ok_or(LoweringError::Unsupported(
                    "literal call boundary target is absent",
                ))?;
            (
                &target.checked_structural_parameters,
                scalar_arguments.len(),
                *coordinate,
            )
        }
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            target_machine,
            scalar_arguments,
            ..
        } => {
            let target = catalogs
                .internal_targets
                .iter()
                .find(|target| target.source == *target_machine)
                .ok_or(LoweringError::Unsupported(
                    "literal call Unit target is absent",
                ))?;
            (
                &target.structural_parameters,
                scalar_arguments.len(),
                *coordinate,
            )
        }
        _ => return unsupported("literal call escaped the composed Unit operation family"),
    };
    let positions = target_parameters
        .iter()
        .map(|parameter| parameter.position as usize)
        .collect::<Vec<_>>();
    let authored = crate::call_source_custody::authored::locate_source(checked, state, coordinate)?;
    let signature = crate::call_source_custody::authored::target_signature(
        checked,
        machine,
        authored.source_target,
    )?;
    let source_positions = crate::call_source_custody::literal_arguments::structural_positions(
        checked,
        &signature,
        arguments.len(),
    )?;
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
        || positions.iter().copied().ne(source_positions
            .into_iter()
            .map(|position| position as usize))
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
                byte_places.push(establish(argument, catalogs, operations)?);
            }
            structural_ordinal += 1;
        } else if scalar_positions.get(scalar_slots.len()) == Some(&argument_position) {
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

fn establish(
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    operations: &mut OperationBuffer,
) -> Result<PlaceId, LoweringError> {
    let bytes = argument
        .byte_sequence_literal()
        .ok_or(LoweringError::Unsupported(
            "literal call argument has no payload",
        ))?;
    let structural_type = lookup_type_id(&catalogs.type_ids, &argument.type_identity)?;
    if !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
        || !catalogs.structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
                )
        })
    {
        return unsupported("literal call argument is not a whole immutable byte view");
    }
    let destination = place_id(allocate_dense(&mut catalogs.next_place)?);
    let declaration_ordinal = u32::try_from(catalogs.temporary_places.len())
        .map_err(|_| LoweringError::Unsupported("literal call declaration ordinal exceeds u32"))?;
    catalogs.temporary_places.push(StructuralPlaceDeclaration {
        id: destination,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        },
    });
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination,
            bytes: bytes.to_vec(),
        },
    });
    Ok(destination)
}
