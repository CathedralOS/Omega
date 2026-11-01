//! Canonical external satisfaction declarations. This wire carries no selected
//! deployment target, executable address, or admission grant.

use super::CodecError;
use super::machine_declaration_wire;
use super::wire::{Reader, Writer};
use super::wire_fields::{ExternalPackage, Field};
use language_semantics::ExternalBindingIdentity;
use terminal_psi::TerminalIdentityIndex;
use terminal_psi::{
    ExternalProviderConformance, ExternalProviderOwner, NominalDeclarationIdentity,
};

pub(super) fn encode(
    writer: &mut Writer,
    candidate: &ExternalProviderConformance,
) -> Result<(), CodecError> {
    writer.u32(candidate.requirement.identity().0);
    writer.u32(candidate.requirement_identity.0);
    ExternalPackage::write(writer, &candidate.requirement_package)?;
    writer.boolean(candidate.requirement_schema.is_some());
    if let Some(schema) = &candidate.requirement_schema {
        writer.u32(schema.identity.0);
        writer.u32(schema.method.0);
        writer.u32(schema.owner.0);
    }
    writer.boolean(candidate.numeric_intrinsic.is_some());
    if let Some(meaning) = candidate.numeric_intrinsic {
        writer.bytes(&meaning.canonical_encoding().ok_or(
            CodecError::MalformedStructuralFoundation("invalid external provider numeric meaning"),
        )?);
    }
    writer.boolean(candidate.target.is_some());
    if let Some(target) = &candidate.target {
        writer.string("external provider target membership", target)?;
    }
    machine_declaration_wire::encode(writer, &candidate.realization)?;
    ExternalPackage::write(writer, &candidate.realization_package)?;
    writer.boolean(candidate.provider.is_some());
    if let Some(provider) = &candidate.provider {
        writer.u32(provider.declaration.identity().0);
        writer.u32(provider.identity.0);
        ExternalPackage::write(writer, &provider.package)?;
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
    let requirement = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
    let requirement_identity = TerminalIdentityIndex(reader.u32()?);
    let requirement_package = ExternalPackage::read(reader)?;
    let requirement_schema = if reader.boolean()? {
        Some(terminal_psi::ExternalRequirementSchema {
            identity: TerminalIdentityIndex(reader.u32()?),
            method: TerminalIdentityIndex(reader.u32()?),
            owner: TerminalIdentityIndex(reader.u32()?),
        })
    } else {
        None
    };
    let numeric_intrinsic = if reader.boolean()? {
        Some(
            language_semantics::numeric_intrinsics::NumericIntrinsic::from_canonical_encoding(
                reader.array()?,
            )
            .ok_or(CodecError::MalformedStructuralFoundation(
                "invalid external provider numeric meaning",
            ))?,
        )
    } else {
        None
    };
    let target = if reader.boolean()? {
        Some(reader.string("external provider target membership")?)
    } else {
        None
    };
    let realization = machine_declaration_wire::decode(reader)?;
    let realization_package = ExternalPackage::read(reader)?;
    let provider = if reader.boolean()? {
        Some(ExternalProviderOwner {
            declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
            identity: TerminalIdentityIndex(reader.u32()?),
            package: ExternalPackage::read(reader)?,
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
        requirement,
        requirement_identity,
        requirement_package,
        requirement_schema,
        numeric_intrinsic,
        target,
        realization,
        realization_package,
        provider,
        binding,
    })
}
