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

use crate::SelectedProviderAdapter;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_installation::{
    AdmittedProviderSelections, AdmittedTargetMachineSelections, SynchronousCallGraph,
};

pub fn retain_selected_provider_adapters(
    selected: &effects::SelectedProviderPlanFacts,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<Vec<SelectedProviderAdapter>, String> {
    backend_plan::require_selected_provider_declarations(selected, declarations)?;
    project_complete_adapters(selected, |plan| {
        let mut matches = declarations
            .iter()
            .filter(|declaration| &declaration.plan == plan);
        let declaration = matches
            .next()
            .ok_or("native selected plan has no declaration custody")?;
        if matches.next().is_some() {
            return Err("native selected plan has competing declaration custody".into());
        }
        declaration
            .provider_declaration
            .clone()
            .filter(|owner| !owner.is_empty())
            .ok_or_else(|| "native checked provider plan has no exact selecting declaration".into())
    })
}

fn project_complete_adapters(
    selected: &effects::SelectedProviderPlanFacts,
    mut declaration: impl FnMut(&ProviderPlan) -> Result<String, String>,
) -> Result<Vec<SelectedProviderAdapter>, String> {
    let mut adapters = Vec::new();
    for plan in selected.plans() {
        let mut owner = None;
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            if plan.provider_type.is_empty()
                || plan.identities.text(row.requirement_identity).is_empty()
                || plan.identity_text(*machine_identity).is_empty()
                || *machine_package_identity != plan.origin_package_identity
            {
                return Err(format!(
                    "selected ProviderPlan `{}` has incomplete checked-adapter custody",
                    plan.name
                ));
            }
            let owner = match &owner {
                Some(owner) => owner,
                None => owner.insert(declaration(plan)?),
            };
            for requirement in selected.selected_row_requirement_identities(
                plan.report_fingerprint(),
                plan.identities.text(row.requirement_identity),
            ) {
                let machine = selected
                    .specialized_requirement_row(requirement)
                    .map_or_else(
                        || plan.identity_text(*machine_identity),
                        |specialization| {
                            selected
                                .identities()
                                .text(specialization.machine_identity)
                                .unwrap_or_default()
                        },
                    );
                if machine.is_empty() {
                    return Err("selected checked realization has an empty machine identity".into());
                }
                adapters.push(SelectedProviderAdapter {
                    provider_declaration: owner.clone(),
                    provider_plan_digest: plan.identity_digest(),
                    selected_requirement_identity: plan
                        .identities
                        .text(row.requirement_identity)
                        .to_owned(),
                    requirement_identity: requirement.to_owned(),
                    provider_identity: plan.provider_type.clone(),
                    machine_identity: machine.to_owned(),
                });
            }
        }
    }
    adapters.sort_by(|left, right| {
        left.requirement_identity
            .cmp(&right.requirement_identity)
            .then_with(|| {
                left.provider_plan_digest
                    .as_bytes()
                    .cmp(right.provider_plan_digest.as_bytes())
            })
    });
    if let Some(duplicate) = adapters.windows(2).find(|rows| {
        rows[0].requirement_identity == rows[1].requirement_identity
            && rows[0].provider_plan_digest == rows[1].provider_plan_digest
    }) {
        return Err(format!(
            "one selected whole provider plan projects repeated checked adapters for exact requirement `{}`",
            duplicate[0].requirement_identity
        ));
    }
    Ok(adapters)
}

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
