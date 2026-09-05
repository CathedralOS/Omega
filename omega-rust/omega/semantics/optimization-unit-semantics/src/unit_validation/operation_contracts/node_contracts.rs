use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_node_operation_contracts(
    function: &PsiOptimizationFunction,
    block: &optimization_unit::OptimizationBlock,
    node_index: usize,
    node: &optimization_unit::OptimizationNode,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    if !stored_dynamic_contract_matches(function, block, node_index, &node.operation)
        || !operation_scalar_types_match(
            function,
            &node.operation,
            definitions,
            functions,
            boundary_machines,
        )
    {
        return Err(
            OptimizationUnitValidationError::ScalarOperationContractMismatch {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).expect("unit node index fits u32"),
            },
        );
    }
    if !operation_structural_call_contract_matches(
        function,
        &node.operation,
        functions,
        boundary_machines,
        structural_types,
        structural_domains,
    ) {
        return Err(
            OptimizationUnitValidationError::StructuralCallContractMismatch {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).expect("unit node index fits u32"),
            },
        );
    }
    if !operation_service_contract_matches(
        function,
        &node.operation,
        functions,
        boundary_machines,
        services,
    ) {
        return Err(
            OptimizationUnitValidationError::OperationServiceContractMismatch {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).expect("unit node index fits u32"),
            },
        );
    }
    Ok(())
}

fn stored_dynamic_contract_matches(
    function: &PsiOptimizationFunction,
    block: &optimization_unit::OptimizationBlock,
    node_index: usize,
    operation: &O,
) -> bool {
    let stores =
        function
            .blocks
            .iter()
            .flat_map(|candidate_block| {
                candidate_block.nodes.iter().enumerate().filter_map(
                    move |(candidate_index, node)| match &node.operation {
                        O::StoreDynamicDescriptor {
                            psi_operation,
                            stored,
                        } => Some((candidate_block.id, candidate_index, psi_operation, stored)),
                        _ => None,
                    },
                )
            })
            .collect::<Vec<_>>();
    match operation {
        O::StoreDynamicDescriptor {
            psi_operation,
            stored,
        } => {
            stored.has_complete_custody(function.machine, *psi_operation)
                && stores
                    .iter()
                    .filter(|(_, _, _, candidate)| {
                        candidate.descriptor.ordinal == stored.descriptor.ordinal
                    })
                    .count()
                    == 1
        }
        O::CallStoredDynamicScalar {
            psi_operation,
            dynamic_dispatch,
            ..
        } => {
            dynamic_dispatch.has_complete_custody(function.machine, *psi_operation)
                && matches!(
                    stores
                        .iter()
                        .filter(|(_, _, _, stored)| *stored == &dynamic_dispatch.stored)
                        .collect::<Vec<_>>()
                        .as_slice(),
                    [(store_block, store_index, _, _)]
                        if *store_block == block.id && *store_index < node_index
                )
        }
        _ => true,
    }
}
