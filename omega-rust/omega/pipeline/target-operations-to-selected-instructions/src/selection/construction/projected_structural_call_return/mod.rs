//! Optimizer module role: executable entrance. Select one atomic projected structural closure.

mod constraints;
mod projection;
mod transfer;

use crate::selection::shared::*;

pub(super) fn select(
    source: &LegalizedProjectedStructuralCallReturn,
    legalized_plan: LegalizedOperationPlanIdentity,
    selection: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedProjectedStructuralCallReturn, SelectedInstructionError> {
    let (projected_qualifications, fragments) = projection::project(source)?;
    let call = constraints::call(&fragments, selection, physical, catalog)?;
    let caller_return = constraints::return_constraint(
        &fragments,
        SelectedStructuralFragmentSite::CallerFunctionResult,
        selection.keys.return_i64,
        physical,
        catalog,
    )?;
    let callee_return = constraints::return_constraint(
        &fragments,
        SelectedStructuralFragmentSite::CalleeFunctionResult,
        selection.keys.return_i64,
        physical,
        catalog,
    )?;
    let caller_argument_transfer = transfer::project(
        &fragments,
        SelectedStructuralFragmentSite::CallerArgumentSource,
        SelectedStructuralFragmentSite::CallerArgumentDestination,
        selection.keys.copy_i64,
        physical,
        catalog,
    )?;
    let callee_return_transfer = transfer::project(
        &fragments,
        SelectedStructuralFragmentSite::CalleeReturnSource,
        SelectedStructuralFragmentSite::CalleeFunctionResult,
        selection.keys.copy_i64,
        physical,
        catalog,
    )?;
    let caller_return_transfer = transfer::project(
        &fragments,
        SelectedStructuralFragmentSite::CallerOperationResult,
        SelectedStructuralFragmentSite::CallerFunctionResult,
        selection.keys.copy_i64,
        physical,
        catalog,
    )?;
    Ok(SelectedProjectedStructuralCallReturn {
        recipe: SelectedProjectedStructuralCallReturnRecipe::OwnedLinearIntegerFragmentV1,
        legalized_plan,
        caller: source.caller.machine,
        callee: source.callee.machine,
        projected_qualifications,
        fragments,
        call,
        caller_return,
        callee_return,
        caller_argument_transfer,
        callee_return_transfer,
        caller_return_transfer,
    })
}
