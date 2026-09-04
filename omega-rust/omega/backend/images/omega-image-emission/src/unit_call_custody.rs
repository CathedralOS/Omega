//! Exact internal Unit-call custody and projected-copy replay.
//!
//! This module validates retained call identity, provenance/code ownership,
//! calling-policy placements, projected structural arguments, claim transfers,
//! exact copy bytes, and call-span containment. It neither assigns layouts nor
//! emits relocations or executable bytes.

use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_machine_code::{
    MachineCodeFunction, SemanticCodeAttribution, SemanticCodeSite, StructuralReturnRecord,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{
    CallSiteOwner, MixedStructuralScalarFunctionAbi, TerminalPsiProvenance,
};
use psi_core::MachineId;

use super::instruction_loads::{
    aarch64_terminal_register, expected_aarch64_memory_load, expected_aarch64_stack_load,
    expected_x86_memory_load, expected_x86_stack_load, x86_terminal_register,
};
use super::unit_scalar_call_custody::{
    expected_aarch64_stack_store, expected_argument_bytes, expected_x86_stack_store,
    validate_source,
};
use super::{ObjectError, ObjectScalarCallStack, ObjectUnitCallStack, ObjectUnitStack};

pub(super) fn validate_unit_affine_scalar_records(
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(function.machine);
    let mut operations = std::collections::BTreeSet::new();
    let mut places = std::collections::BTreeSet::new();
    for record in &function.unit_affine_scalar_records {
        let exact_attribution = function
            .semantic_code_attribution
            .iter()
            .filter(|attribution| {
                attribution.site == SemanticCodeSite::Operation(record.psi_operation)
                    && attribution.operation_ordinal == record.operation_ordinal
                    && attribution.byte_count == 0
            });
        let exact_use = function
            .internal_unit_calls
            .iter()
            .filter(|call| call.operation_ordinal > record.operation_ordinal)
            .flat_map(|call| &call.arguments)
            .filter(|argument| {
                argument.place == record.result.place
                    && argument.path.is_empty()
                    && argument.access == psi_terminal::StructuralAccess::Owned
                    && argument.root_structural_type == record.result.structural_type
                    && argument.structural_type == record.result.structural_type
                    && argument.shape == record.shape
                    && argument.source.shape == record.shape
                    && argument.source.locations.is_empty()
                    && argument.source_byte_offset == 0
                    && argument.source_home_byte_offset == 0
            })
            .count();
        if !operations.insert(record.psi_operation)
            || !places.insert(record.result.place)
            || record.shape != ValueShape::integer(8, 8)
            || record.result.multiplicity != psi_terminal::StructuralMultiplicity::Affine
            || !record.result.qualifications.is_empty()
            || !record.result.projected_qualifications.is_empty()
            || !record.result.claims.is_empty()
            || !matches!(record.value, psi_core::IntegerValue::Signed(value)
                if i64::try_from(value).is_ok())
            || function
                .provenance
                .operations
                .iter()
                .filter(|candidate| **candidate == record.psi_operation)
                .count()
                != 1
            || exact_attribution.count() != 1
            || exact_use != 1
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn validate_mixed_structural_scalar_abi(
    target: NativeTarget,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let Some(abi) = function.mixed_structural_scalar_abi.as_ref() else {
        return Ok(());
    };
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(function.machine);
    let scalar_shapes = abi
        .scalar_parameters
        .iter()
        .map(|parameter| fixed_integer_shape(parameter.scalar_type).ok_or_else(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    let result_shape = super::unit_scalar_call_custody::scalar_home_shape(abi.result.scalar_type)
        .ok_or_else(invalid)?;
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(
                    abi.structural_parameters
                        .iter()
                        .map(|parameter| parameter.shape),
                )
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(|_| invalid())?;
    let scalar_count = abi.scalar_parameters.len();
    let structural_count = abi.structural_parameters.len();
    if structural_count == 0
        || function.fixed_integer_scalar_abi.is_some()
        || function.unit_scalar_abi.is_some()
        || function.scalar_stack.is_none()
        || function.unit_stack.is_some()
        || expected != abi.call_plan
        || abi.call_plan.parameters.len() != scalar_count + structural_count
        || abi.call_plan.result.as_ref() != Some(&abi.result.placement)
        || abi.result.placement.shape != result_shape
        || abi
            .scalar_parameters
            .iter()
            .zip(&scalar_shapes)
            .zip(&abi.call_plan.parameters[..scalar_count])
            .any(|((parameter, shape), placement)| {
                parameter.placement != *placement || placement.shape != *shape
            })
        || abi
            .structural_parameters
            .iter()
            .zip(&abi.call_plan.parameters[scalar_count..])
            .any(|(parameter, placement)| {
                parameter.placement != *placement || placement.shape != parameter.shape
            })
        || abi
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.value)
            .chain(std::iter::once(abi.result.value))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != scalar_count + 1
        || abi
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != structural_count
        || function.scalar_structural_parameters.len() != structural_count
        || function.scalar_structural_parameter_homes.len() != structural_count
        || function
            .scalar_structural_parameters
            .iter()
            .zip(&function.scalar_structural_parameter_homes)
            .zip(&abi.structural_parameters)
            .any(|((parameter, home), retained)| {
                parameter.place != retained.place
                    || parameter.structural_type != retained.structural_type
                    || parameter.multiplicity != retained.multiplicity
                    || parameter.access != retained.access
                    || parameter.shape != retained.shape
                    || home.place != retained.place
                    || home.structural_type != retained.structural_type
                    || home.multiplicity != retained.multiplicity
                    || home.access != retained.access
                    || home.shape != retained.shape
                    || home.source != retained.placement
                    || home.byte_offset != 0
                    || home.indirect
                        != matches!(
                            retained.placement.locations.as_slice(),
                            [omega_calling_conventions::ValueLocation::Indirect { .. }]
                        )
            })
    {
        return Err(invalid());
    }
    Ok(())
}

fn fixed_integer_shape(integer: psi_core::IntegerType) -> Option<ValueShape> {
    if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = integer.bits() / 8;
    Some(ValueShape::integer(bytes, bytes))
}

pub(super) fn unit_scalar_shape(scalar_type: psi_core::ScalarType) -> Option<ValueShape> {
    match scalar_type {
        psi_core::ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        psi_core::ScalarType::Integer(integer) => fixed_integer_shape(integer),
        psi_core::ScalarType::IeeeFloat(_) => None,
    }
}

pub(super) fn structural_result_matches_return(
    result: &omega_machine_code::InternalStructuralCallResult,
    returned: &StructuralReturnRecord,
) -> bool {
    let common = result.operation_result.structural_type == returned.result.structural_type
        && result.operation_result.multiplicity == returned.result.multiplicity
        && result.operation_result.qualifications == returned.result.qualifications
        && result.operation_result.projected_qualifications
            == returned.result.projected_qualifications
        && result.function_result.structural_type == returned.result.structural_type
        && result.function_result.multiplicity == returned.result.multiplicity
        && result.function_result.qualifications == returned.result.qualifications
        && result.function_result.projected_qualifications
            == returned.result.projected_qualifications
        && result.caller_result_placement == returned.result_placement
        && result.callee_result_placement == returned.result_placement;
    if !common {
        return false;
    }
    match returned.result.multiplicity {
        psi_terminal::StructuralMultiplicity::Linear => {
            result.returned_claim_transfers.len() == 1
                && returned.returned_claims.as_slice()
                    == [result.returned_claim_transfers[0].callee_claim]
                && result.operation_result.claims.len() == 1
                && result.operation_result.claims[0].path.is_empty()
                && result.operation_result.claims[0].claim
                    == result.returned_claim_transfers[0].caller_claim
                && result.returned_claims.as_slice()
                    == [result.returned_claim_transfers[0].caller_claim]
        }
        psi_terminal::StructuralMultiplicity::Affine => {
            returned.scalar_parameters.len() == 1
                && returned.returned_claims.is_empty()
                && returned.result.qualifications.is_empty()
                && returned.result.projected_qualifications.is_empty()
                && result.operation_result.claims.is_empty()
                && result.returned_claim_transfers.is_empty()
                && result.returned_claims.is_empty()
        }
        psi_terminal::StructuralMultiplicity::Unrestricted => false,
    }
}

pub(super) fn exact_write_only_projection(
    argument: &omega_machine_code::InternalUnitCallArgumentRecord,
    source: &omega_machine_code::UnitParameterHomeRecord,
    destination: &omega_machine_code::UnitParameterRecord,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
) -> bool {
    let Some(first_index) = argument
        .path
        .iter()
        .position(|segment| matches!(segment, psi_terminal::StructuralPathSegment::FixedIndex(_)))
    else {
        return false;
    };
    if argument.access != psi_terminal::StructuralAccess::WriteOnlyBorrow
        || source.access != psi_terminal::StructuralAccess::WriteOnlyBorrow
        || destination.access != psi_terminal::StructuralAccess::WriteOnlyBorrow
        || source.multiplicity != psi_terminal::StructuralMultiplicity::Unrestricted
        || destination.multiplicity != psi_terminal::StructuralMultiplicity::Unrestricted
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
        || !argument.path[..first_index].iter().all(|segment| {
            matches!(segment,
                psi_terminal::StructuralPathSegment::Field(identity) if !identity.is_empty())
        })
        || !argument.path[first_index..]
            .iter()
            .all(|segment| matches!(segment, psi_terminal::StructuralPathSegment::FixedIndex(_)))
    {
        return false;
    }
    let Some((leaf_type, leaf_shape, byte_offset)) =
        super::structural_condition_layout::replay_structural_projection(
            source.structural_type,
            &argument.path,
            structural_types,
        )
    else {
        return false;
    };
    let Some(root_shape) = super::structural_condition_layout::replay_structural_value_shape(
        source.structural_type,
        structural_types,
    ) else {
        return false;
    };
    let leaf_is_primitive = structural_types.iter().any(|declaration| {
        declaration.id == leaf_type
            && matches!(
                declaration.shape,
                psi_terminal::StructuralTypeShape::PrimitiveScalar(_)
            )
    });
    let expected_shape = ValueShape::borrowed_reference(leaf_shape.byte_size, leaf_shape.alignment);
    leaf_is_primitive
        && argument.place == source.place
        && argument.root_structural_type == source.structural_type
        && argument.structural_type == leaf_type
        && destination.structural_type == leaf_type
        && argument.shape == expected_shape
        && destination.shape == expected_shape
        && source.shape
            == ValueShape::borrowed_reference(root_shape.byte_size, root_shape.alignment)
        && argument.source_byte_offset == byte_offset
        && argument.source == source.source
        && argument.source.shape == source.shape
        && argument.source_home_byte_offset == source.byte_offset
        && source.indirect
        && matches!(
            source.source.locations.as_slice(),
            [omega_calling_conventions::ValueLocation::Indirect { .. }]
        )
}

pub(super) fn validate_internal_unit_call_custody(
    target: NativeTarget,
    function: &MachineCodeFunction,
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    function_bytes: &[u8],
    attribution: &[SemanticCodeAttribution],
    relocations: &[omega_machine_code::InternalCallRelocation],
    internal_unit_calls: &[omega_machine_code::InternalUnitCallRecord],
    parameter_homes: &[omega_machine_code::UnitParameterHomeRecord],
    validated_function_stack: Option<&ObjectUnitStack>,
    validated_call_stack: Option<&ObjectUnitCallStack>,
    validated_scalar_call_stack: Option<&ObjectScalarCallStack>,
    callee_unit_scalar_abi: Option<&omega_machine_code::UnitScalarFunctionAbiRecord>,
    callee_unit_parameters: &[omega_machine_code::UnitParameterRecord],
    callee_mixed_abi: Option<&MixedStructuralScalarFunctionAbi>,
    callee_structural_return: Option<&StructuralReturnRecord>,
    custody: &omega_machine_code::InternalUnitCallRecord,
    affine_cleanup: Option<&omega_machine_code::UnitAffineCleanupRecord>,
    fully_consumed_affine_pair: bool,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
    if function.machine != machine || function.bytes.as_slice() != function_bytes {
        return Err(invalid());
    }
    let Some(relocation) = relocations.iter().find(|relocation| {
        relocation.owner == custody.owner
            && relocation.target == custody.target
            && (relocation.unit_stack.is_some()
                || (affine_cleanup.is_some()
                    && matches!(relocation.owner, CallSiteOwner::CleanupAction { .. })
                    && relocation.scalar_stack.is_some()))
    }) else {
        return Err(invalid());
    };
    if validated_call_stack.is_none() == validated_scalar_call_stack.is_none() {
        return Err(invalid());
    }
    let end = custody
        .code_offset
        .checked_add(custody.byte_count)
        .ok_or_else(invalid)?;
    let relocation_end = relocation.offset.checked_add(4).ok_or_else(invalid)?;
    let linkage_bytes = match target.architecture {
        Architecture::X86_64 => 8,
        Architecture::Aarch64 => 0,
    };
    if custody.result.is_some() && custody.structural_result.is_some() {
        return Err(invalid());
    }
    if custody.scalar_arguments.is_empty()
        && custody.arguments.is_empty()
        && custody.claim_transfers.is_empty()
    {
        if custody.result.is_some() || custody.structural_result.is_some() {
            return Err(invalid());
        }
        let owner_valid = match custody.owner {
            CallSiteOwner::Operation(operation) => {
                provenance.operations.contains(&operation)
                    && attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.site == SemanticCodeSite::Operation(operation)
                                && attribution.operation_ordinal == custody.operation_ordinal
                                && attribution.code_offset == custody.code_offset
                                && attribution.byte_count == custody.byte_count
                        })
                        .count()
                        == 1
            }
            CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                let Some(cleanup) = affine_cleanup else {
                    return Err(invalid());
                };
                let Some(psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal)) =
                    usize::try_from(action_ordinal)
                        .ok()
                        .and_then(|ordinal| cleanup.actions.get(ordinal))
                else {
                    return Err(invalid());
                };
                let cleanup_end = cleanup
                    .code_offset
                    .checked_add(cleanup.byte_count)
                    .ok_or_else(invalid)?;
                provenance.edges.contains(&edge)
                    && cleanup.psi_edge == edge
                    && nominal.cleanup_machine == custody.target
                    && cleanup.code_offset <= custody.code_offset
                    && end <= cleanup_end
                    && attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.site == SemanticCodeSite::Edge(edge)
                                && attribution.operation_ordinal == custody.operation_ordinal
                                && attribution.code_offset == cleanup.code_offset
                                && attribution.byte_count == cleanup.byte_count
                        })
                        .count()
                        == 1
            }
        };
        if custody.byte_count == 0
            || custody.code_offset > relocation.offset
            || relocation_end > end
            || !owner_valid
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let validated_function_stack = validated_function_stack.ok_or_else(invalid)?;
    let validated_call_stack = validated_call_stack.ok_or_else(invalid)?;
    let expected_call_stack_bytes = validated_call_stack
        .transient_bytes
        .checked_sub(linkage_bytes)
        .ok_or_else(invalid)?;
    let CallSiteOwner::Operation(operation) = custody.owner else {
        return Err(invalid());
    };
    let operation_position = provenance
        .operations
        .iter()
        .position(|candidate| *candidate == operation)
        .ok_or_else(invalid)?;
    let callee_mixed_structural_return =
        callee_structural_return.filter(|returned| !returned.scalar_parameters.is_empty());
    if usize::from(callee_unit_scalar_abi.is_some())
        + usize::from(callee_mixed_abi.is_some())
        + usize::from(callee_mixed_structural_return.is_some())
        > 1
    {
        return Err(invalid());
    }
    let expected_plan = omega_calling_conventions::evaluate_call_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &omega_calling_conventions::CallSignature {
            parameters: if let Some(abi) = callee_unit_scalar_abi {
                abi.parameters
                    .iter()
                    .map(|parameter| unit_scalar_shape(parameter.scalar_type).ok_or_else(invalid))
                    .chain(
                        callee_unit_parameters
                            .iter()
                            .map(|parameter| Ok(parameter.shape)),
                    )
                    .collect::<Result<Vec<_>, _>>()?
            } else if let Some(abi) = callee_mixed_abi {
                abi.scalar_parameters
                    .iter()
                    .map(|parameter| fixed_integer_shape(parameter.scalar_type).ok_or_else(invalid))
                    .chain(
                        abi.structural_parameters
                            .iter()
                            .map(|parameter| Ok(parameter.shape)),
                    )
                    .collect::<Result<Vec<_>, _>>()?
            } else if let Some(returned) = callee_mixed_structural_return {
                returned
                    .scalar_parameters
                    .iter()
                    .map(|parameter| fixed_integer_shape(parameter.scalar_type).ok_or_else(invalid))
                    .chain(
                        returned
                            .parameter_placements
                            .iter()
                            .map(|placement| Ok(placement.shape)),
                    )
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                custody
                    .arguments
                    .iter()
                    .map(|argument| argument.shape)
                    .collect()
            },
            result: if let Some(result) = custody.result {
                let bytes = match result {
                    psi_core::ScalarType::Boolean => 1,
                    psi_core::ScalarType::Integer(integer) => integer.bits().div_ceil(8),
                    psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32) => 4,
                    psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64) => 8,
                };
                Some(match result {
                    psi_core::ScalarType::IeeeFloat(_) => {
                        omega_calling_conventions::ValueShape::float(bytes)
                    }
                    _ => omega_calling_conventions::ValueShape::integer(
                        bytes,
                        bytes.next_power_of_two().min(8),
                    ),
                })
            } else if custody.structural_result.is_some() {
                callee_structural_return.map(|returned| returned.shape)
            } else {
                None
            },
        },
    )
    .map_err(|_| invalid())?;
    let exact_write_only_argument =
        |index: usize, argument: &omega_machine_code::InternalUnitCallArgumentRecord| {
            parameter_homes
                .iter()
                .find(|home| home.place == argument.place)
                .zip(callee_unit_parameters.get(index))
                .zip(affine_cleanup)
                .is_some_and(|((source, destination), cleanup)| {
                    exact_write_only_projection(
                        argument,
                        source,
                        destination,
                        &cleanup.structural_types,
                    )
                })
        };
    if let Some(abi) = callee_unit_scalar_abi {
        if expected_plan != abi.call_plan
            || custody.result.is_some()
            || custody.structural_result.is_some()
            || custody.scalar_arguments.len() != abi.parameters.len()
            || custody.arguments.len() != callee_unit_parameters.len()
            || custody
                .scalar_arguments
                .iter()
                .zip(&abi.parameters)
                .enumerate()
                .any(|(index, (argument, parameter))| {
                    usize::try_from(argument.parameter_index) != Ok(index)
                        || argument.destination != parameter.placement
                        || argument.source.scalar_type() != parameter.scalar_type
                })
            || custody
                .arguments
                .iter()
                .zip(callee_unit_parameters)
                .zip(&abi.call_plan.parameters[abi.parameters.len()..])
                .enumerate()
                .any(|(index, ((argument, parameter), placement))| {
                    !exact_write_only_argument(index, argument)
                        && (argument.root_structural_type != parameter.structural_type
                            || argument.structural_type != parameter.structural_type
                            || argument.access != parameter.access
                            || argument.shape != parameter.shape
                            || argument.destination != *placement)
                })
        {
            return Err(invalid());
        }
        validate_mixed_argument_bytes_and_order(
            target,
            function,
            validated_function_stack,
            &expected_plan,
            relocation,
            custody,
        )?;
    } else if let Some(abi) = callee_mixed_abi {
        if expected_plan != abi.call_plan
            || custody.result != Some(abi.result.scalar_type)
            || custody.scalar_arguments.len() != abi.scalar_parameters.len()
            || custody.arguments.len() != abi.structural_parameters.len()
            || custody
                .scalar_arguments
                .iter()
                .zip(&abi.scalar_parameters)
                .enumerate()
                .any(|(index, (argument, parameter))| {
                    usize::try_from(argument.parameter_index) != Ok(index)
                        || argument.destination != parameter.placement
                        || argument.source.scalar_type()
                            != psi_core::ScalarType::Integer(parameter.scalar_type)
                })
            || custody
                .arguments
                .iter()
                .zip(&abi.structural_parameters)
                .any(|(argument, parameter)| {
                    !argument.path.is_empty()
                        || argument.root_structural_type != parameter.structural_type
                        || argument.access != parameter.access
                        || argument.structural_type != parameter.structural_type
                        || argument.shape != parameter.shape
                        || argument.destination != parameter.placement
                })
        {
            return Err(invalid());
        }
        validate_mixed_argument_bytes_and_order(
            target,
            function,
            validated_function_stack,
            &expected_plan,
            relocation,
            custody,
        )?;
    } else if let Some(returned) = callee_mixed_structural_return {
        if custody.result.is_some()
            || custody.structural_result.is_none()
            || expected_plan.parameters.len()
                != returned.scalar_parameters.len() + returned.parameters.len()
            || expected_plan.parameters[..returned.scalar_parameters.len()]
                != returned
                    .scalar_parameters
                    .iter()
                    .map(|parameter| parameter.placement.clone())
                    .collect::<Vec<_>>()
            || expected_plan.parameters[returned.scalar_parameters.len()..]
                != returned.parameter_placements
            || expected_plan.result.as_ref() != Some(&returned.result_placement)
            || custody.scalar_arguments.len() != returned.scalar_parameters.len()
            || custody.arguments.len() != returned.parameters.len()
            || custody
                .scalar_arguments
                .iter()
                .zip(&returned.scalar_parameters)
                .enumerate()
                .any(|(index, (argument, parameter))| {
                    usize::try_from(argument.parameter_index) != Ok(index)
                        || argument.destination != parameter.placement
                        || argument.source.scalar_type()
                            != psi_core::ScalarType::Integer(parameter.scalar_type)
                })
            || custody
                .arguments
                .iter()
                .zip(&returned.parameters)
                .zip(&returned.parameter_placements)
                .any(|((argument, parameter), placement)| {
                    !argument.path.is_empty()
                        || argument.root_structural_type != parameter.structural_type
                        || argument.access != parameter.access
                        || argument.structural_type != parameter.structural_type
                        || argument.shape != placement.shape
                        || argument.destination != *placement
                })
        {
            return Err(invalid());
        }
        validate_mixed_argument_bytes_and_order(
            target,
            function,
            validated_function_stack,
            &expected_plan,
            relocation,
            custody,
        )?;
    }
    let projected_argument_indexes = custody
        .arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| (!argument.path.is_empty()).then_some(index))
        .collect::<std::collections::BTreeSet<_>>();
    let transferred_argument_indexes = custody
        .claim_transfers
        .iter()
        .filter_map(|transfer| usize::try_from(transfer.argument_index).ok())
        .collect::<std::collections::BTreeSet<_>>();
    let projected_home = if projected_argument_indexes.is_empty() {
        None
    } else {
        let [home] = parameter_homes else {
            return Err(invalid());
        };
        if home.byte_offset != 0
            || home.indirect
                != matches!(
                    home.source.locations.as_slice(),
                    [omega_calling_conventions::ValueLocation::Indirect { .. }]
                )
        {
            return Err(invalid());
        }
        let caller_scalar_shapes = function.unit_scalar_abi.as_ref().map_or_else(
            || Ok(Vec::new()),
            |abi| {
                abi.parameters
                    .iter()
                    .map(|parameter| unit_scalar_shape(parameter.scalar_type).ok_or_else(invalid))
                    .collect::<Result<Vec<_>, _>>()
            },
        )?;
        let expected_caller_plan = omega_calling_conventions::evaluate_call_plan(
            omega_calling_conventions::CallingPolicy::native_for_target(target),
            &omega_calling_conventions::CallSignature {
                parameters: caller_scalar_shapes
                    .into_iter()
                    .chain(std::iter::once(home.shape))
                    .collect(),
                result: None,
            },
        )
        .map_err(|_| invalid())?;
        if function
            .unit_scalar_abi
            .as_ref()
            .is_some_and(|abi| abi.call_plan != expected_caller_plan)
            || expected_caller_plan.parameters.last() != Some(&home.source)
        {
            return Err(invalid());
        }
        let stored_bytes = if home.indirect {
            8
        } else {
            u32::from(home.shape.byte_size)
        };
        let expected_frame_bytes = match target.architecture {
            Architecture::X86_64 => stored_bytes.next_multiple_of(16),
            Architecture::Aarch64 => stored_bytes
                .next_multiple_of(8)
                .checked_add(8)
                .map(|bytes| bytes.next_multiple_of(16))
                .ok_or_else(invalid)?,
        };
        if validated_function_stack.frame_bytes != expected_frame_bytes {
            return Err(invalid());
        }
        Some(home)
    };
    let scalar_count = custody.scalar_arguments.len();
    if custody.byte_count == 0
        || custody.code_offset > relocation.offset
        || relocation_end > end
        || !provenance.operations.contains(&operation)
        || attribution
            .iter()
            .filter(|attribution| {
                attribution.site == SemanticCodeSite::Operation(operation)
                    && attribution.operation_ordinal == custody.operation_ordinal
                    && attribution.code_offset == custody.code_offset
                    && attribution.byte_count == custody.byte_count
            })
            .count()
            != 1
        || expected_plan.parameters.len() != scalar_count + custody.arguments.len()
        || custody.arguments.windows(2).any(|pair| {
            pair[0]
                .code_offset
                .checked_add(pair[0].byte_count)
                .is_none_or(|end| end > pair[1].code_offset)
        })
        || custody
            .arguments
            .iter()
            .zip(&expected_plan.parameters[scalar_count..])
            .enumerate()
            .any(|(argument_index, (argument, destination))| {
                let parameter_source = parameter_homes
                    .iter()
                    .find(|home| home.place == argument.place)
                    .is_some_and(|home| {
                        argument.root_structural_type == home.structural_type
                            && argument.source == home.source
                            && argument.source.shape == home.shape
                            && argument.source_home_byte_offset == home.byte_offset
                            && (argument.path.is_empty()
                                || projected_home.is_some_and(|projected| {
                                    projected.place == home.place
                                        && projected.structural_type == home.structural_type
                                }))
                    });
                let local_source = affine_cleanup
                    .and_then(|cleanup| {
                        cleanup.locals.iter().find(|(_, place, structural_type)| {
                            place.id == argument.place
                                && argument.path.is_empty()
                                && argument.access == psi_terminal::StructuralAccess::Owned
                                && argument.root_structural_type == structural_type.id
                                && argument.structural_type == structural_type.id
                                && argument.shape
                                    == omega_calling_conventions::ValueShape::integer(0, 1)
                                && argument.source.shape == argument.shape
                                && argument.source.locations.is_empty()
                                && argument.destination.shape == argument.shape
                                && argument.destination.locations.is_empty()
                                && argument.source_home_byte_offset == 0
                                && matches!(
                                    place.kind,
                                    psi_core::StructuralPlaceKind::TrivialAffineLocal {
                                        structural_type: local_type,
                                        construction: None,
                                        ..
                                    } if local_type == structural_type.id
                                )
                                && matches!(
                                    structural_type.shape,
                                    psi_terminal::StructuralTypeShape::Record { ref fields }
                                        if fields.is_empty()
                                )
                        })
                    })
                    .is_some_and(|(establishment, _, _)| {
                        provenance
                            .operations
                            .iter()
                            .position(|candidate| candidate == establishment)
                            .is_some_and(|position| position < operation_position)
                            && internal_unit_calls
                                .iter()
                                .flat_map(|call| &call.arguments)
                                .filter(|candidate| {
                                    candidate.place == argument.place && candidate.path.is_empty()
                                })
                                .count()
                                == 1
                    });
                let affine_scalar_record_source = function
                    .unit_affine_scalar_records
                    .iter()
                    .find(|record| record.result.place == argument.place)
                    .is_some_and(|record| {
                        record.operation_ordinal < custody.operation_ordinal
                            && record.shape == ValueShape::integer(8, 8)
                            && record.result.structural_type == argument.structural_type
                            && argument.path.is_empty()
                            && argument.access == psi_terminal::StructuralAccess::Owned
                            && argument.root_structural_type == argument.structural_type
                            && argument.shape == record.shape
                            && argument.source.shape == record.shape
                            && argument.source.locations.is_empty()
                            && argument.source_home_byte_offset == 0
                            && record.result.multiplicity
                                == psi_terminal::StructuralMultiplicity::Affine
                            && record.result.qualifications.is_empty()
                            && record.result.projected_qualifications.is_empty()
                            && record.result.claims.is_empty()
                            && matches!(record.value, psi_core::IntegerValue::Signed(value)
                                if i64::try_from(value).is_ok())
                            && provenance
                                .operations
                                .iter()
                                .position(|candidate| candidate == &record.psi_operation)
                                .is_some_and(|position| position < operation_position)
                            && internal_unit_calls
                                .iter()
                                .flat_map(|call| &call.arguments)
                                .filter(|candidate| {
                                    candidate.place == argument.place
                                        && candidate.path.is_empty()
                                        && candidate.access == psi_terminal::StructuralAccess::Owned
                                })
                                .count()
                                == 1
                    });
                let zero_byte_argument = (parameter_source || local_source)
                    && argument.path.is_empty()
                    && argument.byte_count == 0
                    && argument.bytes.is_empty()
                    && argument.shape == omega_calling_conventions::ValueShape::integer(0, 1)
                    && argument.source.locations.is_empty()
                    && argument.destination.locations.is_empty();
                argument.destination != *destination
                    || argument.call_stack_bytes != expected_call_stack_bytes
                    || (!parameter_source && !local_source && !affine_scalar_record_source)
                    || (argument.byte_count == 0 && !zero_byte_argument)
                    || argument.bytes.len() != argument.byte_count
                    || argument
                        .code_offset
                        .checked_add(argument.byte_count)
                        .and_then(|end| function_bytes.get(argument.code_offset..end))
                        != Some(argument.bytes.as_slice())
                    || (!argument.path.is_empty()
                        && expected_projected_copy_bytes(target, argument).as_deref()
                            != Some(argument.bytes.as_slice()))
                    || (affine_scalar_record_source
                        && expected_affine_scalar_record_argument_bytes(target, argument, function)
                            .as_deref()
                            != Some(argument.bytes.as_slice()))
                    || argument.code_offset < custody.code_offset
                    || argument
                        .code_offset
                        .checked_add(argument.byte_count)
                        .is_none_or(|argument_end| argument_end > end)
                    || argument
                        .source_byte_offset
                        .checked_add(u32::from(argument.shape.byte_size))
                        .is_none_or(|end| end > u32::from(argument.source.shape.byte_size))
                    || match argument.path.as_slice() {
                        [] => {
                            argument.source_byte_offset != 0
                                || argument.source.shape != argument.shape
                                || argument.root_structural_type != argument.structural_type
                                || argument.fixed_array_length.is_some()
                                || argument.element_stride.is_some()
                        }
                        _ if exact_write_only_argument(argument_index, argument) => false,
                        [psi_terminal::StructuralPathSegment::FixedIndex(index)] => {
                            let expected_stride = u32::from(argument.shape.byte_size)
                                .next_multiple_of(u32::from(argument.shape.alignment));
                            let Some(length) = argument.fixed_array_length else {
                                return true;
                            };
                            let Some(stride) = argument.element_stride else {
                                return true;
                            };
                            argument.root_structural_type == argument.structural_type
                                || *index >= length
                                || stride != expected_stride
                                || u64::from(stride).checked_mul(*index)
                                    != Some(u64::from(argument.source_byte_offset))
                                || u64::from(stride).checked_mul(length)
                                    != Some(u64::from(argument.source.shape.byte_size))
                                || argument.source.shape.alignment != argument.shape.alignment
                        }
                        [
                            psi_terminal::StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                            psi_terminal::StructuralPathSegment::FixedIndex(
                                inner @ (0..=14),
                            ),
                        ] => {
                            let leaf_stride = u32::from(argument.shape.byte_size)
                                .next_multiple_of(u32::from(argument.shape.alignment));
                            let Some(outer_stride) = argument.element_stride else {
                                return true;
                            };
                            let Some(inner_length) =
                                [
                                    3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32, 9_u32, 10_u32,
                                    11_u32, 12_u32, 13_u32, 14_u32, 15_u32,
                                ]
                                    .into_iter()
                                    .find(|length| {
                                        leaf_stride.checked_mul(*length) == Some(outer_stride)
                                    })
                            else {
                                return true;
                            };
                            let expected_offset = outer_stride
                                .checked_mul(u32::try_from(*outer).unwrap_or(u32::MAX))
                                .and_then(|offset| {
                                    leaf_stride
                                        .checked_mul(u32::try_from(*inner).unwrap_or(u32::MAX))
                                        .and_then(|inner| offset.checked_add(inner))
                                });
                            argument.root_structural_type == argument.structural_type
                                || argument.fixed_array_length != Some(2)
                                || *inner >= u64::from(inner_length)
                                || Some(argument.source_byte_offset) != expected_offset
                                || outer_stride.checked_mul(2)
                                    != Some(u32::from(argument.source.shape.byte_size))
                                || argument.source.shape.alignment != argument.shape.alignment
                        }
                        path @ [psi_terminal::StructuralPathSegment::Field(_), ..]
                            if path.iter().all(|segment| {
                                matches!(segment,
                                    psi_terminal::StructuralPathSegment::Field(identity)
                                        if !identity.is_empty())
                            }) =>
                        {
                            path.is_empty()
                                || argument.root_structural_type == argument.structural_type
                                || argument.fixed_array_length.is_some()
                                || argument.element_stride.is_some()
                                || !argument
                                    .source_byte_offset
                                    .is_multiple_of(u32::from(argument.shape.alignment))
                        }
                        _ => true,
                    }
            })
        || projected_argument_indexes.iter().any(|index| {
            if transferred_argument_indexes.contains(index) {
                return false;
            }
            let Some(argument) = custody.arguments.get(*index) else {
                return true;
            };
            if exact_write_only_argument(*index, argument) {
                return false;
            }
            argument.path.is_empty()
                || (!fully_consumed_affine_pair
                    && affine_cleanup.is_none_or(|cleanup| {
                        !cleanup.actions.iter().any(|action| {
                            matches!(action,
                            psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual)
                                if residual.place == argument.place
                                    && !residual.path.is_empty()
                                    && !residual.path.starts_with(&argument.path)
                                    && !argument.path.starts_with(&residual.path)
                                    && residual.structural_type
                                        != argument.root_structural_type)
                        })
                    }))
        })
        || custody.claim_transfers.iter().any(|transfer| {
            usize::try_from(transfer.argument_index)
                .map_or(true, |index| index >= custody.arguments.len())
        })
        || custody
            .claim_transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != custody.claim_transfers.len()
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_mixed_argument_bytes_and_order(
    target: NativeTarget,
    function: &MachineCodeFunction,
    function_stack: &ObjectUnitStack,
    call_plan: &omega_calling_conventions::CallPlan,
    relocation: &omega_machine_code::InternalCallRelocation,
    custody: &omega_machine_code::InternalUnitCallRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(function.machine);
    let outbound = relocation.unit_stack.ok_or_else(invalid)?.outbound;
    let outbound_bytes = outbound.map_or(0, |area| area.byte_size);
    let mut cursor = match outbound {
        Some(area) => {
            if custody.code_offset != area.allocation_offset {
                return Err(invalid());
            }
            area.allocation_offset
                .checked_add(area.allocation_byte_count)
                .ok_or_else(invalid)?
        }
        None => custody.code_offset,
    };
    for (argument_index, argument) in custody.scalar_arguments.iter().enumerate() {
        if argument.code_offset != cursor {
            return Err(invalid());
        }
        validate_source(
            function,
            custody.operation_ordinal,
            custody.code_offset,
            argument.source,
        )
        .map_err(|_| invalid())?;
        let expected = expected_argument_bytes(
            target,
            call_plan,
            &custody.scalar_arguments,
            argument_index,
            function_stack.frame_bytes,
            outbound_bytes,
        )
        .ok_or_else(invalid)?;
        let argument_end = cursor.checked_add(expected.len()).ok_or_else(invalid)?;
        if argument.byte_count != expected.len()
            || function.bytes.get(cursor..argument_end) != Some(expected.as_slice())
        {
            return Err(invalid());
        }
        cursor = argument_end;
    }
    for argument in &custody.arguments {
        if argument.code_offset != cursor {
            return Err(invalid());
        }
        cursor = cursor
            .checked_add(argument.byte_count)
            .ok_or_else(invalid)?;
    }
    let native_call_start = match target.architecture {
        Architecture::X86_64 => relocation.offset.checked_sub(1).ok_or_else(invalid)?,
        Architecture::Aarch64 => relocation.offset,
    };
    if cursor != native_call_start {
        return Err(invalid());
    }
    cursor = relocation.offset.checked_add(4).ok_or_else(invalid)?;
    if let Some(area) = outbound {
        if area.release_offset != cursor {
            return Err(invalid());
        }
        cursor = area
            .release_offset
            .checked_add(area.release_byte_count)
            .ok_or_else(invalid)?;
    }
    if custody
        .code_offset
        .checked_add(custody.byte_count)
        .is_none_or(|end| end != cursor)
    {
        return Err(invalid());
    }
    Ok(())
}

