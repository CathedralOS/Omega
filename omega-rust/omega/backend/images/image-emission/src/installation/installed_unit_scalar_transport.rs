//! Installation-shape validation for retained fixed-integer ABI and attached-
//! Unit scalar-call transport. Native byte replay remains object-owned.

use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use machine_code::SemanticCodeSite;
use semantic_vocabulary::MachineId;
use target::NativeTarget;
use target_operations::CallSiteOwner;
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

use super::{InstallationError, InstallationRecord, InstalledFunction};

fn fixed_integer_shape(integer: semantic_vocabulary::IntegerType) -> Option<ValueShape> {
    if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = integer.bits() / 8;
    Some(ValueShape::integer(bytes, bytes))
}

fn scalar_home_shape(scalar: semantic_vocabulary::ScalarType) -> Option<ValueShape> {
    match scalar {
        semantic_vocabulary::ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        semantic_vocabulary::ScalarType::Integer(integer) => fixed_integer_shape(integer),
        semantic_vocabulary::ScalarType::IeeeFloat(_) => None,
    }
}

pub(super) fn installed_forwarded_dynamic_scalar_result_is_canonical(
    call: &super::InstalledForwardedDynamicDescriptorCall,
    function: &InstalledFunction,
    target: NativeTarget,
) -> bool {
    let (Some(semantic_result), Some(result)) = (call.semantic_result, call.result.as_ref()) else {
        return call.semantic_result.is_none() && call.result.is_none() && call.byte_count != 0;
    };
    let Some(result_shape) = scalar_home_shape(semantic_result.scalar_type) else {
        return false;
    };
    let Ok(pointer_bytes) = u16::try_from(target.pointer_size) else {
        return false;
    };
    let Ok(pointer_alignment) = u16::try_from(target.pointer_alignment) else {
        return false;
    };
    let pointer = ValueShape::integer(pointer_bytes, pointer_alignment);
    let Ok(expected_plan) = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer; 2],
            result: Some(result_shape),
        },
    ) else {
        return false;
    };
    let Some(expected_result) = expected_plan.result.as_ref() else {
        return false;
    };
    let Some(local_call_offset) = call.text_offset.checked_sub(function.text_offset) else {
        return false;
    };
    let Some(local_call_end) = local_call_offset.checked_add(call.byte_count) else {
        return false;
    };
    let Some(result_end) = result.code_offset.checked_add(result.byte_count) else {
        return false;
    };

    semantic_result.value == result.home.source_value
        && result.home.defining_operation == call.operation
        && result.home.scalar_type == semantic_result.scalar_type
        && result.home.shape == result_shape
        && result.source == *expected_result
        && function.unit_scalar_homes.contains(&result.home)
        && result.byte_count != 0
        && result.code_offset >= local_call_offset
        && result_end == local_call_end
}

pub(super) fn installed_function_scalar_transport_is_canonical(
    function: &InstalledFunction,
    target: NativeTarget,
) -> bool {
    let abi_is_canonical = function
        .fixed_integer_scalar_abi
        .as_ref()
        .is_none_or(|abi| installed_fixed_integer_scalar_abi_is_canonical(abi, target));
    let mixed_abi_is_canonical = function
        .mixed_structural_scalar_abi
        .as_ref()
        .is_none_or(|abi| {
            installed_mixed_structural_scalar_abi_is_canonical(function, abi, target)
        });
    let homes_are_canonical = function
        .unit_scalar_homes
        .iter()
        .enumerate()
        .all(|(index, home)| {
            scalar_home_shape(home.scalar_type) == Some(home.shape)
                && function.unit_scalar_homes[..index].iter().all(|prior| {
                    prior.defining_operation != home.defining_operation
                        && prior.source_value != home.source_value
                        && prior.byte_offset != home.byte_offset
                })
                && index.checked_sub(1).is_none_or(|previous| {
                    function.unit_scalar_homes[previous]
                        .byte_offset
                        .checked_add(8)
                        == Some(home.byte_offset)
                })
        });
    let constants_are_canonical =
        function
            .unit_integer_constants
            .iter()
            .enumerate()
            .all(|(index, constant)| {
                fixed_integer_shape(constant.scalar_type).is_some()
                    && constant.scalar_type.admits(constant.value)
                    && function.unit_integer_constants[..index]
                        .iter()
                        .all(|prior| {
                            prior.defining_operation != constant.defining_operation
                                && prior.source_value != constant.source_value
                                && prior.operation_ordinal < constant.operation_ordinal
                        })
            });
    abi_is_canonical && mixed_abi_is_canonical && homes_are_canonical && constants_are_canonical
}

