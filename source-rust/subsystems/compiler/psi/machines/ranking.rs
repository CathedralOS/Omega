use arena::{Arena, HandleSpan};
use semantic_vocabulary::{BlockId, EdgeId, IntegerType, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRankedScc {
    Natural(Vec<TerminalNaturalCycle>),
}

#[derive(Debug, Clone)]
pub struct TerminalNaturalCycle {
    pub observations: Arena<TerminalIntegerRankValue>,
    pub ranks: Vec<TerminalBlockNaturalRank>,
    pub edges: Vec<TerminalNaturalRankEdge>,
}

impl TerminalNaturalCycle {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct TerminalIntegerRankValue {
    pub integer_type: IntegerType,
    pub value: ValueId,
}

impl Default for TerminalIntegerRankValue {
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
    pub successor_ranks: HandleSpan<TerminalIntegerRankValue>,
    pub comparison: TerminalNaturalRankComparison,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalNaturalRankComparison {
    Preserving,
    Strict,
}
