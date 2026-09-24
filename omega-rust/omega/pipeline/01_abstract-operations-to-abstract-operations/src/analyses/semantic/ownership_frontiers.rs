use optimization_core::{OptimizationUnitIdentity, OwnershipFrontierFactIdentity};
use optimization_unit::{OwnershipFrontierSite, OwnershipFrontierSnapshot, PsiOptimizationUnit};
use semantic_vocabulary::MachineId;

/// Exact immutable verifier fact made available in one optimization revision.
/// The site remains a source Terminal-Psi site; consumers must match that exact
/// site rather than treating the snapshot as a timeless function-wide fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipFrontierAnalysisFact {
    pub identity: OwnershipFrontierFactIdentity,
    pub revision: OptimizationUnitIdentity,
    pub machine: MachineId,
    pub site: OwnershipFrontierSite,
    pub snapshot: OwnershipFrontierSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipFrontierAnalysis {
    pub facts: Vec<OwnershipFrontierAnalysisFact>,
}

impl OwnershipFrontierAnalysis {
    pub fn fact(
        &self,
        machine: MachineId,
        site: OwnershipFrontierSite,
    ) -> Option<&OwnershipFrontierAnalysisFact> {
        self.facts
            .binary_search_by_key(&(machine, site), |fact| (fact.machine, fact.site))
            .ok()
            .map(|index| &self.facts[index])
    }
}

pub(in crate::analyses) fn ownership_frontiers(
    unit: &PsiOptimizationUnit,
) -> OwnershipFrontierAnalysis {
    OwnershipFrontierAnalysis {
        facts: unit
            .ownership_frontier_facts
            .iter()
            .map(|fact| OwnershipFrontierAnalysisFact {
                identity: fact.identity,
                revision: unit.identity,
                machine: fact.machine,
                site: fact.site,
                snapshot: fact.snapshot.clone(),
            })
            .collect(),
    }
}
