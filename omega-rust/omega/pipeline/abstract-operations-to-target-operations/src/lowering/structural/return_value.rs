use super::super::shared::*;
use super::super::structural_layout::structural_shape;

pub(in crate::lowering) fn lower_structural_return_function(
    function: &AbstractFunction,
    result: &terminal_psi::StructuralResultDeclaration,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetFunction, LoweringError> {
    if let Some(lowered) =
        lower_claim_free_affine_mixed_return(function, result, target, structural_types)?
    {
        return Ok(lowered);
    }
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
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
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
    if source.multiplicity != terminal_psi::StructuralMultiplicity::Linear
        || source.structural_type != result.structural_type
        || source.qualifications != result.qualifications
        || source.projected_qualifications != result.projected_qualifications
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
                let semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    structural_type,
                    construction: None,
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
                        terminal_psi::StructuralTypeShape::Record { ref fields } if fields.is_empty()
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
                cleanup.multiplicity != terminal_psi::StructuralMultiplicity::Affine
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
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect(),
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::ReturnStructuralParameter {
            call_plan: call_plan.clone(),
            scalar_parameters: Vec::new(),
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

fn lower_claim_free_affine_mixed_return(
    function: &AbstractFunction,
    result: &terminal_psi::StructuralResultDeclaration,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<Option<TargetFunction>, LoweringError> {
    let ([scalar_parameter], [structural_parameter], [block_entry], [operation]) = (
        function.parameters.as_slice(),
        function.structural_parameters.as_slice(),
        function.block_entries.as_slice(),
        function.operations.as_slice(),
    ) else {
        return Ok(None);
    };
    let AbstractOperation::ReturnStructural {
        psi_edge,
        source,
        returned_claims,
        trivial_affine_locals,
        trivial_affine_discards,
    } = operation
    else {
        return Ok(None);
    };
    let ScalarType::Integer(scalar_type) = scalar_parameter.scalar_type else {
        return Ok(None);
    };
    let Some(scalar_shape) = super::super::scalar_abi::fixed_native_integer_shape(scalar_type)
    else {
        return Ok(None);
    };
    let Some(declaration) = structural_types.get(&result.structural_type).copied() else {
        return Err(LoweringError::UnknownStructuralType(result.structural_type));
    };
    let exact_record = matches!(
        &declaration.shape,
        StructuralTypeShape::Record { fields }
            if matches!(
                fields.as_slice(),
                [field]
                    if matches!(
                        field.field_type,
                        StructuralFieldType::Scalar(ScalarType::Integer(integer))
                            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                && integer.bits() == 64
                    )
            )
    );
    if !exact_record
        || !function.entry_claims.is_empty()
        || !function.published_service_ceiling.is_empty()
        || block_entry.block != function.entry
        || block_entry.operation_offset != 0
        || block_entry.parameters.as_slice() != [*scalar_parameter]
        || structural_parameter.position != 0
        || structural_parameter.is_self
        || structural_parameter.multiplicity != StructuralMultiplicity::Affine
        || structural_parameter.access != StructuralAccess::Owned
        || !structural_parameter.qualifications.is_empty()
        || !structural_parameter.projected_qualifications.is_empty()
        || structural_parameter.structural_type != result.structural_type
        || *source != structural_parameter.place
        || result.place == structural_parameter.place
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !returned_claims.is_empty()
        || !trivial_affine_locals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return Ok(None);
    }
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let structural_shape = structural_shape(
        structural_parameter.structural_type,
        structural_types,
        &mut cache,
        &mut active,
    )?;
    if structural_shape != ValueShape::integer(8, 8) {
        return Ok(None);
    }
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![scalar_shape, structural_shape],
            result: Some(structural_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let Some(scalar_placement) = call_plan.parameters.first().cloned() else {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: 2,
            actual: call_plan.parameters.len(),
        });
    };
    let Some(source_placement) = call_plan.parameters.get(1).cloned() else {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: 2,
            actual: call_plan.parameters.len(),
        });
    };
    let result_placement = call_plan
        .result
        .clone()
        .ok_or(LoweringError::UnsupportedStructuralReturn(function.machine))?;
    require_direct_structural_fragments(function.machine, &source_placement)?;
    require_direct_structural_fragments(function.machine, &result_placement)?;
    Ok(Some(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: Vec::new(),
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::ReturnStructuralParameter {
            call_plan,
            scalar_parameters: vec![FixedIntegerScalarAbiValue {
                value: scalar_parameter.value,
                scalar_type,
                placement: scalar_placement,
            }],
            parameters: vec![structural_parameter.clone()],
            source: structural_parameter.clone(),
            result: result.clone(),
            shape: structural_shape,
            source_placement,
            result_placement,
            psi_edge: *psi_edge,
            returned_claims: Vec::new(),
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    }))
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

pub(in crate::lowering) fn exact_fully_consumed_affine_root(
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &[TargetUnitOperation],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
) -> Option<PlaceId> {
    let ([source_parameter], [parameter]) = (function.structural_parameters.as_slice(), parameters)
    else {
        return None;
    };
    if source_parameter.position != 0
        || source_parameter.is_self
        || !function.entry_claims.is_empty()
        || source_parameter.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || source_parameter.access != terminal_psi::StructuralAccess::Owned
        || !source_parameter.qualifications.is_empty()
        || source_parameter.place != parameter.place
        || source_parameter.structural_type != parameter.structural_type
    {
        return None;
    }
    let mut moved = Vec::new();
    let mut shapes = BTreeMap::new();
    let mut active = BTreeSet::new();
    for operation in operations {
        let TargetUnitOperation::Call {
            callee,
            arguments,
            scalar_arguments,
            claim_transfers,
            ..
        } = operation
        else {
            return None;
        };
        let callee = functions.get(callee).copied()?;
        let ([callee_parameter], [argument]) = (
            callee.structural_parameters.as_slice(),
            arguments.as_slice(),
        ) else {
            return None;
        };
        let (projected_type, shape, offset) =
            super::super::structural_layout::resolve_structural_projection_path(
                parameter.structural_type,
                &argument.path,
                structural_types,
                &mut shapes,
                &mut active,
            )
            .ok()?;
        let metadata = super::super::structural_layout::root_array_projection_metadata(
            parameter.structural_type,
            structural_types,
            &mut shapes,
            &mut active,
        )
        .ok()?;
        let root_shape = super::super::structural_layout::structural_shape(
            parameter.structural_type,
            structural_types,
            &mut shapes,
            &mut active,
        )
        .ok()?;
        if callee.result != AbstractFunctionResult::Unit
            || !callee.parameters.is_empty()
            || !callee.entry_claims.is_empty()
            || callee_parameter.position != 0
            || callee_parameter.is_self
            || callee_parameter.structural_type != projected_type
            || callee_parameter.multiplicity != terminal_psi::StructuralMultiplicity::Affine
            || callee_parameter.access != terminal_psi::StructuralAccess::Owned
            || !callee_parameter.qualifications.is_empty()
            || !claim_transfers.is_empty()
            || !scalar_arguments.is_empty()
            || argument.place != parameter.place
            || argument.access != terminal_psi::StructuralAccess::Owned
            || argument.root_structural_type != parameter.structural_type
            || argument.structural_type != projected_type
            || argument.shape != shape
            || argument.source != parameter.placement
            || argument.source.shape != parameter.shape
            || argument.source_byte_offset != offset
            || (argument.fixed_array_length, argument.element_stride) != metadata
            || parameter.shape != root_shape
            || offset
                .checked_add(u32::from(shape.byte_size))
                .is_none_or(|end| end > u32::from(root_shape.byte_size))
        {
            return None;
        }
        moved.push((argument.path.clone(), argument.structural_type));
    }
    super::super::structural_layout::expected_maximal_residual_subtrees(
        parameter.structural_type,
        &moved,
        structural_types,
        0,
    )
    .is_some_and(|residuals| residuals.is_empty())
    .then_some(parameter.place)
}
