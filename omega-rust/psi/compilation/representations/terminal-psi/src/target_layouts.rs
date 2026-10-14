//! Exact target-dependent layout applications retained by the checked producer.
//!
//! Geometry is semantic input, as for invariant authored records. These rows
//! retain the unclosed application and every checked member's result; selecting
//! one row establishes neither policy evaluation nor permission to access bytes.
use crate::TargetMachineCallable;
use crate::TerminalIdentityIndex;
use semantic_vocabulary::{
    AuthoredFieldStorage, AuthoredRecordStorage, MachineId, StructuralFieldId, StructuralTypeId,
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
    pub family: TargetMachineCallable,
    pub schema_argument: TargetLayoutSchemaArgument,
    pub realizations: Vec<TargetLayoutRealization>,
}

/// Version 1 is the compiler's record Schema ABI: active fields followed by
/// padding to 32, no cases or retired identities. Counts and every unused tail
/// reconstruct from the version. Producers reject other schema shapes rather
/// than discard them. Field keys are exact invocation inputs, not identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutSchemaArgument {
    pub abi_version: u16,
    pub fields: Vec<TargetLayoutSchemaField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLayoutSchemaFieldKind {
    Scalar,
    Nested,
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
pub struct TargetLayoutRealization {
    pub target: String,
    pub member: MachineId,
    pub storage: AuthoredRecordStorage,
    pub fields: Vec<TargetLayoutFieldStorage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutFieldStorage {
    pub field: StructuralFieldId,
    pub storage: AuthoredFieldStorage,
}
