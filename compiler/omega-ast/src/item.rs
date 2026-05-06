#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Data(DataDefinition),
    Use(UseItem),
    Machine(Machine),
    Platform(Platform),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseItem {
    pub path: Vec<String>,
}

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
    pub type_reference: crate::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub contains: Vec<Contains>,
    pub owned_data: Vec<OwnedData>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contains {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub name: String,
    pub type_reference: crate::types::TypeReference,
    pub initial_value: Option<crate::expression::Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
    pub statements: Vec<crate::statement::Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: String,
    pub states: Vec<StateSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: String,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub name: String,
    pub type_reference: crate::types::TypeReference,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}
