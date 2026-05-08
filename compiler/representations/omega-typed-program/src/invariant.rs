use crate::name::ProgramName;
use omega_core::arena::HandleSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub name: ProgramName,
    pub constraints: HandleSpan<crate::types::TypeConstraint>,
}
