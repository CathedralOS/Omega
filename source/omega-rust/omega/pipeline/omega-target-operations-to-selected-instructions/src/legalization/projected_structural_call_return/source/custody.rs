//! Source optimization-node custody retained by the identity legalization.

use omega_legalized_operations::LegalizedStructuralNodeCustody;
use omega_optimization_unit::PsiOptimizationFunction;

pub(super) fn node_custody(
    function: &PsiOptimizationFunction,
) -> Vec<LegalizedStructuralNodeCustody> {
    function.blocks[0]
        .nodes
        .iter()
        .map(|node| LegalizedStructuralNodeCustody {
            fuel: node.fuel.clone(),
            effect: node.effect,
            ownership: node.ownership.clone(),
        })
        .collect()
}
