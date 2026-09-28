//! Optimizer module role: accounting leaf. The block region and node custody a plan declares.
//!
//! A published candidate names the blocks it touches and the exact source
//! custody each affected node carries after the rewrite. The rule derives
//! both from the input function here; the independent validator recomputes
//! the same accounting from the candidate's rows and rejects a declaration
//! that disagrees.

use super::{
    FieldValueResolution, FieldValueSpecializationRewrite, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationFunction, PsiRealizationSite,
};
use semantic_vocabulary::BlockId;
use std::collections::{BTreeMap, BTreeSet};

/// The exact block region and node custody a plan's rows carry, computed from
/// the input function's coordinates. A `Constant` row folds its observation
/// in place: the read's own provenance and fuel stay realized at the same
/// node. A `Forward` row retires its observation node: the read's custody
/// lands at the node that inherits the vacated index — the next surviving
/// input node — and every later node in the block shifts down one coordinate
/// per earlier retirement. Because effects are a function-wide sequence,
/// every block at or after the earliest retired-node block is inside the
/// region, and every provenance-bearing node in a region block whose custody
/// moved — or whose own operation changed — gets one ledger row. `None` only
/// when a node coordinate does not fit the ledger's `u32` index.
pub(crate) fn plan_accounting(
    input_function: &PsiOptimizationFunction,
    plan: &FieldValueSpecializationRewrite,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let machine = plan.machine;
    let mut removals: BTreeMap<BlockId, BTreeSet<u32>> = BTreeMap::new();
    let mut fold_sites: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    let mut use_sites: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    for row in &plan.reads {
        match &row.resolution {
            FieldValueResolution::Constant(_) => {
                fold_sites.insert((row.site.block, row.site.node));
            }
            FieldValueResolution::Forward(forwarded) => {
                removals
                    .entry(row.site.block)
                    .or_default()
                    .insert(row.site.node);
                for site in &forwarded.uses {
                    use_sites.insert((site.block, site.node));
                }
            }
        }
    }
    let earliest_removal = input_function
        .blocks
        .iter()
        .position(|block| removals.contains_key(&block.id));
    let mut affected = BTreeSet::new();
    let mut provenance = Vec::new();
    for (block_position, block) in input_function.blocks.iter().enumerate() {
        let removal = removals.get(&block.id);
        let first_removed = removal.and_then(|set| set.iter().next().copied());
        let in_suffix = earliest_removal.is_some_and(|position| block_position > position);
        let mut block_affected = removal.is_some() || in_suffix;
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).ok()?;
            let coordinate = (block.id, node_index);
            if fold_sites.contains(&coordinate) || use_sites.contains(&coordinate) {
                block_affected = true;
            }
            let retired = removal.is_some_and(|set| set.contains(&node_index));
            let shifted = first_removed.is_some_and(|first| node_index > first);
            if !retired
                && !shifted
                && !in_suffix
                && !fold_sites.contains(&coordinate)
                && !use_sites.contains(&coordinate)
            {
                continue;
            }
            if !retired && node.provenance.is_empty() {
                continue;
            }
            let input_site = PsiRealizationSite::Node(NodeLocation {
                machine,
                block: block.id,
                node: node_index,
            });
            let earlier_removals = removal
                .map(|set| set.iter().filter(|removed| **removed < node_index).count())
                .unwrap_or(0);
            let output_index = node_index.checked_sub(u32::try_from(earlier_removals).ok()?)?;
            let output_site = PsiRealizationSite::Node(NodeLocation {
                machine,
                block: block.id,
                node: output_index,
            });
            provenance.push(ProvenanceRewrite {
                input: input_site,
                disposition: ProvenanceDisposition::RealizedAt(output_site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
        if block_affected {
            affected.insert(block.id);
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
