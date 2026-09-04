//! Canonical format-36 record-field catalogs.

use psi_terminal::StructuralFieldDeclaration;

use super::{
    InstallationError, Reader, push_u32,
    structural_field_codec::{decode_structural_field, encode_structural_field},
};

pub(super) fn encode_structural_fields(
    bytes: &mut Vec<u8>,
    fields: &[StructuralFieldDeclaration],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(fields.len()).map_err(|_| InstallationError::TooManyStructuralFields)?,
    );
    for field in fields {
        encode_structural_field(bytes, field)?;
    }
    Ok(())
}

pub(super) fn decode_structural_fields(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralFieldDeclaration>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyStructuralFields)?;
    if count > reader.remaining() {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut fields = Vec::with_capacity(count);
    for _ in 0..count {
        fields.push(decode_structural_field(reader)?);
    }
    Ok(fields)
}
