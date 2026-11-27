mod selected_exports;

use crate::RetainedTerminalArtifact;
use build_providers::plans::ProviderPlan;
use calling_conventions::BoundaryCallingApplication;
use component_description::VerifiedComponent;
use std::sync::Arc;
use terminal_psi::ProviderCandidateConformance;

#[derive(Debug)]
pub struct AdmittedNativeComponentCall<'input> {
    selected: psi_provider_selections::AdmittedComponentProvider,
    component: &'input VerifiedComponent,
    application: &'input BoundaryCallingApplication,
    plan_digest: build_providers::plans::ProviderPlanDigest,
}

impl<'input> AdmittedNativeComponentCall<'input> {
    pub const fn facts(&self) -> provider_admission::ComponentCallFacts<'_> {
        provider_admission::ComponentCallFacts {
            selection: &self.selected,
            component: self.component,
            application: self.application,
            plan_digest: &self.plan_digest,
        }
    }

    pub const fn selection(&self) -> &psi_provider_selections::AdmittedComponentProvider {
        &self.selected
    }
    pub const fn component(&self) -> &'input VerifiedComponent {
        self.component
    }
    pub const fn application(&self) -> &'input BoundaryCallingApplication {
        self.application
    }
    pub const fn plan_digest(&self) -> &build_providers::plans::ProviderPlanDigest {
        &self.plan_digest
    }
}

/// Exact cross-artifact calls for a whole selected service. Machine and boundary
/// coordinates belong to their respective artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentImport {
    receiver: terminal_psi::TerminalPsiIdentity,
    plan: build_providers::plans::ProviderPlanDigest,
    component: Arc<VerifiedComponent>,
    calls: Vec<psi_provider_selections::AdmittedComponentProvider>,
}

