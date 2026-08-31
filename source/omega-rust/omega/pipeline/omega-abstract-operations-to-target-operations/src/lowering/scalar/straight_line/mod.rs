//! Optimizer module role: executable entrance. Straight-line scalar lowering lifecycle and its exact operation routes.

mod call;
mod exit;
mod integer_arithmetic;
mod integer_conversion;
mod operation;

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
    for abstract_operation in &function.operations {
        if returned.is_some() {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        operation::lower_operation(
            abstract_operation,
            function,
            target,
            functions,
            structural_types,
            &mut values,
            function_result,
            &call_plan,
            &target_structural_parameters,
            &mut provenance,
            &mut returned,
        )?;
    }

    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: None,
        provenance,
        operation: returned.ok_or(LoweringError::FunctionHasNoReturn(function.machine))?,
    })
}
