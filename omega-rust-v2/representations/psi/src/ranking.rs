//! Private ranking witnesses for authored `terminates by` clauses.
//!
//! A witness binds the well-founded relation, each member's rank over its own
//! formals, and one claim for every in-component control edge. It carries no
//! verdict: well-foundedness, every comparison, the rank bounds and the global
//! every-cycle descent rule remain checker obligations. Identities are owned
//! and independent of source trees and of any native representation.
use crate::{BlockId, DeclarationId, EdgeId, MachineId, OperationId, Parameter, ScalarType, Site, ValueId};
use num_bigint::BigInt;

/// The order shared by one reconstructed component. The carrier, including
/// its signedness, enters the relation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankRelation {
    pub carrier: ScalarType,
    pub order: RankOrder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankOrder {
    /// Ranks are naturals of the carrier; descent is `<`, preservation `<=`.
    NaturalDescending,
}

/// A normalized view result. `Operand` is a member formal index in a member
/// rank, and an actual argument value of the caller in an edge claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RankExpression<Operand> {
    /// `Nat::Descending`: the subject is the rank.
    Operand(Operand),
    /// `Nat::BoundedDistance`: unsigned saturating `limit - cursor`.
    SaturatingDistance { cursor: Operand, limit: Operand },
}

impl<Operand: Copy> RankExpression<Operand> {
    pub fn operands(&self) -> Vec<Operand> {
        match self {
            Self::Operand(operand) => vec![*operand],
            Self::SaturatingDistance { cursor, limit } => vec![*cursor, *limit],
        }
    }

    /// Substitute every operand simultaneously; any unresolved operand rejects.
    pub fn try_map<Other>(
        &self,
        mut map: impl FnMut(Operand) -> Option<Other>,
    ) -> Option<RankExpression<Other>> {
        Some(match self {
            Self::Operand(operand) => RankExpression::Operand(map(*operand)?),
            Self::SaturatingDistance { cursor, limit } => RankExpression::SaturatingDistance {
                cursor: map(*cursor)?,
                limit: map(*limit)?,
            },
        })
    }
}

/// An authored `in lower..=upper` constraint on the rank, not on storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankBounds {
    pub lower: BigInt,
    pub upper: BigInt,
    pub upper_inclusive: bool,
    pub site: Site,
}

/// Comparison obligation in *unchecked* Psi. Even a named role must be
/// proved on the actual successor arguments before it can be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankComparison {
    /// No role selected. Checking must establish one of the two comparisons
    /// below in the edge's context, and strict edges must break every cycle.
    /// This is not a third role in a checked component.
    Pending,
    /// `successor_rank <= source_rank`
    Preserving,
    /// `successor_rank < source_rank`
    Strict,
}

/// One in-component edge: the exact terminator edge of the owning machine,
/// the block it leaves, its member target and the target rank substituted
/// with the actual successor arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedEdge {
    pub block: BlockId,
    pub edge: EdgeId,
    /// `None`: `edge` is the terminal call. `Some(op)`: `op` and `edge`
    /// together encode one constant-stack tail transfer, not a recursive
    /// activation. The final call operation supplies the target, actuals and
    /// call requirements; the cleanup-free return must forward its exact
    /// scalar result. A checker can reconstruct this pair from the body.
    pub forwarded_call: Option<OperationId>,
    pub target: MachineId,
    pub successor: RankExpression<ValueId>,
    /// Compare `successor` to the owning member's `source_rank`. The owner
    /// supplies entry requirements; the block and call identify the path and
    /// operation premises. A pending comparison uses the very same inputs.
    pub comparison: RankComparison,
}

/// The private witness of one member machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineRanking {
    /// Exact admitted view declaration that normalized the authored subject.
    pub view: DeclarationId,
    pub relation: RankRelation,
    /// Entry rank over this machine's immutable scalar formals.
    pub rank: RankExpression<usize>,
    pub bounds: Option<RankBounds>,
    pub site: Site,
    /// Canonical member set of the reconstructed component, ascending.
    pub component: Vec<MachineId>,
    /// Claims for every internal edge leaving this member, ascending by edge.
    pub edges: Vec<RankedEdge>,
}

impl MachineRanking {
    /// The source rank at an edge of this member: its rank over the entry
    /// formals. Structural or missing formals have no scalar rank.
    pub fn source_rank(
        &self,
        parameters: &[Parameter],
    ) -> Option<RankExpression<ValueId>> {
        self.rank.try_map(|formal| match parameters.get(formal) {
            Some(Parameter::Scalar(value)) => Some(*value),
            _ => None,
        })
    }
}
