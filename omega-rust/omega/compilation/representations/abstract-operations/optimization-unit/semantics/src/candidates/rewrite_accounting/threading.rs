use crate::candidates::{canonicalize_provenance, provenance_realized_at};
use optimization_unit::{
    NodeLocation, OwnershipFrontierSite, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRealizationSite, ValueDefinition,
};
use semantic_vocabulary::{BlockId, EdgeId};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn reconstruct_linear_thread_bindings(
    parameters: &[ValueDefinition],
    incoming: &[abstract_operations::ValueBinding],
    outgoing: &[abstract_operations::ValueBinding],
) -> Option<Vec<abstract_operations::ValueBinding>> {
    if parameters.len() != incoming.len() {
        return None;
    }
    let replacements = parameters
        .iter()
        .zip(incoming)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some((parameter.value, (binding.argument, binding.scalar_type)))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        outgoing
            .iter()
            .map(|binding| {
                replacements
                    .get(&binding.argument)
                    .map_or(*binding, |(argument, scalar_type)| {
                        abstract_operations::ValueBinding {
                            parameter: binding.parameter,
                            argument: *argument,
                            scalar_type: *scalar_type,
                        }
                    })
            })
            .collect(),
    )
}

pub(crate) fn reconstruct_linear_thread_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    empty: BlockId,
    outgoing: EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(empty),
        OwnershipFrontierSite::EdgeEntry(outgoing),
        OwnershipFrontierSite::EdgeExit(outgoing),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| {
        unit.ownership_frontier_facts
            .iter()
            .find(|fact| fact.machine == function.machine && fact.site == site)
    });
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(Option::is_some)
        && facts
            .windows(2)
            .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}

pub(crate) fn reconstruct_linear_thread_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    empty: NodeLocation,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let predecessor_node = function
        .block(predecessor.block)?
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let empty_node = function
        .block(empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let predecessor_edge = predecessor_node.successors.first()?;
    let empty_edge = empty_node.successors.first()?;
    let output_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: predecessor_edge.psi_edge,
    };
    let predecessor_site = output_site;
    let empty_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: empty_edge.psi_edge,
    };
    let mut affected = BTreeSet::from([predecessor.block, empty.block]);
    let mut realized = vec![
        provenance_realized_at(
            predecessor_site,
            output_site,
            &predecessor_edge.provenance,
            &predecessor_edge.fuel,
        ),
        provenance_realized_at(
            empty_site,
            output_site,
            &empty_edge.provenance,
            &empty_edge.fuel,
        ),
    ];
    effect_audit(
        function,
        empty,
        Some(predecessor),
        &mut affected,
        &mut realized,
    )?;
    Some((affected.into_iter().collect(), realized))
}

pub(crate) fn reconstruct_path_thread_accounting(
    function: &PsiOptimizationFunction,
    empty: NodeLocation,
    incoming_edges: &[EdgeId],
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let empty_node = function
        .block(empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let outgoing = empty_node.successors.first()?;
    let outgoing_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: outgoing.psi_edge,
    };
    let incoming_set = incoming_edges.iter().copied().collect::<BTreeSet<_>>();
    if incoming_set.len() != incoming_edges.len() || incoming_set.is_empty() {
        return None;
    }
    let mut affected = BTreeSet::from([empty.block]);
    let mut realized = Vec::new();
    for block in &function.blocks {
        for node in &block.nodes {
            for edge in &node.successors {
                if !incoming_set.contains(&edge.psi_edge) || edge.target != empty.block {
                    continue;
                }
                affected.insert(block.id);
                let site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: edge.psi_edge,
                };
                realized.push(provenance_realized_at(
                    site,
                    site,
                    &edge.provenance,
                    &edge.fuel,
                ));
                realized.push(provenance_realized_at(
                    outgoing_site,
                    site,
                    &outgoing.provenance,
                    &outgoing.fuel,
                ));
            }
        }
    }
    if realized.len() != incoming_edges.len().checked_mul(2)? {
        return None;
    }
    effect_audit(function, empty, None, &mut affected, &mut realized)?;
    Some((affected.into_iter().collect(), realized))
}

/// The effect-counter audit both thread reconstructions share: expected
/// input/output walk one count per node outside `empty`'s block, and a node
/// whose recorded effects break that chain — other than an optionally
/// excused location — marks its block affected and, when it carries
/// provenance, realizes its own site. Rows leave sorted in canonical order.
fn effect_audit(
    function: &PsiOptimizationFunction,
    empty: NodeLocation,
    except: Option<NodeLocation>,
    affected: &mut BTreeSet<BlockId>,
    realized: &mut Vec<optimization_unit::ProvenanceRewrite>,
) -> Option<()> {
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && except != Some(location) {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(provenance_realized_at(
                        site,
                        site,
                        &node.provenance,
                        &node.fuel,
                    ));
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    canonicalize_provenance(realized);
    Some(())
}
