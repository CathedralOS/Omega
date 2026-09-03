//! Exact machine emission for a whole-root non-observing primitive store.

use std::collections::BTreeMap;

use omega_assigned_target_operations::{
    AssignedUnitBody, AssignedUnitOperation, AssignedUnitScalarArgumentSource,
};
use omega_calling_conventions::ValueShape;
use omega_machine_code::{
    InternalUnitScalarArgumentSourceRecord, UnitWriteOnlyPrimitiveStoreRecord,
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
    let AssignedUnitScalarArgumentSource::IntegerImmediate {
        defining_operation,
        source_value,
        scalar_type,
        value,
    } = *source
    else {
        return Err(invalid());
    };
    let parameter_index = usize::try_from(destination.position).map_err(|_| invalid())?;
    let parameter = body.parameters.get(parameter_index).ok_or_else(invalid)?;
    let byte_size = require_native_integer_width(source_value, scalar_type)? / 8;
    let expected_shape = ValueShape::borrowed_reference(byte_size, byte_size.min(8));
    if established_integer_constants.get(&source_value)
        != Some(&(defining_operation, scalar_type, value))
        || destination.is_self
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || destination_type.id != destination.structural_type
        || destination_type.shape
            != StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(scalar_type))
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
    let bits = integer_bits(source_value, scalar_type, value)?;
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
        source: InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        },
        parameter_home_byte_offset,
        parameter_home_indirect,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
        bytes: bytes[code_offset..].to_vec(),
    })
}
