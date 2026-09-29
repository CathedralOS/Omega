//! Ordinary structural call operands and Unit-call orchestration.

use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::structural_layout::{
    bounded_byte_field_geometry, checked_align_up_u32, resolve_structural_field_path,
    resolve_structural_projection_path, structural_parameter_shape, structural_shape,
};
use super::super::structural_signature::StructuralCallSignature;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractStructuralCallResult,
};
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{
    IeeeFloatFormat, MachineId, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralHomeRequirement,
    TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarArgumentSource,
    TargetUnitScalarCallArgument, TerminalPsiProvenance,
};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_structural_unit_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    local_homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    established_views: &BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    scalar_values: &BTreeMap<ValueId, super::scalar_call::KnownUnitInteger>,
    scalar_aliases: &BTreeMap<ValueId, ValueId>,
    boolean_constants: &BTreeMap<ValueId, (OperationId, bool)>,
    ieee_float_constants: &BTreeMap<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>,
    boolean_parameters: &BTreeMap<ValueId, target_operations::TargetScalarBlockValue>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallStructural {
        psi_operation,
        callee,
        arguments: scalar_arguments,
        structural_arguments,
        dynamic_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        result: AbstractStructuralCallResult::Unit,
    } = operation
    else {
        unreachable!("structural Unit-call lowering receives only Unit calls")
    };
    if !dynamic_arguments.is_empty() {
        unreachable!("a Unit call forwarding descriptors takes the descriptor-pair lowering")
    }
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    if callee_function.result != AbstractFunctionResult::Unit {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    }
    if scalar_arguments.len() != callee_function.parameters.len() {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee: *callee,
            expected: callee_function.parameters.len(),
            actual: scalar_arguments.len(),
        });
    }
    if structural_arguments.len() != callee_function.structural_parameters.len() {
        return Err(LoweringError::StructuralCallArgumentCountMismatch {
            callee: *callee,
            expected: callee_function.structural_parameters.len(),
            actual: structural_arguments.len(),
        });
    }
    let scalar_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| match parameter.scalar_type {
            ScalarType::Boolean => Ok(ValueShape::integer(1, 1)),
            ScalarType::Integer(integer_type) => fixed_native_integer_shape(integer_type)
                .ok_or(LoweringError::UnitCallTargetKindMismatch(*callee)),
            ScalarType::IeeeFloat(format) => Ok(ValueShape::float(match format {
                IeeeFloatFormat::Binary32 => 4,
                IeeeFloatFormat::Binary64 => 8,
            })),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = StructuralCallSignature::derive(
        &scalar_shapes,
        &callee_function.structural_parameters,
        None,
        structural_types,
        shape_cache,
        active,
    )?;
    let callee_shapes = signature.structural_shapes();
    let callee_plan = signature.plan(target)?;
    let scalar_arguments = scalar_arguments
        .iter()
        .zip(&callee_function.parameters)
        .zip(&scalar_shapes)
        .zip(&callee_plan.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), expected_shape), placement))| {
                let source_value = scalar_aliases.get(source_value).unwrap_or(source_value);
                let source = match parameter.scalar_type {
                    ScalarType::Boolean => {
                        if let Some((caller_parameter_index, _)) = function
                            .parameters
                            .iter()
                            .enumerate()
                            .find(|(_, caller_parameter)| {
                                caller_parameter.value == *source_value
                                    && caller_parameter.scalar_type == ScalarType::Boolean
                            })
                        {
                            TargetUnitScalarArgumentSource::Parameter {
                                parameter_index: u32::try_from(caller_parameter_index).map_err(
                                    |_| LoweringError::UnitCallTargetKindMismatch(*callee),
                                )?,
                                source_value: *source_value,
                                scalar_type: ScalarType::Boolean,
                            }
                        } else if let Some((defining_operation, value)) =
                            boolean_constants.get(source_value).copied()
                        {
                            TargetUnitScalarArgumentSource::BooleanImmediate {
                                defining_operation,
                                source_value: *source_value,
                                value,
                            }
                        } else if let Some(parameter) = boolean_parameters.get(source_value) {
                            if parameter.value != *source_value
                                || parameter.scalar_type != ScalarType::Boolean
                            {
                                return Err(LoweringError::CallArgumentTypeMismatch {
                                    callee: *callee,
                                    argument: *source_value,
                                });
                            }
                            TargetUnitScalarArgumentSource::BlockParameter(*parameter)
                        } else {
                            return Err(LoweringError::UnknownValue(*source_value));
                        }
                    }
                    ScalarType::Integer(parameter_type) => {
                        let known = scalar_values
                            .get(source_value)
                            .copied()
                            .ok_or(LoweringError::UnknownValue(*source_value))?;
                        if known.scalar_type() != parameter_type {
                            return Err(LoweringError::CallArgumentTypeMismatch {
                                callee: *callee,
                                argument: *source_value,
                            });
                        }
                        known.into_target_source(*source_value)
                    }
                    ScalarType::IeeeFloat(format) => {
                        if let Some((caller_parameter_index, _)) = function
                            .parameters
                            .iter()
                            .enumerate()
                            .find(|(_, caller_parameter)| {
                                caller_parameter.value == *source_value
                                    && caller_parameter.scalar_type == parameter.scalar_type
                            })
                        {
                            TargetUnitScalarArgumentSource::Parameter {
                                parameter_index: u32::try_from(caller_parameter_index).map_err(
                                    |_| LoweringError::UnitCallTargetKindMismatch(*callee),
                                )?,
                                source_value: *source_value,
                                scalar_type: parameter.scalar_type,
                            }
                        } else {
                            let (defining_operation, value) = ieee_float_constants
                                .get(source_value)
                                .copied()
                                .filter(|(_, value)| value.format() == format)
                                .ok_or(LoweringError::CallArgumentTypeMismatch {
                                    callee: *callee,
                                    argument: *source_value,
                                })?;
                            TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                                defining_operation,
                                source_value: *source_value,
                                value,
                            }
                        }
                    }
                };
                if source.scalar_type() != parameter.scalar_type
                    || placement.shape != *expected_shape
                {
                    return Err(LoweringError::CallArgumentTypeMismatch {
                        callee: *callee,
                        argument: *source_value,
                    });
                }
                Ok(TargetUnitScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| LoweringError::UnitCallTargetKindMismatch(*callee))?,
                    source,
                    placement: placement.clone(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(callee_shapes.iter().copied())
        .zip(callee_plan.parameters.iter().skip(scalar_arguments.len()))
        .map(|(((argument, callee_parameter), shape), destination)| {
            lower_structural_argument(
                argument,
                callee_parameter,
                shape,
                destination,
                *callee,
                function,
                structural_types,
                parameters_by_place,
                local_homes,
                established_views,
                operations,
                shape_cache,
                active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    operations.push(TargetUnitOperation::Call {
        origin: target_operations::NativeCallOrigin::Authored,
        psi_operation: *psi_operation,
        callee: *callee,
        call_plan: callee_plan,
        result: target_operations::TargetCallResult::Unit,
        scalar_arguments,
        arguments,
        claim_transfers: claim_transfers.clone(),
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Resolve an ordinary call operand independently of the callee's return kind.
/// Fixed-array views retain the caller's actual storage and exact extent; the
/// descriptor is an ABI presentation, not a change to the source place's type.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_structural_argument(
    argument: &terminal_psi::StructuralArgument,
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    shape: ValueShape,
    destination: &ValuePlacement,
    callee: MachineId,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    local_homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    established_views: &BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    operations: &[TargetUnitOperation],
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<TargetStructuralArgument, LoweringError> {
    // Whole-root domain preconditions bind by exact roster equality: the
    // callee parameter's declared qualifications must equal the roster the
    // caller's source place carries — a caller structural parameter's own
    // declared roster, a produced structural home's result roster, or the
    // minted roster an established byte-sequence literal carries. Every
    // other source carries no qualifications and satisfies only an
    // unqualified parameter. Verified discharge happened upstream; lowering
    // transports the authority without re-proving it.
    let source_qualifications = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .map(|parameter| parameter.qualifications.as_slice())
        .or_else(|| {
            super::projected_result::source(operations, argument.place)
                .map(|(home, _)| home.qualifications())
        })
        .or_else(|| {
            local_homes
                .get(&argument.place)
                .map(TargetStructuralHomeRequirement::qualifications)
        })
        .or_else(|| {
            operations.iter().find_map(|operation| {
                let TargetUnitOperation::EstablishByteSequenceLiteral {
                    place,
                    qualifications,
                    ..
                } = operation
                else {
                    return None;
                };
                (place.id == argument.place).then_some(qualifications.as_slice())
            })
        })
        .unwrap_or(&[]);
    // An inline owner field carries typed predicates in its field declaration,
    // not the enclosing root's domain roster. Optimized admission has checked
    // both directions of its writable predicate contract; the field geometry
    // below must still match the callee's exact bounded-owner capacity.
    let projected_byte_owner = !argument.path.is_empty()
        && structural_types
            .get(&callee_parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
            });
    if !projected_byte_owner && source_qualifications != callee_parameter.qualifications.as_slice()
    {
        return Err(LoweringError::StructuralCallArgumentTypeMismatch {
            callee,
            place: argument.place,
        });
    }
    if let Some((source, structural_type)) = established_views
        .get(&argument.place)
        .map(|(producer, structural_type)| {
            (
                target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: *producer,
                },
                *structural_type,
            )
        })
        .or_else(|| {
            super::super::scalar::byte_views::block_source(
                function,
                argument.place,
                structural_types,
            )
        })
    {
        if !argument.path.is_empty()
            || argument.access != StructuralAccess::SharedBorrow
            || !super::super::scalar::byte_views::is_immutable_byte_carrier_parameter(
                callee_parameter,
                structural_types,
            )
            || structural_type != callee_parameter.structural_type
            || shape != ValueShape::borrowed_reference(16, 8)
        {
            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                callee,
                place: argument.place,
            });
        }
        return Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source,
            destination: destination.clone(),
        });
    }
    let result_source = super::projected_result::source(operations, argument.place);
    let (source_structural_type, source_shape, source_placement) =
        if let Some(source) = parameters_by_place.get(&argument.place).copied() {
            (source.structural_type, source.shape, &source.placement)
        } else if let Some((home, placement)) = result_source {
            (home.structural_type(), home.layout.shape(), placement)
        } else if let Some(home) = local_homes.get(&argument.place) {
            // A dominating local home has no incoming placement: it lends its
            // own storage as a whole fixed array presented through a view.
            return local_array_loan(
                argument,
                callee_parameter,
                shape,
                destination,
                callee,
                home,
                structural_types,
                shape_cache,
                active,
            )?
            .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                machine: function.machine,
                place: argument.place,
            });
        } else {
            return Err(LoweringError::UnknownStructuralArgumentPlace {
                machine: function.machine,
                place: argument.place,
            });
        };
    // Fixed byte windows and element views are call presentations of original
    // backing, not immutable subslice producers or a new owned structural value.
    if let Some(root) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        && let Some(loan) = fixed_array_view_argument(
            argument,
            callee_parameter,
            shape,
            destination,
            callee,
            root,
            source_shape.byte_size,
            source_placement.clone().into(),
            structural_types,
            shape_cache,
            active,
        )?
    {
        return Ok(loan);
    }
    // Borrowing selects the same original storage for every permitted access
    // attenuation. The existing resolver reconstructs each field/index hop.
    let exact_borrowed_projection = argument.access == callee_parameter.access
        && callee_parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameters_by_place
            .get(&argument.place)
            .is_some_and(|source| {
                matches!(
                    (source.access, argument.access),
                    (
                        StructuralAccess::MutableBorrow,
                        StructuralAccess::SharedBorrow
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    ) | (
                        StructuralAccess::SharedBorrow,
                        StructuralAccess::SharedBorrow
                    ) | (
                        StructuralAccess::WriteOnlyBorrow,
                        StructuralAccess::WriteOnlyBorrow
                    )
                ) && source.multiplicity == StructuralMultiplicity::Unrestricted
            })
        && argument
            .path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)));
    let mut byte_field_capacity = None;
    let (projected_type, projected_shape, source_byte_offset, fixed_array_length, element_stride) =
        match argument.path.as_slice() {
            [] => (source_structural_type, source_shape, 0, None, None),
            path if argument.access == StructuralAccess::Owned
                && callee_parameter.multiplicity == StructuralMultiplicity::Affine
                && (result_source.is_some()
                    || parameters_by_place
                        .get(&argument.place)
                        .is_some_and(|source| {
                            source.access == StructuralAccess::Owned
                                && source.multiplicity == StructuralMultiplicity::Affine
                        })) =>
            {
                let (selected_type, selected_shape, offset) = resolve_structural_projection_path(
                    source_structural_type,
                    path,
                    structural_types,
                    shape_cache,
                    active,
                )?;
                let (length, stride) =
                    super::super::structural_layout::root_array_projection_metadata(
                        source_structural_type,
                        structural_types,
                        shape_cache,
                        active,
                    )?;
                (selected_type, selected_shape, offset, length, stride)
            }
            path if exact_borrowed_projection => {
                let (projected_type, projected_shape, offset) = resolve_structural_projection_path(
                    source_structural_type,
                    path,
                    structural_types,
                    shape_cache,
                    active,
                )
                .map_err(|_| {
                    LoweringError::StructuralCallArgumentTypeMismatch {
                        callee,
                        place: argument.place,
                    }
                })?;
                if !matches!(
                    structural_types
                        .get(&projected_type)
                        .map(|declaration| &declaration.shape),
                    Some(
                        StructuralTypeShape::PrimitiveScalar(_)
                            | StructuralTypeShape::Record { .. }
                    )
                ) {
                    return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                        callee,
                        place: argument.place,
                    });
                }
                (projected_type, projected_shape, offset, None, None)
            }
            [StructuralPathSegment::FixedIndex(index)] => {
                let declaration = structural_types
                    .get(&source_structural_type)
                    .copied()
                    .ok_or(LoweringError::UnknownStructuralType(source_structural_type))?;
                let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                    return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                        callee,
                        place: argument.place,
                    });
                };
                if *index >= length {
                    return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                        callee,
                        place: argument.place,
                    });
                }
                let element_shape =
                    structural_shape(element, structural_types, shape_cache, active)?;
                let stride = checked_align_up_u32(
                    element_shape.byte_size,
                    u32::from(element_shape.alignment),
                )
                .ok_or(LoweringError::StructuralTypeTooLarge(
                    source_structural_type,
                ))?;
                let offset = u64::from(stride)
                    .checked_mul(*index)
                    .and_then(|offset| u32::try_from(offset).ok())
                    .ok_or(LoweringError::StructuralTypeTooLarge(
                        source_structural_type,
                    ))?;
                (element, element_shape, offset, Some(length), Some(stride))
            }
            path @ [StructuralPathSegment::Field(_), ..]
                if path
                    .iter()
                    .all(|segment| matches!(segment, StructuralPathSegment::Field(_))) =>
            {
                match resolve_structural_field_path(
                    source_structural_type,
                    path,
                    structural_types,
                    shape_cache,
                    active,
                ) {
                    Ok((field_type, field_shape, offset)) => {
                        (field_type, field_shape, offset, None, None)
                    }
                    Err(_) => {
                        // An inline bounded byte field has no structural type
                        // identity of its own; its record still supplies exact
                        // offset and capacity for a borrowed-view presentation.
                        let Some((field_offset, capacity)) = bounded_byte_field_geometry(
                            source_structural_type,
                            path,
                            structural_types,
                            shape_cache,
                            active,
                        )?
                        else {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee,
                                place: argument.place,
                            });
                        };
                        byte_field_capacity = Some(capacity);
                        (
                            callee_parameter.structural_type,
                            structural_shape(
                                callee_parameter.structural_type,
                                structural_types,
                                shape_cache,
                                active,
                            )?,
                            field_offset,
                            None,
                            None,
                        )
                    }
                }
            }
            _ => {
                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                    callee,
                    place: argument.place,
                });
            }
        };
    // A bounded inline byte field lends its live bytes through a fresh
    // borrowed-view descriptor. `source_byte_offset` names the field itself:
    // the descriptor's length is the live length word stored there and the
    // bytes begin eight bytes later. Declared capacity bounds the presentation;
    // it is never the descriptor's length.
    if let Some(capacity) = byte_field_capacity {
        let source_is_subloan = parameters_by_place
            .get(&argument.place)
            .is_some_and(|source| {
                source.multiplicity == StructuralMultiplicity::Unrestricted
                    && source.projected_qualifications.is_empty()
                    && match argument.access {
                        StructuralAccess::SharedBorrow => matches!(
                            source.access,
                            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
                        ),
                        StructuralAccess::MutableBorrow => {
                            source.access == StructuralAccess::MutableBorrow
                        }
                        StructuralAccess::WriteOnlyBorrow => matches!(
                            source.access,
                            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                        ),
                        _ => false,
                    }
            });
        let presented_is_byte_view = matches!(
            structural_types
                .get(&callee_parameter.structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            ))
        );
        let presented_is_owner = structural_types
            .get(&callee_parameter.structural_type)
            .is_some_and(|declaration| {
                declaration.shape
                    == StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                    )
            });
        let expected_shape = if presented_is_owner {
            structural_parameter_shape(projected_shape, callee_parameter.access)
        } else {
            ValueShape::borrowed_reference(16, 8)
        };
        if !(presented_is_byte_view || presented_is_owner)
            || argument.access != callee_parameter.access
            || !matches!(
                argument.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
            || callee_parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || (!presented_is_owner && !callee_parameter.qualifications.is_empty())
            || !callee_parameter.projected_qualifications.is_empty()
            || !source_is_subloan
            || (!presented_is_owner && argument.access == StructuralAccess::WriteOnlyBorrow)
            || shape != expected_shape
            || u64::from(source_byte_offset)
                .checked_add(8)
                .and_then(|end| end.checked_add(capacity))
                .is_none_or(|end| end > u64::from(source_shape.byte_size))
        {
            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                callee,
                place: argument.place,
            });
        }
        return Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: source_structural_type,
            structural_type: callee_parameter.structural_type,
            shape,
            source_byte_offset,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source: source_placement.clone().into(),
            destination: destination.clone(),
        });
    }
    let projected_parameter_shape =
        structural_parameter_shape(projected_shape, callee_parameter.access);
    if projected_type != callee_parameter.structural_type
        || argument.access != callee_parameter.access
        || projected_parameter_shape != shape
        || shape
            .byte_size
            .checked_add(source_byte_offset)
            .is_none_or(|end| end > source_shape.byte_size)
    {
        return Err(LoweringError::StructuralCallArgumentTypeMismatch {
            callee,
            place: argument.place,
        });
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: source_structural_type,
        structural_type: projected_type,
        shape,
        source_byte_offset,
        fixed_array_length,
        element_stride,
        indices: Vec::new(),
        source: source_placement.clone().into(),
        destination: destination.clone(),
    })
}

