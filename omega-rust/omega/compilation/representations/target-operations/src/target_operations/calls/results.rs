//! Result custody is independent of a direct call's argument roster.
//!
//! Unit has no result. Scalars retain their exact home even when unused.
//! Structural results additionally retain the declared ownership and reference
//! transfers; a bare reference can return custody without allocating a home.
//! Receiving stages reconstruct these facts from the callee and pre-call state.

use crate::{
    TargetReferenceResult, TargetStructuralHomeRequirement, TargetUnitScalarHomeRequirement,
};
use terminal_psi::{
    StructuralOperationResult, StructuralResultClaimTransfer, StructuralResultDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetCallResult {
    Unit,
    Scalar(TargetUnitScalarHomeRequirement),
    Structural {
        result: StructuralOperationResult,
        callee_result: StructuralResultDeclaration,
        result_home: Option<TargetStructuralHomeRequirement>,
        reference_results: Vec<TargetReferenceResult>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
    },
}

impl TargetCallResult {
    pub const fn scalar_home(&self) -> Option<&TargetUnitScalarHomeRequirement> {
        match self {
            Self::Scalar(home) => Some(home),
            Self::Unit | Self::Structural { .. } => None,
        }
    }
}
