use crate::ir::types::TypeConstraint;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub name: String,
    pub constraints: Vec<TypeConstraint>,
}
