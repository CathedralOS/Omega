//! Bind native call demands to the current optimized code values and entries.

use machine_emission::StagedOptimizedFunctionFragmentEmissionSource;
use terminal_semantics::callable_values::ReachableCallableInvocations;

pub(crate) fn prepare(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<ReachableCallableInvocations, String> {
    prepare_from(source, source.optimized_target().target_operations().entry)
}

pub(crate) fn prepare_from(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    entry: semantic_vocabulary::MachineId,
) -> Result<ReachableCallableInvocations, String> {
    let target = source.optimized_target();
    let program = optimization_unit_semantics::callable_values::optimized_callable_values(
        target.optimized().unit(),
    )
    .map_err(str::to_owned)?;
    let roots = source
        .lowering_facts()
        .callable_invocation_roots(target.target_operations())?;
    terminal_semantics::callable_values::reachable_callable_invocations_with_roots(
        &program,
        [entry],
        &roots,
    )
    .map_err(str::to_owned)
}

pub(crate) fn call_site(
    owner: target_operations::CallSiteOwner,
) -> Result<terminal_semantics::callable_values::CallableInvocationSite, String> {
    use semantic_vocabulary::InvocationSite as Invocation;
    use target_operations::CallSiteOwner as Owner;
    use terminal_semantics::callable_values::CallableInvocationSite as Site;
    Ok(match owner {
        Owner::Operation(operation) => Site::Invocation(Invocation::Operation(operation)),
        Owner::InvokeNoNormal(edge) => Site::Invocation(Invocation::InvokeNoNormal(edge)),
        Owner::ExternalCompletion(edge) => Site::Invocation(Invocation::ExternalCompletion(edge)),
        Owner::CleanupAction {
            edge,
            action_ordinal,
        } => Site::CleanupAction {
            edge,
            action_ordinal,
        },
        Owner::Installation { .. } => {
            return Err("installed call cannot replace a callable source occurrence".into());
        }
    })
}
