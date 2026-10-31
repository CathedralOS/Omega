use arena::{Arena, HandleSpan};
use semantic_vocabulary::{BlockId, EdgeId, IntegerType, ValueId};

/// Private progress evidence for a machine's actual control graph. The
/// representation grants no finite fuel or native realization on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRankedScc {
    Natural(Vec<TerminalNaturalCycle>),
}

/// Ordered fixed-integer observations for one complete cyclic component.
/// A singleton is the ordinary scalar order. Tuples compare lexicographically:
/// a strict earlier component permits any formed suffix. Every row retains the
/// same arity and corresponding signed/unsigned carriers.
#[derive(Debug, Clone)]
pub struct TerminalNaturalCycle {
    pub observations: Arena<TerminalIntegerRankValue>,
    /// Canonical block order, with complete ordered component spans.
    pub ranks: Vec<TerminalBlockNaturalRank>,
    /// Canonical edge order, including preserving implementation-staging edges.
    pub edges: Vec<TerminalNaturalRankEdge>,
}

impl TerminalNaturalCycle {
    /// The first row supplies the signature; every read checks the complete
    /// arity and each carrier. An absent suffix cannot become a scalar rank.
    pub fn rank_values(
        &self,
        values: HandleSpan<TerminalIntegerRankValue>,
    ) -> Option<&[TerminalIntegerRankValue]> {
        let signature = self.observations.span(self.ranks.first()?.values)?;
        let values = self.observations.span(values)?;
        (!signature.is_empty()
            && values.len() == signature.len()
            && values.iter().zip(signature).all(|(value, expected)| {
                !value.integer_type.is_address() && value.integer_type == expected.integer_type
            }))
        .then_some(values)
    }

    /// Optional scalar optimizations must explicitly decline tuple ranks.
    pub fn scalar_rank_value(
        &self,
        values: HandleSpan<TerminalIntegerRankValue>,
    ) -> Option<&TerminalIntegerRankValue> {
        let [value] = self.rank_values(values)? else {
            return None;
        };
        Some(value)
    }
}

// Portable equality names SSA observations, not the arena allocation history.
// The codec rebuilds component spans under its own owner; shared input spans
// and independently inserted equal rows have the same portable meaning.
impl PartialEq for TerminalNaturalCycle {
    fn eq(&self, other: &Self) -> bool {
        self.ranks.len() == other.ranks.len()
            && self.edges.len() == other.edges.len()
            && self.ranks.iter().zip(&other.ranks).all(|(left, right)| {
                left.block == right.block
                    && self.observations.span(left.values) == other.observations.span(right.values)
            })
            && self.edges.iter().zip(&other.edges).all(|(left, right)| {
                left.edge == right.edge
                    && left.source == right.source
                    && left.target == right.target
                    && left.comparison == right.comparison
                    && self.observations.span(left.successor_ranks)
                        == other.observations.span(right.successor_ranks)
            })
    }
}
impl Eq for TerminalNaturalCycle {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalIntegerRankValue {
    pub integer_type: IntegerType,
    pub value: ValueId,
}

impl Default for TerminalIntegerRankValue {
    /// Arena's ZII slot has no rank meaning. Its address carrier is explicitly
    /// excluded by rank_values; live observations always carry fixed words.
    fn default() -> Self {
        Self {
            integer_type: IntegerType::supported_address_representation(),
            value: ValueId::FIRST,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalBlockNaturalRank {
    pub block: BlockId,
    pub values: HandleSpan<TerminalIntegerRankValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalNaturalRankEdge {
    pub edge: EdgeId,
    pub source: BlockId,
    pub target: BlockId,
    /// Complete target tuple after exact substitution through this successor.
    pub successor_ranks: HandleSpan<TerminalIntegerRankValue>,
    pub comparison: TerminalNaturalRankComparison,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalNaturalRankComparison {
    Preserving,
    Strict,
}
