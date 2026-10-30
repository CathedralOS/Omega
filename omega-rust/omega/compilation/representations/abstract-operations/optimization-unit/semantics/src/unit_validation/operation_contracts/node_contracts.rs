use crate::unit_validation::operation_contracts::scalar_types::operation_scalar_types_match;
use crate::unit_validation::operation_contracts::service_calls::{
    operation_service_contract_matches, operation_structural_call_contract_matches,
};
use crate::{OperationSite, OptimizationUnitValidationError};
use optimization_unit::{PsiOptimizationFunction, ValueDefinition};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use std::collections::BTreeMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_node_operation_contracts(
    unit: &optimization_unit::PsiOptimizationUnit,
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
    call_accesses: &crate::current_ownership::CallAccesses,
) -> Result<(), OptimizationUnitValidationError> {
    if !operation_scalar_types_match(
        unit,
        function,
        &node.operation,
        definitions,
        functions,
        boundary_machines,
    ) {
        return Err(
            OptimizationUnitValidationError::ScalarOperationContractMismatch(OperationSite::new(
                function.machine,
                block.id,
                u32::try_from(node_index).expect("unit node index fits u32"),
            )),
        );
    }
    if !operation_structural_call_contract_matches(
        unit,
        function,
        &node.operation,
        functions,
        boundary_machines,
        structural_types,
        structural_domains,
        call_accesses,
    ) {
        return Err(
            OptimizationUnitValidationError::StructuralCallContractMismatch(OperationSite::new(
                function.machine,
                block.id,
                u32::try_from(node_index).expect("unit node index fits u32"),
            )),
        );
    }
    if !operation_service_contract_matches(
        unit,
        function,
        &node.operation,
        functions,
        boundary_machines,
        services,
    ) {
        return Err(
            OptimizationUnitValidationError::OperationServiceContractMismatch(OperationSite::new(
                function.machine,
                block.id,
                u32::try_from(node_index).expect("unit node index fits u32"),
            )),
        );
    }
    Ok(())
}
