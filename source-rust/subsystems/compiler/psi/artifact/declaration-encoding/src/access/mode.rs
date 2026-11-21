//! Access modes on structural parameters, arguments and stored views.
use crate::CodecError;
use crate::fields::Field;
use crate::wire::{Reader, Writer};
use terminal_psi::StructuralAccess;

pub struct Access;

impl Field<StructuralAccess> for Access {
    fn write(writer: &mut Writer, value: &StructuralAccess) -> Result<(), CodecError> {
        encode_structural_access(writer, *value);
        Ok(())
    }

    fn read(reader: &mut Reader<'_>) -> Result<StructuralAccess, CodecError> {
        decode_structural_access(reader)
    }
}

pub fn encode_structural_access(writer: &mut Writer, access: StructuralAccess) {
    writer.u8(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

pub fn decode_structural_access(reader: &mut Reader<'_>) -> Result<StructuralAccess, CodecError> {
    decode_structural_access_tag(reader.u8()?)
}

pub fn decode_structural_access_tag(tag: u8) -> Result<StructuralAccess, CodecError> {
    match tag {
        1 => Ok(StructuralAccess::Owned),
        2 => Ok(StructuralAccess::SharedBorrow),
        3 => Ok(StructuralAccess::MutableBorrow),
        4 => Ok(StructuralAccess::WriteOnlyBorrow),
        tag => Err(CodecError::InvalidTag("StructuralAccess", tag)),
    }
}
