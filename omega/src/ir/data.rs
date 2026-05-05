use crate::ir::types::TypeReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDefinition {
    pub name: String,
    pub members: Vec<DataMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub name: String,
    pub type_reference: TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub name: String,
}
