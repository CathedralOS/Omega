//! Ordinary scalar-call lowering within the straight-line route.

use super::*;

pub(super) fn lower(
    operation: &AbstractOperation,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::Call {
        psi_operation,
        result,
        scalar_type,
        callee,
        arguments,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("straight-line call lowering receives an ordinary call")
    };
    let value = lower_call(
        *psi_operation,
        *result,
        *scalar_type,
        *callee,
        arguments,
        requirement_obligations,
        crash_continuations,
        values,
        target,
        functions,
    )?;
    insert_value(values, *result, value)?;
    provenance.operations.push(*psi_operation);
    Ok(())
}
