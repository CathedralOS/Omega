//! Semantic successor provenance and decoded branch evidence.

use abstract_operations::ValueBinding;
use optimization_unit::FuelSettlement;
use register_model::RegisterViewId;
use selected_instructions::{MachineEncodedEffects, SelectedBlockId};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentBranchEvidence {
    Conditional(FunctionFragmentConditionalBranchEvidence),
    Jump(FunctionFragmentJumpEvidence),
}

impl FunctionFragmentBranchEvidence {
    pub const fn as_conditional(&self) -> Option<&FunctionFragmentConditionalBranchEvidence> {
        match self {
            Self::Conditional(value) => Some(value),
            Self::Jump(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentJumpEvidence {
    pub source_block: SelectedBlockId,
    pub target_edge: EdgeId,
    pub target_block: SelectedBlockId,
    pub target_offset: u64,
    pub byte_displacement: i64,
    pub decoded_effects: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentControlProvenance {
    None,
    Jump {
        successor: FunctionFragmentSuccessorProvenance,
    },
    DirectInternalCall {
        callee: MachineId,
    },
    ConditionalBranch {
        predicate: FunctionFragmentConditionalBranchPredicate,
        when_taken: FunctionFragmentSuccessorProvenance,
        when_fallthrough: FunctionFragmentSuccessorProvenance,
    },
    Return {
        psi_return_edge: EdgeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentConditionalBranchPredicate {
    NonZeroV1,
    U64LessThanV1,
    I64LessThanV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentSuccessorProvenance {
    pub psi_edge: EdgeId,
    pub block: SelectedBlockId,
    pub source_target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentConditionalBranchEvidence {
    pub predicate: FunctionFragmentConditionalBranchPredicate,
    pub source_block: SelectedBlockId,
    pub when_taken_edge: EdgeId,
    pub when_taken_block: SelectedBlockId,
    pub when_taken_offset: u64,
    pub when_fallthrough_edge: EdgeId,
    pub when_fallthrough_block: SelectedBlockId,
    pub when_fallthrough_offset: u64,
    pub byte_displacement: i64,
    pub decoded_register_reads: Vec<RegisterViewId>,
    pub decoded_effects: MachineEncodedEffects,
}
