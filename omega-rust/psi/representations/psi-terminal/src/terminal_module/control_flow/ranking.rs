use psi_core::{BlockId, EdgeId, IntegerType, IntegerValue, ValueId};

/// One exact ranked strongly connected component in Terminal-Psi identity.
///
/// The current representation admits only the deliberately narrow unsigned
/// countdown shape. The row names Terminal identities exclusively; frontend
/// arena handles and source coordinates cannot survive this boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalRankedScc {
    pub header: BlockId,
    pub rank_parameter: ValueId,
    pub rank_type: IntegerType,
    pub lower_bound: IntegerValue,
    pub upper_bound: IntegerValue,
    /// Strictly ordered by `edge`; every cyclic edge must appear exactly once.
    pub covered_cyclic_edges: Vec<TerminalRankedSccEdge>,
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
