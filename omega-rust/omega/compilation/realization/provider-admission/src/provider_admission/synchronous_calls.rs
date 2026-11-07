//! Join an inspection graph to this receiver's admitted provider choices.
//! These edges are executable calls, not code-retention or authority roots.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_installation::{
    AdmittedProviderSelections, AdmittedTargetMachineSelections, SynchronousCallGraph,
};

/// The program's admitted choices remain available when a private body is
/// realized under a smaller executable provider scope. This context supports
/// inspection of the original entry; it does not install providers in that body.
#[derive(Debug)]
pub struct AdmittedProgramProviderContext {
    psi: terminal_psi::TerminalPsiIdentity,
    profile: target::TargetProfile,
    selected_plans: [u8; 32],
    providers: Option<AdmittedProviderSelections>,
    entry_bridge: Option<(BoundaryMachineId, MachineId)>,
}

impl AdmittedProgramProviderContext {
    pub(crate) fn new(
        psi: terminal_psi::TerminalPsiIdentity,
        profile: target::TargetProfile,
        selected_plans: &effects::SelectedProviderPlanFacts,
        providers: Option<&AdmittedProviderSelections>,
        entry_bridge: Option<(BoundaryMachineId, MachineId)>,
    ) -> Self {
        Self {
            psi,
            profile,
            selected_plans: *selected_plans.identity_digest().as_bytes(),
            providers: providers.cloned(),
            entry_bridge,
        }
    }

    /// Reuse is bound to the same artifact, exact deployment and complete
    /// selected-plan closure. A private invocation still admits its own calls.
    pub fn require(
        &self,
        psi: terminal_psi::TerminalPsiIdentity,
        profile: target::TargetProfile,
        selected_plans: &effects::SelectedProviderPlanFacts,
    ) -> Result<(), String> {
        if self.psi != psi {
            return Err("program provider context names another Terminal artifact".into());
        }
        if self.profile != profile {
            return Err("program provider context names another target profile".into());
        }
        if self.selected_plans != *selected_plans.identity_digest().as_bytes() {
            return Err("program provider context names another selected-plan closure".into());
        }
        Ok(())
    }

    pub const fn providers(&self) -> Option<&AdmittedProviderSelections> {
        self.providers.as_ref()
    }

    pub const fn entry_bridge(&self) -> Option<(BoundaryMachineId, MachineId)> {
        self.entry_bridge
    }
}

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
