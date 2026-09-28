//! Branch destinations, target-relative displacement, and decoded effects.

use selected_instructions::{MachineEncodedEffects, SelectedBlockId};
use semantic_vocabulary::EdgeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedBranchEvidence {
    Conditional(ResolvedConditionalBranchEvidence),
    Jump(ResolvedJumpEvidence),
}

impl ResolvedBranchEvidence {
    pub const fn as_conditional(&self) -> Option<&ResolvedConditionalBranchEvidence> {
        match self {
            Self::Conditional(value) => Some(value),
            Self::Jump(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedJumpEvidence {
    pub source_block: SelectedBlockId,
    pub target_edge: EdgeId,
    pub target_block: SelectedBlockId,
    pub target_offset: u64,
    pub byte_displacement: i64,
    pub decoded_effects: MachineEncodedEffects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedConditionalBranchPredicate {
    NonZeroV1,
    U64LessThanV1,
    I64LessThanV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConditionalBranchEvidence {
    pub predicate: ResolvedConditionalBranchPredicate,
    pub source_block: SelectedBlockId,
    pub when_taken_edge: EdgeId,
    pub when_taken_block: SelectedBlockId,
    pub when_taken_offset: u64,
    pub when_fallthrough_edge: EdgeId,
    pub when_fallthrough_block: SelectedBlockId,
    pub when_fallthrough_offset: u64,
    /// x86-64 measures from instruction end; AArch64 measures from the branch
    /// word address. The target decoder independently checks this convention.
    pub byte_displacement: i64,
    pub decoded_register_reads: Vec<register_model::RegisterViewId>,
    pub decoded_effects: MachineEncodedEffects,
}
