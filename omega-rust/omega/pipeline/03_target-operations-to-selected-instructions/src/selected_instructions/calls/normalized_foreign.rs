//! Evaluated normalized foreign-call contract attached to a selected
//! instruction. The roster row retains the complete producer custody — typed
//! locator, evaluated boundary-entry plan, provider execution, argument and
//! result homes — that the instruction's `{boundary, ordinal}` kind indexes.
use crate::selected_instructions::SelectedInstructionId;
use semantic_vocabulary::OperationId;
use terminal_psi_to_abstract_operations::optimization_unit::{EffectLink, OwnershipEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedNormalizedForeignCall {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub call: crate::legalized_operations::LegalizedNormalizedForeignCall,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}
