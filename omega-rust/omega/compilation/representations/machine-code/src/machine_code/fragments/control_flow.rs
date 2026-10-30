//! Semantic successor provenance and decoded branch evidence.

use abstract_operations::ValueBinding;
use optimization_unit::FuelSettlement;
use selected_instructions::SelectedBlockId;
use semantic_vocabulary::{BlockId, BoundaryMachineId, EdgeId};

use crate::machine_code::layout::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
};

/// The fragment retains the resolved branch evidence verbatim.
pub type FunctionFragmentBranchEvidence = ResolvedBranchEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentControlProvenance {
    /// Operation-owned invalid-assertion termination, never an authored edge.
    OperationTrap {
        operation: semantic_vocabulary::OperationId,
    },
    /// The selected source identity retains the checked guard and frontier;
    /// this row joins their exact crash site to its terminating bytes.
    Crash {
        psi_edge: EdgeId,
        cause: terminal_psi::CrashCause,
    },
    HostedExitProcess {
        completion_edge: EdgeId,
    },
    None,
    Jump {
        successor: FunctionFragmentSuccessorProvenance,
    },
    InternalCall {
        target: selected_instructions::SelectedCallTarget,
        owner: target_operations::CallSiteOwner,
    },
    /// Direct evaluated normalized foreign call. The `{boundary, ordinal}`
    /// pair names the selected roster row owning the evaluated locator; the
    /// unresolved import field rides the foreign fixup channel and binds only
    /// at object construction.
    NormalizedForeignCall {
        invocation_site: semantic_vocabulary::InvocationSite,
        boundary: BoundaryMachineId,
        ordinal: u32,
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

pub type FunctionFragmentConditionalBranchPredicate = ResolvedConditionalBranchPredicate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentSuccessorProvenance {
    pub role: selected_instructions::SelectedSuccessorRole,
    pub psi_edge: Option<EdgeId>,
    pub block: SelectedBlockId,
    pub source_target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub fuel: Vec<FuelSettlement>,
}

pub type FunctionFragmentConditionalBranchEvidence = ResolvedConditionalBranchEvidence;
