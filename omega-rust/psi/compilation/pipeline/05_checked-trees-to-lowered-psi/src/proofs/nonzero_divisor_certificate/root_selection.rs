//! Root-bound selection for exact-cast and affine custody certificates.
//!
//! A goal `a <= b` about a value that an exact cast spine or a checked affine
//! definition word reaches is proved in two steps. A *root source* proposes a
//! bound on some root value built from cited facts, and a *completion*
//! carries that root bound to the goal: [`Completion::CastCustody`] through
//! the exact cast chain from the root to a goal endpoint,
//! [`Completion::AffineCustody`] through one checked affine word. Every root
//! source hands its candidates, in source order, to whichever completion the
//! entry asked for; sources never know which completion consumes them.
//!
//! Search order decides which certificate is emitted, and certificate bytes
//! are part of the compiled artifact. Each entry therefore names its sources
//! as one fixed precedence list ([`CAST_CUSTODY_SOURCES`],
//! [`AFFINE_CUSTODY_SOURCES`]); [`select`] tries them in that order and stops
//! at the first completed certificate. A source only proposes candidates: the
//! completion checks each certificate before returning it, and the kernel
//! re-checks it again at admission.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::affine_custody::{self, DefinitionIndex};
use super::cast_custody;

mod alias;
mod cast_spine;
mod literal;
mod retained;
mod roster;
#[cfg(test)]
mod tests;

/// One order-bound question: the goal, and the scope whose assumptions and
/// semantic axioms a certificate may cite by position.
#[derive(Clone, Copy)]
struct Request<'a> {
    context: &'a PropositionContext,
    goal: &'a Proposition,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

/// How a proposed root bound is carried to the goal.
enum Completion<'a> {
    /// Through the exact integer-cast chain from the root to a goal endpoint.
    CastCustody,
    /// Through one checked affine definition word from the root to a goal
    /// endpoint, searched with the scope's shared definition index.
    AffineCustody(&'a mut DefinitionIndex),
}

impl Completion<'_> {
    fn complete(
        &mut self,
        request: Request,
        root: &ScalarTerm,
        root_bound: ProofNode,
    ) -> Option<ProofNode> {
        let Request {
            context,
            goal,
            assumptions,
            semantic_axioms,
        } = request;
        match self {
            Self::CastCustody => cast_custody::prove_from_root(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                root_bound,
            ),
            Self::AffineCustody(definitions) => affine_custody::prove_from_root(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
                root,
                root_bound,
            ),
        }
    }
}

/// Carries one proposed `(root, root bound)` candidate to the goal; `None`
/// rejects the candidate and lets the source propose its next one.
type Complete<'a> = dyn FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode> + 'a;

/// Where a proposed root bound comes from. Each source enumerates cited facts
/// in source order (assumptions, then semantic axioms) and proposes every
/// candidate it can build, stopping at the first one the completion accepts.
///
/// The one- and two-alias sources stay separate on purpose: each walks a
/// fixed number of cited equalities, so no source takes a hop count or
/// searches an alias graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RootSource {
    /// A cited `a <= b` bounds whichever endpoints are values.
    RetainedBound,
    /// A cited `root == literal` closes at `literal <= root` and
    /// `root <= literal`.
    ClosedLandedLiteral,
    /// As [`Self::ClosedLandedLiteral`] through one cited value alias
    /// `root == alias`, `alias == literal`.
    ClosedLandedLiteralThroughAlias,
    /// A cited bound on an alias one cited equality away from the root.
    OneAlias,
    /// Two distinct cited bounds `left <= middle <= right` joined by
    /// transitivity, rooted at either value endpoint.
    TwoCitationChain,
    /// A two-citation chain whose endpoint is renamed through one cited value
    /// alias.
    TwoCitationChainThroughAlias,
    /// A cited bound on an alias two distinct cited equalities away.
    TwoAlias,
    /// A cited guard rejoined, through proved equalities, onto the
    /// non-cast source of a goal endpoint.
    RelationalGuard,
    /// The exact cast chain's own carrier bound: `Truth` at the non-cast
    /// source of a goal endpoint.
    CastTruth,
    /// A cited `root == literal` closed against the goal's literal side
    /// remapped into the root's carrier.
    GoalLandedLiteral,
    /// A cited bound on an alias whose literal is at least as strong as the
    /// goal's literal side, renamed through one cited equality.
    StrongerAliasBound,
    /// As [`Self::GoalLandedLiteral`] through one cited alias equality.
    GoalLandedLiteralThroughAlias,
    /// A full integer-selection proof of the goal's literal bound on the
    /// non-cast source, citing every assumption but only the semantic axioms
    /// before the spine's first cast.
    SourceSideBound,
}

