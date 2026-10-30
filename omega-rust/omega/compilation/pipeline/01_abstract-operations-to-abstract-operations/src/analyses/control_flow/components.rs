//! Canonical strongly connected components. The decomposition itself is
//! [`crate::cycles`]; this module states the block projection it runs over and
//! which machines that projection constrains.

use std::collections::BTreeSet;

use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::MachineId;

use super::{StronglyConnectedComponentAnalysis, graph::control_flow};
use crate::cycles::strongly_connected_components;

impl StronglyConnectedComponentAnalysis {
    /// Machines whose block projection contains a multi-block cyclic component
    /// or a singleton self-loop. Callers decide how cycles constrain a rewrite.
    pub(crate) fn cyclic_machines(&self, unit: &PsiOptimizationUnit) -> BTreeSet<MachineId> {
        unit.functions
            .iter()
            .filter(|function| {
                let Some((_, components)) = self.function(function.machine) else {
                    return false;
                };
                components.iter().any(|component| {
                    component.len() > 1
                        || component.iter().any(|block| {
                            function.block(*block).is_some_and(|owner| {
                                owner.successors().any(|edge| edge.target == *block)
                            })
                        })
                })
            })
            .map(|function| function.machine)
            .collect()
    }
}

pub(in crate::analyses) fn block_components(
    unit: &PsiOptimizationUnit,
) -> StronglyConnectedComponentAnalysis {
    let cfg = control_flow(unit);
    StronglyConnectedComponentAnalysis {
        functions: cfg
            .functions
            .iter()
            .map(|function| {
                let graph = function
                    .blocks
                    .iter()
                    .map(|block| (block.block, block.successors.clone()))
                    .collect();
                (function.machine, strongly_connected_components(&graph))
            })
            .collect(),
    }
}
