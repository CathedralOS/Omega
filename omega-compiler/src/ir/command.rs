use crate::ir::types::TypeReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSignature {
    pub name: String,
    pub parameters: Vec<CommandParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandParameter {
    pub name: String,
    pub type_reference: TypeReference,
    pub is_mutable: bool,
}
