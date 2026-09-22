//! Derived-metadata restamping for a transformed function.
//!
//! Optimizer applications mutate operations and node custody; every field a
//! unit derives from operation shape must then be rebuilt or the unit no
//! longer validates. This restamp is the producer-side derivation: the
//! independent validator recomputes each field from the operation list
//! itself and compares, so a transform that drifts from the contract fails
//! validation rather than silently carrying stale metadata.

use super::super::{
    BTreeSet, EffectLink, PsiOptimizationFunction, ValueDefinition, ValueDefinitionSite, ValueUse,
};
use super::OptimizationUnitBuildError;
use super::control_flow::operation_edges;
use super::facts::collect_fact;
use super::scalar_dataflow::{operation_definition, operation_uses};
use super::structural_custody::{collect_places, operation_ownership};

/// Rebuilds every derived-metadata field of `function` in place from its
/// operations: each node's definitions, uses, successor edges (provenance
/// and fuel custody preserved by edge identity), ownership events, and
/// sequential effect links; then the function's fact index and declared
/// place set. Provenance and fuel custody on the nodes themselves is
/// transform custody — it is kept, not rederived.
pub fn restamp_psi_function_derived_metadata(
    function: &mut PsiOptimizationFunction,
) -> Result<(), OptimizationUnitBuildError> {
    let machine = function.machine;
    let mut declared_places = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| block.structural_parameters.iter()),
        )
        .map(|parameter| parameter.place)
        .chain(
            function
                .entry_claim_declarations
                .iter()
                .map(|claim| claim.input),
        )
        .chain(function.result.structural().map(|result| result.place))
        .collect::<BTreeSet<_>>();
    let mut facts = Vec::new();
    let mut effect_token = 0u64;
    for block in &mut function.blocks {
        for (local_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(local_index)
                .map_err(|_| OptimizationUnitBuildError::NodeIndexOverflow(machine))?;
            node.definitions = operation_definition(&node.operation)
                .into_iter()
                .map(|(value, scalar_type)| ValueDefinition {
                    value,
                    scalar_type,
                    site: ValueDefinitionSite::Node {
                        block: block.id,
                        node: node_index,
                    },
                })
                .collect();
            node.uses = operation_uses(&node.operation)
                .into_iter()
                .map(|value| ValueUse {
                    value,
                    block: block.id,
                    node: node_index,
                })
                .collect();
            // Rebuilt edges keep the custody their predecessors carried: a
            // fused or enriched edge keeps its provenance roster and fuel
            // settlement, matched by its Psi edge identity.
            let mut successors = operation_edges(&node.operation);
            for edge in &mut successors {
                if let Some(existing) = node
                    .successors
                    .iter()
                    .find(|existing| existing.psi_edge == edge.psi_edge)
                {
                    edge.provenance = existing.provenance.clone();
                    edge.fuel = existing.fuel.clone();
                }
            }
            node.successors = successors;
            node.ownership = operation_ownership(&node.operation);
            node.effect = EffectLink {
                input: effect_token,
                output: effect_token + 1,
            };
            effect_token += 1;
            collect_places(&node.operation, &mut declared_places);
            collect_fact(&node.operation, &mut facts);
        }
    }
    function.facts = facts;
    function.declared_places = declared_places;
    Ok(())
}