/// A whole owned local array lent by borrow presents the same fixed backing an
/// incoming borrowed parameter would. Terminal verification admits exactly this
/// loan for an unrestricted, unqualified, claim-free operation result named with
/// an empty path; the presented declaration carries the argument's own access
/// for window geometry while the home itself stays owned by this activation.
/// `Ok(None)` means the home is not lendable through this parameter's view.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn local_array_loan(
    argument: &terminal_psi::StructuralArgument,
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    shape: ValueShape,
    destination: &ValuePlacement,
    callee: MachineId,
    home: &TargetStructuralHomeRequirement,
    structural_types: &StructuralTypeLookup<'_>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<TargetStructuralArgument>, LoweringError> {
    let Some((producer, result)) = home.operation_result() else {
        return Ok(None);
    };
    if result.place != argument.place
        || !argument.path.is_empty()
        || !matches!(
            argument.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Ok(None);
    }
    let presented = terminal_psi::StructuralParameterDeclaration {
        place: result.place,
        position: callee_parameter.position,
        is_self: false,
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        access: argument.access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    fixed_array_view_argument(
        argument,
        callee_parameter,
        shape,
        destination,
        callee,
        &presented,
        home.layout.shape().byte_size,
        TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        },
        structural_types,
        shape_cache,
        active,
    )
}

