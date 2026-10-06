//! Case edges define destination parameters after observing the selected payload.
use optimization_unit::FuelSettlement;
use semantic_vocabulary::{BlockId, EdgeId, StructuralCaseId, StructuralFieldId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralCaseSuccessor {
    pub edge: EdgeId,
    pub target: BlockId,
    pub case: StructuralCaseId,
    pub case_tag: i32,
    pub payloads: Vec<LegalizedStructuralCasePayload>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
    pub cleanup_calls: Vec<(target_operations::CallSiteOwner, super::LegalizedScalarCall)>,
    pub fuel: Vec<FuelSettlement>,
}

/// The definition belongs to the target block; the boundary producer defines
/// only the structural result, not this scalar arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizedStructuralCasePayload {
    pub field: StructuralFieldId,
    pub field_byte_offset: u32,
    pub parameter: crate::LegalizedValueDefinition,
}
