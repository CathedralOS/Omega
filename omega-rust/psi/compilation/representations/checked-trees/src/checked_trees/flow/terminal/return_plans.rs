//! The one result-bearing return plan a checked machine carries.
//!
//! The return rosters remain separate tables because each retains the
//! structural-type roster its lowering emits and outside readers iterate them
//! by field. This view names the family once, so a consumer asks which return
//! plan a machine lowers instead of walking every roster and repeating the
//! same admission beside each. A zero-input payloadless case constructor is
//! not a return family: it is an ordinary Unit-effect body.

use symbols::SymbolHandle;

use crate::checked_trees::flow::FlowFacts;
use crate::checked_trees::flow::terminal::{
    CheckedBoundaryScalarReturnMachinePlan, CheckedClaimFreeAffineStructuralReturnMachinePlan,
    CheckedStructuralScalarReturnMachinePlan, CheckedTerminalSignatureEligibility,
};

/// The result-bearing plan one machine lowers. Each variant borrows the row
/// of the roster that admitted it. The rosters are filled by independent
/// recognizers over the same machines, so a machine admitted by more than one
/// resolves in this declaration order, which is the producer-family order
/// terminal lowering has always dispatched in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedReturnPlan<'a> {
    /// One scalar result computed over a structural frontier, followed by its
    /// post-result cleanup.
    StructuralScalar(&'a CheckedStructuralScalarReturnMachinePlan),
    /// One scalar result returned by a bodyless boundary call that consumes
    /// the structural claim frontier.
    BoundaryScalar(&'a CheckedBoundaryScalarReturnMachinePlan),
    /// One whole claim-free owned-affine parameter returned unchanged.
    ClaimFreeAffine(&'a CheckedClaimFreeAffineStructuralReturnMachinePlan),
}

impl<'a> CheckedReturnPlan<'a> {
    /// The return plan `machine` lowers, if any roster admitted it.
    pub fn for_machine(flow: &'a FlowFacts, machine: SymbolHandle) -> Option<Self> {
        flow.terminal_structural_scalar_returns
            .for_machine(machine)
            .map(Self::StructuralScalar)
            .or_else(|| {
                flow.terminal_boundary_scalar_returns
                    .for_machine(machine)
                    .map(Self::BoundaryScalar)
            })
            .or_else(|| {
                flow.terminal_structural_returns
                    .claim_free_affine_for_machine(machine)
                    .map(Self::ClaimFreeAffine)
            })
    }

    /// The selected machine this plan returns from.
    pub fn machine(self) -> SymbolHandle {
        match self {
            Self::StructuralScalar(plan) => plan.machine,
            Self::BoundaryScalar(plan) => plan.machine,
            Self::ClaimFreeAffine(plan) => plan.machine,
        }
    }

    /// Whether `signature` is the selection this plan lowers under: an
    /// attached signature when the plan retains its owner, either free
    /// signature otherwise, exactly as a Unit plan admits it. A free machine
    /// that reaches services is selected `FreeUnitEffect`.
    pub fn admits_signature(self, signature: CheckedTerminalSignatureEligibility) -> bool {
        let attached = match self {
            Self::StructuralScalar(plan) => plan.attachment_type_identity.is_some(),
            Self::BoundaryScalar(plan) => plan.attachment_type_identity.is_some(),
            Self::ClaimFreeAffine(plan) => plan.attachment_type_identity.is_some(),
        };
        !attached
            && matches!(
                signature,
                CheckedTerminalSignatureEligibility::Eligible
                    | CheckedTerminalSignatureEligibility::FreeUnitEffect
            )
    }
}
