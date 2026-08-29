use super::super::shared::*;
use super::super::structural_layout::structural_shape;

pub(in crate::lowering) fn lower_structural_return_function(
    function: &AbstractFunction,
    result: &psi_terminal::StructuralResultDeclaration,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetFunction, LoweringError> {
    if function.structural_parameters.is_empty() {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    let [entry_claim] = function.entry_claims.as_slice() else {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    };
    let [block_entry] = function.block_entries.as_slice() else {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    };
    let [
        AbstractOperation::ReturnStructural {
            psi_edge,
            source: returned_source,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        },
    ] = function.operations.as_slice()
    else {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    };
    if !function.parameters.is_empty()
        || !function.published_service_ceiling.is_empty()
        || block_entry.block != function.entry
        || block_entry.operation_offset != 0
        || result.multiplicity != psi_terminal::StructuralMultiplicity::Linear
        || function
            .structural_parameters
            .iter()
            .enumerate()
            .any(|(index, parameter)| {
                parameter.is_self || usize::try_from(parameter.position) != Ok(index)
            })
        || function
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<BTreeSet<_>>()
            .len()
            != function.structural_parameters.len()
        || !entry_claim.path.is_empty()
        || returned_claims.as_slice() != [entry_claim.claim]
    {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    let source_index = 0;
    let source = &function.structural_parameters[source_index];
    if source.place != *returned_source {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    if source.multiplicity != psi_terminal::StructuralMultiplicity::Linear
        || source.structural_type != result.structural_type
        || source.qualifications != result.qualifications
        || source.place == result.place
        || entry_claim.input != source.place
    {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    let expected_cleanup = trivial_affine_locals
        .iter()
        .rev()
        .map(|(_, local, _)| local.id)
        .chain(
            function
                .structural_parameters
                .iter()
                .skip(1)
                .rev()
                .map(|parameter| parameter.place),
        )
        .collect::<Vec<_>>();
    if trivial_affine_discards != &expected_cleanup
        || trivial_affine_locals
            .iter()
            .enumerate()
            .any(|(index, (_, local, local_type))| {
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
            } = local.kind
            else {
                return true;
            };
            usize::try_from(declaration_ordinal) != Ok(index)
                || local.id == source.place
                || local.id == result.place
                || function
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == local.id)
                || structural_types.get(&structural_type).is_none_or(|declaration| {
                    *declaration != local_type
                        || declaration.identity.is_empty()
                        || !matches!(
                        declaration.shape,
                        psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                    )
                })
        })
        || trivial_affine_locals
            .iter()
            .map(|(_, local, _)| local.id)
            .collect::<BTreeSet<_>>()
            .len()
            != trivial_affine_locals.len()
        || function
            .structural_parameters
            .iter()
            .skip(1)
            .any(|cleanup| {
                cleanup.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                    || !cleanup.qualifications.is_empty()
                    || cleanup.place == result.place
            })
    {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let parameter_shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                &mut cache,
                &mut active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape = parameter_shapes[source_index];
    if shape.class != ValueClass::Integer
        || !((shape.byte_size == 8 && shape.alignment == 8) || (9..=16).contains(&shape.byte_size))
    {
        return Err(LoweringError::UnsupportedStructuralReturnShape {
            machine: function.machine,
            byte_size: shape.byte_size,
        });
    }
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: parameter_shapes,
            result: Some(shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let Some(source_placement) = call_plan.parameters.get(source_index) else {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.structural_parameters.len(),
            actual: call_plan.parameters.len(),
        });
    };
    let Some(result_placement) = call_plan.result.as_ref() else {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    };
    require_direct_structural_fragments(function.machine, source_placement)?;
    require_direct_structural_fragments(function.machine, result_placement)?;
    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: TerminalPsiProvenance {
            operations: trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect(),
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::ReturnStructuralParameter {
            call_plan: call_plan.clone(),
            parameters: function.structural_parameters.clone(),
            source: source.clone(),
            result: result.clone(),
            shape,
            source_placement: source_placement.clone(),
            result_placement: result_placement.clone(),
            psi_edge: *psi_edge,
            returned_claims: returned_claims.clone(),
            trivial_affine_locals: trivial_affine_locals.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
        },
    })
}

pub(in crate::lowering) fn require_direct_structural_fragments(
    machine: MachineId,
    placement: &ValuePlacement,
) -> Result<(), LoweringError> {
    if placement.shape.class != ValueClass::Integer
        || !((placement.shape.byte_size == 8 && placement.shape.alignment == 8)
            || (9..=16).contains(&placement.shape.byte_size))
        || !(1..=2).contains(&placement.locations.len())
    {
        return Err(LoweringError::UnsupportedStructuralReturnPlacement(machine));
    }
    let mut expected_offset = 0_u16;
    for location in &placement.locations {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = *location
        else {
            return Err(LoweringError::UnsupportedStructuralReturnPlacement(machine));
        };
        let expected_size = (placement.shape.byte_size - expected_offset).min(8);
        if value_byte_offset != expected_offset || byte_size != expected_size {
            return Err(LoweringError::UnsupportedStructuralReturnPlacement(machine));
        }
        expected_offset = expected_offset
            .checked_add(byte_size)
            .ok_or(LoweringError::UnsupportedStructuralReturnPlacement(machine))?;
    }
    if expected_offset != placement.shape.byte_size {
        return Err(LoweringError::UnsupportedStructuralReturnPlacement(machine));
    }
    Ok(())
}

pub(in crate::lowering) fn exact_fully_consumed_affine_pair_root(
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &[TargetUnitOperation],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
) -> Option<PlaceId> {
    let ([source_parameter], [parameter], [first, second]) = (
        function.structural_parameters.as_slice(),
        parameters,
        operations,
    ) else {
        return None;
    };
    if source_parameter.position != 0
        || source_parameter.is_self
        || !function.entry_claims.is_empty()
        || source_parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || source_parameter.access != psi_terminal::StructuralAccess::Owned
        || !source_parameter.qualifications.is_empty()
        || source_parameter.place != parameter.place
        || source_parameter.structural_type != parameter.structural_type
    {
        return None;
    }
    let root = structural_types.get(&parameter.structural_type).copied()?;
    let StructuralTypeShape::FixedArray { element, length: 2 } = root.shape else {
        return None;
    };
    if !matches!(
        structural_types
            .get(&element)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Record { .. })
    ) {
        return None;
    }
    let moved_index = |operation: &TargetUnitOperation| {
        let TargetUnitOperation::Call {
            callee,
            arguments,
            claim_transfers,
            ..
        } = operation
        else {
            return None;
        };
        let callee = functions.get(callee).copied()?;
        let [callee_parameter] = callee.structural_parameters.as_slice() else {
            return None;
        };
        let [argument] = arguments.as_slice() else {
            return None;
        };
        let [StructuralPathSegment::FixedIndex(index @ (0 | 1))] = argument.path.as_slice() else {
            return None;
        };
        let stride = argument.element_stride?;
        let expected_stride = u32::from(argument.shape.byte_size)
            .checked_next_multiple_of(u32::from(argument.shape.alignment))?;
        (callee.result == AbstractFunctionResult::Unit
            && callee.parameters.is_empty()
            && callee.entry_claims.is_empty()
            && callee_parameter.position == 0
            && !callee_parameter.is_self
            && callee_parameter.structural_type == element
            && callee_parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
            && callee_parameter.access == psi_terminal::StructuralAccess::Owned
            && callee_parameter.qualifications.is_empty()
            && claim_transfers.is_empty()
            && argument.place == parameter.place
            && argument.access == psi_terminal::StructuralAccess::Owned
            && argument.root_structural_type == parameter.structural_type
            && argument.structural_type == element
            && argument.fixed_array_length == Some(2)
            && stride == expected_stride
            && argument.source == parameter.placement
            && argument.source.shape == parameter.shape
            && argument.source.shape.alignment == argument.shape.alignment
            && u32::from(argument.source.shape.byte_size) == stride.checked_mul(2)?
            && argument.source_byte_offset == stride.checked_mul(u32::try_from(*index).ok()?)?)
        .then_some((*index, argument.shape, stride))
    };
    let first = moved_index(first)?;
    let second = moved_index(second)?;
    (first.0 != second.0 && first.1 == second.1 && first.2 == second.2).then_some(parameter.place)
}
