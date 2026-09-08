//! Explicit structural source roles in installation format 88.
use super::value_placement_codec::{decode_register, register_tag};
use super::{InstallationError, Reader, push_u16, push_u32};
use calling_conventions::IndirectPointerLocation;
use machine_code::StructuralSourceLocation;
pub(super) fn encode(
    bytes: &mut Vec<u8>,
    source: StructuralSourceLocation,
) -> Result<(), InstallationError> {
    match source {
        StructuralSourceLocation::Stack { byte_offset } => {
            bytes.push(1);
            push_u32(bytes, byte_offset);
        }
        StructuralSourceLocation::IncomingIndirectPointer { register } => {
            bytes.push(2);
            bytes.push(register_tag(register)?);
        }
        StructuralSourceLocation::IncomingBorrowedPointer { location } => match location {
            IndirectPointerLocation::Register(register) => {
                bytes.push(3);
                bytes.push(register_tag(register)?);
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset,
                alignment,
            } => {
                bytes.push(4);
                push_u32(bytes, stack_byte_offset);
                push_u16(bytes, alignment);
            }
        },
    }
    Ok(())
}
pub(super) fn decode(
    reader: &mut Reader<'_>,
) -> Result<StructuralSourceLocation, InstallationError> {
    match reader.u8()? {
        1 => Ok(StructuralSourceLocation::Stack {
            byte_offset: reader.u32()?,
        }),
        2 => Ok(StructuralSourceLocation::IncomingIndirectPointer {
            register: decode_register(reader.u8()?)?,
        }),
        3 => Ok(StructuralSourceLocation::IncomingBorrowedPointer {
            location: IndirectPointerLocation::Register(decode_register(reader.u8()?)?),
        }),
        4 => Ok(StructuralSourceLocation::IncomingBorrowedPointer {
            location: IndirectPointerLocation::Stack {
                stack_byte_offset: reader.u32()?,
                alignment: reader.u16()?,
            },
        }),
        tag => Err(InstallationError::InvalidStructuralSourceLocationTag(tag)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn borrowed_pointer_wire_roles_retain_exact_stack_geometry() {
        let mut register = Vec::new();
        encode(
            &mut register,
            StructuralSourceLocation::IncomingBorrowedPointer {
                location: IndirectPointerLocation::Register(
                    calling_conventions::MachineRegister::Aarch64X(0),
                ),
            },
        )
        .unwrap();
        assert_eq!(register, [3, 4]);
        let mut stack = Vec::new();
        encode(
            &mut stack,
            StructuralSourceLocation::IncomingBorrowedPointer {
                location: IndirectPointerLocation::Stack {
                    stack_byte_offset: 0x0102_0304,
                    alignment: 8,
                },
            },
        )
        .unwrap();
        assert_eq!(stack, [4, 4, 3, 2, 1, 8, 0]);
        // The raw codec retains the exact proposal; receiving ABI validation
        // separately rejects invalid geometry rather than normalizing it.
        for field in 1..stack.len() {
            let mut changed = stack.clone();
            changed[field] ^= 1;
            assert_ne!(
                decode(&mut Reader::new(&changed)),
                decode(&mut Reader::new(&stack))
            );
        }
        assert!(decode(&mut Reader::new(&[3, u8::MAX])).is_err());
    }

    #[test]
    fn source_roles_round_trip_without_collapsing_registers_into_stack_offsets() {
        for source in [
            StructuralSourceLocation::Stack { byte_offset: 32 },
            StructuralSourceLocation::IncomingIndirectPointer {
                register: calling_conventions::MachineRegister::X86Rcx,
            },
            StructuralSourceLocation::IncomingBorrowedPointer {
                location: IndirectPointerLocation::Register(
                    calling_conventions::MachineRegister::Aarch64X(0),
                ),
            },
            StructuralSourceLocation::IncomingBorrowedPointer {
                location: IndirectPointerLocation::Stack {
                    stack_byte_offset: 32,
                    alignment: 8,
                },
            },
        ] {
            let mut bytes = Vec::new();
            encode(&mut bytes, source).unwrap();
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode(&mut reader).unwrap(), source);
            assert_eq!(reader.remaining(), 0);
            for length in 0..bytes.len() {
                assert!(decode(&mut Reader::new(&bytes[..length])).is_err());
            }
            if matches!(
                source,
                StructuralSourceLocation::IncomingBorrowedPointer { .. }
            ) {
                assert_eq!(source.stack_byte_offset(), None);
            }
        }
        assert_eq!(
            decode(&mut Reader::new(&[9])),
            Err(InstallationError::InvalidStructuralSourceLocationTag(9))
        );
    }
}
