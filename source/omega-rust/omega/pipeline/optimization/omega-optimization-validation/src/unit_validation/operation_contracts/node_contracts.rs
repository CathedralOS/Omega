use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_node_operation_contracts(
    function: &PsiOptimizationFunction,
    block: &omega_optimization_unit::OptimizationBlock,
    node_index: usize,
    node: &omega_optimization_unit::OptimizationNode,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    if !operation_scalar_types_match(
        function,
        &node.operation,
        definitions,
        functions,
        boundary_machines,
    ) {
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
