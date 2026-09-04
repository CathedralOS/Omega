use super::*;

pub(crate) fn reconstruct_non_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    patch: NonAdjacentBlockMergeRewrite,
    substitutions: &[ScalarSubstitution],
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)?;
    if target_position == predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor = &function.blocks[predecessor_position];
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = &function.blocks[target_position];
    let first = target.nodes.first()?;
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let mut realized = if !first.provenance.is_empty() {
        vec![omega_optimization_unit::ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                patch.predecessor,
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else if !first.successors.is_empty() {
        first
            .successors
            .iter()
            .map(|successor| omega_optimization_unit::ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    } else {
        return None;
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
        realized.push(omega_optimization_unit::ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }

    let mut input_effect = 0u64;
    let mut input_starts = BTreeMap::new();
    for block in &function.blocks {
        input_starts.insert(block.id, input_effect);
        input_effect = input_effect.checked_add(u64::try_from(block.nodes.len()).ok()?)?;
    }
    let mut output_effect = 0u64;
    let mut effect_shifted = BTreeSet::new();
    for block in &function.blocks {
        if block.id == patch.target {
            continue;
        }
        if input_starts.get(&block.id).copied()? != output_effect {
            effect_shifted.insert(block.id);
        }
        let output_nodes = if block.id == patch.predecessor.block {
            block
                .nodes
                .len()
                .checked_sub(1)?
                .checked_add(target.nodes.len())?
        } else {
            block.nodes.len()
        };
        output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
    }

    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([patch.predecessor.block, patch.target]);
    affected.extend(effect_shifted.iter().copied());
    for block in &function.blocks {
        if block.id == patch.target {
            continue;
        }
        let mut changed_uses = BTreeSet::new();
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node
                .uses
                .iter()
                .any(|row| substituted_values.contains(&row.value))
            {
                changed_uses.insert(node_index);
                affected.insert(block.id);
            }
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty()
                || (!effect_shifted.contains(&block.id) && !changed_uses.contains(&node_index))
            {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}
