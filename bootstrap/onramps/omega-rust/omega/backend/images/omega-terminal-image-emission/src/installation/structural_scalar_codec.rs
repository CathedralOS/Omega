//! Canonical scalar vocabulary shared by format-34 structural codecs.

use psi_core::StructuralDomainId;
use psi_terminal::StructuralMultiplicity;

use super::{Reader, TerminalInstallationError, push_u32, push_u64};

pub(super) fn encode_identity(
    bytes: &mut Vec<u8>,
    identity: &str,
) -> Result<(), TerminalInstallationError> {
    push_u32(
        bytes,
        u32::try_from(identity.len())
            .map_err(|_| TerminalInstallationError::StructuralTypeIdentityTooLong)?,
    );
    bytes.extend_from_slice(identity.as_bytes());
    Ok(())
}

pub(super) fn decode_identity(
    reader: &mut Reader<'_>,
) -> Result<String, TerminalInstallationError> {
    let len = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::StructuralTypeIdentityTooLong)?;
    let identity = std::str::from_utf8(reader.take(len)?)
        .map_err(|_| TerminalInstallationError::InvalidStructuralTypeIdentity)?
        .to_owned();
    if identity.is_empty() {
        return Err(TerminalInstallationError::InvalidStructuralTypeIdentity);
    }
    Ok(identity)
}

pub(super) fn encode_domains(
    bytes: &mut Vec<u8>,
    domains: &[StructuralDomainId],
) -> Result<(), TerminalInstallationError> {
    push_u32(
        bytes,
        u32::try_from(domains.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralQualifications)?,
    );
    for domain in domains {
        push_u64(bytes, domain.get());
    }
    Ok(())
}

pub(super) fn decode_domains(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralDomainId>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralQualifications)?;
    let mut domains = Vec::with_capacity(count);
    for _ in 0..count {
        domains.push(StructuralDomainId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("domain"),
        )?);
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

pub(super) fn decode_multiplicity(
    value: u8,
) -> Result<StructuralMultiplicity, TerminalInstallationError> {
    match value {
        1 => Ok(StructuralMultiplicity::Unrestricted),
        2 => Ok(StructuralMultiplicity::Affine),
        3 => Ok(StructuralMultiplicity::Linear),
        _ => Err(TerminalInstallationError::InvalidStructuralMultiplicity(
            value,
        )),
    }
}
