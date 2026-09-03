//! Exact machine emission for a whole-root non-observing primitive store.

use std::collections::BTreeMap;

use omega_assigned_target_operations::{
    AssignedUnitBody, AssignedUnitOperation, AssignedUnitWriteOnlyPrimitiveStoreSource,
};
use omega_calling_conventions::ValueShape;
use omega_machine_code::{
    UnitWriteOnlyPrimitiveStoreRecord, UnitWriteOnlyPrimitiveStoreSourceRecord,
};
use omega_target::{Architecture, NativeTarget};
use psi_core::{IntegerType, IntegerValue, OperationId, ScalarType, ValueId};
use psi_terminal::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

use super::structural_scalar::{
    emit_aarch64_unit_store_immediate, emit_x86_64_unit_store_immediate,
};
use super::{Aarch64UnitParameterHome, X86UnitParameterHome};
use crate::{EmissionError, integer_bits, require_native_integer_width};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_write_only_primitive_store(
    operation: &AssignedUnitOperation,
    body: &AssignedUnitBody,
    target: NativeTarget,
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    established_integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    established_boolean_constants: &BTreeMap<ValueId, (OperationId, bool, usize)>,
    established_ieee_float_constants: &BTreeMap<
        ValueId,
        (OperationId, psi_core::IeeeFloatValue, usize),
    >,
    bytes: &mut Vec<u8>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<UnitWriteOnlyPrimitiveStoreRecord, EmissionError> {
    let AssignedUnitOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination,
        destination_type,
        destination_placement,
        source,
    } = operation
    else {
        unreachable!("whole-root store router supplied another operation")
    };
    let invalid = || EmissionError::InvalidWriteOnlyPrimitiveStoreCustody(*psi_operation);
    let (source_record, destination_scalar_type, byte_size, bits) = match *source {
        AssignedUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            if established_integer_constants.get(&source_value)
                != Some(&(defining_operation, scalar_type, value))
            {
                return Err(invalid());
            }
            let byte_size = require_native_integer_width(source_value, scalar_type)? / 8;
            (
                UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                },
                ScalarType::Integer(scalar_type),
                byte_size,
                integer_bits(source_value, scalar_type, value)?,
            )
        }
        AssignedUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            let Some((retained_operation, retained_value, definition_ordinal)) =
                established_boolean_constants.get(&source_value).copied()
            else {
                return Err(invalid());
            };
            if retained_operation != defining_operation || retained_value != value {
                return Err(invalid());
            }
            (
                UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                },
                ScalarType::Boolean,
                1,
                u64::from(value),
            )
        }
        AssignedUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            let Some((retained_operation, retained_value, definition_ordinal)) =
                established_ieee_float_constants.get(&source_value).copied()
            else {
                return Err(invalid());
            };
            if retained_operation != defining_operation || retained_value != value {
                return Err(invalid());
            }
            let (byte_size, bits) = match value {
                psi_core::IeeeFloatValue::Binary32(bits) => (4, u64::from(bits)),
                psi_core::IeeeFloatValue::Binary64(bits) => (8, bits),
            };
            (
                UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                },
                ScalarType::IeeeFloat(value.format()),
                byte_size,
                bits,
            )
        }
    };
    let parameter_index = usize::try_from(destination.position).map_err(|_| invalid())?;
    let parameter = body.parameters.get(parameter_index).ok_or_else(invalid)?;
    let expected_shape = ValueShape::borrowed_reference(byte_size, byte_size.min(8));
    if destination.is_self
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || destination_type.id != destination.structural_type
        || destination_type.shape != StructuralTypeShape::PrimitiveScalar(destination_scalar_type)
        || !body
            .structural_types
            .iter()
            .any(|candidate| candidate == destination_type)
        || parameter.place != destination.place
        || parameter.structural_type != destination.structural_type
        || parameter.multiplicity != destination.multiplicity
        || parameter.access != destination.access
        || parameter.projected_qualifications != destination.projected_qualifications
        || parameter.shape != expected_shape
        || &parameter.placement != destination_placement
    {
        return Err(invalid());
    }
    let (parameter_home_byte_offset, parameter_home_indirect) = match target.architecture {
        Architecture::X86_64 => {
            let home = x86_homes
                .iter()
                .find(|home| home.place == destination.place)
                .ok_or(EmissionError::MissingUnitParameterHome(destination.place))?;
            if home.source != *destination_placement || home.shape != parameter.shape {
                return Err(EmissionError::UnitParameterHomeMismatch(destination.place));
            }
            emit_x86_64_unit_store_immediate(bytes, home, 0, byte_size, bits)?;
            (home.byte_offset, home.indirect)
        }
        Architecture::Aarch64 => {
            let home = aarch64_homes
                .iter()
                .find(|home| home.place == destination.place)
                .ok_or(EmissionError::MissingUnitParameterHome(destination.place))?;
            if home.source != *destination_placement || home.shape != parameter.shape {
                return Err(EmissionError::UnitParameterHomeMismatch(destination.place));
            }
            emit_aarch64_unit_store_immediate(bytes, home, 0, byte_size, bits)?;
            (home.byte_offset, home.indirect)
        }
    };
    Ok(UnitWriteOnlyPrimitiveStoreRecord {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        destination_type: destination_type.clone(),
        destination_placement: destination_placement.clone(),
        source: source_record,
        parameter_home_byte_offset,
        parameter_home_indirect,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
        bytes: bytes[code_offset..].to_vec(),
    })
}
