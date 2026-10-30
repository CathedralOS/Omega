//! Accepted obligation facts joined to the operation that carries them.
//!
//! An operation that carries an obligation discharges it through one accepted
//! fact on the unit's retained roster, keyed by the machine, the owning
//! provenance and the obligation. Projection, independent replay and the
//! runtime-index join each need that row; the join lives here so a caller
//! names the obligation and keeps its own refusal instead of respelling the
//! same three-way key.

use crate::LegalizationError;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{ObligationId, OperationId};

/// The accepted fact rows `owner` recorded for `obligation` on this function's
/// machine, in the unit's retained order. A valid unit names at most one row.
pub(crate) fn facts<'unit>(
    unit: &'unit PsiOptimizationUnit,
    optimized: &PsiOptimizationFunction,
    owner: impl Into<optimization_unit::PsiProvenance>,
    obligation: ObligationId,
) -> impl Iterator<Item = &'unit optimization_unit::AcceptedObligationFact> {
    let owner = owner.into();
    let machine = optimized.machine;
    unit.accepted_obligation_facts.iter().filter(move |fact| {
        fact.machine == machine && fact.owner == owner && fact.obligation == obligation
    })
}

/// The accepted fact identity `owner` carries for `obligation`, or the custody
/// error when the unit recorded none.
pub(crate) fn accepted_fact(
    unit: &PsiOptimizationUnit,
    optimized: &PsiOptimizationFunction,
    owner: impl Into<optimization_unit::PsiProvenance>,
    obligation: ObligationId,
) -> Result<optimization_core::AcceptedObligationFactIdentity, LegalizationError> {
    facts(unit, optimized, owner, obligation)
        .next()
        .map(|fact| fact.identity)
        .ok_or_else(LegalizationError::custody)
}

/// Whether the optimized function references `obligation` from `operation`.
pub(crate) fn references_obligation(
    optimized: &PsiOptimizationFunction,
    operation: OperationId,
    obligation: ObligationId,
) -> bool {
    optimized.facts.iter().any(|fact| {
        matches!(fact,
            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
            if *referenced == obligation && *support == operation)
    })
}
