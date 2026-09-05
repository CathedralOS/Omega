use optimization_unit::{
    OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot, PsiOptimizationUnit,
    recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{EdgeId, MachineId, OperationId};

use crate::rules::tests::{constant_conditional_same_target_unit, id};

pub(crate) fn constant_merge_barrier_unit() -> PsiOptimizationUnit {
    let mut unit = constant_conditional_same_target_unit(true);
    let machine = id(651, MachineId::new);
    let snapshot = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: Vec::new(),
        partial_custody: Vec::new(),
    };
    // Exact source-site custody keeps the fold valid. Deliberately omitting a
    // merge-block frontier makes the later block-merge witness incomplete, so
    // the constant-fold roster row owns the fixture's sole candidate.
    unit.ownership_frontier_facts = [
        OwnershipFrontierSite::OperationEntry(id(655, OperationId::new)),
        OwnershipFrontierSite::OperationExit(id(655, OperationId::new)),
        OwnershipFrontierSite::EdgeEntry(id(656, EdgeId::new)),
        OwnershipFrontierSite::EdgeExit(id(656, EdgeId::new)),
        OwnershipFrontierSite::EdgeEntry(id(657, EdgeId::new)),
        OwnershipFrontierSite::EdgeExit(id(657, EdgeId::new)),
        OwnershipFrontierSite::EdgeEntry(id(658, EdgeId::new)),
        OwnershipFrontierSite::EdgeExit(id(658, EdgeId::new)),
    ]
    .into_iter()
    .map(|site| OwnershipFrontierFact::new(unit.psi, machine, site, snapshot.clone()))
    .collect();
    unit.ownership_frontier_facts
        .sort_by_key(|fact| (fact.machine, fact.site));
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}