pub(super) fn installed_mixed_structural_scalar_abi_is_canonical(
    function: &InstalledFunction,
    abi: &target_operations::MixedStructuralScalarFunctionAbi,
    target: NativeTarget,
) -> bool {
    let Some(scalar_shapes) = abi
        .scalar_parameters
        .iter()
        .map(|parameter| fixed_integer_shape(parameter.scalar_type))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(result_shape) = scalar_home_shape(abi.result.scalar_type) else {
        return false;
    };
    let Ok(expected) = evaluate_call_plan(
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
    ) else {
        return false;
    };
    let scalar_count = abi.scalar_parameters.len();
    let structural_count = abi.structural_parameters.len();
    structural_count != 0
        && function.fixed_integer_scalar_abi.is_none()
        && function.scalar_stack.is_some()
        && function.unit_stack.is_none()
        && expected == abi.call_plan
        && abi.call_plan.parameters.len() == scalar_count + structural_count
        && abi.call_plan.result.as_ref() == Some(&abi.result.placement)
        && abi.result.placement.shape == result_shape
        && abi
            .scalar_parameters
            .iter()
            .zip(&scalar_shapes)
            .zip(&abi.call_plan.parameters[..scalar_count])
            .all(|((parameter, shape), placement)| {
                parameter.placement == *placement && placement.shape == *shape
            })
        && abi
            .structural_parameters
            .iter()
            .zip(&abi.call_plan.parameters[scalar_count..])
            .all(|(parameter, placement)| {
                parameter.placement == *placement && placement.shape == parameter.shape
            })
        && abi
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.value)
            .chain(std::iter::once(abi.result.value))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == scalar_count + 1
        && abi
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == structural_count
}

pub(super) fn installed_fixed_integer_scalar_abi_is_canonical(
    abi: &target_operations::FixedIntegerScalarFunctionAbi,
    target: NativeTarget,
) -> bool {
    let Some(parameter_shapes) = abi
        .parameters
        .iter()
        .map(|parameter| fixed_integer_shape(parameter.scalar_type))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(result_shape) = fixed_integer_shape(abi.result.scalar_type) else {
        return false;
    };
    let Ok(expected_plan) = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: parameter_shapes.clone(),
            result: Some(result_shape),
        },
    ) else {
        return false;
    };
    expected_plan == abi.call_plan
        && abi.parameters.len() == abi.call_plan.parameters.len()
        && abi
            .parameters
            .iter()
            .zip(&parameter_shapes)
            .zip(&abi.call_plan.parameters)
            .all(|((parameter, shape), placement)| {
                parameter.placement == *placement && placement.shape == *shape
            })
        && abi.call_plan.result.as_ref() == Some(&abi.result.placement)
        && abi.result.placement.shape == result_shape
        && abi
            .parameters
            .iter()
            .map(|parameter| parameter.value)
            .chain(std::iter::once(abi.result.value))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == abi.parameters.len() + 1
}

pub(super) fn validate_installed_unit_scalar_calls(
    record: &InstallationRecord,
    functions: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    let mut previous_key = None;
    for installed in &record.internal_unit_scalar_calls {
        let function = functions.get(&installed.machine).copied().ok_or(
            InstallationError::InvalidInternalUnitScalarCall(installed.machine),
        )?;
        let custody = &installed.custody;
        let target_abi = functions
            .get(&custody.target)
            .and_then(|target| target.fixed_integer_scalar_abi.as_ref())
            .ok_or(InstallationError::InvalidInternalUnitScalarCall(
                installed.machine,
            ))?;
        let CallSiteOwner::Operation(owner) = custody.owner else {
            return Err(InstallationError::InvalidInternalUnitScalarCall(
                installed.machine,
            ));
        };
        let key = (
            installed.machine,
            custody.operation_ordinal,
            custody.code_offset,
        );
        let expected_text_offset = function
            .text_offset
            .checked_add(custody.code_offset)
            .ok_or(InstallationError::InstalledScalarOffsetNotRepresentable)?;
        let call_end = custody
            .code_offset
            .checked_add(custody.byte_count)
            .ok_or(InstallationError::InstalledScalarOffsetNotRepresentable)?;
        let result_end = custody
            .result
            .code_offset
            .checked_add(custody.result.byte_count)
            .ok_or(InstallationError::InstalledScalarOffsetNotRepresentable)?;
        let result_home_is_exact = function
            .unit_scalar_homes
            .iter()
            .any(|home| home == &custody.result.home);
        let arguments_are_exact = custody.arguments.len() == target_abi.parameters.len()
            && custody
                .arguments
                .iter()
                .enumerate()
                .all(|(index, argument)| {
                    let Some(parameter) = target_abi.parameters.get(index) else {
                        return false;
                    };
                    let source_is_exact = match argument.source {
                        machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                            ..
                        } => false,
                        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                            defining_operation,
                            source_value,
                            scalar_type,
                            value,
                        } => function.unit_integer_constants.iter().any(|constant| {
                            constant.defining_operation == defining_operation
                                && constant.source_value == source_value
                                && constant.scalar_type == scalar_type
                                && constant.value == value
                                && constant.operation_ordinal < custody.operation_ordinal
                        }),
                        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate { .. } => false,
                        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
                            function.unit_scalar_homes.iter().any(|candidate| candidate == &home)
                                && record.internal_unit_scalar_calls.iter().any(|prior| {
                                    prior.machine == installed.machine
                                        && prior.custody.operation_ordinal
                                            < custody.operation_ordinal
                                        && prior.custody.result.home == home
                                })
                        }
                    };
                    let argument_end = argument.code_offset.checked_add(argument.byte_count);
                    u32::try_from(index) == Ok(argument.parameter_index)
                        && argument.destination == parameter.placement
                        && argument.source.scalar_type()
                            == semantic_vocabulary::ScalarType::Integer(parameter.scalar_type)
                        && source_is_exact
                        && argument.byte_count != 0
                        && argument.code_offset >= custody.code_offset
                        && argument_end.is_some_and(|end| end <= call_end)
                });
        if previous_key.is_some_and(|previous| previous >= key)
            || installed.text_offset != expected_text_offset
            || custody.call_plan != target_abi.call_plan
            || custody.byte_count == 0
            || call_end > function.byte_count
            || custody.result.byte_count == 0
            || custody.result.code_offset < custody.code_offset
            || result_end > call_end
            || custody.result.home.defining_operation != owner
            || custody.result.home.scalar_type
                != semantic_vocabulary::ScalarType::Integer(target_abi.result.scalar_type)
            || custody.result.source != target_abi.result.placement
            || !result_home_is_exact
            || !arguments_are_exact
        {
            return Err(InstallationError::InvalidInternalUnitScalarCall(
                installed.machine,
            ));
        }
        previous_key = Some(key);
    }
    Ok(())
}

