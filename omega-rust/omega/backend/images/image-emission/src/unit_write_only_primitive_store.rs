//! Independent object replay for whole-root non-observing primitive stores.

use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use machine_code::{
    MachineCodeFunction, SemanticCodeSite, UnitWriteOnlyPrimitiveStoreRecord,
    UnitWriteOnlyPrimitiveStoreSourceRecord,
};
use semantic_vocabulary::ScalarType;
use target::NativeTarget;
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

use super::unit_structural_scalar_field_store::{
    expected_home_store_bytes, expected_incoming_parameter_store_bytes,
    expected_parameter_store_bytes, expected_store_bytes, integer_bits,
};
use super::{ObjectError, ObjectUnitStack};

pub(super) fn validate_unit_write_only_primitive_stores(
    target: NativeTarget,
    function: &MachineCodeFunction,
    validated_function_stack: Option<&ObjectUnitStack>,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(function.machine);
    let mut previous = None;
    for store in &function.unit_write_only_primitive_stores {
        let key = (store.operation_ordinal, store.code_offset);
        if previous.is_some_and(|previous| previous >= key)
            || validate_store(target, function, validated_function_stack, store).is_none()
        {
            return Err(invalid());
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_store(
    target: NativeTarget,
    function: &MachineCodeFunction,
    validated_function_stack: Option<&ObjectUnitStack>,
    store: &UnitWriteOnlyPrimitiveStoreRecord,
) -> Option<()> {
    let parameter_index = usize::try_from(store.destination.position).ok()?;
    let parameter = function.unit_parameters.get(parameter_index)?;
    let home = function.unit_parameter_homes.get(parameter_index)?;
    let (source_is_exact, destination_scalar_type, byte_size, bits) = match store.source {
        UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => {
            let abi = function.unit_scalar_abi.as_ref()?;
            let scalar_parameter_index = usize::try_from(parameter_index).ok()?;
            let scalar_parameter = abi.parameters.get(scalar_parameter_index)?;
            let (scalar_shape, byte_size) = native_scalar_shape(scalar_type)?;
            let (expected_location, placed_byte_size) =
                match scalar_parameter.placement.locations.as_slice() {
                    [
                        ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size,
                        },
                    ] => (
                        machine_code::UnitScalarParameterLocationRecord::Register(*register),
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
                    _ => return None,
                };
            let mut parameter_shapes = abi
                .parameters
                .iter()
                .map(|parameter| {
                    let (shape, _) = native_scalar_shape(parameter.scalar_type)?;
                    (parameter.placement.shape == shape).then_some(shape)
                })
                .collect::<Option<Vec<_>>>()?;
            parameter_shapes.push(parameter.shape);
            let expected_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: parameter_shapes,
                    result: None,
                },
            )
            .ok()?;
            (
                function.unit_parameters.len() == 1
                    && scalar_parameter.value == source_value
                    && scalar_parameter.scalar_type == scalar_type
                    && scalar_parameter.placement.shape == scalar_shape
                    && placed_byte_size == byte_size
                    && location == expected_location
                    && abi.call_plan == expected_plan
                    && abi.call_plan.parameters.get(scalar_parameter_index)
                        == Some(&scalar_parameter.placement)
                    && abi.call_plan.parameters.get(abi.parameters.len()) == Some(&home.source),
                scalar_type,
                byte_size,
                None,
            )
        }
        UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
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
            let byte_size = scalar_type.bits().checked_div(8)?;
            (
                source_count == 1
                    && matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
                    && !scalar_type.is_address()
                    && scalar_type.admits(value),
                ScalarType::Integer(scalar_type),
                byte_size,
                Some(integer_bits(scalar_type, value)?),
            )
        }
        UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => (
            definition_ordinal < store.operation_ordinal
                && function
                    .provenance
                    .operations
                    .iter()
                    .filter(|operation| **operation == defining_operation)
                    .count()
                    == 1
                && exact_zero_code_definition_count(
                    function,
                    defining_operation,
                    definition_ordinal,
                    store.code_offset,
                ) == 1
                && function.unit_integer_constants.iter().all(|constant| {
                    constant.defining_operation != defining_operation
                        && constant.source_value != source_value
                })
                && function.unit_scalar_homes.iter().all(|home| {
                    home.defining_operation != defining_operation
                        && home.source_value != source_value
                })
                && zero_code_source_is_consistent(function, store.source),
            ScalarType::Boolean,
            1,
            Some(u64::from(value)),
        ),
        UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            let (byte_size, bits) = match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => (4, u64::from(bits)),
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => (8, bits),
            };
            (
                definition_ordinal < store.operation_ordinal
                    && function
                        .provenance
                        .operations
                        .iter()
                        .filter(|operation| **operation == defining_operation)
                        .count()
                        == 1
                    && exact_zero_code_definition_count(
                        function,
                        defining_operation,
                        definition_ordinal,
                        store.code_offset,
                    ) == 1
                    && function.unit_integer_constants.iter().all(|constant| {
                        constant.defining_operation != defining_operation
                            && constant.source_value != source_value
                    })
                    && function.unit_scalar_homes.iter().all(|home| {
                        home.defining_operation != defining_operation
                            && home.source_value != source_value
                    })
                    && zero_code_source_is_consistent(function, store.source),
                ScalarType::IeeeFloat(value.format()),
                byte_size,
                Some(bits),
            )
        }
        UnitWriteOnlyPrimitiveStoreSourceRecord::Home(source_home) => {
            let source_count = function
                .internal_unit_scalar_calls
                .iter()
                .filter(|call| {
                    call.result.home == source_home
                        && call.operation_ordinal < store.operation_ordinal
                })
                .count();
            let home_count = function
                .unit_scalar_homes
                .iter()
                .filter(|home| **home == source_home)
                .count();
            let ScalarType::Integer(integer) = source_home.scalar_type else {
                return None;
            };
            let (shape, byte_size) = native_scalar_shape(source_home.scalar_type)?;
            (
                source_count == 1 && home_count == 1 && source_home.shape == shape,
                ScalarType::Integer(integer),
                byte_size,
                None,
            )
        }
    };
    if store.destination_type.shape != StructuralTypeShape::PrimitiveScalar(destination_scalar_type)
    {
        return None;
    }
    let expected_shape = ValueShape::borrowed_reference(byte_size, byte_size.min(8));
    let [
        ValueLocation::Indirect {
            copy_stack_byte_offset: None,
            byte_size: placement_byte_size,
            alignment: placement_alignment,
            ..
        },
    ] = home.source.locations.as_slice()
    else {
        return None;
    };
    let destination_type_count = function
        .unit_affine_cleanup
        .as_ref()?
        .structural_types
        .iter()
        .filter(|candidate| *candidate == &store.destination_type)
        .count();
    if !source_is_exact
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
        || *placement_byte_size != byte_size
        || *placement_alignment != byte_size.min(8)
        || store.parameter_home_byte_offset != home.byte_offset
        || !store.parameter_home_indirect
        || !function
            .provenance
            .operations
            .contains(&store.psi_operation)
        || exact_attribution_count(function, store) != 1
        || target.pointer_size != 8
        || target.pointer_alignment != 8
    {
        return None;
    }
    let expected = match store.source {
        UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
            location: machine_code::UnitScalarParameterLocationRecord::Register(register),
            ..
        } => expected_parameter_store_bytes(target, home, 0, byte_size, register)?,
        UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
            location: machine_code::UnitScalarParameterLocationRecord::IncomingStack { byte_offset },
            ..
        } => expected_incoming_parameter_store_bytes(
            target,
            home,
            0,
            byte_size,
            byte_offset,
            validated_function_stack?.frame_bytes,
        )?,
        UnitWriteOnlyPrimitiveStoreSourceRecord::Home(source_home) => {
            expected_home_store_bytes(target, home, 0, byte_size, source_home)?
        }
        _ => expected_store_bytes(target, home, 0, byte_size, bits?)?,
    };
    let end = store.code_offset.checked_add(store.byte_count)?;
    if store.byte_count == 0
        || store.byte_count != expected.len()
        || store.bytes != expected
        || function.bytes.get(store.code_offset..end) != Some(expected.as_slice())
    {
        return None;
    }
    Some(())
}

