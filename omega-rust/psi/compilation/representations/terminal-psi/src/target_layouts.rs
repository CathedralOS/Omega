//! Exact target-dependent layout applications retained by the checked producer.
//!
//! Geometry is semantic input, as for invariant authored records. These rows
//! retain the unclosed application and every checked member's result; selecting
//! one row establishes neither policy evaluation nor permission to access bytes.
use crate::TerminalIdentityIndex;
use semantic_vocabulary::{
    AuthoredFieldStorage, AuthoredRecordStorage, StructuralFieldId, StructuralTypeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayoutApplicationId(pub u32);

/// Absence of authored storage means ordinary packing only in the Packed arm.
/// A pending application must never fall through to that interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralRecordStorage {
    Packed,
    /// A placed interpretation has no ordinary owned record storage.
    /// Its catalog describes the backing geometry; establishment lends it.
    PlacedView(AuthoredRecordStorage),
    /// One projected transfer container, still incapable of ordinary ownership.
    PlacedAccessor(AuthoredRecordStorage),
    Authored(AuthoredRecordStorage),
    TargetLayout(LayoutApplicationId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutApplication {
    pub id: LayoutApplicationId,
    pub record: StructuralTypeId,
    pub schema: StructuralTypeId,
    pub policy_identity: TerminalIdentityIndex,
    /// Exact checked plan callable contract, independent from a runtime family.
    pub plan_identity: TerminalIdentityIndex,
    /// Independent complete issuer input roster; alternatives must cover it exactly.
    pub target_inputs: Vec<TargetLayoutInput>,
    pub realizations: Vec<TargetLayoutRealization>,
}

/// Version 3 retains canonical numbered record Schema order independently
/// of semantic declaration order: active fields,
/// retired field identities and fixed unused-row padding. Case-bearing
/// schemas require a separate placement vocabulary and cannot enter this ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutSchemaArgument {
    pub abi_version: u16,
    pub fields: Vec<TargetLayoutSchemaField>,
    pub retired_field_identities: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLayoutSchemaFieldKind {
    Scalar,
    Nested,
    Repeated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutSchemaField {
    pub record_field: StructuralFieldId,
    pub schema_field: StructuralFieldId,
    pub key: u64,
    pub identity: Option<u64>,
    pub byte_size: u64,
    pub byte_alignment: u64,
    pub kind: TargetLayoutSchemaFieldKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutInput {
    pub target: String,
    pub pointer_size: u64,
    pub pointer_alignment: u64,
    pub little_endian: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutRealization {
    pub schema_argument: TargetLayoutSchemaArgument,
    pub target: String,
    pub pointer_size: u64,
    pub pointer_alignment: u64,
    pub little_endian: bool,
    pub storage: AuthoredRecordStorage,
    pub fields: Vec<TargetLayoutFieldStorage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutFieldStorage {
    pub field: StructuralFieldId,
    pub storage: AuthoredFieldStorage,
}
