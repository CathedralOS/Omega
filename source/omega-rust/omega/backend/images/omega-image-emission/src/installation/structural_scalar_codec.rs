//! Canonical scalar vocabulary shared by format-36 structural codecs.

use psi_core::StructuralDomainId;
use psi_terminal::{StructuralAccess, StructuralMultiplicity};

use super::{InstallationError, Reader, push_u32, push_u64};

pub(super) fn encode_identity(
    bytes: &mut Vec<u8>,
    identity: &str,
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(identity.len())
            .map_err(|_| InstallationError::StructuralTypeIdentityTooLong)?,
    );
    bytes.extend_from_slice(identity.as_bytes());
    Ok(())
}

pub(super) fn decode_identity(reader: &mut Reader<'_>) -> Result<String, InstallationError> {
    let len = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::StructuralTypeIdentityTooLong)?;
    let identity = std::str::from_utf8(reader.take(len)?)
        .map_err(|_| InstallationError::InvalidStructuralTypeIdentity)?
        .to_owned();
    if identity.is_empty() {
        return Err(InstallationError::InvalidStructuralTypeIdentity);
    }
    Ok(identity)
}

pub(super) fn encode_domains(
    bytes: &mut Vec<u8>,
    domains: &[StructuralDomainId],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(domains.len())
            .map_err(|_| InstallationError::TooManyStructuralQualifications)?,
    );
    for domain in domains {
        push_u64(bytes, domain.get());
    }
    Ok(())
}

pub(super) fn decode_domains(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralDomainId>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyStructuralQualifications)?;
    let mut domains = Vec::with_capacity(count);
    for _ in 0..count {
        domains.push(
            StructuralDomainId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroStructuralReturnIdentity("domain"))?,
        );
    }
    Ok(domains)
}

pub(super) fn multiplicity_tag(value: StructuralMultiplicity) -> u8 {
    match value {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    }
}

pub(super) fn decode_multiplicity(value: u8) -> Result<StructuralMultiplicity, InstallationError> {
    match value {
        1 => Ok(StructuralMultiplicity::Unrestricted),
        2 => Ok(StructuralMultiplicity::Affine),
        3 => Ok(StructuralMultiplicity::Linear),
        _ => Err(InstallationError::InvalidStructuralMultiplicity(value)),
    }
}

pub(super) fn access_tag(value: StructuralAccess) -> u8 {
    match value {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    }
}

pub(super) fn decode_access(value: u8) -> Result<StructuralAccess, InstallationError> {
    match value {
        1 => Ok(StructuralAccess::Owned),
        2 => Ok(StructuralAccess::SharedBorrow),
        3 => Ok(StructuralAccess::MutableBorrow),
        4 => Ok(StructuralAccess::WriteOnlyBorrow),
        _ => Err(InstallationError::InvalidStructuralAccess(value)),
    }
}
