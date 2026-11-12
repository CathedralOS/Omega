use crate::{RetainedTerminalArtifact, TerminalNativeRealizationProposal};
use calling_conventions::BoundaryCallingApplication;
use component_description::VerifiedComponent;
use effects::provider_plan::ProviderPlan;
use std::sync::Arc;
use terminal_psi::ProviderCandidateConformance;

pub struct ComponentExportInput<'input> {
    pub candidate: &'input ProviderCandidateConformance,
    pub calling_application: &'input BoundaryCallingApplication,
}

pub struct ComponentNativeInputs<'input> {
    component: &'input VerifiedComponent,
    proposal: &'input TerminalNativeRealizationProposal,
    exports: Vec<ComponentExportInput<'input>>,
    plan_digest: effects::provider_plan::ProviderPlanDigest,
}

#[derive(Debug)]
pub struct AdmittedNativeComponentCall<'input> {
    selected: terminal_installation::AdmittedComponentProvider,
    component: &'input VerifiedComponent,
    application: &'input BoundaryCallingApplication,
    plan_digest: effects::provider_plan::ProviderPlanDigest,
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

    pub const fn selection(&self) -> &terminal_installation::AdmittedComponentProvider {
        &self.selected
    }
    pub const fn component(&self) -> &'input VerifiedComponent {
        self.component
    }
    pub const fn application(&self) -> &'input BoundaryCallingApplication {
        self.application
    }
    pub const fn plan_digest(&self) -> &effects::provider_plan::ProviderPlanDigest {
        &self.plan_digest
    }

    pub(crate) fn replaces_local_adapter(&self, adapter: &crate::SelectedProviderAdapter) -> bool {
        adapter.provider_plan_digest == self.plan_digest
            && self
                .component
                .module()
                .boundary_machine(self.selected.provider_boundary())
                .and_then(|boundary| self.component.module().identities.text(boundary.identity))
                == Some(adapter.requirement_identity.as_str())
    }
}

impl<'input> ComponentNativeInputs<'input> {
    pub fn admit_native_calls(
        &self,
        receiver: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Vec<AdmittedNativeComponentCall<'input>>, String> {
        let selections = self.admit_receiver(receiver)?;
        if selections.len() != self.exports.len() {
            return Err("component native call admission changed its exact export roster".into());
        }
        Ok(selections
            .into_iter()
            .zip(&self.exports)
            .map(|(selected, export)| AdmittedNativeComponentCall {
                selected,
                component: self.component,
                application: export.calling_application,
                plan_digest: self.plan_digest,
            })
            .collect())
    }

