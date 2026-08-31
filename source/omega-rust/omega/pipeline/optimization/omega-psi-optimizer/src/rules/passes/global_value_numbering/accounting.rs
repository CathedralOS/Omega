//! Shared effect checks and provenance accounting for GVN rewrites.

use super::*;

pub(super) fn exact_pure_scalar_effect(
    unit: &PsiOptimizationUnit,
    effects: &crate::EffectSummaryAnalysis,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> bool {
    effects.nodes.iter().any(|row| {
        row.revision == unit.identity
            && row.machine == machine
            && row.block == block
            && row.node == node
            && row.class == crate::EffectClass::PureScalar
            && row.observable == crate::EffectKnowledge::No
            && row.structural_state == crate::EffectKnowledge::No
            && row.crash == crate::EffectKnowledge::No
            && row.suspension == crate::EffectKnowledge::No
    })
}

pub(super) fn phi_translated_cse_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    redundant: NodeLocation,
    incoming: &[PhiTranslatedScalarIncoming],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == redundant.block)?;
    let node_position = usize::try_from(redundant.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut affected = incoming
        .iter()
        .map(|row| row.source)
        .chain([block.id])
        .collect::<BTreeSet<_>>();
    let mut provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(redundant),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(redundant)),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for row in incoming {
        let source = function
            .blocks
            .iter()
            .find(|block| block.id == row.source)?;
        let edge = source
            .nodes
            .iter()
            .flat_map(|node| &node.successors)
            .find(|edge| edge.psi_edge == row.edge && edge.target == redundant.block)?;
        if !edge.provenance.is_empty() {
            let site = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: edge.psi_edge,
            };
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: edge.provenance.clone(),
                fuel: edge.fuel.clone(),
            });
        }
    }
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    for later in function.blocks.iter().skip(block_position + 1) {
        affected.insert(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), provenance))
}
