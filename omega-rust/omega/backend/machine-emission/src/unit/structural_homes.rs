//! Exact stores and subsequent memory views for structural call-result homes.

use super::*;
use assigned_target_operations::AssignedStructuralHome;
use machine_code::InternalStructuralResultHomeRecord;

pub(super) fn call_home(
    operation: &AssignedUnitOperation,
) -> Result<Option<(&AssignedStructuralHome, &ValuePlacement)>, EmissionError> {
    let AssignedUnitOperation::StructuralResultCall {
        psi_operation,
        result,
        result_home: Some(home),
        call_plan,
        scalar_arguments,
        ..
    } = operation
    else {
        return Ok(None);
    };
    let invalid = || EmissionError::InvalidStructuralScalarCallCustody(*psi_operation);
    let shape = home.requirement.layout.shape();
    let placement = call_plan.result.as_ref().ok_or_else(invalid)?;
    if home.requirement.defining_operation != *psi_operation
        || home.requirement.result != *result
        || home.requirement.layout
            != target_operations::TargetStructuralHomeLayout::Aggregate(shape)
        || !((shape.byte_size == 8 && shape.alignment == 8) || (9..=16).contains(&shape.byte_size))
        || shape.alignment == 0
        || shape.class != ValueClass::Integer
        || placement.shape != shape
        || home.byte_offset % u32::from(shape.alignment.max(8)) != 0
        || !scalar_arguments.is_empty()
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || placement.locations.len() != usize::from(shape.byte_size.div_ceil(8))
        || placement
            .locations
            .iter()
            .enumerate()
            .any(|(index, location)| {
                !matches!(location, ValueLocation::Register { value_byte_offset, byte_size, .. }
                if usize::from(*value_byte_offset) == index * 8
                    && matches!(*byte_size, 1 | 2 | 4 | 8)
                    && usize::from(*byte_size)
                        == (usize::from(shape.byte_size) - index * 8).min(8))
            })
    {
        return Err(invalid());
    }
    Ok(Some((home, placement)))
}

pub(super) fn emit_result_stores(
    operation: &AssignedUnitOperation,
    target: NativeTarget,
    frame_bytes: u32,
    bytes: &mut Vec<u8>,
) -> Result<Option<InternalStructuralResultHomeRecord>, EmissionError> {
    let Some((home, placement)) = call_home(operation)? else {
        return Ok(None);
    };
    let invalid =
        || EmissionError::InvalidStructuralScalarCallCustody(home.requirement.defining_operation);
    if home
        .byte_offset
        .checked_add(u32::from(placement.shape.byte_size))
        .is_none_or(|end| end > frame_bytes)
    {
        return Err(invalid());
    }
    let code_offset = bytes.len();
    for location in &placement.locations {
        let ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } = location
        else {
            return Err(invalid());
        };
        let offset = home
            .byte_offset
            .checked_add(u32::from(*value_byte_offset))
            .ok_or_else(invalid)?;
        match target.architecture {
            Architecture::X86_64 => emit_x86_64_stack_store_width(
                bytes,
                x86_unit_register(*register)?,
                offset,
                *byte_size,
            )?,
            Architecture::Aarch64 => {
                let instruction = aarch64_unit_stack_access(
                    aarch64_store_base(*byte_size)?,
                    aarch64_unit_register(*register)?,
                    offset,
                    *byte_size,
                )?;
                bytes.extend_from_slice(&instruction.to_le_bytes());
            }
        }
    }
    Ok(Some(InternalStructuralResultHomeRecord {
        requirement: home.requirement.clone(),
        home_byte_offset: home.byte_offset,
        code_offset,
        byte_count: bytes.len() - code_offset,
        bytes: bytes[code_offset..].to_vec(),
    }))
}

/// Result memory views are private copy inputs, never published parameter rows.
pub(super) fn call_sources(
    preceding: &[AssignedUnitOperation],
    x86_parameters: &[X86UnitStructuralHome],
    aarch64_parameters: &[Aarch64UnitStructuralHome],
) -> Result<(Vec<X86UnitStructuralHome>, Vec<Aarch64UnitStructuralHome>), EmissionError> {
    let mut x86 = x86_parameters.to_vec();
    let mut aarch64 = aarch64_parameters.to_vec();
    for operation in preceding {
        let Some((home, placement)) = call_home(operation)? else {
            continue;
        };
        let place = home.requirement.result.place;
        if x86.iter().any(|source| source.place == place)
            || aarch64.iter().any(|source| source.place == place)
        {
            return Err(EmissionError::InvalidStructuralScalarCallCustody(
                home.requirement.defining_operation,
            ));
        }
        x86.push(X86UnitStructuralHome {
            place,
            shape: placement.shape,
            source: placement.clone(),
            byte_offset: home.byte_offset,
            indirect: false,
        });
        aarch64.push(Aarch64UnitStructuralHome {
            place,
            shape: placement.shape,
            source: placement.clone(),
            byte_offset: home.byte_offset,
            indirect: false,
        });
    }
    Ok((x86, aarch64))
}
