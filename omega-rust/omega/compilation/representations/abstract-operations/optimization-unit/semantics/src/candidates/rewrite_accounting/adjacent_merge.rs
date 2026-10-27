use crate::candidates::{canonicalize_provenance, provenance_realized_at, realize_node_provenance};
use optimization_unit::{
    AdjacentBlockMergeRewrite, NodeLocation, OwnershipFrontierSite, OwnershipFrontierWitness,
    OwnershipFrontierWitnessRow, PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite,
    ScalarSubstitution,
};
use semantic_vocabulary::{BlockId, EdgeId};
use std::collections::BTreeSet;

pub(crate) fn reconstruct_adjacent_merge_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    target: BlockId,
) -> bool {
    reconstruct_adjacent_merge_ownership_witness(unit, function, incoming, target).is_some()
}

pub(crate) fn reconstruct_adjacent_merge_ownership_witness(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    target: BlockId,
) -> Option<OwnershipFrontierWitness> {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| {
        unit.ownership_frontier_facts
            .iter()
            .find(|fact| fact.machine == function.machine && fact.site == site)
    });
    if facts.iter().all(Option::is_none) {
        return (function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty())
        .then_some(OwnershipFrontierWitness { rows: Vec::new() });
    }
    if !facts.iter().all(Option::is_some)
        || !facts
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

pub(crate) fn reconstruct_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    patch: AdjacentBlockMergeRewrite,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let predecessor_position = function.block_index(patch.predecessor.block)?;
    let target_position = function.block_index(patch.target)?;
    if target_position != predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor = &function.blocks[predecessor_position];
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node.edge(patch.incoming_edge)?;
    let target = &function.blocks[target_position];
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let mut affected = BTreeSet::from([predecessor.id, target.id]);
    let first = target.nodes.first()?;
    let mut realized = if first.successors.is_empty() {
        vec![provenance_realized_at(
            incoming_site,
            PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: predecessor.id,
                node: patch.predecessor.node,
            }),
            &incoming.provenance,
            &incoming.fuel,
        )]
    } else {
        first
            .successors
            .iter()
            .map(|successor| {
                provenance_realized_at(
                    incoming_site,
                    PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: successor.psi_edge,
                    },
                    &incoming.provenance,
                    &incoming.fuel,
                )
            })
            .collect()
    };
    for (node_index, node) in target.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target.id,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.id,
            node: patch
                .predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(provenance_realized_at(
            input,
            output,
            &node.provenance,
            &node.fuel,
        ));
    }
    for block in function.blocks.iter().skip(target_position + 1) {
        affected.insert(block.id);
        realize_node_provenance(
            function.machine,
            block.id,
            block.nodes.iter().enumerate(),
            &mut realized,
        )?;
    }
    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        if affected.contains(&block.id) {
            continue;
        }
        let changed_nodes = block
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.uses
                    .iter()
                    .any(|row| substituted_values.contains(&row.value))
            })
            .collect::<Vec<_>>();
        if changed_nodes.is_empty() {
            continue;
        }
        affected.insert(block.id);
        realize_node_provenance(
            function.machine,
            block.id,
            changed_nodes.into_iter(),
            &mut realized,
        )?;
    }
    canonicalize_provenance(&mut realized);
    Some((affected.into_iter().collect(), realized))
}
