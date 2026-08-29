use super::shared::*;

pub(super) fn validate_structural_placement(
    place: psi_core::PlaceId,
    placement: &omega_calling_conventions::ValuePlacement,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    let [location] = placement.locations.as_slice() else {
        return Err(AssignmentError::UnsupportedStructuralPlacement(place));
    };
    let ValueLocation::Register { register, .. } = location else {
        return match location {
            ValueLocation::Stack {
                value_byte_offset: 0,
                byte_size,
                alignment,
                ..
            } if u16::try_from(placement.shape.byte_size) == Ok(*byte_size)
                && u16::try_from(placement.shape.alignment) == Ok(*alignment) =>
            {
                Ok(())
            }
            _ => Err(AssignmentError::UnsupportedStructuralPlacement(place)),
        };
    };
    validate_structural_register(place, *register, architecture)
}

pub(super) fn validate_direct_structural_return_placement(
    place: psi_core::PlaceId,
    placement: &omega_calling_conventions::ValuePlacement,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    if placement.shape.class != ValueClass::Integer
        || !((placement.shape.byte_size == 8 && placement.shape.alignment == 8)
            || (9..=16).contains(&placement.shape.byte_size))
    {
        return Err(AssignmentError::UnsupportedStructuralPlacement(place));
    }
    if placement.locations.len() == 1 {
        return validate_structural_placement(place, placement, architecture);
    }
    if placement.locations.len() != 2 || !(9..=16).contains(&placement.shape.byte_size) {
        return Err(AssignmentError::UnsupportedStructuralPlacement(place));
    }
    let mut expected_offset = 0_u16;
    for location in &placement.locations {
        let ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } = *location
        else {
            return Err(AssignmentError::UnsupportedStructuralPlacement(place));
        };
        let expected_size = (placement.shape.byte_size - expected_offset).min(8);
        if value_byte_offset != expected_offset || byte_size != expected_size {
            return Err(AssignmentError::UnsupportedStructuralPlacement(place));
        }
        validate_structural_register(place, register, architecture)?;
        expected_offset = expected_offset
            .checked_add(byte_size)
            .ok_or(AssignmentError::UnsupportedStructuralPlacement(place))?;
    }
    if expected_offset != placement.shape.byte_size {
        return Err(AssignmentError::UnsupportedStructuralPlacement(place));
    }
    Ok(())
}

fn validate_structural_register(
    place: psi_core::PlaceId,
    register: MachineRegister,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    let matches_architecture = match (architecture, register) {
        (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rax)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rcx)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rdx)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rbx)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rsp)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rbp)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rsi)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rdi)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R8)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R9)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R10)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R11)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R12)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R13)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R14)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R15)
        | (Architecture::Aarch64, omega_target_operations::MachineRegister::Aarch64X(0..=30)) => {
            true
        }
        _ => false,
    };
    if !matches_architecture {
        return Err(AssignmentError::StructuralRegisterArchitectureMismatch {
            place,
            register,
            architecture,
        });
    }
    Ok(())
}

pub(super) fn assign_direct_location(
    source_value: ValueId,
    location: ScalarParameterLocation,
    architecture: Architecture,
) -> Result<AssignedScalarLocation, AssignmentError> {
    Ok(match location {
        ScalarParameterLocation::Register(register) => {
            require_register_architecture(source_value, register, architecture)?;
            AssignedScalarLocation::Register(register)
        }
        ScalarParameterLocation::IncomingStack { byte_offset } => {
            AssignedScalarLocation::IncomingStack { byte_offset }
        }
    })
}

pub(super) fn require_register_architecture(
    value: ValueId,
    register: MachineRegister,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    let matches = match architecture {
        Architecture::Aarch64 => matches!(register, MachineRegister::Aarch64X(0..=30)),
        Architecture::X86_64 => matches!(
            register,
            MachineRegister::X86Rax
                | MachineRegister::X86Rcx
                | MachineRegister::X86Rdx
                | MachineRegister::X86Rbx
                | MachineRegister::X86Rsp
                | MachineRegister::X86Rbp
                | MachineRegister::X86Rsi
                | MachineRegister::X86Rdi
                | MachineRegister::X86R8
                | MachineRegister::X86R9
                | MachineRegister::X86R10
                | MachineRegister::X86R11
                | MachineRegister::X86R12
                | MachineRegister::X86R13
                | MachineRegister::X86R14
                | MachineRegister::X86R15
        ),
    };
    if matches {
        Ok(())
    } else {
        Err(AssignmentError::ParameterRegisterArchitectureMismatch {
            value,
            register,
            architecture,
        })
    }
}

pub(super) fn x86_expression_scratch_conflict(register: MachineRegister) -> bool {
    matches!(
        register,
        MachineRegister::X86Rax | MachineRegister::X86R10 | MachineRegister::X86R11
    )
}
