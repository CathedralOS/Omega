//! Optimizer module role: stage group. Control-flow analyses, cataloged by the exact graph question they answer.
use semantic_vocabulary::{BlockId, MachineId};
mod analysis_root;
mod call_graph;
mod components;
mod countdown_induction;
mod countdown_invariant_constant_placement;
mod countdown_invariant_constants;
mod dominance;
mod graph;
mod loops;
pub(crate) use analysis_root::AnalysisRootError;
pub(in crate::analyses) use analysis_root::{
    AnalysisRoot, validate_analysis_root, validate_unit_identity,
};
pub(super) use call_graph::call_graph;
pub(super) use components::block_components;

pub(crate) use countdown_induction::analyze_counted_loops;
pub(crate) use countdown_induction::{
    CountedLoopAnalysisError, UnsignedCountdownLoopSummary, ValidatedCountedLoopAnalysis,
};

pub(crate) use countdown_invariant_constant_placement::analyze_countdown_invariant_constant_placement;
pub(crate) use countdown_invariant_constant_placement::{
    CountdownInvariantConstantPlacementAnalysisError,
    ValidatedCountdownInvariantConstantPlacementAnalysis,
};

pub(crate) use countdown_invariant_constants::analyze_countdown_invariant_constants;
pub(crate) use countdown_invariant_constants::validate_moved_constant_locations;
pub(crate) use countdown_invariant_constants::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, ValidatedCountdownInvariantConstantAnalysis,
    expected_constant, invariant_constant_row, sole_provenance_node,
};
pub(crate) use dominance::block_dominates;
pub(super) use dominance::dominators;
pub(super) use graph::control_flow;
pub(super) use loops::loops;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ExitKind {
    Normal,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlockControlFlow {
    pub block: BlockId,
    pub predecessors: Vec<BlockId>,
    pub successors: Vec<BlockId>,
    pub exits: Vec<ExitKind>,
    pub reachable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionControlFlow {
    pub machine: MachineId,
    pub entry: BlockId,
    pub blocks: Vec<BlockControlFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ControlFlowAnalysis {
    pub functions: Vec<FunctionControlFlow>,
}

pub(crate) type BlockDominators = (BlockId, Vec<BlockId>);
pub(crate) type FunctionDominators = (MachineId, Vec<BlockDominators>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DominatorAnalysis {
    pub functions: Vec<FunctionDominators>,
}

impl DominatorAnalysis {
    pub fn function(&self, machine: MachineId) -> Option<&FunctionDominators> {
        self.functions
            .iter()
            .find(|(candidate, _)| *candidate == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StronglyConnectedComponentAnalysis {
    pub functions: Vec<(MachineId, Vec<Vec<BlockId>>)>,
}

impl StronglyConnectedComponentAnalysis {
    pub fn function(&self, machine: MachineId) -> Option<&(MachineId, Vec<Vec<BlockId>>)> {
        self.functions
            .iter()
            .find(|(candidate, _)| *candidate == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoopRegion {
    /// Natural-loop header when one node dominates every entry. `None` marks
    /// an irreducible region with multiple entries.
    pub header: Option<BlockId>,
    pub blocks: Vec<BlockId>,
    pub irreducible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoopAnalysis {
    pub functions: Vec<(MachineId, Vec<LoopRegion>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallGraphAnalysis {
    /// Static calls and semantic machine transitions that must keep a target
    /// reachable, including nominal cleanup invoked on return.
    pub callees: Vec<(MachineId, Vec<MachineId>)>,
    pub components: Vec<Vec<MachineId>>,
    pub recursive_components: Vec<Vec<MachineId>>,
}
