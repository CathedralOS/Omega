//! Exact source-free inputs for separately realized component entries.
//!
//! Descriptive verification and native production have distinct owners. This
//! join borrows the original verified artifact and the completed native proposal
//! retained by package compilation; it cannot reopen source or grant installed
//! entry authority. Export machine IDs always remain beside their artifact.

use super::{RetainedTerminalArtifact, TerminalNativeRealizationProposal};
use calling_conventions::BoundaryCallingApplication;
use component_description::VerifiedComponent;
use effects::provider_plan::ProviderPlan;
use terminal_psi::ProviderCandidateConformance;

/// One exported requirement with both sides' complete agreed ABI application.
/// Reading this row establishes neither an installed address nor an era pin.
pub struct ComponentExportInput<'input> {
    pub candidate: &'input ProviderCandidateConformance,
    pub calling_application: &'input BoundaryCallingApplication,
}

/// Verified semantic closure joined to its own native production inputs.
/// No local ID is meaningful without `component.artifact()`.
pub struct ComponentNativeInputs<'input> {
    component: &'input VerifiedComponent,
    proposal: &'input TerminalNativeRealizationProposal,
    exports: Vec<ComponentExportInput<'input>>,
    plan_digest: effects::provider_plan::ProviderPlanDigest,
}

/// Checked semantic/native agreement at one cross-artifact import. This is a
/// symbolic call selection, not a mapped address or installed resource grant.
/// Its private fields retain the actual verified provider and complete ABI;
/// compact IDs cannot construct an alternative component crossing.
#[derive(Debug)]
pub struct AdmittedNativeComponentCall<'input> {
    selected: terminal_installation::AdmittedComponentProvider,
    component: &'input VerifiedComponent,
    application: &'input BoundaryCallingApplication,
    plan_digest: effects::provider_plan::ProviderPlanDigest,
}

impl<'input> AdmittedNativeComponentCall<'input> {
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
            && adapter.requirement_identity == self.application.requirement_identity
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

    /// Close the receiver's portable requirement against every selected export.
    /// The returned selections still require a real installed component
    /// instance and its entry/resource/era admission before invocation.
    pub fn admit_receiver(
        &self,
        receiver: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Vec<terminal_installation::AdmittedComponentProvider>, String> {
        let module = self.component.module();
        self.exports
            .iter()
            .map(|export| {
                let requirement = &export.calling_application.requirement_identity;
                let mut receiving = receiver
                    .module()
                    .boundary_machines
                    .iter()
                    .filter(|boundary| {
                        receiver.module().identities.text(boundary.identity)
                            == Some(requirement.as_str())
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

    /// Require the exact retained artifact, target and complete ABI/opaque
    /// applications selected by the receiver. Equal geometry or compact report
    /// coordinates cannot substitute another declaration or representation.
    pub fn join(
        component: &'input VerifiedComponent,
        retained: &'input RetainedTerminalArtifact,
        plan: &ProviderPlan,
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
            .selected_plan_exports(plan)
            .map_err(|error| error.to_string())?;
        let mut exports = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let requirement = component
                .module()
                .identities
                .text(candidate.requirement_identity)
                .ok_or("component export has no canonical requirement identity")?;
            let producer = one_application(
                &proposal.boundary_applications().calling,
                requirement,
                "component export",
            )?;
            let receiver =
                one_application(receiving_applications, requirement, "component import")?;
            for application in [producer, receiver] {
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
            exports.push(ComponentExportInput {
                candidate,
                calling_application: producer,
            });
        }
        Ok(Self {
            component,
            proposal,
            exports,
            plan_digest: plan.identity_digest(),
        })
    }
}

fn one_application<'input>(
    applications: &'input [BoundaryCallingApplication],
    requirement: &str,
    role: &str,
) -> Result<&'input BoundaryCallingApplication, String> {
    let mut matching = applications
        .iter()
        .filter(|row| row.requirement_identity == requirement);
    let application = matching
        .next()
        .ok_or_else(|| format!("{role} `{requirement}` has no complete calling application",))?;
    if matching.next().is_some() {
        return Err(format!(
            "{role} `{requirement}` has competing calling applications"
        ));
    }
    Ok(application)
}
