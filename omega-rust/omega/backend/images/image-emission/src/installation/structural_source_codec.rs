//! Explicit structural source roles in installation format 80.
use super::value_placement_codec::{decode_register, register_tag};
use super::{InstallationError, Reader, push_u32};
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
        tag => Err(InstallationError::InvalidStructuralSourceLocationTag(tag)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_roles_round_trip_without_collapsing_registers_into_stack_offsets() {
        for source in [
            StructuralSourceLocation::Stack { byte_offset: 32 },
            StructuralSourceLocation::IncomingIndirectPointer {
                register: calling_conventions::MachineRegister::X86Rcx,
            },
        ] {
            let mut bytes = Vec::new();
            encode(&mut bytes, source).unwrap();
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode(&mut reader).unwrap(), source);
            assert_eq!(reader.remaining(), 0);
        }
        assert_eq!(
            decode(&mut Reader::new(&[9])),
            Err(InstallationError::InvalidStructuralSourceLocationTag(9))
        );
    }
}
