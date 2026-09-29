//! Closed result categories for boundary, descriptor and structural calls.

use crate::AbstractResult;
use terminal_psi::{
    OutcomeSpecificCallEvidence, StructuralOperationResult, StructuralResultClaimTransfer,
};

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

/// Closed result role of one descriptor-dispatched call.
///
/// The selected callable row fixes the role; descriptor custody does not.
/// Unit is an authored role rather than an absent scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractDynamicCallResult {
    Unit,
    Scalar(AbstractResult),
}

impl AbstractDynamicCallResult {
    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub const fn scalar(&self) -> Option<AbstractResult> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit => None,
        }
    }
}

/// Closed result role of one direct structural-lane machine call.
///
/// The role decides how the call's arguments are admitted: a Unit call admits
/// its structural arguments under the Unit projection policy, a scalar-result
/// call under the projected policy, and a structural-result call by the
/// callee's result shape. Consumers read the role from this field, never from
/// which rosters happen to be empty. Unit is an authored role rather than an
/// absent scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractStructuralCallResult {
    Unit,
    Scalar(AbstractResult),
    /// The result place and returned-claim correspondence remain semantic
    /// custody; target lowering may realize only a deliberately bounded ABI
    /// subset. `returned_claim_transfers` and `selected_evidence` exist only
    /// beside a structural result.
    Structural {
        result: StructuralOperationResult,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        selected_evidence: Vec<OutcomeSpecificCallEvidence>,
    },
}

impl AbstractStructuralCallResult {
    pub const fn scalar(&self) -> Option<AbstractResult> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit | Self::Structural { .. } => None,
        }
    }
}