impl ComponentImport {
    pub fn admit(
        receiver: terminal_codec::TerminalArtifactView<'_>,
        component: Arc<VerifiedComponent>,
        plan: &ProviderPlan,
    ) -> Result<Self, String> {
        let candidates = selected_exports::selected_plan_export_positions(&component, plan)
            .map_err(|error| error.to_string())?;
        let calls = candidates
            .iter()
            .map(|position| {
                admit_export(
                    receiver,
                    &component,
                    &component.module().interfaces.provider_candidates[*position],
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            receiver: receiver.identity(),
            plan: plan.identity_digest(),
            component,
            calls,
        })
    }

    pub fn plan(&self) -> build_providers::plans::ProviderPlanDigest {
        self.plan
    }
    pub fn matches(
        &self,
        receiver: terminal_psi::TerminalPsiIdentity,
        plan: &ProviderPlan,
    ) -> bool {
        self.receiver == receiver && self.plan == plan.identity_digest()
    }
    pub fn component(&self) -> &Arc<VerifiedComponent> {
        &self.component
    }
    pub fn calls(&self) -> &[psi_provider_selections::AdmittedComponentProvider] {
        &self.calls
    }
}

/// Require the exact reached Independent roster before suppressing any local
/// adapter. An authenticated import from a different receiver cannot discharge it.
pub(super) fn local_adapters(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    profile: target::TargetProfile,
    selected: &build_providers::selections::SelectedProviderPlanFacts,
    imports: &[ComponentImport],
) -> Result<target_bound_program::SelectedProviderAdapters, String> {
    let module = artifact.module();
    let mut required = Vec::new();
    for choice in &module.interfaces.provider_selections {
        if choice.target != profile.target_name()
            || choice.composition_mode
                != semantic_vocabulary::source_selections::CompositionMode::Independent
        {
            continue;
        }
        let schema = module.service_schema(choice.schema_declaration, choice.schema_application)?;
        for plan in selected.plans() {
            if plan.target != choice.target
                || plan.schema.declaration.as_ref() != schema.surface.as_ref()
                || plan.provider_type_package_identity != choice.provider_package
                || Some(plan.provider_type.as_str())
                    != module.identities.text(choice.provider_identity)
                || plan.provider_declaration.as_deref()
                    != module.identities.text(choice.provider_declaration)
                || plan.application_identity()
                    != choice
                        .provider_application
                        .and_then(|index| module.identities.text(index))
            {
                continue;
            }
            let matching = imports
                .iter()
                .filter(|import| import.matches(artifact.identity(), plan))
                .count();
            if matching != 1 {
                return Err(
                    "independent native provider requires one exact receiving import".into(),
                );
            }
            required.push(plan.identity_digest());
        }
    }
    if required.len() != imports.len()
        || imports
            .iter()
            .any(|import| !required.contains(&import.plan()))
    {
        return Err(
            "native component import roster differs from the selected Independent occurrences"
                .into(),
        );
    }
    Ok(
        target_bound_program::SelectedProviderAdapters::from_selected_plans(selected)?
            .without_imports(&required),
    )
}

fn admit_export(
    receiver: terminal_codec::TerminalArtifactView<'_>,
    component: &VerifiedComponent,
    candidate: &ProviderCandidateConformance,
) -> Result<psi_provider_selections::AdmittedComponentProvider, String> {
    let module = component.module();
    let abi = module.calling_abi_identity(candidate.boundary)?;
    let requirement = module
        .identities
        .text(candidate.requirement_identity)
        .ok_or("component export lost its exact invocation identity")?;
    let mut receiving = receiver
        .module()
        .interfaces
        .boundary_machines
        .iter()
        .filter(|boundary| {
            receiver.module().identities.text(boundary.identity) == Some(requirement)
                && receiver.module().calling_abi_identity(boundary.id).as_ref() == Ok(&abi)
        });
    let boundary = receiving.next().ok_or_else(|| {
        format!("component import `{requirement}` has no receiving Terminal declaration",)
    })?;
    if receiving.next().is_some() {
        return Err(format!(
            "component import `{requirement}` has competing Terminal declarations"
        ));
    }

    let selected = psi_provider_selections::ProviderInstallationSelection {
        boundary: candidate.boundary,
        provider_identity: module
            .identities
            .text(candidate.provider_identity)
            .ok_or("component export lost its provider identity")?
            .to_owned(),
        provider_declaration: module
            .identities
            .text(candidate.provider_declaration.identity())
            .ok_or("component export lost its selecting declaration")?
            .to_owned(),
        candidate: candidate.candidate,
    };
    let admitted = psi_provider_selections::admit_component_provider(
        receiver,
        boundary.id,
        component.artifact().view(),
        &selected,
    )?;
    if !admitted.selection().contains_candidate(candidate) {
        return Err("component export admission changed its exact checked candidate".into());
    }
    Ok(admitted)
}

impl ComponentImport {
    pub fn native_call<'input>(
        &'input self,
        index: usize,
        producer: &'input RetainedTerminalArtifact,
        plan: &ProviderPlan,
        receiving_applications: &[BoundaryCallingApplication],
    ) -> Result<AdmittedNativeComponentCall<'input>, String> {
        let selected = self
            .calls
            .get(index)
            .ok_or("component has no selected export at this position")?;
        let proposal = producer.native_realization_proposal();
        if self.plan != plan.identity_digest()
            || self.component.artifact() != producer.artifact()
            || proposal.target_profile().target_name() != plan.target
            || !matches!(proposal.scope_source().root(), target_bound_program::NativeRoot::Component { export, .. } if export == selected)
        {
            return Err(
                "component export, target or selection differs from its admitted import".into(),
            );
        }
        let abi = self
            .component
            .module()
            .calling_abi_identity(selected.provider_boundary())?;
        let expected = plan.schema.calling_application(&abi);
        let target = proposal.native_target();
        let producer_application = one_application(
            &proposal.boundary_applications().calling,
            &abi,
            expected,
            target,
            "component export",
        )?;
        let receiver_application = one_application(
            receiving_applications,
            &abi,
            expected,
            target,
            "component import",
        )?;
        if producer_application != receiver_application
            || producer_application.dispatch_only_formal.is_some()
            || producer_application.recomputed_commitment()? != producer_application.commitment
        {
            return Err(format!(
                "component import/export calling applications differ for {}",
                abi.requirement_identity
            ));
        }
        Ok(AdmittedNativeComponentCall {
            selected: selected.clone(),
            component: &self.component,
            application: producer_application,
            plan_digest: self.plan,
        })
    }
}

fn one_application<'input>(
    applications: &'input [BoundaryCallingApplication],
    abi: &semantic_vocabulary::CallingAbiIdentity,
    expected: Option<&build_providers::plans::ServiceCallingPlan>,
    target: target::NativeTarget,
    role: &str,
) -> Result<&'input BoundaryCallingApplication, String> {
    let mut matching = applications.iter().filter(|row| {
        &row.abi == abi
            && row.target == target
            && expected.map_or_else(
                || {
                    row.plan.plan().call.policy
                        == calling_conventions::CallingPolicy::native_for_target(target)
                },
                |expected| row.commitment == expected.commitment.as_bytes(),
            )
    });
    let requirement = &abi.requirement_identity;
    let application = matching
        .next()
        .ok_or_else(|| format!("{role} `{requirement}` has no complete calling application",))?;
    if matching.next().is_some() {
        return Err(format!(
            "{role} `{requirement}` has competing calling applications"
        ));
    }
    if expected
        .is_some_and(|expected| expected.report_fingerprint != application.report_fingerprint)
    {
        return Err(format!(
            "{role} `{requirement}` has a stale calling report coordinate"
        ));
    }
    Ok(application)
}
