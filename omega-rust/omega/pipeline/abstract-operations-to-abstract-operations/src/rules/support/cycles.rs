//! Cyclic-component freeze roster shared by independently named Psi passes.

use std::collections::BTreeSet;

use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::MachineId;

use crate::StronglyConnectedComponentAnalysis;

/// Machines whose block projection contains a cyclic component — a
/// multi-block SCC or a singleton self-loop — are frozen byte-exact for the
/// specialization families, matching their bespoke cycle rosters.
pub(in crate::rules) fn frozen_machines(
    unit: &PsiOptimizationUnit,
    scc: &StronglyConnectedComponentAnalysis,
) -> BTreeSet<MachineId> {
    unit.functions
        .iter()
        .filter(|function| {
            let Some((_, components)) = scc
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
            else {
                return false;
            };
            components.iter().any(|component| {
                component.len() > 1
                    || component.iter().any(|block| {
                        function
                            .blocks
                            .iter()
                            .find(|candidate| candidate.id == *block)
                            .is_some_and(|owner| {
                                owner
                                    .nodes
                                    .iter()
                                    .flat_map(|node| &node.successors)
                                    .any(|edge| edge.target == *block)
                            })
                    })
            })
        })
        .map(|function| function.machine)
        .collect()
}
