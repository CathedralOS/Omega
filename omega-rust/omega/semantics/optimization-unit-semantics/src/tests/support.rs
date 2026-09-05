use super::*;

pub(crate) fn refresh_identity(unit: &mut PsiOptimizationUnit) {
    unit.identity = recompute_psi_optimization_unit_identity(unit);
}

pub(crate) fn refresh_node_derivatives(
    unit: &mut PsiOptimizationUnit,
    function_index: usize,
    block_index: usize,
    node_index: usize,
) {
    let block = unit.functions[function_index].blocks[block_index].id;
    let node_index = u32::try_from(node_index).expect("test node index fits u32");
    let operation = unit.functions[function_index].blocks[block_index].nodes[node_index as usize]
        .operation
        .clone();
    let node = &mut unit.functions[function_index].blocks[block_index].nodes[node_index as usize];
    node.definitions = expected_definitions(&operation, block, node_index);
    node.uses = expected_uses(&operation, block, node_index);
    node.provenance = expected_provenance(&operation);
    node.successors = expected_edges(&operation);
    node.ownership = expected_ownership(&operation);
    unit.functions[function_index].facts = reconstruct_fact_index(&unit.functions[function_index]);
    refresh_identity(unit);
}

pub(crate) fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero test identity")
}
