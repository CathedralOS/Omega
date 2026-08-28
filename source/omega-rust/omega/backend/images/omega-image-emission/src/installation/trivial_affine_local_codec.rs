//! Canonical format-36 trivial affine local and local-type rows.

use psi_core::{PlaceId, StructuralPlaceKind, StructuralTypeId};
use psi_terminal::{StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape};

use super::{InstallationError, Reader, push_u32, push_u64};

pub(super) fn encode_trivial_affine_local(
    bytes: &mut Vec<u8>,
    local: &StructuralPlaceDeclaration,
) -> Result<(), InstallationError> {
    let StructuralPlaceKind::TrivialAffineLocal {
        declaration_ordinal,
        structural_type,
    } = local.kind
    else {
        return Err(InstallationError::InvalidStructuralReturnLocal);
    };
    push_u64(bytes, local.id.get());
    push_u32(bytes, declaration_ordinal);
    push_u32(bytes, 0);
    push_u64(bytes, structural_type.get());
    Ok(())
}

pub(super) fn encode_trivial_affine_local_type(
    bytes: &mut Vec<u8>,
    declaration: &StructuralTypeDeclaration,
) -> Result<(), InstallationError> {
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(InstallationError::InvalidStructuralReturnLocal);
    };
    if !fields.is_empty() {
        return Err(InstallationError::InvalidStructuralReturnLocal);
    }
    push_u64(bytes, declaration.id.get());
    push_u32(
        bytes,
        u32::try_from(declaration.identity.len())
            .map_err(|_| InstallationError::StructuralTypeIdentityTooLong)?,
    );
    bytes.extend_from_slice(declaration.identity.as_bytes());
    push_u32(bytes, 0);
    Ok(())
}

pub(super) fn decode_trivial_affine_local(
    reader: &mut Reader<'_>,
) -> Result<StructuralPlaceDeclaration, InstallationError> {
    let id = PlaceId::new(reader.u64()?).ok_or(InstallationError::ZeroStructuralReturnIdentity(
        "local place",
    ))?;
    let declaration_ordinal = reader.u32()?;
    if reader.u32()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
        InstallationError::ZeroStructuralReturnIdentity("local type"),
    )?;
    Ok(StructuralPlaceDeclaration {
        id,
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
        },
    })
}

pub(super) fn decode_trivial_affine_local_type(
    reader: &mut Reader<'_>,
) -> Result<StructuralTypeDeclaration, InstallationError> {
    let id = StructuralTypeId::new(reader.u64()?).ok_or(
        InstallationError::ZeroStructuralReturnIdentity("local type declaration"),
    )?;
    let identity_len = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::StructuralTypeIdentityTooLong)?;
    let identity = std::str::from_utf8(reader.take(identity_len)?)
        .map_err(|_| InstallationError::InvalidStructuralTypeIdentity)?
        .to_owned();
    if identity.is_empty() {
        return Err(InstallationError::InvalidStructuralTypeIdentity);
    }
    if reader.u32()? != 0 {
        return Err(InstallationError::InvalidStructuralReturnLocal);
    }
    Ok(StructuralTypeDeclaration {
        id,
        identity,
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    })
}
