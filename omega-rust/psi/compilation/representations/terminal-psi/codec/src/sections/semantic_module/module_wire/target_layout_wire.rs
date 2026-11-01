//! Canonical unclosed layout applications and complete checked alternatives.
use super::super::structural_type_wire::{
    decode_authored_field_storage, encode_authored_field_storage,
};
use super::super::wire_fields::{
    Field, Flag, Id, Index, List, Optional, Text, U16, U64, wire_enum, wire_record,
};
use super::super::{
    CodecError,
    wire::{Reader, Writer},
};
use semantic_vocabulary::{AuthoredFieldStorage, AuthoredRecordStorage};
use terminal_psi::{
    LayoutApplicationId, TargetLayoutApplication, TargetLayoutFieldStorage, TargetLayoutInput,
    TargetLayoutRealization, TargetLayoutSchemaArgument, TargetLayoutSchemaField,
    TargetLayoutSchemaFieldKind,
};

wire_record! {
    Application: TargetLayoutApplication {
        id: ApplicationId,
        record: Id,
        schema: Id,
        policy_identity: Index,
        plan_identity: Index,
        target_inputs: List<Input>,
        realizations: List<Realization>,
    }
    Input: TargetLayoutInput {
        target: Text,
        pointer_size: U64,
        pointer_alignment: U64,
        little_endian: Flag,
    }
    Realization: TargetLayoutRealization {
        schema_argument: SchemaArgument,
        target: Text,
        pointer_size: U64,
        pointer_alignment: U64,
        little_endian: Flag,
        storage: RecordStorage,
        fields: List<FieldStorage>,
    }
    RecordStorage: AuthoredRecordStorage { byte_size: U64, byte_alignment: U64 }
    FieldStorage: TargetLayoutFieldStorage { field: Id, storage: RequiredStorage }
    SchemaArgument: TargetLayoutSchemaArgument {
        abi_version: U16,
        fields: List<SchemaField>,
        retired_field_identities: List<U64>,
    }
    SchemaField: TargetLayoutSchemaField {
        record_field: Id,
        schema_field: Id,
        key: U64,
        identity: Optional<U64>,
        byte_size: U64,
        byte_alignment: U64,
        kind: SchemaFieldKind,
    }
}

wire_enum! {
    SchemaFieldKind: TargetLayoutSchemaFieldKind, "TargetLayoutSchemaFieldKind" {
        0 => Scalar,
        1 => Nested,
        2 => Repeated,
    }
}

struct ApplicationId;
impl Field<LayoutApplicationId> for ApplicationId {
    fn write(writer: &mut Writer, value: &LayoutApplicationId) -> Result<(), CodecError> {
        writer.u32(value.0);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<LayoutApplicationId, CodecError> {
        Ok(LayoutApplicationId(reader.u32()?))
    }
}

/// A checked alternative always carries its field storage.
struct RequiredStorage;
impl Field<AuthoredFieldStorage> for RequiredStorage {
    fn write(writer: &mut Writer, value: &AuthoredFieldStorage) -> Result<(), CodecError> {
        encode_authored_field_storage(writer, Some(value))
    }
    fn read(reader: &mut Reader<'_>) -> Result<AuthoredFieldStorage, CodecError> {
        decode_authored_field_storage(reader)?.ok_or(CodecError::MalformedStructuralFoundation(
            "layout alternative has absent field storage",
        ))
    }
}
