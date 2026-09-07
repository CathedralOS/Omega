use semantic_vocabulary::{BlockId, EdgeId, IntegerType, IntegerValue, ValueId};

/// Private progress evidence for a machine's actual control graph. Neither
/// variant grants finite fuel or native realization.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalRankedScc {
    UnsignedCountdown(TerminalUnsignedCountdownScc),
    Natural(Vec<TerminalNaturalCycle>),
}

impl TerminalRankedScc {
    pub fn as_unsigned_countdown(&self) -> Option<&TerminalUnsignedCountdownScc> {
        match self {
            Self::UnsignedCountdown(component) => Some(component),
            Self::Natural(_) => None,
        }
    }

    pub fn as_unsigned_countdown_mut(&mut self) -> Option<&mut TerminalUnsignedCountdownScc> {
        match self {
            Self::UnsignedCountdown(component) => Some(component),
            Self::Natural(_) => None,
        }
    }
}

/// Existing exact countdown migration route. Consumers of this shape must
/// select it explicitly rather than interpreting arbitrary ranking evidence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalUnsignedCountdownScc {
    pub header: BlockId,
    pub rank_parameter: ValueId,
    pub rank_type: IntegerType,
    pub lower_bound: IntegerValue,
    pub upper_bound: IntegerValue,
    /// Strictly ordered by `edge`; every cyclic edge must appear exactly once.
    pub covered_cyclic_edges: Vec<TerminalRankedSccEdge>,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalRankedSccEdge {
    pub edge: EdgeId,
    pub source: BlockId,
    pub target: BlockId,
    pub guard: TerminalRankedGuard,
    pub successor_argument: TerminalRankedSuccessorArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalRankedGuard {
    UnsignedParameterPositive {
        block: BlockId,
        edge: EdgeId,
        condition: ValueId,
        parameter: ValueId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalRankedSuccessorArgument {
    UnsignedParameterMinusOne {
        argument_index: u32,
        argument: ValueId,
        source_parameter: ValueId,
        target_parameter: ValueId,
    },
}
