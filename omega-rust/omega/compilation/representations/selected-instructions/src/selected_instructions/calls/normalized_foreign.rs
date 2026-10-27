//! Evaluated normalized foreign-call contract attached to a selected
//! instruction. The roster row retains the complete producer custody — typed
//! locator, evaluated boundary-entry plan, provider execution, argument and
//! result homes — that the instruction's `{boundary, ordinal}` kind indexes.
//! Returning calls are operation-owned. External completions remain edge-owned
//! through the same ABI transport; they have no result or floating-control
//! restoration and end in containment if the admitted provider returns.
use crate::SelectedInstructionId;
use optimization_unit::{EffectLink, OwnershipEvent};
use semantic_vocabulary::InvocationSite;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedNormalizedForeignCall {
    pub instruction: SelectedInstructionId,
    /// A terminal completion retains its edge; it is not a returning Unit operation.
    pub invocation_site: InvocationSite,
    pub call: legalized_operations::LegalizedNormalizedForeignCall,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

pub mod return_transport;
