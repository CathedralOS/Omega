use crate::ir::types::TypeReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: String,
    pub parameters: Vec<StateParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub name: String,
    pub type_reference: TypeReference,
    pub is_mutable: bool,
}
