//! Independent optimizer-node and function custody comparison.

use omega_legalized_operations::LegalizedStructuralNodeCustody;
use omega_optimization_unit::PsiOptimizationFunction;

pub(super) fn nodes_match(
    proposed: &[LegalizedStructuralNodeCustody],
    unit: &PsiOptimizationFunction,
) -> bool {
    let [block] = unit.blocks.as_slice() else {
        return false;
    };
    proposed.len() == block.nodes.len()
        && proposed.iter().zip(&block.nodes).all(|(proposed, node)| {
            proposed.fuel == node.fuel
                && proposed.effect == node.effect
                && proposed.ownership == node.ownership
        })
}

pub(super) fn function_matches(
    unit: &PsiOptimizationFunction,
    source: &omega_abstract_operations::AbstractFunction,
    count: usize,
) -> bool {
    let [block] = unit.blocks.as_slice() else {
        return false;
    };
    unit.machine == source.machine
        && unit.entry == source.entry
        && unit.structural_parameters == source.structural_parameters
        && unit.result == source.result
        && unit.entry_claim_declarations == source.entry_claims
        && unit.published_service_ceiling == source.published_service_ceiling
        && block.id == source.entry
        && block.parameters.is_empty()
        && block.nodes.len() == count
        && block
            .nodes
            .iter()
            .map(|node| &node.operation)
            .eq(source.operations.iter())
}
