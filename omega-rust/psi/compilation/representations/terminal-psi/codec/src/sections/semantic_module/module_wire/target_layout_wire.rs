//! Canonical unclosed layout applications and complete checked alternatives.
use super::super::structural_type_wire::{
    decode_authored_field_storage, encode_authored_field_storage,
};
use super::super::{
    CodecError,
    wire::{Reader, Writer, decode_counted},
};
use terminal_psi::*;

pub(super) fn encode(
    writer: &mut Writer,
    application: &TargetLayoutApplication,
) -> Result<(), CodecError> {
    writer.u32(application.id.0);
    writer.id(application.record);
    writer.id(application.schema);
    writer.u32(application.policy_identity.0);
    writer.u32(application.plan_identity.0);
    writer.u16(application.schema_argument.abi_version);
    writer.len(
        "layout schema fields",
        application.schema_argument.fields.len(),
    )?;
    for field in &application.schema_argument.fields {
        writer.id(field.record_field);
        writer.id(field.schema_field);
        writer.u64(field.key);
        writer.boolean(field.identity.is_some());
        if let Some(identity) = field.identity {
            writer.u64(identity);
        }
        writer.u64(field.byte_size);
        writer.u64(field.byte_alignment);
        writer.u8(match field.kind {
            TargetLayoutSchemaFieldKind::Scalar => 0,
            TargetLayoutSchemaFieldKind::Nested => 1,
        });
    }
    writer.len("layout realizations", application.realizations.len())?;
    for realization in &application.realizations {
        writer.string("layout target", &realization.target)?;
        writer.u64(realization.pointer_size);
        writer.u64(realization.pointer_alignment);
        writer.boolean(realization.little_endian);
        writer.u64(realization.storage.byte_size);
        writer.u64(realization.storage.byte_alignment);
        writer.len("layout field storage", realization.fields.len())?;
        for field in &realization.fields {
            writer.id(field.field);
            encode_authored_field_storage(writer, Some(&field.storage))?;
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<TargetLayoutApplication, CodecError> {
    Ok(TargetLayoutApplication {
        id: LayoutApplicationId(reader.u32()?),
        record: reader.id("StructuralTypeId")?,
        schema: reader.id("StructuralTypeId")?,
        policy_identity: TerminalIdentityIndex(reader.u32()?),
        plan_identity: TerminalIdentityIndex(reader.u32()?),
        schema_argument: TargetLayoutSchemaArgument {
            abi_version: reader.u16()?,
            fields: decode_counted(reader, |reader| {
                Ok(TargetLayoutSchemaField {
                    record_field: reader.id("StructuralFieldId")?,
                    schema_field: reader.id("StructuralFieldId")?,
                    key: reader.u64()?,
                    identity: if reader.boolean()? {
                        Some(reader.u64()?)
                    } else {
                        None
                    },
                    byte_size: reader.u64()?,
                    byte_alignment: reader.u64()?,
                    kind: match reader.u8()? {
                        0 => TargetLayoutSchemaFieldKind::Scalar,
                        1 => TargetLayoutSchemaFieldKind::Nested,
                        tag => {
                            return Err(CodecError::InvalidTag("TargetLayoutSchemaFieldKind", tag));
                        }
                    },
                })
            })?,
        },
        realizations: decode_counted(reader, |reader| {
            Ok(TargetLayoutRealization {
                target: reader.string("layout target")?,
                pointer_size: reader.u64()?,
                pointer_alignment: reader.u64()?,
                little_endian: reader.boolean()?,
                storage: semantic_vocabulary::AuthoredRecordStorage {
                    byte_size: reader.u64()?,
                    byte_alignment: reader.u64()?,
                },
                fields: decode_counted(reader, |reader| {
                    Ok(TargetLayoutFieldStorage {
                        field: reader.id("StructuralFieldId")?,
                        storage: decode_authored_field_storage(reader)?.ok_or(
                            CodecError::MalformedStructuralFoundation(
                                "layout alternative has absent field storage",
                            ),
                        )?,
                    })
                })?,
            })
        })?,
    })
}
