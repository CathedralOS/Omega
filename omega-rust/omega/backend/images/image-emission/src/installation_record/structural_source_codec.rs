//! Explicit structural source roles in the current installation envelope.
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
        StructuralSourceLocation::IncomingIndirectStackPointer {
            stack_byte_offset,
            alignment,
        } => {
            bytes.push(5);
            push_u32(bytes, stack_byte_offset);
            push_u16(bytes, alignment);
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
        5 => Ok(StructuralSourceLocation::IncomingIndirectStackPointer {
            stack_byte_offset: reader.u32()?,
            alignment: reader.u16()?,
        }),
        tag => Err(InstallationError::InvalidStructuralSourceLocationTag(tag)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_stack_pointer_wire_role_preserves_raw_geometry() {
        let source = StructuralSourceLocation::IncomingIndirectStackPointer {
            stack_byte_offset: 0x0102_0308,
            alignment: 8,
        };
        let mut bytes = Vec::new();
        encode(&mut bytes, source).unwrap();
        assert_eq!(bytes, [5, 8, 3, 2, 1, 8, 0]);
        assert_eq!(decode(&mut Reader::new(&bytes)).unwrap(), source);
        let mut borrowed = bytes.clone();
        borrowed[0] = 4;
        assert_ne!(decode(&mut Reader::new(&borrowed)).unwrap(), source);

        // Raw installation decoding must preserve even invalid proposals. The
        // receiving plan rejects wrong offsets/alignment; decoding cannot repair
        // them into the canonical pointer slot or confuse it with local storage.
        for (stack_byte_offset, alignment) in [
            (0x0102_0309, 8),
            (0x0102_0310, 8),
            (0x0102_0308, 0),
            (0x0102_0308, 3),
            (0x0102_0308, 16),
            (u32::MAX, u16::MAX),
        ] {
            let proposal = StructuralSourceLocation::IncomingIndirectStackPointer {
                stack_byte_offset,
                alignment,
            };
            let mut encoded = Vec::new();
            encode(&mut encoded, proposal).unwrap();
            let mut reader = Reader::new(&encoded);
            let decoded = decode(&mut reader).unwrap();
            assert_eq!(reader.remaining(), 0);
            assert_eq!(decoded, proposal);
            assert_ne!(decoded, source);
            assert_eq!(decoded.stack_byte_offset(), None);
        }
    }

    #[test]
    fn existing_source_role_tags_remain_stable() {
        for (source, expected) in [
            (
                StructuralSourceLocation::Stack { byte_offset: 32 },
                vec![1, 32, 0, 0, 0],
            ),
            (
                StructuralSourceLocation::IncomingIndirectPointer {
                    register: calling_conventions::MachineRegister::Aarch64X(0),
                },
                vec![2, 4],
            ),
            (
                StructuralSourceLocation::IncomingBorrowedPointer {
                    location: IndirectPointerLocation::Register(
                        calling_conventions::MachineRegister::Aarch64X(0),
                    ),
                },
                vec![3, 4],
            ),
            (
                StructuralSourceLocation::IncomingBorrowedPointer {
                    location: IndirectPointerLocation::Stack {
                        stack_byte_offset: 32,
                        alignment: 8,
                    },
                },
                vec![4, 32, 0, 0, 0, 8, 0],
            ),
        ] {
            let mut bytes = Vec::new();
            encode(&mut bytes, source).unwrap();
            assert_eq!(bytes, expected);
            assert_eq!(decode(&mut Reader::new(&expected)).unwrap(), source);
        }
    }
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
            StructuralSourceLocation::IncomingIndirectStackPointer {
                stack_byte_offset: 32,
                alignment: 8,
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
            assert_eq!(
                source.stack_byte_offset(),
                match source {
                    StructuralSourceLocation::Stack { byte_offset } => Some(byte_offset),
                    _ => None,
                }
            );
        }
        assert_eq!(
            decode(&mut Reader::new(&[9])),
            Err(InstallationError::InvalidStructuralSourceLocationTag(9))
        );
    }
}