pub(super) fn validate_installed_unit_structural_scalar_field_stores(
    record: &InstallationRecord,
    functions: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    for function in functions.values().copied() {
        let invalid = || InstallationError::InvalidUnitStructuralScalarFieldStore(function.machine);
        let mut previous = None;
        for store in &function.unit_structural_scalar_field_stores {
            let key = (store.operation_ordinal, store.code_offset);
            let parameter_index =
                usize::try_from(store.destination.position).map_err(|_| invalid())?;
            let parameter = function
                .unit_parameters
                .get(parameter_index)
                .ok_or_else(invalid)?;
            let home = function
                .unit_parameter_homes
                .get(parameter_index)
                .ok_or_else(invalid)?;
            let (source_is_exact, width, bits) = match store.source {
                machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                    parameter_index,
                    source_value,
                    scalar_type,
                    location,
                } => {
                    let abi = function.unit_scalar_abi.as_ref().ok_or_else(invalid)?;
                    let scalar_parameter_index =
                        usize::try_from(parameter_index).map_err(|_| invalid())?;
                    let scalar_parameter = abi
                        .parameters
                        .get(scalar_parameter_index)
                        .ok_or_else(invalid)?;
                    let (scalar_shape, width) =
                        installed_native_scalar_shape(scalar_type).ok_or_else(invalid)?;
                    let (expected_location, placed_byte_size) =
                        match scalar_parameter.placement.locations.as_slice() {
                            [
                                ValueLocation::Register {
                                    register,
                                    value_byte_offset: 0,
                                    byte_size,
                                },
                            ] => (
                                machine_code::UnitScalarParameterLocationRecord::Register(
                                    *register,
                                ),
                                *byte_size,
                            ),
                            [
                                ValueLocation::Stack {
                                    stack_byte_offset,
                                    value_byte_offset: 0,
                                    byte_size,
                                    ..
                                },
                            ] => (
                                machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                                    byte_offset: *stack_byte_offset,
                                },
                                *byte_size,
                            ),
                            _ => return Err(invalid()),
                        };
                    let mut parameter_shapes = abi
                        .parameters
                        .iter()
                        .map(|parameter| {
                            let (shape, _) = installed_native_scalar_shape(parameter.scalar_type)?;
                            (parameter.placement.shape == shape).then_some(shape)
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(invalid)?;
                    parameter_shapes.push(parameter.shape);
                    let expected_plan = evaluate_call_plan(
                        CallingPolicy::native_for_target(record.target),
                        &CallSignature {
                            parameters: parameter_shapes,
                            result: None,
                        },
                    )
                    .map_err(|_| invalid())?;
                    (
                        function.unit_parameters.len() == 1
                            && scalar_parameter.value == source_value
                            && scalar_parameter.scalar_type == scalar_type
                            && scalar_parameter.placement.shape == scalar_shape
                            && placed_byte_size == width
                            && location == expected_location
                            && abi.call_plan == expected_plan
                            && abi.call_plan.parameters.get(scalar_parameter_index)
                                == Some(&scalar_parameter.placement)
                            && abi.call_plan.parameters.get(abi.parameters.len())
                                == Some(&home.source),
                        width,
                        None,
                    )
                }
                machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                } => {
                    let source_count = function
                        .unit_integer_constants
                        .iter()
                        .filter(|constant| {
                            constant.defining_operation == defining_operation
                                && constant.source_value == source_value
                                && constant.scalar_type == scalar_type
                                && constant.value == value
                                && constant.operation_ordinal < store.operation_ordinal
                        })
                        .count();
                    let width = scalar_type.bits().checked_div(8).ok_or_else(invalid)?;
                    (
                        source_count == 1
                            && matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
                            && !scalar_type.is_address()
                            && scalar_type.admits(value),
                        width,
                        Some(
                            crate::unit_structural_scalar_field_store::integer_bits(
                                scalar_type,
                                value,
                            )
                            .ok_or_else(invalid)?,
                        ),
                    )
                }
                machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                } => {
                    let definition_count = record
                        .semantic_code_attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.machine == function.machine
                                && attribution.attribution.site
                                    == SemanticCodeSite::Operation(defining_operation)
                                && attribution.attribution.operation_ordinal == definition_ordinal
                                && attribution.attribution.code_offset <= store.code_offset
                                && attribution.attribution.byte_count == 0
                        })
                        .count();
                    (
                        definition_ordinal < store.operation_ordinal
                            && definition_count == 1
                            && function.unit_integer_constants.iter().all(|constant| {
                                constant.defining_operation != defining_operation
                                    && constant.source_value != source_value
                            })
                            && function.unit_scalar_homes.iter().all(|home| {
                                home.defining_operation != defining_operation
                                    && home.source_value != source_value
                            })
                            && installed_projected_zero_code_source_is_consistent(
                                function,
                                store.source,
                            ),
                        1,
                        Some(u64::from(value)),
                    )
                }
                machine_code::InternalUnitScalarArgumentSourceRecord::Home(source_home) => {
                    if !matches!(
                        source_home.scalar_type,
                        semantic_vocabulary::ScalarType::Integer(_)
                    ) {
                        return Err(invalid());
                    }
                    let source_count = record
                        .internal_unit_scalar_calls
                        .iter()
                        .filter(|call| {
                            call.machine == function.machine
                                && call.custody.result.home == source_home
                                && call.custody.operation_ordinal < store.operation_ordinal
                        })
                        .count();
                    let home_count = function
                        .unit_scalar_homes
                        .iter()
                        .filter(|home| **home == source_home)
                        .count();
                    let (shape, width) = installed_native_scalar_shape(source_home.scalar_type)
                        .ok_or_else(invalid)?;
                    (
                        source_count == 1 && home_count == 1 && source_home.shape == shape,
                        width,
                        None,
                    )
                }
            };
            let expected_bytes = match store.source {
                machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                    location:
                        machine_code::UnitScalarParameterLocationRecord::Register(register),
                    ..
                } => crate::unit_structural_scalar_field_store::expected_parameter_store_bytes(
                    record.target,
                    home,
                    store.field_byte_offset,
                    width,
                    register,
                )
                .ok_or_else(invalid)?,
                machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                    location:
                        machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                            byte_offset,
                        },
                    ..
                } => crate::unit_structural_scalar_field_store::expected_incoming_parameter_store_bytes(
                    record.target,
                    home,
                    store.field_byte_offset,
                    width,
                    byte_offset,
                    function.unit_stack.as_ref().ok_or_else(invalid)?.frame_bytes,
                )
                .ok_or_else(invalid)?,
                machine_code::InternalUnitScalarArgumentSourceRecord::Home(source_home) => {
                    crate::unit_structural_scalar_field_store::expected_home_store_bytes(
                        record.target,
                        home,
                        store.field_byte_offset,
                        width,
                        source_home,
                    )
                    .ok_or_else(invalid)?
                }
                _ => crate::unit_structural_scalar_field_store::expected_store_bytes(
                    record.target,
                    home,
                    store.field_byte_offset,
                    width,
                    bits.ok_or_else(invalid)?,
                )
                .ok_or_else(invalid)?,
            };
            let end = store
                .code_offset
                .checked_add(store.byte_count)
                .ok_or_else(invalid)?;
            let exact_attribution_count = record
                .semantic_code_attribution
                .iter()
                .filter(|attribution| {
                    attribution.machine == function.machine
                        && attribution.attribution.site
                            == SemanticCodeSite::Operation(store.psi_operation)
                        && attribution.attribution.operation_ordinal == store.operation_ordinal
                        && attribution.attribution.code_offset == store.code_offset
                        && attribution.attribution.byte_count == store.byte_count
                })
                .count();
            if previous.is_some_and(|previous| previous >= key)
                || (store.destination.is_self
                    && function.attachment != Some(store.destination.structural_type))
                || !matches!(
                    store.destination.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
                || parameter.place != store.destination.place
                || parameter.structural_type != store.destination.structural_type
                || parameter.multiplicity != store.destination.multiplicity
                || parameter.access != store.destination.access
                || home.place != parameter.place
                || home.structural_type != parameter.structural_type
                || home.multiplicity != parameter.multiplicity
                || home.access != parameter.access
                || home.shape != parameter.shape
                || store.destination_placement != home.source
                || Some(store.parameter_home_byte_offset) != home.location.stack_byte_offset()
                || store.parameter_home_indirect != home.indirect
                || !terminal_psi::is_bounded_structural_scalar_store_path(&store.path)
                || !source_is_exact
                || store
                    .field_byte_offset
                    .checked_add(u32::from(width))
                    .is_none_or(|field_end| field_end > u32::from(parameter.shape.byte_size))
                || store.byte_count == 0
                || store.byte_count != store.bytes.len()
                || store.bytes != expected_bytes
                || end > function.byte_count
                || exact_attribution_count != 1
            {
                return Err(invalid());
            }
            previous = Some(key);
        }
    }
    Ok(())
}

