//! Read call demands from the current form's prepared invocation records.

use terminal_semantics::callable_values::ReachableCallableInvocations;

pub(crate) fn prepare_with<'a>(
    calls: &machine_code::call_origins::PhysicalCallOrigins,
    contracts: &'a selected_instructions::SelectedProgramContracts,
    entry: semantic_vocabulary::MachineId,
) -> Result<&'a ReachableCallableInvocations, String> {
    if calls.function(entry).is_none() {
        return Err(format!(
            "entry {} has no current native function",
            entry.get()
        ));
    }
    contracts
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
    owner: target_vocabulary::CallSiteOwner,
) -> Result<terminal_semantics::callable_values::CallableInvocationSite, String> {
    use semantic_vocabulary::InvocationSite as Invocation;
    use target_vocabulary::CallSiteOwner as Owner;
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
