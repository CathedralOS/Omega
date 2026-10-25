//! Provenance accounting for common-subexpression elimination across join parameters.

use std::collections::BTreeSet;

use crate::analyses::block_dominates;
use crate::rules::control_flow_cleanup::provenance_realized_at;
use optimization_unit::{
    NodeLocation, PhiTranslatedScalarIncoming, ProvenanceRewrite, PsiOptimizationFunction,
    PsiRealizationSite,
};
use semantic_vocabulary::{BlockId, OperationId, ScalarType, ValueId};

pub(super) fn phi_translated_cse_accounting(
    function: &PsiOptimizationFunction,
    redundant: NodeLocation,
    incoming: &[PhiTranslatedScalarIncoming],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let block_position = function.block_index(redundant.block)?;
    let node_position = usize::try_from(redundant.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut affected = incoming
        .iter()
        .map(|row| row.source)
        .chain([block.id])
        .collect::<BTreeSet<_>>();
    let mut provenance = vec![provenance_realized_at(
        PsiRealizationSite::Node(redundant),
        PsiRealizationSite::Node(redundant),
        &removed.provenance,
        &removed.fuel,
    )];
    for row in incoming {
        let source = function.block(row.source)?;
        let edge = source
            .successors()
            .find(|edge| edge.psi_edge == row.edge && edge.target == redundant.block)?;
        if !edge.provenance.is_empty() {
            let site = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: edge.psi_edge,
            };
            provenance.push(provenance_realized_at(
                site,
                site,
                &edge.provenance,
                &edge.fuel,
            ));
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
        provenance.push(provenance_realized_at(
            PsiRealizationSite::Node(old),
            PsiRealizationSite::Node(new),
            &node.provenance,
            &node.fuel,
        ));
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
            provenance.push(provenance_realized_at(
                site,
                site,
                &node.provenance,
                &node.fuel,
            ));
        }
    }
    provenance.sort_by_key(ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), provenance))
}

use std::collections::BTreeMap;

/// Eligibility + incoming-leader collection shared by the phi-translated
/// scalar rules: the redundant site must sit in a join block whose parameters
/// the key references, whose result is used, and whose provenance does not
/// leak into the receiver; then every incoming edge must translate the key to
/// an existing in-scope leader or the candidate is incomplete. `None` plays
/// the per-redundant `continue`.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_phi_translated_incoming<K>(
    function: &PsiOptimizationFunction,
    redundant: NodeLocation,
    redundant_result: ValueId,
    scalar_type: ScalarType,
    use_definitions: &crate::UseDefinitionAnalysis,
    value_types: &BTreeMap<ValueId, ScalarType>,
    machine_dominators: &[(BlockId, Vec<BlockId>)],
    leaders: &[(K, NodeLocation, OperationId, ValueId, ScalarType)],
    key_references_parameters: impl Fn(&BTreeSet<ValueId>) -> bool,
    translate: impl Fn(&BTreeMap<ValueId, ValueId>) -> Option<K>,
) -> Option<(
    Vec<PhiTranslatedScalarIncoming>,
    Vec<BlockId>,
    Vec<ProvenanceRewrite>,
    u32,
)>
where
    K: Copy + PartialEq,
{
    let block = function.block(redundant.block)?;
    let parameter_values = block
        .parameters
        .iter()
        .map(|parameter| parameter.value)
        .collect::<BTreeSet<_>>();
    if parameter_values.is_empty()
        || !key_references_parameters(&parameter_values)
        || !use_definitions.uses.iter().any(|(machine, use_site)| {
            *machine == function.machine && use_site.value == redundant_result
        })
    {
        return None;
    }
    let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
    let redundant_node = block.nodes.get(redundant_index)?;
    let receiver = block.nodes.get(redundant_index + 1)?;
    if receiver
        .provenance
        .iter()
        .any(|source| redundant_node.provenance.contains(source))
    {
        return None;
    }

    let mut incoming = Vec::new();
    let mut complete = true;
    for source in &function.blocks {
        for (owner_index, owner) in source.nodes.iter().enumerate() {
            let owner_index = u32::try_from(owner_index).expect("optimization node index fits u32");
            for edge in owner
                .successors
                .iter()
                .filter(|edge| edge.target == block.id)
            {
                if edge.bindings.len() != block.parameters.len() {
                    complete = false;
                    continue;
                }
                let mut translation = BTreeMap::new();
                for (parameter, binding) in block.parameters.iter().zip(&edge.bindings) {
                    if binding.parameter != parameter.value
                        || binding.scalar_type != parameter.scalar_type
                        || value_types.get(&binding.argument) != Some(&binding.scalar_type)
                    {
                        complete = false;
                        break;
                    }
                    translation.insert(parameter.value, binding.argument);
                }
                if !complete {
                    continue;
                }
                let Some(translated_key) = translate(&translation) else {
                    complete = false;
                    continue;
                };
                let leader = leaders
                    .iter()
                    .filter(|(candidate_key, location, _, _, candidate_type)| {
                        candidate_key == &translated_key
                            && *candidate_type == scalar_type
                            && ((location.block == source.id && location.node < owner_index)
                                || (location.block != source.id
                                    && block_dominates(
                                        machine_dominators,
                                        location.block,
                                        source.id,
                                    )))
                    })
                    .min_by_key(|(_, location, _, _, _)| {
                        let depth = machine_dominators
                            .iter()
                            .find(|(candidate, _)| *candidate == location.block)
                            .map_or(usize::MAX, |(_, rows)| rows.len());
                        (depth, *location)
                    });
                let Some((_, leader, leader_operation, leader_result, _)) = leader else {
                    complete = false;
                    continue;
                };
                incoming.push(PhiTranslatedScalarIncoming {
                    source: source.id,
                    edge: edge.psi_edge,
                    leader: *leader,
                    leader_operation: *leader_operation,
                    leader_result: *leader_result,
                });
            }
        }
    }
    if !complete || incoming.len() < 2 {
        return None;
    }
    incoming.sort_by_key(|row| (row.edge, row.source));
    let (affected_blocks, provenance) =
        phi_translated_cse_accounting(function, redundant, &incoming)?;
    let parameter_position =
        u32::try_from(block.parameters.len()).expect("optimization block parameter count fits u32");
    Some((incoming, affected_blocks, provenance, parameter_position))
}
