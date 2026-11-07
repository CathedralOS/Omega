//! Join an inspection graph to this receiver's admitted provider choices.
//! These edges are executable calls, not code-retention or authority roots.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_installation::{
    AdmittedProviderSelections, AdmittedTargetMachineSelections, SynchronousCallGraph,
};

/// Read the selected synchronous closure without admitting physical authority.
/// Psi receives only checked coordinates; this Omega owner checks the exact
/// artifact and target before exposing the receiver's selected provider bodies.
pub fn selected_synchronous_call_graph<'module>(
    artifact: terminal_codec::TerminalArtifactView<'module>,
    target_members: Option<&'module AdmittedTargetMachineSelections>,
    entry_bridge: Option<(BoundaryMachineId, MachineId)>,
    installation: Option<&AdmittedProviderSelections>,
    profile: target::TargetProfile,
) -> Result<SynchronousCallGraph<'module>, String> {
    let identity = artifact.identity();
    let module = artifact.module();
    if target_members.is_some_and(|members| {
        members.terminal_psi() != identity || members.target() != profile.target_name()
    }) {
        return Err("synchronous provider graph has another artifact or target selection".into());
    }
    let mut calls = Vec::new();
    if let Some(installation) = installation {
        if installation.terminal_psi() != identity {
            return Err("synchronous provider graph has another provider installation".into());
        }
        calls.extend(installation.providers());
        calls.extend(
            installation
                .assumed_bindings()
                .iter()
                .map(|binding| (binding.required, binding.implementation)),
        );
        for &(_, machine) in &calls {
            let callable = terminal_psi::TargetMachineCallable::Machine(machine);
            if let Some(members) = target_members {
                members
                    .entry(identity, module, callable)
                    .map_err(|error| format!("synchronous provider body target: {error:?}"))?;
            } else if module.target_machine_families.iter().any(|family| {
                family
                    .members
                    .iter()
                    .any(|member| member.callable == callable)
            }) {
                return Err("synchronous provider body needs its exact target selection".into());
            }
        }
    }
    SynchronousCallGraph::new(module, target_members, entry_bridge)?.with_boundary_calls(calls)
}
