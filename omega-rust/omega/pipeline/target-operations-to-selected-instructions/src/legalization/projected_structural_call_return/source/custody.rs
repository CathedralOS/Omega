//! Source optimization-node custody retained by the identity legalization.

use legalized_operations::LegalizedStructuralNodeCustody;
use optimization_unit::PsiOptimizationFunction;

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
