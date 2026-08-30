use super::boundary_settlements::claim_completion_only_boundary_is_exact;
use super::cleanup::validate_bounded_nominal_cleanup_body;
use super::shared::*;
use super::structural::exact_fully_consumed_affine_pair_root;
use super::structural_layout::{
    checked_align_up_u32, expected_maximal_residual_subtrees, is_partial_cleanup_path,
    resolve_structural_field_path, structural_shape,
};

mod boundary_call;
mod return_unit;

use boundary_call::lower_boundary_call;
use return_unit::lower_unit_return;

pub(super) fn lower_unit_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
) -> Result<TargetFunction, LoweringError> {
    if !function.parameters.is_empty() {
        return Err(LoweringError::UnitFunctionHasScalarParameters(
            function.machine,
        ));
    }
    if function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || !function.block_entries[0].parameters.is_empty()
    {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }
    if let Some(AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. }) = function
        .operations
        .iter()
        .find(|operation| matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. }))
    {
        return Err(LoweringError::UnsupportedWriteOnlyPrimitiveStore {
            machine: function.machine,
            operation: *psi_operation,
        });
    }

    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let parameter_shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                &mut shape_cache,
                &mut active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: parameter_shapes.clone(),
        result: None,
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    if call_plan.parameters.len() != function.structural_parameters.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.structural_parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    let parameters = function
        .structural_parameters
        .iter()
        .zip(parameter_shapes)
        .zip(&call_plan.parameters)
        .map(
            |((parameter, shape), placement)| TargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                shape,
                placement: placement.clone(),
            },
        )
        .collect::<Vec<_>>();
    let parameters_by_place = parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect::<BTreeMap<_, _>>();

    let mut operations = Vec::with_capacity(function.operations.len());
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = false;
    let mut established_byte_sequences =
        BTreeMap::<PlaceId, (OperationId, StructuralTypeDeclaration, Vec<u8>)>::new();
    let mut integer_constants =
        BTreeMap::<ValueId, (OperationId, IntegerType, IntegerValue)>::new();
    let mut nonreturning_boundary = false;
    for operation in &function.operations {
        if returned {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. } => {
                return Err(LoweringError::UnsupportedWriteOnlyPrimitiveStore {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::EstablishPayloadlessCase { .. } => {
                return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
            }
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                bytes,
            } => {
                if nonreturning_boundary
                    || !matches!(
                        (&place.kind, &structural_type.shape),
                        (
                            psi_core::StructuralPlaceKind::ByteSequenceLiteral {
                                structural_type: place_type,
                                ..
                            },
                            StructuralTypeShape::ByteSequence(
                                psi_terminal::ByteSequenceCarrier::BorrowedView
                            )
                        ) if *place_type == structural_type.id
                    )
                    || established_byte_sequences
                        .insert(
                            place.id,
                            (*psi_operation, structural_type.clone(), bytes.clone()),
                        )
                        .is_some()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TargetUnitOperation::EstablishByteSequenceLiteral {
                    psi_operation: *psi_operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                    bytes: bytes.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => {
                operations.push(TargetUnitOperation::EstablishTrivialAffineLocal {
                    psi_operation: *psi_operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
            } => {
                let callee_function = functions
                    .get(callee)
                    .copied()
                    .ok_or(LoweringError::UnknownCallTarget(*callee))?;
                if callee_function.result != AbstractFunctionResult::Unit
                    || !callee_function.parameters.is_empty()
                {
                    return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
                }
                if structural_arguments.len() != callee_function.structural_parameters.len() {
                    return Err(LoweringError::StructuralCallArgumentCountMismatch {
                        callee: *callee,
                        expected: callee_function.structural_parameters.len(),
                        actual: structural_arguments.len(),
                    });
                }
                let callee_shapes = callee_function
                    .structural_parameters
                    .iter()
                    .map(|parameter| {
                        structural_shape(
                            parameter.structural_type,
                            structural_types,
                            &mut shape_cache,
                            &mut active,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let callee_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: callee_shapes.clone(),
                        result: None,
                    },
                )
                .map_err(LoweringError::AbiPlan)?;
                let arguments = structural_arguments
                    .iter()
                    .zip(&callee_function.structural_parameters)
                    .zip(callee_shapes)
                    .zip(&callee_plan.parameters)
                    .map(|(((argument, callee_parameter), shape), destination)| {
                        let source = parameters_by_place.get(&argument.place).copied().ok_or(
                            LoweringError::UnknownStructuralArgumentPlace {
                                machine: function.machine,
                                place: argument.place,
                            },
                        )?;
                        let (
                            projected_type,
                            projected_shape,
                            source_byte_offset,
                            fixed_array_length,
                            element_stride,
                        ) = match argument.path.as_slice() {
                            [] => (source.structural_type, source.shape, 0, None, None),
                            [StructuralPathSegment::FixedIndex(index)] => {
                                let declaration = structural_types
                                    .get(&source.structural_type)
                                    .copied()
                                    .ok_or(LoweringError::UnknownStructuralType(
                                        source.structural_type,
                                    ))?;
                                let StructuralTypeShape::FixedArray { element, length } =
                                    declaration.shape
                                else {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                };
                                if *index >= length {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                }
                                let element_shape = structural_shape(
                                    element,
                                    structural_types,
                                    &mut shape_cache,
                                    &mut active,
                                )?;
                                let stride = checked_align_up_u32(
                                    u32::from(element_shape.byte_size),
                                    u32::from(element_shape.alignment),
                                )
                                .ok_or(
                                    LoweringError::StructuralTypeTooLarge(source.structural_type),
                                )?;
                                let offset = u64::from(stride)
                                    .checked_mul(*index)
                                    .and_then(|offset| u32::try_from(offset).ok())
                                    .ok_or(LoweringError::StructuralTypeTooLarge(
                                        source.structural_type,
                                    ))?;
                                (element, element_shape, offset, Some(length), Some(stride))
                            }
                            [
                                StructuralPathSegment::FixedIndex(outer_index),
                                StructuralPathSegment::FixedIndex(inner_index),
                            ] => {
                                let declaration = structural_types
                                    .get(&source.structural_type)
                                    .copied()
                                    .ok_or(LoweringError::UnknownStructuralType(
                                        source.structural_type,
                                    ))?;
                                let StructuralTypeShape::FixedArray {
                                    element: inner_type,
                                    length: 2,
                                } = declaration.shape
                                else {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                };
                                let inner_declaration = structural_types
                                    .get(&inner_type)
                                    .copied()
                                    .ok_or(LoweringError::UnknownStructuralType(inner_type))?;
                                let StructuralTypeShape::FixedArray {
                                    element: leaf_type,
                                    length: 3,
                                } = inner_declaration.shape
                                else {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                };
                                if *outer_index >= 2 || *inner_index >= 3 {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                }
                                let inner_shape = structural_shape(
                                    inner_type,
                                    structural_types,
                                    &mut shape_cache,
                                    &mut active,
                                )?;
                                let leaf_shape = structural_shape(
                                    leaf_type,
                                    structural_types,
                                    &mut shape_cache,
                                    &mut active,
                                )?;
                                let outer_stride = checked_align_up_u32(
                                    u32::from(inner_shape.byte_size),
                                    u32::from(inner_shape.alignment),
                                )
                                .ok_or(
                                    LoweringError::StructuralTypeTooLarge(source.structural_type),
                                )?;
                                let inner_stride = checked_align_up_u32(
                                    u32::from(leaf_shape.byte_size),
                                    u32::from(leaf_shape.alignment),
                                )
                                .ok_or(
                                    LoweringError::StructuralTypeTooLarge(source.structural_type),
                                )?;
                                let offset = u64::from(outer_stride)
                                    .checked_mul(*outer_index)
                                    .and_then(|offset| {
                                        u64::from(inner_stride)
                                            .checked_mul(*inner_index)
                                            .and_then(|inner| offset.checked_add(inner))
                                    })
                                    .and_then(|offset| u32::try_from(offset).ok())
                                    .ok_or(LoweringError::StructuralTypeTooLarge(
                                        source.structural_type,
                                    ))?;
                                (leaf_type, leaf_shape, offset, Some(2), Some(outer_stride))
                            }
                            path @ [StructuralPathSegment::Field(_), ..]
                                if path.iter().all(|segment| {
                                    matches!(segment, StructuralPathSegment::Field(_))
                                }) =>
                            {
                                let (field_type, field_shape, offset) =
                                    resolve_structural_field_path(
                                        source.structural_type,
                                        path,
                                        structural_types,
                                        &mut shape_cache,
                                        &mut active,
                                    )
                                    .map_err(|_| {
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        }
                                    })?;
                                (field_type, field_shape, offset, None, None)
                            }
                            _ => {
                                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                    callee: *callee,
                                    place: argument.place,
                                });
                            }
                        };
                        if projected_type != callee_parameter.structural_type
                            || projected_shape != shape
                            || u32::from(shape.byte_size)
                                .checked_add(source_byte_offset)
                                .is_none_or(|end| end > u32::from(source.shape.byte_size))
                        {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        }
                        Ok(TargetStructuralArgument {
                            place: argument.place,
                            access: argument.access,
                            path: argument.path.clone(),
                            root_structural_type: source.structural_type,
                            structural_type: projected_type,
                            shape,
                            source_byte_offset,
                            fixed_array_length,
                            element_stride,
                            source: source.placement.clone(),
                            destination: destination.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                operations.push(TargetUnitOperation::Call {
                    psi_operation: *psi_operation,
                    callee: *callee,
                    arguments,
                    claim_transfers: claim_transfers.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::PortWrite {
                psi_operation,
                service,
                port,
                value,
            } => {
                operations.push(TargetUnitOperation::PortWrite {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::BoundaryCall { .. } => lower_boundary_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                boundary_machines,
                settlements,
                installed_calls,
                &parameters_by_place,
                &mut shape_cache,
                &mut active,
                &established_byte_sequences,
                &integer_constants,
                &mut operations,
                &mut provenance,
                &mut nonreturning_boundary,
            )?,
            AbstractOperation::ReturnUnit { .. } => lower_unit_return(
                operation,
                function,
                &parameters,
                &mut operations,
                structural_types,
                functions,
                nonreturning_boundary,
                &mut provenance,
                &mut returned,
            )?,
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type: ScalarType::Integer(scalar_type),
                value,
            } => {
                if nonreturning_boundary
                    || integer_constants
                        .insert(*result, (*psi_operation, *scalar_type, *value))
                        .is_some()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TargetUnitOperation::IntegerConstant {
                    psi_operation: *psi_operation,
                    result: *result,
                    scalar_type: *scalar_type,
                    value: *value,
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::Crash { .. }
            | AbstractOperation::Call { .. }
            | AbstractOperation::CallStructuralScalar { .. }
            | AbstractOperation::CallStructural { .. }
            | AbstractOperation::IntegerConstant { .. }
            | AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::BooleanStructuralField { .. }
            | AbstractOperation::BooleanNot { .. }
            | AbstractOperation::BooleanEqual { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. }
            | AbstractOperation::IntegerBitwiseNot { .. }
            | AbstractOperation::IntegerWiden { .. }
            | AbstractOperation::IntegerExactCast { .. }
            | AbstractOperation::IntegerBitwiseAnd { .. }
            | AbstractOperation::IntegerBitwiseOr { .. }
            | AbstractOperation::IntegerBitwiseXor { .. }
            | AbstractOperation::WrappingIntegerShiftLeft { .. }
            | AbstractOperation::WrappingIntegerShiftRight { .. }
            | AbstractOperation::ExactIntegerShiftLeft { .. }
            | AbstractOperation::ExactIntegerShiftRight { .. }
            | AbstractOperation::WrappingIntegerAdd { .. }
            | AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::SaturatingIntegerAdd { .. }
            | AbstractOperation::WrappingIntegerSubtract { .. }
            | AbstractOperation::ExactIntegerSubtract { .. }
            | AbstractOperation::SaturatingIntegerSubtract { .. }
            | AbstractOperation::WrappingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerMultiply { .. }
            | AbstractOperation::SaturatingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerDivide { .. }
            | AbstractOperation::ExactIntegerRemainder { .. }
            | AbstractOperation::WrappingIntegerDivide { .. }
            | AbstractOperation::WrappingIntegerRemainder { .. }
            | AbstractOperation::SaturatingIntegerDivide { .. }
            | AbstractOperation::SaturatingIntegerRemainder { .. }
            | AbstractOperation::Jump { .. }
            | AbstractOperation::Conditional { .. }
            | AbstractOperation::Return { .. }
            | AbstractOperation::ReturnStructural { .. } => {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
        }
    }
    if !returned {
        return Err(LoweringError::FunctionHasNoReturn(function.machine));
    }
    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance,
        operation: TargetOperation::UnitBody(TargetUnitBody {
            structural_types: structural_types
                .values()
                .map(|declaration| (*declaration).clone())
                .collect(),
            call_plan,
            parameters,
            operations,
        }),
    })
}
