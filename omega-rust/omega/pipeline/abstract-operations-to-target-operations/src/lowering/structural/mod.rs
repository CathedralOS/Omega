//! Optimizer module role: stage group.
//! Register-fragment constraints for structural graph returns.
use super::shared::*;

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
