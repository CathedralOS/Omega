//! Case edges define destination parameters after observing the selected payload.
use optimization_unit::FuelSettlement;
use semantic_vocabulary::{BlockId, EdgeId, PlaceId, StructuralCaseId, StructuralFieldId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralCaseSuccessor {
    pub edge: EdgeId,
    pub target: BlockId,
    pub case: StructuralCaseId,
    pub case_tag: i32,
    pub payloads: Vec<LegalizedStructuralCasePayload>,
    pub trivial_affine_discards: Vec<PlaceId>,
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
