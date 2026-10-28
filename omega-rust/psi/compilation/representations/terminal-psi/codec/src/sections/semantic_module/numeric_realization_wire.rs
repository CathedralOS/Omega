//! Canonical external satisfaction declarations. This wire carries no selected
//! deployment target, executable address, or admission grant.

use super::CodecError;
use super::machine_declaration_wire;
use super::wire::{Reader, Writer};
use semantic_vocabulary::PackageKeyIdentity;
use terminal_psi::TerminalIdentityIndex;
use terminal_psi::{NominalDeclarationIdentity, NumericRealization, NumericRealizationOwner};

pub(super) fn encode(
    writer: &mut Writer,
    candidate: &NumericRealization,
) -> Result<(), CodecError> {
    writer.u32(candidate.requirement.identity().0);
    writer.u32(candidate.requirement_identity.0);
    encode_package(writer, candidate.requirement_package);
    writer.bytes(&candidate.numeric_intrinsic.canonical_encoding().ok_or(
        CodecError::MalformedStructuralFoundation("invalid numeric realization meaning"),
    )?);
    writer.boolean(candidate.target.is_some());
    if let Some(target) = &candidate.target {
        writer.string("external provider target membership", target)?;
    }
    machine_declaration_wire::encode(writer, &candidate.realization)?;
    encode_package(writer, candidate.realization_package);
    writer.boolean(candidate.provider.is_some());
    if let Some(provider) = &candidate.provider {
        writer.u32(provider.declaration.identity().0);
        writer.u32(provider.identity.0);
        encode_package(writer, provider.package);
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<NumericRealization, CodecError> {
    let requirement = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
    let requirement_identity = TerminalIdentityIndex(reader.u32()?);
    let requirement_package = decode_package(reader)?;
    let numeric_intrinsic =
        language_semantics::numeric_intrinsics::NumericIntrinsic::from_canonical_encoding(
            reader.array()?,
        )
        .ok_or(CodecError::MalformedStructuralFoundation(
            "invalid numeric realization meaning",
        ))?;
    let target = if reader.boolean()? {
        Some(reader.string("external provider target membership")?)
    } else {
        None
    };
    let realization = machine_declaration_wire::decode(reader)?;
    let realization_package = decode_package(reader)?;
    let provider = if reader.boolean()? {
        Some(NumericRealizationOwner {
            declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
            identity: TerminalIdentityIndex(reader.u32()?),
            package: decode_package(reader)?,
        })
    } else {
        None
    };
    Ok(NumericRealization {
        requirement,
        requirement_identity,
        requirement_package,
        numeric_intrinsic,
        target,
        realization,
        realization_package,
        provider,
    })
}

fn encode_package(writer: &mut Writer, package: Option<PackageKeyIdentity>) {
    writer.boolean(package.is_some());
    if let Some(package) = package {
        writer.bytes(&package.digest());
    }
}

fn decode_package(reader: &mut Reader<'_>) -> Result<Option<PackageKeyIdentity>, CodecError> {
    if !reader.boolean()? {
        return Ok(None);
    }
    PackageKeyIdentity::from_digest(reader.array()?)
        .map(Some)
        .ok_or(CodecError::MalformedStructuralFoundation(
            "empty external provider package identity",
        ))
}
