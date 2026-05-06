use omega_core::arena::HandleSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub name: String,
    pub constraints: HandleSpan<crate::ir::types::TypeConstraint>,
}
