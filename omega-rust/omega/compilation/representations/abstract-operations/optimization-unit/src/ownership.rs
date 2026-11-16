// The optimization unit attaches the shared ownership facts once, after checking them.

pub use native_vocabulary::ownership_frontiers::{
    OwnershipFrontierFact, OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
    OwnershipFrontierPartialCustody, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    encode_ownership_frontier_site,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipFrontierFactIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidFactIdentity,
    NonCanonicalOrder,
    NonCanonicalSnapshot,
}

optimization_core::invalid_error_display!(
    OwnershipFrontierFactIndexError,
    "ownership frontier fact index"
);

pub fn attach_ownership_frontier_facts(
    mut unit: super::PsiOptimizationUnit,
    facts: Vec<OwnershipFrontierFact>,
) -> Result<super::PsiOptimizationUnit, OwnershipFrontierFactIndexError> {
    if !unit.ownership_frontier_facts.is_empty() {
        return Err(OwnershipFrontierFactIndexError::AlreadyAttached);
    }
    if facts.iter().any(|fact| fact.psi != unit.psi) {
        return Err(OwnershipFrontierFactIndexError::TerminalIdentityMismatch);
    }
    if facts.iter().any(|fact| !fact.has_canonical_identity()) {
        return Err(OwnershipFrontierFactIndexError::InvalidFactIdentity);
    }
    if facts
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
    {
        return Err(OwnershipFrontierFactIndexError::NonCanonicalOrder);
    }
    if facts
        .iter()
        .any(|fact| !canonical_ownership_frontier_snapshot(&fact.snapshot))
    {
        return Err(OwnershipFrontierFactIndexError::NonCanonicalSnapshot);
    }
    unit.ownership_frontier_facts = facts;
    unit.identity = super::recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

pub fn canonical_ownership_frontier_snapshot(snapshot: &OwnershipFrontierSnapshot) -> bool {
    strictly_ordered_by(&snapshot.claims, |claim| claim.claim)
        && strictly_ordered_by(&snapshot.owned_places, |place| place.place)
        && strictly_ordered_by(&snapshot.partial_custody, |partial| partial.place)
        && snapshot
            .partial_custody
            .iter()
            .all(|partial| partial.moved_paths.windows(2).all(|pair| pair[0] < pair[1]))
}

fn strictly_ordered_by<T, K: Ord>(values: &[T], key: impl Fn(&T) -> K) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}