fn installed_projected_zero_code_source_is_consistent(
    function: &InstalledFunction,
    source: machine_code::InternalUnitScalarArgumentSourceRecord,
) -> bool {
    let machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
        defining_operation,
        source_value,
        ..
    } = source
    else {
        return false;
    };
    function
        .unit_structural_scalar_field_stores
        .iter()
        .all(|candidate| {
            let candidate_source = candidate.source;
            if matches!(
                candidate_source,
                machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                    defining_operation: candidate_operation,
                    source_value: candidate_value,
                    ..
                } if candidate_operation == defining_operation || candidate_value == source_value
            ) {
                candidate_source == source
            } else {
                true
            }
        })
}

pub(super) fn validate_installed_unit_write_only_primitive_stores(
    record: &InstallationRecord,
    functions: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    for function in functions.values().copied() {
        let invalid = || InstallationError::InvalidUnitWriteOnlyPrimitiveStore(function.machine);
        let mut previous = None;
        for store in &function.unit_write_only_primitive_stores {
            let key = (store.operation_ordinal, store.code_offset);
            let parameter_index =
                usize::try_from(store.destination.position).map_err(|_| invalid())?;
            let parameter = function
                .unit_parameters
                .get(parameter_index)
                .ok_or_else(invalid)?;
            let home = function
                .unit_parameter_homes
                .get(parameter_index)
                .ok_or_else(invalid)?;
            let (source_is_exact, destination_scalar_type, width, bits) = match store.source {
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    parameter_index,
                    source_value,
                    scalar_type,
                    location,
                } => {
                    let abi = function.unit_scalar_abi.as_ref().ok_or_else(invalid)?;
                    let scalar_parameter_index =
                        usize::try_from(parameter_index).map_err(|_| invalid())?;
                    let scalar_parameter = abi
                        .parameters
                        .get(scalar_parameter_index)
                        .ok_or_else(invalid)?;
                    let (scalar_shape, width) =
                        installed_native_scalar_shape(scalar_type).ok_or_else(invalid)?;
                    let (expected_location, placed_byte_size) =
                        match scalar_parameter.placement.locations.as_slice() {
                            [
                                ValueLocation::Register {
                                    register,
                                    value_byte_offset: 0,
                                    byte_size,
                                },
                            ] => (
                                machine_code::UnitScalarParameterLocationRecord::Register(
                                    *register,
                                ),
                                *byte_size,
                            ),
                            [
                                ValueLocation::Stack {
                                    stack_byte_offset,
                                    value_byte_offset: 0,
                                    byte_size,
                                    ..
                                },
                            ] => (
                                machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                                    byte_offset: *stack_byte_offset,
                                },
                                *byte_size,
                            ),
                            _ => return Err(invalid()),
                        };
                    let mut parameter_shapes = abi
                        .parameters
                        .iter()
                        .map(|parameter| {
                            let (shape, _) = installed_native_scalar_shape(parameter.scalar_type)?;
                            (parameter.placement.shape == shape).then_some(shape)
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(invalid)?;
                    parameter_shapes.push(parameter.shape);
                    let expected_plan = evaluate_call_plan(
                        CallingPolicy::native_for_target(record.target),
                        &CallSignature {
                            parameters: parameter_shapes,
                            result: None,
                        },
                    )
                    .map_err(|_| invalid())?;
                    (
                        function.unit_parameters.len() == 1
                            && scalar_parameter.value == source_value
                            && scalar_parameter.scalar_type == scalar_type
                            && scalar_parameter.placement.shape == scalar_shape
                            && placed_byte_size == width
                            && location == expected_location
                            && abi.call_plan == expected_plan
                            && abi.call_plan.parameters.get(scalar_parameter_index)
                                == Some(&scalar_parameter.placement)
                            && abi.call_plan.parameters.get(abi.parameters.len())
                                == Some(&home.source),
                        scalar_type,
                        width,
                        None,
                    )
                }
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                } => {
                    let source_count = function
                        .unit_integer_constants
                        .iter()
                        .filter(|constant| {
                            constant.defining_operation == defining_operation
                                && constant.source_value == source_value
                                && constant.scalar_type == scalar_type
                                && constant.value == value
                                && constant.operation_ordinal < store.operation_ordinal
                        })
                        .count();
                    let width = scalar_type.bits().checked_div(8).ok_or_else(invalid)?;
                    (
                        source_count == 1
                            && matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
                            && !scalar_type.is_address()
                            && scalar_type.admits(value),
                        semantic_vocabulary::ScalarType::Integer(scalar_type),
                        width,
                        Some(
                            crate::unit_structural_scalar_field_store::integer_bits(
                                scalar_type,
                                value,
                            )
                            .ok_or_else(invalid)?,
                        ),
                    )
                }
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                } => {
                    let definition_count = record
                        .semantic_code_attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.machine == function.machine
                                && attribution.attribution.site
                                    == SemanticCodeSite::Operation(defining_operation)
                                && attribution.attribution.operation_ordinal == definition_ordinal
                                && attribution.attribution.code_offset <= store.code_offset
                                && attribution.attribution.byte_count == 0
                        })
                        .count();
                    (
                        definition_ordinal < store.operation_ordinal
                            && definition_count == 1
                            && function.unit_integer_constants.iter().all(|constant| {
                                constant.defining_operation != defining_operation
                                    && constant.source_value != source_value
                            })
                            && function.unit_scalar_homes.iter().all(|home| {
                                home.defining_operation != defining_operation
                                    && home.source_value != source_value
                            })
                            && installed_zero_code_store_source_is_consistent(
                                function,
                                store.source,
                            ),
                        semantic_vocabulary::ScalarType::Boolean,
                        1,
                        Some(u64::from(value)),
                    )
                }
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                } => {
                    let definition_count = record
                        .semantic_code_attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.machine == function.machine
                                && attribution.attribution.site
                                    == SemanticCodeSite::Operation(defining_operation)
                                && attribution.attribution.operation_ordinal == definition_ordinal
                                && attribution.attribution.code_offset <= store.code_offset
                                && attribution.attribution.byte_count == 0
                        })
                        .count();
                    let (width, bits) = match value {
                        semantic_vocabulary::IeeeFloatValue::Binary32(bits) => (4, u64::from(bits)),
                        semantic_vocabulary::IeeeFloatValue::Binary64(bits) => (8, bits),
                    };
                    (
                        definition_ordinal < store.operation_ordinal
                            && definition_count == 1
                            && function.unit_integer_constants.iter().all(|constant| {
                                constant.defining_operation != defining_operation
                                    && constant.source_value != source_value
                            })
                            && function.unit_scalar_homes.iter().all(|home| {
                                home.defining_operation != defining_operation
                                    && home.source_value != source_value
                            })
                            && installed_zero_code_store_source_is_consistent(
                                function,
                                store.source,
                            ),
                        semantic_vocabulary::ScalarType::IeeeFloat(value.format()),
                        width,
                        Some(bits),
                    )
                }
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Home(source_home) => {
                    let source_count = record
                        .internal_unit_scalar_calls
                        .iter()
                        .filter(|call| {
                            call.machine == function.machine
                                && call.custody.result.home == source_home
                                && call.custody.operation_ordinal < store.operation_ordinal
                        })
                        .count();
                    let home_count = function
                        .unit_scalar_homes
                        .iter()
                        .filter(|home| **home == source_home)
                        .count();
                    let semantic_vocabulary::ScalarType::Integer(integer) = source_home.scalar_type
                    else {
                        return Err(invalid());
                    };
                    let (shape, width) = installed_native_scalar_shape(source_home.scalar_type)
                        .ok_or_else(invalid)?;
                    (
                        source_count == 1 && home_count == 1 && source_home.shape == shape,
                        semantic_vocabulary::ScalarType::Integer(integer),
                        width,
                        None,
                    )
                }
            };
            if store.destination_type.shape
                != StructuralTypeShape::PrimitiveScalar(destination_scalar_type)
            {
                return Err(invalid());
            }
            let expected_shape = ValueShape::borrowed_reference(width, width.min(8));
            let [
                ValueLocation::Indirect {
                    copy_stack_byte_offset: None,
                    byte_size: placement_byte_size,
                    alignment: placement_alignment,
                    ..
                },
            ] = home.source.locations.as_slice()
            else {
                return Err(invalid());
            };
            let destination_type_count = function
                .unit_affine_cleanup
                .as_ref()
                .ok_or_else(invalid)?
                .structural_types
                .iter()
                .filter(|candidate| *candidate == &store.destination_type)
                .count();
            let expected_bytes = match store.source {
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    location:
                        machine_code::UnitScalarParameterLocationRecord::Register(register),
                    ..
                } => crate::unit_structural_scalar_field_store::expected_parameter_store_bytes(
                    record.target,
                    home,
                    0,
                    width,
                    register,
                )
                .ok_or_else(invalid)?,
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    location:
                        machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                            byte_offset,
                        },
                    ..
                } => crate::unit_structural_scalar_field_store::expected_incoming_parameter_store_bytes(
                    record.target,
                    home,
                    0,
                    width,
                    byte_offset,
                    function.unit_stack.as_ref().ok_or_else(invalid)?.frame_bytes,
                )
                .ok_or_else(invalid)?,
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Home(source_home) => {
                    crate::unit_structural_scalar_field_store::expected_home_store_bytes(
                        record.target,
                        home,
                        0,
                        width,
                        source_home,
                    )
                    .ok_or_else(invalid)?
                }
                _ => crate::unit_structural_scalar_field_store::expected_store_bytes(
                    record.target,
                    home,
                    0,
                    width,
                    bits.ok_or_else(invalid)?,
                )
                .ok_or_else(invalid)?,
            };
            let end = store
                .code_offset
                .checked_add(store.byte_count)
                .ok_or_else(invalid)?;
            let exact_attribution_count = record
                .semantic_code_attribution
                .iter()
                .filter(|attribution| {
                    attribution.machine == function.machine
                        && attribution.attribution.site
                            == SemanticCodeSite::Operation(store.psi_operation)
                        && attribution.attribution.operation_ordinal == store.operation_ordinal
                        && attribution.attribution.code_offset == store.code_offset
                        && attribution.attribution.byte_count == store.byte_count
                })
                .count();
            if previous.is_some_and(|previous| previous >= key)
                || !source_is_exact
                || destination_type_count != 1
                || store.destination_type.identity.is_empty()
                || store.destination_type.id != store.destination.structural_type
                || store.destination.is_self
                || store.destination.multiplicity != StructuralMultiplicity::Unrestricted
                || !matches!(
                    store.destination.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
                || !store.destination.qualifications.is_empty()
                || !store.destination.projected_qualifications.is_empty()
                || parameter.place != store.destination.place
                || parameter.structural_type != store.destination.structural_type
                || parameter.multiplicity != store.destination.multiplicity
                || parameter.access != store.destination.access
                || parameter.shape != expected_shape
                || home.place != parameter.place
                || home.structural_type != parameter.structural_type
                || home.multiplicity != parameter.multiplicity
                || home.access != parameter.access
                || home.shape != parameter.shape
                || home.source.shape != expected_shape
                || home.source != store.destination_placement
                || !home.indirect
                || *placement_byte_size != width
                || *placement_alignment != width.min(8)
                || Some(store.parameter_home_byte_offset) != home.location.stack_byte_offset()
                || !store.parameter_home_indirect
                || store.byte_count == 0
                || store.byte_count != store.bytes.len()
                || store.bytes != expected_bytes
                || end > function.byte_count
                || exact_attribution_count != 1
                || record.target.pointer_size != 8
                || record.target.pointer_alignment != 8
            {
                return Err(invalid());
            }
            previous = Some(key);
        }
    }
    Ok(())
}