fn expected_affine_scalar_record_argument_bytes(
    target: NativeTarget,
    argument: &omega_machine_code::InternalUnitCallArgumentRecord,
    function: &MachineCodeFunction,
) -> Option<Vec<u8>> {
    let record = function
        .unit_affine_scalar_records
        .iter()
        .find(|record| record.result.place == argument.place)?;
    let psi_core::IntegerValue::Signed(value) = record.value else {
        return None;
    };
    let bits = u64::from_le_bytes(i64::try_from(value).ok()?.to_le_bytes());
    let [
        omega_calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = argument.destination.locations.as_slice()
    else {
        return None;
    };
    match target.architecture {
        Architecture::X86_64 => {
            let register = x86_terminal_register(*register)?;
            let mut bytes = vec![0x48 | ((register >> 3) & 1), 0xb8 | (register & 7)];
            bytes.extend_from_slice(&bits.to_le_bytes());
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let register = aarch64_terminal_register(*register)?;
            let mut bytes = Vec::new();
            for chunk in 0..4 {
                let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
                if chunk == 0 || immediate != 0 {
                    let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
                    bytes.extend_from_slice(
                        &(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register))
                            .to_le_bytes(),
                    );
                }
            }
            Some(bytes)
        }
    }
}

pub(super) fn expected_projected_copy_bytes(
    target: NativeTarget,
    argument: &omega_machine_code::InternalUnitCallArgumentRecord,
) -> Option<Vec<u8>> {
    if argument.shape.class == omega_calling_conventions::ValueClass::BorrowedReference
        && argument.source.shape.class == omega_calling_conventions::ValueClass::BorrowedReference
    {
        let [
            omega_calling_conventions::ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            },
        ] = argument.destination.locations.as_slice()
        else {
            return None;
        };
        if *byte_size != argument.shape.byte_size || *alignment != argument.shape.alignment {
            return None;
        }
        let home = argument
            .call_stack_bytes
            .checked_add(argument.source_home_byte_offset)?;
        return match target.architecture {
            Architecture::X86_64 => {
                let destination = match *pointer {
                    omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                        x86_terminal_register(register)?
                    }
                    omega_calling_conventions::IndirectPointerLocation::Stack { .. } => 11,
                };
                let mut bytes = Vec::new();
                expected_x86_stack_load(&mut bytes, destination, home, 8)?;
                if argument.source_byte_offset != 0 {
                    bytes.extend_from_slice(&[
                        0x48 | ((destination >> 3) & 1),
                        0x81,
                        0xc0 | (destination & 7),
                    ]);
                    bytes.extend_from_slice(&argument.source_byte_offset.to_le_bytes());
                }
                if let omega_calling_conventions::IndirectPointerLocation::Stack {
                    stack_byte_offset,
                    ..
                } = *pointer
                {
                    expected_x86_stack_store(&mut bytes, destination, stack_byte_offset);
                }
                Some(bytes)
            }
            Architecture::Aarch64 => {
                let destination = match *pointer {
                    omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                        aarch64_terminal_register(register)?
                    }
                    omega_calling_conventions::IndirectPointerLocation::Stack { .. } => 9,
                };
                let mut instructions = vec![expected_aarch64_stack_load(destination, home, 8)?];
                let upper = argument.source_byte_offset >> 12;
                let lower = argument.source_byte_offset & 0xfff;
                if upper != 0 {
                    instructions.push(
                        0x9140_0000
                            | (upper << 10)
                            | (u32::from(destination) << 5)
                            | u32::from(destination),
                    );
                }
                if lower != 0 {
                    instructions.push(
                        0x9100_0000
                            | (lower << 10)
                            | (u32::from(destination) << 5)
                            | u32::from(destination),
                    );
                }
                if let omega_calling_conventions::IndirectPointerLocation::Stack {
                    stack_byte_offset,
                    ..
                } = *pointer
                {
                    instructions.push(expected_aarch64_stack_store(
                        destination,
                        stack_byte_offset,
                    )?);
                }
                Some(
                    instructions
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect(),
                )
            }
        };
    }
    let [
        omega_calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = argument.destination.locations.as_slice()
    else {
        return None;
    };
    if argument.shape != omega_calling_conventions::ValueShape::integer(8, 8) {
        return None;
    }
    let home = argument
        .call_stack_bytes
        .checked_add(argument.source_home_byte_offset)?;
    match target.architecture {
        Architecture::X86_64 => {
            let destination = x86_terminal_register(*register)?;
            let mut bytes = Vec::new();
            if matches!(
                argument.source.locations.as_slice(),
                [omega_calling_conventions::ValueLocation::Indirect { .. }]
            ) {
                expected_x86_stack_load(&mut bytes, 11, home, 8)?;
                expected_x86_memory_load(
                    &mut bytes,
                    destination,
                    11,
                    argument.source_byte_offset,
                    8,
                )?;
            } else {
                let offset = home.checked_add(argument.source_byte_offset)?;
                expected_x86_stack_load(&mut bytes, destination, offset, 8)?;
            }
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let destination = aarch64_terminal_register(*register)?;
            let mut instructions = Vec::new();
            if matches!(
                argument.source.locations.as_slice(),
                [omega_calling_conventions::ValueLocation::Indirect { .. }]
            ) {
                instructions.push(expected_aarch64_stack_load(9, home, 8)?);
                instructions.push(expected_aarch64_memory_load(
                    destination,
                    9,
                    argument.source_byte_offset,
                    8,
                )?);
            } else {
                instructions.push(expected_aarch64_stack_load(
                    destination,
                    home.checked_add(argument.source_byte_offset)?,
                    8,
                )?);
            }
            Some(
                instructions
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            )
        }
    }
}
