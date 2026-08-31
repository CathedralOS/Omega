//! Ownership-frontier identity required before threading an empty block.

use omega_optimization_unit::{
    OwnershipFrontierSite, PsiOptimizationFunction, PsiOptimizationUnit,
};
use psi_core::{BlockId, EdgeId};

use crate::OwnershipFrontierAnalysis;

pub(super) fn linear_thread_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    frontiers: &OwnershipFrontierAnalysis,
    incoming: EdgeId,
    empty: BlockId,
    outgoing: EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(empty),
        OwnershipFrontierSite::EdgeEntry(outgoing),
        OwnershipFrontierSite::EdgeExit(outgoing),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| frontiers.fact(function.machine, site));
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(|fact| {
        fact.is_some_and(|fact| fact.revision == unit.identity && fact.machine == function.machine)
    }) && facts
        .windows(2)
        .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}