fn installed_native_scalar_shape(
    scalar_type: semantic_vocabulary::ScalarType,
) -> Option<(ValueShape, u16)> {
    match scalar_type {
        semantic_vocabulary::ScalarType::Boolean => Some((ValueShape::integer(1, 1), 1)),
        semantic_vocabulary::ScalarType::Integer(integer)
            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            let width = integer.bits().checked_div(8)?;
            Some((ValueShape::integer(width, width.min(8)), width))
        }
        _ => None,
    }
}

fn installed_zero_code_store_source_is_consistent(
    function: &InstalledFunction,
    source: machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord,
) -> bool {
    let Some(defining_operation) = source.defining_operation() else {
        return false;
    };
    let source_value = source.source_value();
    function
        .unit_write_only_primitive_stores
        .iter()
        .all(|candidate| {
            let candidate_source = candidate.source;
            if candidate_source.defining_operation() == Some(defining_operation)
                || candidate_source.source_value() == source_value
            {
                candidate_source == source
            } else {
                true
            }
        })
}

#[cfg(test)]
mod tests {
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use machine_code::{ForeignCallScalarResultRecord, UnitScalarHomeRecord};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType, ValueId,
    };

    use super::*;

    fn forwarded_result_fixture(
        scalar_type: ScalarType,
    ) -> (
        super::super::InstalledForwardedDynamicDescriptorCall,
        InstalledFunction,
    ) {
        let target = NativeTarget::linux_x64();
        let shape = scalar_home_shape(scalar_type).expect("supported scalar home");
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 2],
                result: Some(shape),
            },
        )
        .expect("forwarded descriptor plan");
        let operation = OperationId::new(7).expect("operation");
        let value = ValueId::new(8).expect("value");
        let home = UnitScalarHomeRecord {
            defining_operation: operation,
            source_value: value,
            scalar_type,
            shape,
            byte_offset: 16,
        };
        let call = super::super::InstalledForwardedDynamicDescriptorCall {
            machine: MachineId::new(1).expect("caller"),
            operation,
            callee: MachineId::new(2).expect("callee"),
            application_commitment:
                terminal_psi::ClosedConformanceApplicationCommitment::from_digest([1; 32]),
            source: terminal_psi::StructuralArgument {
                place: PlaceId::new(3).expect("source"),
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
            semantic_result: Some(abstract_operations::AbstractResult { value, scalar_type }),
            result: Some(ForeignCallScalarResultRecord {
                home,
                source: plan.result.expect("result placement"),
                code_offset: 30,
                byte_count: 10,
            }),
            text_offset: 120,
            byte_count: 20,
        };
        let function = InstalledFunction {
            machine: call.machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            unit_scalar_abi: None,
            structural_call_scalar_return: None,
            text_offset: 100,
            byte_count: 50,
            unit_stack: None,
            scalar_stack: None,
            unit_call_stacks: Vec::new(),
            scalar_call_stacks: Vec::new(),
            foreign_call_stacks: Vec::new(),
            unit_body: true,
            ranked_u32_countdown: false,
            unit_parameters: Vec::new(),
            unit_parameter_homes: Vec::new(),
            unit_scalar_homes: vec![home],
            unit_integer_constants: Vec::new(),
            unit_affine_scalar_records: Vec::new(),
            unit_structural_scalar_field_stores: Vec::new(),
            unit_write_only_primitive_stores: Vec::new(),
            scalar_structural_scalar_field_stores: Vec::new(),
            unit_affine_cleanup: None,
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
        };
        (call, function)
    }

    #[test]
    fn installed_forwarded_result_rejoins_semantic_home_placement_and_span() {
        let target = NativeTarget::linux_x64();
        let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
        for scalar_type in [integer, ScalarType::Boolean] {
            let (valid, function) = forwarded_result_fixture(scalar_type);
            assert!(installed_forwarded_dynamic_scalar_result_is_canonical(
                &valid, &function, target
            ));

            let mut wrong_semantic_value = valid.clone();
            wrong_semantic_value.semantic_result.as_mut().unwrap().value =
                ValueId::new(99).expect("different value");
            assert!(!installed_forwarded_dynamic_scalar_result_is_canonical(
                &wrong_semantic_value,
                &function,
                target
            ));

            let mut wrong_semantic_type = valid.clone();
            wrong_semantic_type
                .semantic_result
                .as_mut()
                .unwrap()
                .scalar_type = match scalar_type {
                ScalarType::Boolean => integer,
                ScalarType::Integer(_) => ScalarType::Boolean,
                ScalarType::IeeeFloat(_) => unreachable!(),
            };
            assert!(!installed_forwarded_dynamic_scalar_result_is_canonical(
                &wrong_semantic_type,
                &function,
                target
            ));

            let mut wrong_home = valid.clone();
            wrong_home.result.as_mut().unwrap().home.source_value =
                ValueId::new(99).expect("different value");
            assert!(!installed_forwarded_dynamic_scalar_result_is_canonical(
                &wrong_home,
                &function,
                target
            ));

            let mut wrong_shape = valid.clone();
            wrong_shape.result.as_mut().unwrap().home.shape = ValueShape::integer(8, 8);
            assert!(!installed_forwarded_dynamic_scalar_result_is_canonical(
                &wrong_shape,
                &function,
                target
            ));

            let mut missing_home = function.clone();
            missing_home.unit_scalar_homes.clear();
            assert!(!installed_forwarded_dynamic_scalar_result_is_canonical(
                &valid,
                &missing_home,
                target
            ));

            let mut wrong_placement = valid.clone();
            wrong_placement.result.as_mut().unwrap().source.shape = ValueShape::integer(8, 8);
            assert!(!installed_forwarded_dynamic_scalar_result_is_canonical(
                &wrong_placement,
                &function,
                target
            ));

            let mut truncated_result = valid;
            truncated_result.result.as_mut().unwrap().byte_count -= 1;
            assert!(!installed_forwarded_dynamic_scalar_result_is_canonical(
                &truncated_result,
                &function,
                target
            ));
        }
    }
}
