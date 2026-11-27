//! Frame the normal return of a provider with no escaping or interior loans.
//! Its borrowed arguments may have changed, while disjoint cells keep their facts.

use super::activation::Bindings;
use super::storage::{Storage, Stored};

use terminal_psi::{
    BoundaryDeclarationRole, BoundaryMachineDeclaration, CheckedBoundaryExecution,
    ModuleInvocationInputs, Operation, OperationResult, TargetMachineCallable, TerminalModule,
};

pub(super) fn normal_return(
    module: &TerminalModule,
    row: &BoundaryMachineDeclaration,
    operation: &Operation,
    call: &ModuleInvocationInputs<'_, TargetMachineCallable>,
    bindings: &Bindings,
    memory: &mut Storage,
    pending_provider: bool,
) -> Option<bool> {
    let contract = row.declaration.as_ref()?;
    if row.role != BoundaryDeclarationRole::Callable
        || contract.execution != CheckedBoundaryExecution::Provider
        || call.required_contract.is_some()
        || call.arguments.len() != row.scalar_parameters.len()
        || call.structural_arguments.len() != row.structural_parameters.len()
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
    let mut loans = Vec::new();
    for (formal, actual) in row
        .structural_parameters
        .iter()
        .zip(call.structural_arguments)
    {
        if formal.access != actual.access
            || terminal_psi::structural_type_has_borrowed_storage_by(
                |id| module.structural_type(id),
                formal.structural_type,
            )
        {
            return None;
        }
        if formal.access != terminal_psi::StructuralAccess::Owned {
            loans.push(memory.locate(bindings, actual.place, &actual.path)?);
        }
    }
    for loan in loans {
        memory.set(
            &loan,
            if pending_provider {
                Stored::PENDING_PROVIDER
            } else {
                Stored::Unknown
            },
        );
    }
    Some(
        row.unconditional_termination
            && row.scalar_requires.is_empty()
            && row.requires.is_empty()
            && row.scalar_domain_requirements.is_empty()
            && row.crash_routes.is_empty()
            && !contract.source.published_may_suspend
            && !contract.source.published_may_block,
    )
}
