//! Read call demands from the current form's prepared invocation records.

use machine_emission::StagedOptimizedFunctionFragmentEmissionSource;
use terminal_semantics::callable_values::ReachableCallableInvocations;

/// Borrow the exact entry summary already bound during native preparation.
/// Image emission neither revisits optimized code nor rebuilds registrar roots.
pub(crate) fn prepare_from(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    entry: semantic_vocabulary::MachineId,
) -> Result<&ReachableCallableInvocations, String> {
    if source.physical_program().function(entry).is_none() {
        return Err(format!(
            "entry {} has no current native function",
            entry.get()
        ));
    }
    source
        .physical_program()
        .contracts
        .execution()
        .and_then(|facts| facts.callable_invocations_from(entry))
        .ok_or_else(|| {
            format!(
                "entry {} has no exact prepared callable invocation summary",
                entry.get(),
            )
        })
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
