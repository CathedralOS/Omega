use crate::name::ProgramName;
use crate::types::TypeReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: ProgramName,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<TypeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub name: ProgramName,
    pub type_reference: TypeReference,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}
