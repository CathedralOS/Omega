//! Provenance uniqueness, logical fuel, and effect-chain replay.

use super::control_flow::block_reaches;
use super::*;

pub(crate) fn validate_provenance_fuel_effects(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut node_provenance = BTreeMap::<PsiProvenance, Vec<(BlockId, bool)>>::new();
    let mut edge_provenance = BTreeMap::<PsiProvenance, BTreeSet<EdgeId>>::new();
    let mut edge_shapes = BTreeMap::<EdgeId, (BlockId, BlockId)>::new();
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let index = u32::try_from(index).expect("unit node index was built as u32");
            if node.provenance.is_empty()
                && node.successors.is_empty()
                && !matches!(node.operation, O::DynamicDescriptorParameter { .. })
            {
                return Err(OptimizationUnitValidationError::IncompleteProvenance {
                    machine: function.machine,
                    block: block.id,
                    node: index,
                });
            }
            let unique_node_sources = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            if unique_node_sources.len() != node.provenance.len() {
                return Err(OptimizationUnitValidationError::DuplicateProvenance(
                    *node
                        .provenance
                        .first()
                        .expect("duplicated provenance is nonempty"),
                ));
            }
            let is_exact_terminal = node.successors.is_empty()
                && matches!(
                    node.operation,
                    O::Return { .. }
                        | O::ReturnUnit { .. }
                        | O::ReturnStructural { .. }
                        | O::Crash { .. }
                );
            for site in &node.provenance {
                if edge_provenance.contains_key(site) {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(*site));
                }
                node_provenance
                    .entry(*site)
                    .or_default()
                    .push((block.id, is_exact_terminal));
            }
            let source_sites = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            let settled_sites = node
                .fuel
                .iter()
                .map(|settlement| settlement.site)
                .collect::<BTreeSet<_>>();
            if source_sites != settled_sites
                || node.fuel.len() != node.provenance.len()
                || node
                    .fuel
                    .iter()
                    .zip(&node.provenance)
                    .any(|(settlement, source)| settlement.site != *source || settlement.units != 1)
            {
                return Err(
                    OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    },
                );
            }
            for settlement in &node.fuel {
                let _ = settlement;
            }
            for edge in &node.successors {
                edge_shapes.insert(edge.psi_edge, (block.id, edge.target));
                if edge.provenance.is_empty()
                    || edge.provenance.first() != Some(&PsiProvenance::Edge(edge.psi_edge))
                    || edge
                        .provenance
                        .iter()
                        .any(|site| !matches!(site, PsiProvenance::Edge(_)))
                {
                    return Err(OptimizationUnitValidationError::IncompleteProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    });
                }
                let source_sites = edge.provenance.iter().copied().collect::<BTreeSet<_>>();
                if source_sites.len() != edge.provenance.len()
                    || node_provenance
                        .keys()
                        .any(|site| source_sites.contains(site))
                {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(
                        *edge
                            .provenance
                            .first()
                            .expect("edge provenance is nonempty"),
                    ));
                }
                if edge.fuel.len() != edge.provenance.len()
                    || edge
                        .fuel
                        .iter()
                        .zip(&edge.provenance)
                        .any(|(settlement, source)| {
                            settlement.site != *source || settlement.units != 1
                        })
                {
                    return Err(
                        OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                            machine: function.machine,
                            block: block.id,
                            node: index,
                        },
                    );
                }
                for source in &edge.provenance {
                    edge_provenance
                        .entry(*source)
                        .or_default()
                        .insert(edge.psi_edge);
                }
            }
            if node.effect.input != expected_effect || node.effect.output != expected_effect + 1 {
                return Err(OptimizationUnitValidationError::BrokenEffectChain {
                    machine: function.machine,
                    expected: expected_effect,
                    actual: node.effect.input,
                });
            }
            expected_effect += 1;
        }
    }
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .iter()
                    .flat_map(|node| node.successors.iter().map(|edge| edge.target))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (source, occurrences) in node_provenance {
        if occurrences.len() < 2 {
            continue;
        }
        if !matches!(source, PsiProvenance::Edge(_))
            || occurrences.iter().any(|(_, terminal)| !terminal)
        {
            return Err(OptimizationUnitValidationError::DuplicateProvenance(source));
        }
        for (index, (left, _)) in occurrences.iter().enumerate() {
            for (right, _) in &occurrences[index + 1..] {
                if left == right
                    || block_reaches(&successors, *left, *right)
                    || block_reaches(&successors, *right, *left)
                {
                    return Err(
                        OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source),
                    );
                }
            }
        }
    }
    for (source, occurrences) in edge_provenance {
        let occurrences = occurrences.into_iter().collect::<Vec<_>>();
        for (index, left) in occurrences.iter().enumerate() {
            let (_, left_target) = edge_shapes[left];
            for right in &occurrences[index + 1..] {
                let (right_owner, right_target) = edge_shapes[right];
                let (left_owner, _) = edge_shapes[left];
                if block_reaches(&successors, left_target, right_owner)
                    || block_reaches(&successors, right_target, left_owner)
                {
                    return Err(
                        OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source),
                    );
                }
            }
        }
    }
    Ok(())
}
