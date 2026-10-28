//! Canonical external satisfaction declarations. This wire carries no selected
//! target, executable address, or admission grant.

use super::super::machine_declaration_wire;
use super::{CodecError, Reader, TerminalIdentityIndex, Writer};
use language_semantics::ExternalBindingIdentity;
use semantic_vocabulary::PackageKeyIdentity;
use terminal_psi::{
    ExternalProviderConformance, ExternalProviderOwner, NominalDeclarationIdentity,
};

pub(super) fn encode(
    writer: &mut Writer,
    candidate: &ExternalProviderConformance,
) -> Result<(), CodecError> {
    machine_declaration_wire::encode(writer, &candidate.realization)?;
    encode_package(writer, candidate.realization_package);
    writer.boolean(candidate.provider.is_some());
    if let Some(provider) = &candidate.provider {
        writer.u32(provider.declaration.identity().0);
        writer.u32(provider.identity.0);
        encode_package(writer, provider.package);
    }
    writer.u8(candidate.binding.mechanism().identity_tag());
    match &candidate.binding {
        ExternalBindingIdentity::CompilerIntrinsic => {}
        ExternalBindingIdentity::Syscall { number } => writer.bytes(&number.to_le_bytes()),
        ExternalBindingIdentity::VtableSlot { index } => writer.bytes(&index.to_le_bytes()),
        ExternalBindingIdentity::VtableField { field }
        | ExternalBindingIdentity::TableFunction { field } => {
            writer.string("external binding field", field)?
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<ExternalProviderConformance, CodecError> {
    let realization = machine_declaration_wire::decode(reader)?;
    let realization_package = decode_package(reader)?;
    let provider = if reader.boolean()? {
        Some(ExternalProviderOwner {
            declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
            identity: TerminalIdentityIndex(reader.u32()?),
            package: decode_package(reader)?,
        })
    } else {
        None
    };
    let binding = match reader.u8()? {
        2 => ExternalBindingIdentity::Syscall {
            number: i64::from_le_bytes(reader.array()?),
        },
        3 => ExternalBindingIdentity::CompilerIntrinsic,
        4 => ExternalBindingIdentity::VtableSlot {
            index: i64::from_le_bytes(reader.array()?),
        },
        5 => ExternalBindingIdentity::VtableField {
            field: reader.string("external binding field")?,
        },
        6 => ExternalBindingIdentity::TableFunction {
            field: reader.string("external binding field")?,
        },
        tag => return Err(CodecError::InvalidTag("external provider binding", tag)),
    };
    Ok(ExternalProviderConformance {
        realization,
        realization_package,
        provider,
        binding,
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