    pub const fn component(&self) -> &'input VerifiedComponent {
        self.component
    }
    pub const fn proposal(&self) -> &'input TerminalNativeRealizationProposal {
        self.proposal
    }
    pub fn exports(&self) -> &[ComponentExportInput<'input>] {
        &self.exports
    }
    pub const fn plan_digest(&self) -> &effects::provider_plan::ProviderPlanDigest {
        &self.plan_digest
    }

    pub fn admit_receiver(
        &self,
        receiver: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Vec<terminal_installation::AdmittedComponentProvider>, String> {
        let module = self.component.module();
        self.exports
            .iter()
            .map(|export| {
                let requirement = module
                    .identities
                    .text(export.candidate.requirement_identity)
                    .ok_or("component export lost its exact invocation identity")?;
                let mut receiving = receiver
                    .module()
                    .boundary_machines
                    .iter()
                    .filter(|boundary| {
                        receiver.module().identities.text(boundary.identity) == Some(requirement)
                            && receiver.module().calling_abi_identity(boundary.id).as_ref()
                                == Ok(&export.calling_application.abi)
                    });
                let boundary = receiving.next().ok_or_else(|| {
                    format!(
                        "component import `{requirement}` has no receiving Terminal declaration",
                    )
                })?;
                if receiving.next().is_some() {
                    return Err(format!(
                        "component import `{requirement}` has competing Terminal declarations"
                    ));
                }
                let candidate = export.candidate;
                let selected = terminal_installation::ProviderInstallationSelection {
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
                let admitted = terminal_installation::admit_component_provider(
                    receiver,
                    boundary.id,
                    self.component.artifact().view(),
                    &selected,
                )?;
                if !admitted.selection().contains_candidate(candidate) {
                    return Err(
                        "component export admission changed its exact checked candidate".into(),
                    );
                }
                Ok(admitted)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedComponentNativeInputs {
    component: Arc<VerifiedComponent>,
    retained: Arc<RetainedTerminalArtifact>,
    exports: Vec<(usize, usize)>,
    plan_digest: effects::provider_plan::ProviderPlanDigest,
}

impl RetainedComponentNativeInputs {
    pub fn borrow(&self) -> Result<ComponentNativeInputs<'_>, String> {
        let proposal = self
            .retained
            .native_realization_proposal()
            .ok_or("retained component lost its native proposal")?;
        let exports = self
            .exports
            .iter()
            .map(|&(candidate_position, application_position)| {
                Ok(ComponentExportInput {
                    candidate: self
                        .component
                        .module()
                        .provider_candidates
                        .get(candidate_position)
                        .ok_or("retained component lost its selected export")?,
                    calling_application: proposal
                        .boundary_applications()
                        .calling
                        .get(application_position)
                        .ok_or("retained component lost its selected calling application")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(ComponentNativeInputs {
            component: &self.component,
            proposal,
            exports,
            plan_digest: self.plan_digest,
        })
    }
    pub fn join(
        component: Arc<VerifiedComponent>,
        retained: Arc<RetainedTerminalArtifact>,
        plan: &ProviderPlan,
        selecting_declaration: &str,
        receiving_applications: &[BoundaryCallingApplication],
    ) -> Result<Self, String> {
        if component.artifact() != retained.artifact() {
            return Err(
                "component verdict and native product retain different canonical artifacts".into(),
            );
        }
        let proposal = retained
            .native_realization_proposal()
            .ok_or("independent component has no completed native realization proposal")?;
        if proposal.target_profile().target_name() != plan.target {
            return Err("component native target differs from the selected provider plan".into());
        }
        let candidates = component
            .selected_plan_export_positions(plan, selecting_declaration)
            .map_err(|error| error.to_string())?;
        let mut exports = Vec::with_capacity(candidates.len());
        for candidate_position in candidates {
            let candidate = &component.module().provider_candidates[candidate_position];
            let requirement = component
                .module()
                .identities
                .text(candidate.requirement_identity)
                .ok_or("component export has no canonical requirement identity")?;
            let abi = component
                .module()
                .calling_abi_identity(candidate.boundary)?;
            let expected = plan.schema.calling_application(&abi);
            let target = proposal.target_profile().native_target();
            let (producer_position, producer) = one_application(
                &proposal.boundary_applications().calling,
                &abi,
                expected,
                target,
                "component export",
            )?;
            let (_, receiver) = one_application(
                receiving_applications,
                &abi,
                expected,
                target,
                "component import",
            )?;
            for application in [producer, receiver] {
                if application.dispatch_only_parameter_count != 0 {
                    return Err(
                        "component calling application cannot omit dispatch-only formals".into(),
                    );
                }
                if application.recomputed_commitment()? != application.commitment {
                    return Err(format!(
                        "component calling application for `{requirement}` has a stale commitment"
                    ));
                }
            }
            if producer != receiver {
                return Err(format!(
                    "component import/export calling applications differ for `{requirement}`"
                ));
            }
            exports.push((candidate_position, producer_position));
        }
        Ok(Self {
            component,
            retained,
            exports,
            plan_digest: plan.identity_digest(),
        })
    }
}

fn one_application<'input>(
    applications: &'input [BoundaryCallingApplication],
    abi: &semantic_vocabulary::CallingAbiIdentity,
    expected: Option<&effects::provider_plan::ServiceCallingPlan>,
    target: target::NativeTarget,
    role: &str,
) -> Result<(usize, &'input BoundaryCallingApplication), String> {
    let mut matching = applications.iter().enumerate().filter(|(_, row)| {
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
        .is_some_and(|expected| expected.report_fingerprint != application.1.report_fingerprint)
    {
        return Err(format!(
            "{role} `{requirement}` has a stale calling report coordinate"
        ));
    }
    Ok(application)
}
