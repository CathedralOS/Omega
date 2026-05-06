use crate::ir::types::TypeReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: String,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<TypeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub name: String,
    pub type_reference: TypeReference,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}
