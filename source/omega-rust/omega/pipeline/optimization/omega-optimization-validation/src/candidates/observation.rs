//! Closed scalar observation boundaries shared by scalar rewrites.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedScalarObservationBoundary {
    pub location: NodeLocation,
    pub live_in: Vec<ValueId>,
    pub live_out: Vec<ValueId>,
}

pub fn reconstruct_closed_scalar_node_boundary(
    unit: &PsiOptimizationUnit,
    location: NodeLocation,
) -> Option<ClosedScalarObservationBoundary> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == location.machine)?;
    let mut live_entry = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut live_exit = live_entry.clone();
    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let next_exit = block
                .nodes
                .last()
                .into_iter()
                .flat_map(|node| &node.successors)
                .filter_map(|edge| live_entry.get(&edge.target))
                .flat_map(|values| values.iter().copied())
                .collect::<BTreeSet<_>>();
            let mut next_entry = next_exit.clone();
            for node in block.nodes.iter().rev() {
                for definition in &node.definitions {
                    next_entry.remove(&definition.value);
                }
                next_entry.extend(node.uses.iter().map(|use_site| use_site.value));
            }
            for parameter in &block.parameters {
                next_entry.remove(&parameter.value);
            }
            if live_exit[&block.id] != next_exit {
                live_exit.insert(block.id, next_exit);
                changed = true;
            }
            if live_entry[&block.id] != next_entry {
                live_entry.insert(block.id, next_entry);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let block = function
        .blocks
        .iter()
        .find(|block| block.id == location.block)?;
    let target = usize::try_from(location.node).ok()?;
    if target >= block.nodes.len() {
        return None;
    }
    let mut live = live_exit[&block.id].clone();
    for (node_index, node) in block.nodes.iter().enumerate().rev() {
        let live_out = live.clone();
        for definition in &node.definitions {
            live.remove(&definition.value);
        }
        live.extend(node.uses.iter().map(|use_site| use_site.value));
        if node_index == target {
            return Some(ClosedScalarObservationBoundary {
                location,
                live_in: live.iter().copied().collect(),
                live_out: live_out.iter().copied().collect(),
            });
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPsiRewrite {
    pub(in crate::candidates) unit: PsiOptimizationUnit,
    pub(in crate::candidates) candidate: OptimizationCandidateIdentity,
    pub(in crate::candidates) validator: OptimizationValidatorIdentity,
    pub(in crate::candidates) provenance: Vec<omega_optimization_unit::ProvenanceRewrite>,
}

impl ValidatedPsiRewrite {
    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub const fn candidate(&self) -> OptimizationCandidateIdentity {
        self.candidate
    }

    pub const fn validator(&self) -> OptimizationValidatorIdentity {
        self.validator
    }

    /// Validator-accepted source disposition and fuel accounting. Consumers
    /// must ledger this value rather than re-reading the proposal.
    pub fn provenance(&self) -> &[omega_optimization_unit::ProvenanceRewrite] {
        &self.provenance
    }

    pub fn into_unit(self) -> PsiOptimizationUnit {
        self.unit
    }
}
