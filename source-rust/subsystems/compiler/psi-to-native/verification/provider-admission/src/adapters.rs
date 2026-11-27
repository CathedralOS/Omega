// Selected provider adapter retention and synchronous call-graph construction.
//
// Source custody qualifies whole plans before release, and the selected closure
// carries Terminal-derived specialization and family aliases.
// `retain_selected_provider_adapters` preserves those joins without reopening
// source facts or merging selecting declarations that share a realizing
// machine.
//
// `AdmittedProgramProviderContext` and `selected_synchronous_call_graph` join an
// inspection call graph to this receiver's admitted provider choices so a
// private body realized under a smaller executable provider scope can still
// inspect the original entry without installing providers in that body.

use psi_provider_selections::AdmittedProviderSelections;
use psi_target_layouts::TerminalTargetContext;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
pub use target_bound_program::retain_selected_provider_adapters;
use terminal_psi::TerminalModule;
use terminal_semantics::ModuleAnalyses;
use terminal_semantics::synchronous_calls::SynchronousCallGraph;

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
        selected_plans: &build_providers::selections::SelectedProviderPlanFacts,
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

    pub fn require(
        &self,
        psi: terminal_psi::TerminalPsiIdentity,
        profile: target::TargetProfile,
        selected_plans: &build_providers::selections::SelectedProviderPlanFacts,
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

    pub const fn profile(&self) -> target::TargetProfile {
        self.profile
    }

    pub const fn providers(&self) -> Option<&AdmittedProviderSelections> {
        self.providers.as_ref()
    }

    pub const fn entry_bridge(&self) -> Option<(BoundaryMachineId, MachineId)> {
        self.entry_bridge
    }
}

/// `analyses` are the admitted program's shared analyses; `None` when they
/// could not be built, and the graph then refuses as it always has.
pub fn selected_synchronous_call_graph<'module>(
    artifact: terminal_codec::TerminalArtifactView<'module>,
    analyses: Option<&'module ModuleAnalyses<'_>>,
    target_members: Option<&'module TerminalTargetContext>,
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
        for &(_, machine) in &calls {
            let callable = terminal_psi::TargetMachineCallable::Machine(machine);
            if let Some(members) = target_members {
                members
                    .entry(identity, module, callable)
                    .map_err(|error| format!("synchronous provider body target: {error:?}"))?;
            }
        }
    }
    admission_call_graph(module, analyses, entry_bridge, calls)
}

/// A synchronous call graph that reads the shared analyses' callable values
/// instead of building its own, with the same refusals as
/// `SynchronousCallGraph::new(module, entry_bridge)?.with_boundary_calls(calls)`.
///
/// The entry bridge differs from a supplied boundary body only in winning
/// the lookup for its boundary, so a declared bridge joins the supplied
/// bodies in place of any body supplied for the same boundary. The supplied
/// bodies are checked on their own first, so their refusals stay the same.
/// Without analyses, or with a bridge whose boundary or body the module does
/// not declare, the graph is built alone, so its refusal is the one an
/// unshared graph gives.
pub(crate) fn admission_call_graph<'module>(
    module: &'module TerminalModule,
    analyses: Option<&'module ModuleAnalyses<'_>>,
    entry_bridge: Option<(BoundaryMachineId, MachineId)>,
    calls: Vec<(BoundaryMachineId, MachineId)>,
) -> Result<SynchronousCallGraph<'module>, String> {
    let declared = |(boundary, machine): (BoundaryMachineId, MachineId)| {
        module.boundary_machine(boundary).is_some() && module.machine(machine).is_some()
    };
    let Some(analyses) = analyses.filter(|_| entry_bridge.is_none_or(declared)) else {
        return SynchronousCallGraph::new(module, entry_bridge)?.with_boundary_calls(calls);
    };
    let Some(bridge) = entry_bridge else {
        return analyses.calls().with_boundary_calls(calls);
    };
    analyses
        .calls()
        .with_boundary_calls(calls.iter().copied())?;
    analyses.calls().with_boundary_calls(
        calls
            .into_iter()
            .filter(|(boundary, _)| *boundary != bridge.0)
            .chain([bridge]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use build_providers::plans::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod};

    fn plan() -> ProviderPlan {
        let mut plan = ProviderPlan {
            name: "selected".into(),
            provider_type: "Provider".into(),
            provider_declaration: Some("FirstMap".into()),
            row_numeric_realizations: vec![None],
            target: "windows_x86_64".into(),
            ..Default::default()
        };
        let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
        declaration.trait_name = declaration.identities.intern("Boundary");
        declaration.methods.push(ServiceMethod {
            name: "read".into(),
            requirement_owner: "Boundary".into(),
            requirement_identity: declaration.identities.intern("Boundary::read"),
            ..Default::default()
        });
        let requirement = plan.identities.intern("Boundary::read");
        plan.rows.push(ProviderPlanRow {
            method: "read".into(),
            requirement_identity: requirement,
            declared_requirement_identity: requirement,
            calling_signature: None,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: plan.identities.intern("Shared::read"),
                machine_package_identity: None,
            },
        });
        plan
    }

    #[test]
    fn program_context_rejects_another_selector_of_the_same_implementation() {
        let first_plan = plan();
        let mut other_plan = first_plan.clone();
        other_plan.provider_declaration = Some("SecondMap".into());
        assert_eq!(first_plan.identity_digest(), other_plan.identity_digest());
        let facts = |plan| {
            build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
                .unwrap()
        };
        let first = facts(first_plan);
        let other = facts(other_plan);
        let adapters =
            target_bound_program::SelectedProviderAdapters::from_selected_plans(&first).unwrap();
        adapters.require_selection(&first).unwrap();
        assert_eq!(
            adapters.require_selection(&other).unwrap_err(),
            "physical adapter rows belong to another provider selection",
        );
        for (selected, owner) in [(&first, "FirstMap"), (&other, "SecondMap")] {
            let adapters = retain_selected_provider_adapters(selected).unwrap();
            assert_eq!(adapters.len(), 1);
            assert_eq!(adapters[0].provider_declaration, owner);
            assert_eq!(adapters[0].machine_identity, "Shared::read");
        }
        let psi = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
        };
        let profile = target::TargetProfile::WindowsX64;
        let context = AdmittedProgramProviderContext::new(psi, profile, &first, None, None);
        assert!(context.require(psi, profile, &first).is_ok());
        assert_eq!(
            context.require(psi, profile, &other).unwrap_err(),
            "program provider context names another selected-plan closure"
        );
        let mut missing = plan();
        missing.provider_declaration = None;
        assert_eq!(
            retain_selected_provider_adapters(&facts(missing)).unwrap_err(),
            "native checked provider plan has no exact selecting declaration"
        );
    }
}
