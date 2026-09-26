//! Shared ownership-frontier custody for CFG rewrites that erase a unique incoming edge.

use optimization_unit::{
    OwnershipFrontierSite, OwnershipFrontierWitness, OwnershipFrontierWitnessRow,
    PsiOptimizationFunction, PsiOptimizationUnit,
};
use semantic_vocabulary::{BlockId, EdgeId};

use crate::OwnershipFrontierAnalysis;

pub(super) fn merge_boundary_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    frontiers: &OwnershipFrontierAnalysis,
    incoming: EdgeId,
    target: BlockId,
) -> bool {
    merge_boundary_ownership_witness(unit, function, frontiers, incoming, target).is_some()
}

pub(super) fn merge_boundary_ownership_witness(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    frontiers: &OwnershipFrontierAnalysis,
    incoming: EdgeId,
    target: BlockId,
) -> Option<OwnershipFrontierWitness> {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| frontiers.fact(function.machine, site));
    if facts.iter().all(Option::is_none) {
        return (function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty())
        .then_some(OwnershipFrontierWitness { rows: Vec::new() });
    }
    if !facts.iter().all(|fact| {
        fact.is_some_and(|fact| fact.revision == unit.identity && fact.machine == function.machine)
    }) || !facts
        .windows(2)
        .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
    {
        return None;
    }
    let mut rows = facts
        .into_iter()
        .map(|fact| {
            let fact = fact.expect("complete ownership frontier fact set");
            OwnershipFrontierWitnessRow {
                site: fact.site,
                fact: fact.identity,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by_key(|row| row.site);
    Some(OwnershipFrontierWitness { rows })
}