pub(crate) fn native_scalar_shape(scalar_type: ScalarType) -> Option<(ValueShape, u16)> {
    match scalar_type {
        ScalarType::Boolean => Some((ValueShape::integer(1, 1), 1)),
        ScalarType::Integer(integer)
            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            let byte_size = integer.bits().checked_div(8)?;
            Some((ValueShape::integer(byte_size, byte_size.min(8)), byte_size))
        }
        _ => None,
    }
}

fn zero_code_source_is_consistent(
    function: &MachineCodeFunction,
    source: UnitWriteOnlyPrimitiveStoreSourceRecord,
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

fn exact_zero_code_definition_count(
    function: &MachineCodeFunction,
    defining_operation: semantic_vocabulary::OperationId,
    definition_ordinal: usize,
    latest_code_offset: usize,
) -> usize {
    function
        .semantic_code_attribution
        .iter()
        .filter(|row| {
            row.site == SemanticCodeSite::Operation(defining_operation)
                && row.operation_ordinal == definition_ordinal
                && row.code_offset <= latest_code_offset
                && row.byte_count == 0
        })
        .count()
}

fn exact_attribution_count(
    function: &MachineCodeFunction,
    store: &UnitWriteOnlyPrimitiveStoreRecord,
) -> usize {
    function
        .semantic_code_attribution
        .iter()
        .filter(|row| {
            row.site == SemanticCodeSite::Operation(store.psi_operation)
                && row.operation_ordinal == store.operation_ordinal
                && row.code_offset == store.code_offset
                && row.byte_count == store.byte_count
        })
        .count()
}
