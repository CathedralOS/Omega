//! Independent object replay for whole-root non-observing primitive stores.

use omega_calling_conventions::{ValueLocation, ValueShape};
use omega_machine_code::{
    MachineCodeFunction, SemanticCodeSite, UnitWriteOnlyPrimitiveStoreRecord,
    UnitWriteOnlyPrimitiveStoreSourceRecord,
};
use omega_target::NativeTarget;
use psi_core::ScalarType;
use psi_terminal::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

use super::ObjectError;
use super::unit_structural_scalar_field_store::{expected_store_bytes, integer_bits};

pub(super) fn validate_unit_write_only_primitive_stores(
    target: NativeTarget,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(function.machine);
    let mut previous = None;
    for store in &function.unit_write_only_primitive_stores {
        let key = (store.operation_ordinal, store.code_offset);
        if previous.is_some_and(|previous| previous >= key)
            || validate_store(target, function, store).is_none()
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
    store: &UnitWriteOnlyPrimitiveStoreRecord,
) -> Option<()> {
    let parameter_index = usize::try_from(store.destination.position).ok()?;
    let parameter = function.unit_parameters.get(parameter_index)?;
    let home = function.unit_parameter_homes.get(parameter_index)?;
    let (source_is_exact, destination_scalar_type, byte_size, bits) = match store.source {
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
                integer_bits(scalar_type, value)?,
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
                && boolean_source_is_consistent(
                    function,
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                ),
            ScalarType::Boolean,
            1,
            u64::from(value),
        ),
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
    let expected = expected_store_bytes(target, home, 0, byte_size, bits)?;
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

fn boolean_source_is_consistent(
    function: &MachineCodeFunction,
    defining_operation: psi_core::OperationId,
    source_value: psi_core::ValueId,
    value: bool,
    definition_ordinal: usize,
) -> bool {
    function
        .unit_write_only_primitive_stores
        .iter()
        .all(|candidate| match candidate.source {
            UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
                defining_operation: candidate_operation,
                source_value: candidate_value,
                value: candidate_literal,
                definition_ordinal: candidate_ordinal,
            } if candidate_operation == defining_operation || candidate_value == source_value => {
                candidate_operation == defining_operation
                    && candidate_value == source_value
                    && candidate_literal == value
                    && candidate_ordinal == definition_ordinal
            }
            _ => true,
        })
}

fn exact_zero_code_definition_count(
    function: &MachineCodeFunction,
    defining_operation: psi_core::OperationId,
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
