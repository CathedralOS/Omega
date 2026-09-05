//! Semantic successor provenance and decoded branch evidence.

use omega_abstract_operations::ValueBinding;
use omega_optimization_unit::FuelSettlement;
use omega_register_model::RegisterViewId;
use omega_selected_instructions::{MachineEncodedEffects, SelectedBlockId};
use psi_core::{BlockId, EdgeId, MachineId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentControlProvenance {
    None,
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
