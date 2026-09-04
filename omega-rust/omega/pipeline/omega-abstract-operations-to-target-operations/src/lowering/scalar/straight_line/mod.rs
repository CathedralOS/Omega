//! Optimizer module role: executable entrance. Straight-line scalar lowering lifecycle and its exact operation routes.

mod call;
mod exit;
mod integer_arithmetic;
mod integer_bitwise;
mod integer_comparison;
mod integer_conversion;
mod integer_division;
mod integer_shift;
mod operation;
mod structural_scalar_field;

use super::*;

/// Evaluate operations in source order, then seal the single terminal target
/// operation together with the provenance accumulated along that route.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_straight_line(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    mut values: BTreeMap<ValueId, KnownScalar>,
    function_result: AbstractResult,
    call_plan: CallPlan,
    target_structural_parameters: Vec<TargetStructuralParameter>,
) -> Result<TargetFunction, LoweringError> {
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = None;
    let mut structural_scalar_field_stores = Vec::new();
    for (operation_index, abstract_operation) in function.operations.iter().enumerate() {
        if returned.is_some() {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        operation::lower_operation(
            abstract_operation,
            operation_index,
            function,
            target,
            functions,
            structural_types,
            &mut values,
            function_result,
            &call_plan,
            &target_structural_parameters,
            &mut provenance,
            &mut structural_scalar_field_stores,
            &mut returned,
        )?;
    }

    let mut operation = returned.ok_or(LoweringError::FunctionHasNoReturn(function.machine))?;
    if !structural_scalar_field_stores.is_empty() {
        operation = TargetOperation::ScalarReturnAfterStructuralScalarFieldStores {
            stores: structural_scalar_field_stores,
            scalar: Box::new(operation),
            structural_types: structural_types
                .values()
                .map(|declaration| (*declaration).clone())
                .collect(),
            call_plan: call_plan.clone(),
            structural_parameters: target_structural_parameters.clone(),
        };
    }

    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance,
        operation,
    })
}
