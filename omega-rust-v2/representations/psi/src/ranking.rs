//! Private ranking witnesses for authored `terminates by` clauses.
//!
//! A witness binds the well-founded relation, each member's rank over its own
//! formals and integer constants, and one claim for every in-component control
//! edge. It carries no verdict: well-foundedness, every comparison, the rank
//! bounds, the global every-cycle descent rule and the judgment of a member
//! that authored no rank remain checker obligations. Identities are owned and
//! independent of source trees and of any native representation.
use crate::{BlockId, CallSite, DeclarationId, EdgeId, MachineId, Parameter, ScalarType, Site, ValueId};
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

/// One operand of a normalized rank. `Input` is a member formal index in a
/// member rank, and an actual argument value of the caller in an edge claim.
/// A constant is the evaluated value of a closed integer subject, such as a
/// named `const`; substitution never changes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RankOperand<Input> {
    Input(Input),
    Constant(BigInt),
}

impl<Input: Copy> RankOperand<Input> {
    pub fn input(&self) -> Option<Input> {
        match self {
            Self::Input(input) => Some(*input),
            Self::Constant(_) => None,
        }
    }

    fn try_map<Other>(
        &self,
        map: &mut impl FnMut(Input) -> Option<Other>,
    ) -> Option<RankOperand<Other>> {
        Some(match self {
            Self::Input(input) => RankOperand::Input(map(*input)?),
            Self::Constant(value) => RankOperand::Constant(value.clone()),
        })
    }
}

/// A normalized view result over [`RankOperand`]s.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RankExpression<Input> {
    /// `Nat::Descending`: the subject is the rank.
    Operand(RankOperand<Input>),
    /// `Nat::BoundedDistance`: unsigned saturating `limit - cursor`.
    SaturatingDistance { cursor: RankOperand<Input>, limit: RankOperand<Input> },
}

impl<Input: Copy> RankExpression<Input> {
    /// The inputs in operand order; constants contribute none.
    pub fn inputs(&self) -> Vec<Input> {
        match self {
            Self::Operand(operand) => operand.input().into_iter().collect(),
            Self::SaturatingDistance { cursor, limit } => cursor.input().into_iter().chain(limit.input()).collect(),
        }
    }

    /// Substitute every input simultaneously; any unresolved input rejects.
    pub fn try_map<Other>(
        &self,
        mut map: impl FnMut(Input) -> Option<Other>,
    ) -> Option<RankExpression<Other>> {
        Some(match self {
            Self::Operand(operand) => RankExpression::Operand(operand.try_map(&mut map)?),
            Self::SaturatingDistance { cursor, limit } => RankExpression::SaturatingDistance {
                cursor: cursor.try_map(&mut map)?,
                limit: limit.try_map(&mut map)?,
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

/// One in-component invocation. Rank substitution is at the call, not at
/// the caller's later resumption or exit. This is not runtime eligibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedEdge {
    pub block: BlockId,
    pub call: CallSite,
    /// For an ordinary scalar call immediately forwarded by a cleanup-free
    /// return, the matching return edge. Other continuations remain in the
    /// body; a rank claim does not turn them into constant-stack transfers.
    pub forwarding_return: Option<EdgeId>,
    pub target: MachineId,
    /// The target's entry rank over the actual successor arguments. `None`
    /// when the target member authored no `terminates by` clause: the edge
    /// still belongs to the component, but there is no target rank to
    /// substitute and the comparison stays `Pending`.
    pub successor: Option<RankExpression<ValueId>>,
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
    /// Entry rank over this machine's immutable scalar formals and constants.
    pub rank: RankExpression<usize>,
    pub bounds: Option<RankBounds>,
    pub site: Site,
    /// Canonical member set of the reconstructed component, ascending. It
    /// includes members without a ranking of their own; their outgoing edges
    /// are the calls and terminal transfers of their bodies.
    pub component: Vec<MachineId>,
    /// Claims for every internal edge leaving this member, in block order, operations before terminator.
    pub edges: Vec<RankedEdge>,
}

impl MachineRanking {
    /// The source rank at an edge of this member: its rank over the entry
    /// formals, constants unchanged. Structural or missing formals have no
    /// scalar rank.
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