impl RootSource {
    fn find(self, request: Request, complete: &mut Complete) -> Option<ProofNode> {
        match self {
            Self::RetainedBound => retained::retained_bound(request, complete),
            Self::ClosedLandedLiteral => literal::closed_landed_literal(request, complete),
            Self::ClosedLandedLiteralThroughAlias => {
                literal::closed_landed_literal_through_alias(request, complete)
            }
            Self::OneAlias => alias::one_alias(request, complete),
            Self::TwoCitationChain => retained::two_citation_chain(request, complete),
            Self::TwoCitationChainThroughAlias => {
                retained::two_citation_chain_through_alias(request, complete)
            }
            Self::TwoAlias => alias::two_alias(request, complete),
            Self::RelationalGuard => cast_spine::relational_guard(request, complete),
            Self::CastTruth => cast_spine::cast_truth(request, complete),
            Self::GoalLandedLiteral => literal::goal_landed_literal(request, complete),
            Self::StrongerAliasBound => alias::stronger_alias_bound(request, complete),
            Self::GoalLandedLiteralThroughAlias => {
                literal::goal_landed_literal_through_alias(request, complete)
            }
            Self::SourceSideBound => cast_spine::source_side_bound(request, complete),
        }
    }
}

/// Precedence of root sources completed through exact cast custody.
const CAST_CUSTODY_SOURCES: [RootSource; 9] = [
    RootSource::RetainedBound,
    RootSource::RelationalGuard,
    RootSource::CastTruth,
    RootSource::GoalLandedLiteral,
    RootSource::OneAlias,
    RootSource::StrongerAliasBound,
    RootSource::GoalLandedLiteralThroughAlias,
    RootSource::TwoAlias,
    RootSource::SourceSideBound,
];

/// Precedence of root sources completed through affine custody.
const AFFINE_CUSTODY_SOURCES: [RootSource; 7] = [
    RootSource::RetainedBound,
    RootSource::ClosedLandedLiteral,
    RootSource::ClosedLandedLiteralThroughAlias,
    RootSource::OneAlias,
    RootSource::TwoCitationChain,
    RootSource::TwoCitationChainThroughAlias,
    RootSource::TwoAlias,
];

/// Try `sources` in order, carrying each proposed root bound through
/// `completion`, and return the first completed certificate.
fn select(
    request: Request,
    sources: &[RootSource],
    mut completion: Completion,
) -> Option<ProofNode> {
    sources.iter().find_map(|source| {
        source.find(request, &mut |root, root_bound| {
            completion.complete(request, root, root_bound)
        })
    })
}

/// Prove `a <= b` through the exact cast chain from a proposed root bound.
pub(super) fn prove_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return None;
    }
    let request = Request {
        context,
        goal,
        assumptions,
        semantic_axioms,
    };
    select(request, &CAST_CUSTODY_SOURCES, Completion::CastCustody)
}

/// Prove a strict or non-strict order goal by carrying one rejoined guard
/// across the exact cast chain of a goal endpoint.
pub(super) fn prove_relational_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let request = Request {
        context,
        goal,
        assumptions,
        semantic_axioms,
    };
    select(
        request,
        &[RootSource::RelationalGuard],
        Completion::CastCustody,
    )
}

/// Prove `a <= b` through affine custody, then through the cast-adjacent
/// compositions whose affine word resumes after one exact cast spine. The
/// answer is memoized per goal in the scope's definition index.
pub(super) fn prove_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = definitions.cached_affine_proof(goal) {
        return proof;
    }
    definitions.begin_affine_proof(goal);
    let request = Request {
        context,
        goal,
        assumptions,
        semantic_axioms,
    };
    let proof = select(
        request,
        &AFFINE_CUSTODY_SOURCES,
        Completion::AffineCustody(definitions),
    )
    .or_else(|| cast_spine::prove_after_cast_spine(request, definitions))
    .or_else(|| cast_spine::prove_across_cast_spine(request, definitions));
    definitions.cache_affine_proof(goal, proof.clone());
    proof
}

/// Prove `a <= b` through affine custody alone, without the cast-adjacent
/// compositions. Unlike [`prove_affine_bound`], this answer is neither read
/// from nor recorded in the definition index's per-goal memo.
pub(super) fn prove_affine_bound_without_cast(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let request = Request {
        context,
        goal,
        assumptions,
        semantic_axioms,
    };
    select(
        request,
        &AFFINE_CUSTODY_SOURCES,
        Completion::AffineCustody(definitions),
    )
}
