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
    pub type_reference: crate::ast::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub contains: Vec<Contains>,
    pub commands: Vec<CommandDefinition>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contains {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub statements: Vec<crate::ast::statement::Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDefinition {
    pub signature: CommandSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: String,
    pub commands: Vec<CommandSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSignature {
    pub name: String,
    pub parameters: Vec<CommandParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandParameter {
    pub name: String,
    pub type_reference: crate::ast::types::TypeReference,
    pub is_mutable: bool,
}
