//! Canonical format-34 sum-case catalogs.

use psi_core::StructuralCaseId;
use psi_terminal::StructuralCaseDeclaration;

use super::{
    Reader, TerminalInstallationError, push_u32, push_u64,
    structural_field_codec::{decode_structural_field, encode_structural_field},
    structural_scalar_codec::{decode_identity, encode_identity},
};

pub(super) fn encode_structural_cases(
    bytes: &mut Vec<u8>,
    cases: &[StructuralCaseDeclaration],
) -> Result<(), TerminalInstallationError> {
    push_u32(
        bytes,
        u32::try_from(cases.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralCases)?,
    );
    for case in cases {
        push_u64(bytes, case.id.get());
        encode_identity(bytes, &case.identity)?;
        push_u32(
            bytes,
            u32::try_from(case.fields.len())
                .map_err(|_| TerminalInstallationError::TooManyStructuralFields)?,
        );
        for field in &case.fields {
            encode_structural_field(bytes, field)?;
        }
    }
    Ok(())
}

pub(super) fn decode_structural_cases(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralCaseDeclaration>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralCases)?;
    if count > reader.remaining() {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut cases = Vec::with_capacity(count);
    for _ in 0..count {
        let id = StructuralCaseId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("structural case"),
        )?;
        let identity = decode_identity(reader)?;
        let field_count = usize::try_from(reader.u32()?)
            .map_err(|_| TerminalInstallationError::TooManyStructuralFields)?;
        if field_count > reader.remaining() {
            return Err(TerminalInstallationError::UnexpectedEnd);
        }
        let mut fields = Vec::with_capacity(field_count);
        for _ in 0..field_count {
            fields.push(decode_structural_field(reader)?);
        }
        cases.push(StructuralCaseDeclaration {
            id,
            identity,
            fields,
        });
    }
    Ok(cases)
}
