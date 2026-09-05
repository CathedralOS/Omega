//! Optimizer module role: executable entrance. Scalar-operation coordination shared by straight-line and conditional lowering.
mod direct;
mod integer_binary;
mod integer_operation;
mod shift;
use super::*;
pub(super) use integer_binary::{IntegerBinaryKind, lower_conditional_integer_binary};
pub(super) use shift::{
    WrappingShiftKind, lower_exact_shift_left, lower_exact_shift_right, lower_wrapping_shift,
};
/// Route non-control scalar work through the direct scalar families first,
/// then through the complete integer-operation family.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_conditional_scalar_operation(
    operation: &AbstractOperation,
    machine: MachineId,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<semantic_vocabulary::OperationId>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_parameters: &[TargetStructuralParameter],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<bool, LoweringError> {
    if direct::try_lower_direct_scalar(
        operation,
        machine,
        values,
        provenance,
        target,
        functions,
        structural_parameters,
        structural_types,
    )? {
        return Ok(true);
    }
    integer_operation::try_lower_integer_operation(operation, values, provenance)
}
