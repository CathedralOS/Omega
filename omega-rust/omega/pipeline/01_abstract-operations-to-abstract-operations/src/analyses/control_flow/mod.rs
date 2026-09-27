//! Optimizer module role: stage group. Control-flow analyses, cataloged by the exact graph question they answer.
use semantic_vocabulary::{BlockId, MachineId};
mod call_graph;
mod components;
mod countdown_induction;
mod countdown_invariant_constant_placement;
mod countdown_invariant_constants;
mod dominance;
mod graph;
mod loops;
pub(super) use call_graph::call_graph;
pub(super) use components::block_components;

#[cfg(any(test, feature = "test-support"))]
pub use countdown_induction::CountedLoopAnalysisSnapshot;
pub(crate) use countdown_induction::analyze_counted_loops;
#[cfg(any(test, feature = "test-support"))]
pub(crate) use countdown_induction::validate_counted_loop_analysis;
pub use countdown_induction::{
    CountedLoopAnalysisError, UnsignedCountdownLoopSummary, ValidatedCountedLoopAnalysis,
};

pub(crate) use countdown_invariant_constant_placement::analyze_countdown_invariant_constant_placement;
#[cfg(any(test, feature = "test-support"))]
pub(crate) use countdown_invariant_constant_placement::validate_countdown_invariant_constant_placement_analysis;
pub use countdown_invariant_constant_placement::{
    CountdownInvariantConstantPlacementAnalysisError,
    ValidatedCountdownInvariantConstantPlacementAnalysis,
};
#[cfg(any(test, feature = "test-support"))]
pub use countdown_invariant_constant_placement::{
    CountdownInvariantConstantPlacementAnalysisSnapshot,
    UnsignedCountdownInvariantConstantPlacements,
};

#[cfg(any(test, feature = "test-support"))]
pub use countdown_invariant_constants::CountdownInvariantConstantAnalysisSnapshot;
pub(crate) use countdown_invariant_constants::analyze_countdown_invariant_constants;
#[cfg(any(test, feature = "test-support"))]
pub(crate) use countdown_invariant_constants::validate_countdown_invariant_constant_analysis;
pub use countdown_invariant_constants::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, ValidatedCountdownInvariantConstantAnalysis,
};
pub(super) use dominance::dominators;
pub(super) use graph::control_flow;
pub(super) use loops::loops;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExitKind {
    Normal,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockControlFlow {
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
pub struct ControlFlowAnalysis {
    pub functions: Vec<FunctionControlFlow>,
}

pub type BlockDominators = (BlockId, Vec<BlockId>);
pub type FunctionDominators = (MachineId, Vec<BlockDominators>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DominatorAnalysis {
    pub functions: Vec<FunctionDominators>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StronglyConnectedComponentAnalysis {
    pub functions: Vec<(MachineId, Vec<Vec<BlockId>>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopRegion {
    /// Natural-loop header when one node dominates every entry. `None` marks
    /// an irreducible region with multiple entries.
    pub header: Option<BlockId>,
    pub blocks: Vec<BlockId>,
    pub irreducible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopAnalysis {
    pub functions: Vec<(MachineId, Vec<LoopRegion>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallGraphAnalysis {
    /// Static calls and semantic machine transitions that must keep a target
    /// reachable, including nominal cleanup invoked on return.
    pub callees: Vec<(MachineId, Vec<MachineId>)>,
    pub components: Vec<Vec<MachineId>>,
    pub recursive_components: Vec<Vec<MachineId>>,
}