/// Present a fixed array's original backing through a borrowed byte or element
/// view. `root` declares that backing as the call presents it: an incoming
/// parameter, or a local home presented through the loan. The callee receives a
/// descriptor over the exact extent Terminal proved, while `source` still names
/// the caller's own storage rather than a descriptor allocation. A byte window
/// may select a constant range of a field; an element view lends the whole
/// array at `argument.path` with the element's aligned stride. `Ok(None)` means
/// the argument is no fixed-array view of `root`.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn fixed_array_view_argument(
    argument: &terminal_psi::StructuralArgument,
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    shape: ValueShape,
    destination: &ValuePlacement,
    callee: MachineId,
    root: &terminal_psi::StructuralParameterDeclaration,
    root_byte_size: u32,
    source: TargetStructuralArgumentSource,
    structural_types: &StructuralTypeLookup<'_>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<TargetStructuralArgument>, LoweringError> {
    let mismatch = || LoweringError::StructuralCallArgumentTypeMismatch {
        callee,
        place: argument.place,
    };
    let too_large = || LoweringError::StructuralTypeTooLarge(root.structural_type);
    let (source_byte_offset, length, stride, backing_end) = if let Some(window) =
        terminal_semantics::fixed_byte_array_window(
            structural_types.values().copied(),
            root,
            argument,
            callee_parameter,
        ) {
        let backing_offset = if window.backing_path.is_empty() {
            0
        } else {
            resolve_structural_field_path(
                root.structural_type,
                window.backing_path,
                structural_types,
                shape_cache,
                active,
            )?
            .2
        };
        let offset = u64::from(backing_offset)
            .checked_add(window.offset)
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or_else(too_large)?;
        let end = u64::from(backing_offset).checked_add(window.backing_length);
        (offset, window.length, 1, end)
    } else if let Some(length) = terminal_semantics::fixed_element_array_extent(
        structural_types.values().copied(),
        root,
        argument,
        callee_parameter,
    ) {
        let (array_type, backing_offset) = if argument.path.is_empty() {
            (root.structural_type, 0)
        } else {
            let (array_type, _, backing_offset) = resolve_structural_field_path(
                root.structural_type,
                &argument.path,
                structural_types,
                shape_cache,
                active,
            )?;
            (array_type, backing_offset)
        };
        let Some(StructuralTypeShape::FixedArray { element, .. }) = structural_types
            .get(&array_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(mismatch());
        };
        let element_shape = structural_shape(*element, structural_types, shape_cache, active)?;
        let stride =
            checked_align_up_u32(element_shape.byte_size, u32::from(element_shape.alignment))
                .ok_or_else(too_large)?;
        let end = u64::from(stride)
            .checked_mul(length)
            .and_then(|extent| u64::from(backing_offset).checked_add(extent));
        (backing_offset, length, stride, end)
    } else {
        return Ok(None);
    };
    if shape != ValueShape::borrowed_reference(16, 8)
        || backing_end.is_none_or(|end| end > u64::from(root_byte_size))
    {
        return Err(mismatch());
    }
    Ok(Some(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root.structural_type,
        structural_type: callee_parameter.structural_type,
        shape,
        source_byte_offset,
        fixed_array_length: Some(length),
        element_stride: Some(stride),
        indices: Vec::new(),
        source,
        destination: destination.clone(),
    }))
}
