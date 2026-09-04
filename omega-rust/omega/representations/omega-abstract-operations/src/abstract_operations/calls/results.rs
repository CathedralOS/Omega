//! Closed result categories for boundary invocations.

use crate::AbstractResult;
use psi_terminal::StructuralOperationResult;

/// Closed target-neutral result role of one bodyless boundary invocation.
///
/// Unit is an authored result role rather than an absent scalar. Structural
/// results retain the complete caller-local place and qualification frontier
/// published by Terminal Psi; target lowering must either assign that exact
/// value a physical home or reject the realization explicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractBoundaryResult {
    Unit,
    Scalar(AbstractResult),
    Structural(StructuralOperationResult),
}

impl AbstractBoundaryResult {
    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub const fn scalar(&self) -> Option<AbstractResult> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&StructuralOperationResult> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}
