use crate::TerminalIdentityIndex;
use semantic_vocabulary::{
    AuthoredFieldStorage, AuthoredRecordStorage, StructuralFieldId, StructuralTypeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayoutApplicationId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralRecordStorage {
    Packed,
    AccessView(AuthoredRecordStorage),
    Accessor(AuthoredRecordStorage),
    Authored(AuthoredRecordStorage),
    TargetLayout(LayoutApplicationId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutApplication {
    pub id: LayoutApplicationId,
    pub record: StructuralTypeId,
    pub schema: StructuralTypeId,
    pub policy_identity: TerminalIdentityIndex,
    pub plan_identity: TerminalIdentityIndex,
    pub target_inputs: Vec<TargetLayoutInput>,
    pub realizations: Vec<TargetLayoutRealization>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLayoutFieldLayoutSource {
    Default,
    Scalar,
    Explicit,
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
    pub layout_source: TargetLayoutFieldLayoutSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutInput {
    pub target: String,
    pub pointer_size: u64,
    pub pointer_alignment: u64,
    pub case_tag_size: u64,
    pub case_tag_alignment: u64,
    pub little_endian: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutRealization {
    pub schema_argument: TargetLayoutSchemaArgument,
    pub target: String,
    pub pointer_size: u64,
    pub pointer_alignment: u64,
    pub case_tag_size: u64,
    pub case_tag_alignment: u64,
    pub little_endian: bool,
    pub storage: AuthoredRecordStorage,
    pub fields: Vec<TargetLayoutFieldStorage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayoutFieldStorage {
    pub field: StructuralFieldId,
    pub storage: AuthoredFieldStorage,
}
