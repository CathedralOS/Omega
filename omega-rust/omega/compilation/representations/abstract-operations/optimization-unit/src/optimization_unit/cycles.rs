//! Optimizer module role: carrier leaf. Canonical component identities and optimizer-analysis custody.

use semantic_vocabulary::{
    BlockId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId,
};

/// One canonical executable control edge belonging to or crossing an SCC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CycleComponentEdge {
    pub edge: EdgeId,
    pub source: BlockId,
    pub target: BlockId,
}

/// Semantic identity of one finite cyclic component.
///
/// The owning machine and complete canonical internal-edge roster are the
/// identity. Members and boundary edges are derived topology carried beside it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CycleComponentId {
    pub machine: MachineId,
    pub internal_edges: Vec<CycleComponentEdge>,
}

/// Current optimizer topology for one independently authenticated component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerCycleComponent {
    pub id: CycleComponentId,
    pub members: Vec<BlockId>,
    pub entries: Vec<CycleComponentEdge>,
    pub exits: Vec<CycleComponentEdge>,
}

/// Replayable, non-authoritative component snapshot.
///
/// Callers may persist or mutate this data. Only
/// independent validation confers optimizer-analysis authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerCycleComponentSnapshot {
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub components: Vec<OptimizerCycleComponent>,
}

/// The closed unsigned-countdown ranking rule currently understood by the
/// optimizer. Every coordinate is retained so later loop analyses need not
/// reinterpret the Terminal ranked-SCC row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerUnsignedCountdownRankingCertificate {
    pub component: CycleComponentId,
    pub header: BlockId,
    pub rank_parameter: ValueId,
    pub rank_type: IntegerType,
    pub lower_bound: IntegerValue,
    pub upper_bound: IntegerValue,
    pub guard: OptimizerUnsignedPositiveGuard,
    pub descent: OptimizerUnsignedMinusOneDescent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizerUnsignedPositiveGuard {
    pub block: BlockId,
    pub edge: EdgeId,
    pub condition: ValueId,
    pub parameter: ValueId,
    pub zero: ValueId,
    pub zero_operation: OperationId,
    pub comparison_operation: OperationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizerUnsignedMinusOneDescent {
    pub backedge: CycleComponentEdge,
    pub argument_index: u32,
    pub argument: ValueId,
    pub source_parameter: ValueId,
    pub target_parameter: ValueId,
    pub one: ValueId,
    pub one_operation: OperationId,
    pub subtract_operation: OperationId,
    pub subtract_obligation: semantic_vocabulary::ObligationId,
}

/// Replayable ranking-certificate data without analysis authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerRankingCertificateSnapshot {
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub certificates: Vec<OptimizerUnsignedCountdownRankingCertificate>,
}
