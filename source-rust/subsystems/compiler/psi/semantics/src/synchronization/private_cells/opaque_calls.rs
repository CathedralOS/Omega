//! Continue on a boundary's normal return when no argument or result can
//! carry a loan to the caller's storage. A possible return makes facts
//! downstream conditional until another proof establishes arrival.

use semantic_vocabulary::{BoundaryCompletion, BoundaryMachineId};
use terminal_psi::{
    BoundaryDeclarationRole, CheckedBoundaryExecution, ModuleInvocationInputs, Operation,
    OperationResult, TargetMachineCallable, TerminalModule,
};

pub(super) fn returns_without_storage(
    module: &TerminalModule,
    slot: BoundaryMachineId,
    operation: &Operation,
    call: &ModuleInvocationInputs<'_, TargetMachineCallable>,
) -> Option<bool> {
    let row = module.boundary_machine(slot)?;
    let contract = row.declaration.as_ref()?;
    if row.role != BoundaryDeclarationRole::Callable
        || row.completion != BoundaryCompletion::NormalReturn
        || contract.execution != CheckedBoundaryExecution::Provider
        || call.required_contract.is_some()
        || !call.arguments.is_empty()
        || !call.structural_arguments.is_empty()
        || !row.scalar_parameters.is_empty()
        || !row.structural_parameters.is_empty()
        || !matches!(
            operation.result,
            OperationResult::Unit | OperationResult::Scalar(_)
        )
        || !row.reference_outputs.is_empty()
        || !row.program_local_root_introductions.is_empty()
        || !row.fixed_service_reach.is_empty()
        || !row.published_service_ceiling.is_empty()
        || !row.published_synchronous_entries.is_empty()
    {
        return None;
    }
    Some(
        row.unconditional_termination
            && row.scalar_requires.is_empty()
            && row.crash_routes.is_empty()
            && !contract.source.published_may_suspend
            && !contract.source.published_may_block,
    )
}
