//! Structural-result routes.

mod direct_call_return;
mod return_value;

pub(super) use return_value::{
    exact_fully_consumed_affine_pair_root, lower_structural_return_function,
    require_direct_structural_fragments,
};

use super::shared::*;

pub(super) fn lower_structural_function(
    function: &AbstractFunction,
    result: &psi_terminal::StructuralResultDeclaration,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetFunction, LoweringError> {
    if let Some(lowered) =
        direct_call_return::lower_direct_return(function, target, functions, structural_types)?
    {
        return Ok(lowered);
    }
    lower_structural_return_function(function, result, target, structural_types)
}
