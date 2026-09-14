use semantic_vocabulary::{BlockId, EdgeId, IntegerType, ValueId};

/// Private progress evidence for a machine's actual control graph. The
/// representation grants no finite fuel or native realization on its own.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalRankedScc {
    Natural(Vec<TerminalNaturalCycle>),
}

/// Producer-selected natural ranks for one complete cyclic component. The
/// verifier derives topology and checks that these rows cover it exactly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalNaturalCycle {
    pub rank_type: IntegerType,
    /// Canonical block order; rank values are actual unsigned SSA observations.
    pub ranks: Vec<TerminalBlockNaturalRank>,
    /// Canonical edge order, including preserving implementation-staging edges.
    pub edges: Vec<TerminalNaturalRankEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalBlockNaturalRank {
    pub block: BlockId,
    pub value: ValueId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalNaturalRankEdge {
    pub edge: EdgeId,
    pub source: BlockId,
    pub target: BlockId,
    /// The target rank after exact substitution through this successor.
    pub successor_rank: ValueId,
    pub comparison: TerminalNaturalRankComparison,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalNaturalRankComparison {
    Preserving,
    Strict,
}
