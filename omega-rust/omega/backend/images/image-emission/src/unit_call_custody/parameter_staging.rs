//! Independently replay structural entry staging before the first Unit call.
//!
//! The caller rejoins the canonical ABI and frame roster. This replay retains
//! each incoming fragment's source geometry and stores only its declared bytes,
//! including packed fragments and saved indirect pointers.

use calling_conventions::{IndirectPointerLocation, ValueLocation};
use machine_code::UnitParameterHomeRecord;
use target::{Architecture, NativeTarget};

use super::{packed_fragment, projected_copy};
use crate::instruction_loads::{
    aarch64_terminal_register, expected_aarch64_stack_load, expected_x86_stack_load,
    x86_terminal_register,
};

pub(crate) fn expected_bytes(
    target: NativeTarget,
    homes: &[UnitParameterHomeRecord],
    frame_bytes: u32,
) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    for home in homes {
        let home_offset = home.location.stack_byte_offset()?;
        if home.indirect {
            let [ValueLocation::Indirect { pointer, .. }] = home.source.locations.as_slice() else {
                return None;
            };
            let register = match *pointer {
                IndirectPointerLocation::Register(register) => match target.architecture {
                    Architecture::X86_64 => x86_terminal_register(register)?,
                    Architecture::Aarch64 => aarch64_terminal_register(register)?,
                },
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let register = scratch(target.architecture);
                    stack_load(
                        &mut bytes,
                        target.architecture,
                        register,
                        incoming_offset(target.architecture, frame_bytes, stack_byte_offset)?,
                        8,
                    )?;
                    register
                }
            };
            stack_store(&mut bytes, target.architecture, register, home_offset, 8)?;
            continue;
        }
        for fragment in &home.source.locations {
            let (value_byte_offset, byte_size) = match *fragment {
                ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                }
                | ValueLocation::Stack {
                    value_byte_offset,
                    byte_size,
                    ..
                } => (value_byte_offset, byte_size),
                ValueLocation::Indirect { .. } => return None,
            };
            let destination = home_offset.checked_add(u32::from(value_byte_offset))?;
            let register = match *fragment {
                ValueLocation::Register { register, .. } => match target.architecture {
                    Architecture::X86_64 => x86_terminal_register(register)?,
                    Architecture::Aarch64 => aarch64_terminal_register(register)?,
                },
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let register = scratch(target.architecture);
                    stack_load(
                        &mut bytes,
                        target.architecture,
                        register,
                        incoming_offset(target.architecture, frame_bytes, stack_byte_offset)?,
                        byte_size,
                    )?;
                    register
                }
                ValueLocation::Indirect { .. } => return None,
            };
            stack_store(
                &mut bytes,
                target.architecture,
                register,
                destination,
                byte_size,
            )?;
        }
    }
    Some(bytes)
}

fn scratch(architecture: Architecture) -> u8 {
    match architecture {
        Architecture::X86_64 => 0,
        Architecture::Aarch64 => 9,
    }
}

fn incoming_offset(
    architecture: Architecture,
    frame_bytes: u32,
    stack_byte_offset: u32,
) -> Option<u32> {
    frame_bytes
        .checked_add(match architecture {
            Architecture::X86_64 => 8,
            Architecture::Aarch64 => 0,
        })?
        .checked_add(stack_byte_offset)
}

fn stack_load(
    bytes: &mut Vec<u8>,
    architecture: Architecture,
    register: u8,
    offset: u32,
    width: u16,
) -> Option<()> {
    match architecture {
        Architecture::X86_64 if packed_fragment::is_packed(width) => {
            packed_fragment::x86_load(bytes, register, offset, width, false)?
        }
        Architecture::X86_64 => expected_x86_stack_load(bytes, register, offset, width)?,
        Architecture::Aarch64 if packed_fragment::is_packed(width) => {
            packed_fragment::aarch64_load(bytes, register, offset, width, false)?
        }
        Architecture::Aarch64 => bytes.extend_from_slice(
            &expected_aarch64_stack_load(register, offset, width)?.to_le_bytes(),
        ),
    }
    Some(())
}

fn stack_store(
    bytes: &mut Vec<u8>,
    architecture: Architecture,
    register: u8,
    offset: u32,
    width: u16,
) -> Option<()> {
    match architecture {
        Architecture::X86_64 if packed_fragment::is_packed(width) => {
            packed_fragment::x86_store(bytes, register, offset, width)?
        }
        Architecture::X86_64 => projected_copy::x86_stack_store(bytes, register, offset, width)?,
        Architecture::Aarch64 if packed_fragment::is_packed(width) => {
            packed_fragment::aarch64_store(bytes, register, offset, width)?
        }
        Architecture::Aarch64 => bytes.extend_from_slice(
            &projected_copy::aarch64_stack_store(register, offset, width)?.to_le_bytes(),
        ),
    }
    Some(())
}
