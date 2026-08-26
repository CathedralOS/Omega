#![forbid(unsafe_code)]

//! Transactional deployment composition above compilation.
//!
//! Compilation may produce a non-visible terminal component candidate, but it
//! cannot install code, bind provider occurrences, admit progress evidence, or
//! publish a runnable component. This owner joins those independently supplied
//! values without allowing a failed step to discard the one-shot installation
//! registry claim.

use omega_compiler::{TerminalComponentCandidate, TerminalComponentCandidateParts};
use omega_component_publication::{InstalledRunnableComponent, bind_installed_runnable_component};
use omega_executable_installation::InstalledCode;
use omega_external_roots::{
    ComponentProgressDemandIdentity, ComponentProgressReceiptBinding,
    InstalledComponentProgressClosure, InstalledRootLedger,
    ProgressProfileEstablishmentAttestation, ProviderOccurrencePlanBinding,
};
use omega_terminal_image_emission::{
    bind_installed_terminal_artifact,
    build_terminal_installation_record_with_selected_provider_plans_and_evidence,
    decode_terminal_installation_record, encode_terminal_installation_record,
};
use psi_core::ProfileDecisionId;

/// One authored association between a pending component demand and the exact
/// provider attestation intended to discharge it.
#[derive(Debug, Clone)]
pub struct ComponentProgressAttestationBinding {
    demand: ComponentProgressDemandIdentity,
    attestation: ProgressProfileEstablishmentAttestation,
}

impl ComponentProgressAttestationBinding {
    pub fn new(
        demand: ComponentProgressDemandIdentity,
        attestation: ProgressProfileEstablishmentAttestation,
    ) -> Self {
        Self {
            demand,
            attestation,
        }
    }
}

/// A candidate and exact installed-code occurrence after the sole registry
/// claim has been issued. Dropping or returning this session never pretends the
/// claim can be issued again; the live ledger remains beside the code custody.
#[derive(Debug)]
#[must_use = "a claimed deployment session retains the one-shot installation registry"]
pub struct TerminalComponentDeploymentSession {
    candidate: TerminalComponentCandidateParts,
    installed: InstalledCode,
    roots: InstalledRootLedger,
}

impl TerminalComponentDeploymentSession {
    pub const fn candidate(&self) -> &TerminalComponentCandidateParts {
        &self.candidate
    }

    pub const fn installed(&self) -> &InstalledCode {
        &self.installed
    }

    pub const fn roots(&self) -> &InstalledRootLedger {
        &self.roots
    }

    /// Seal the complete selected provider-plan set to exact installed
    /// occurrences. Rejection returns the still-live claimed session and the
    /// caller's original bindings.
    pub fn seal_provider_occurrences(
        mut self,
        bindings: Vec<ProviderOccurrencePlanBinding>,
    ) -> Result<ProviderClosedTerminalComponentDeployment, Box<ProviderClosureError>> {
        if let Err(diagnostic) = self.roots.seal_provider_occurrence_closure(
            &self.candidate.selected_provider_plans,
            bindings.clone(),
        ) {
            return Err(Box::new(ProviderClosureError {
                session: self,
                bindings,
                diagnostic: diagnostic.0,
            }));
        }
        Ok(ProviderClosedTerminalComponentDeployment { session: self })
    }
}

#[derive(Debug)]
pub struct BeginDeploymentError {
    candidate: TerminalComponentCandidate,
    installed: InstalledCode,
    diagnostic: String,
}

impl BeginDeploymentError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (TerminalComponentCandidate, InstalledCode) {
        (self.candidate, self.installed)
    }
}

impl std::fmt::Display for BeginDeploymentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for BeginDeploymentError {}

fn preflight_terminal_component_deployment(
    candidate: &TerminalComponentCandidate,
    installed: &InstalledCode,
) -> Result<(), String> {
    if candidate.target().architecture != installed.architecture() {
        return Err(
            "terminal component target differs from the installed-code architecture".into(),
        );
    }
    let Some(final_compiler_text) = candidate
        .image()
        .output()
        .final_text_bytes
        .get(..candidate.object().text_bytes().len())
    else {
        return Err("terminal component image truncates its compiler-authored object text".into());
    };
    if !installed.binds_exact_materialized_artifact_bytes(
        candidate.object().text_bytes(),
        final_compiler_text,
    ) {
        return Err(
            "installed code does not contain the candidate's exact unrelocated and materialized text"
                .into(),
        );
    }
    Ok(())
}

/// Preflight exact target and byte custody before burning the one-shot
/// registry claim, then transfer the compiler candidate into deployment.
pub fn begin_terminal_component_deployment(
    candidate: TerminalComponentCandidate,
    mut installed: InstalledCode,
) -> Result<TerminalComponentDeploymentSession, Box<BeginDeploymentError>> {
    let reject = |candidate, installed, diagnostic| {
        Err(Box::new(BeginDeploymentError {
            candidate,
            installed,
            diagnostic,
        }))
    };
    if let Err(diagnostic) = preflight_terminal_component_deployment(&candidate, &installed) {
        return reject(candidate, installed, diagnostic);
    }
    let roots = match InstalledRootLedger::claim(&mut installed) {
        Ok(roots) => roots,
        Err(diagnostic) => return reject(candidate, installed, diagnostic.0),
    };
    Ok(TerminalComponentDeploymentSession {
        candidate: candidate.into_parts(),
        installed,
        roots,
    })
}

/// Rejected handoff of an already-claimed, completely torn-down root ledger.
/// All three linear inputs remain available for an exact retry.
#[derive(Debug)]
pub struct BeginClaimedDeploymentError {
    candidate: TerminalComponentCandidate,
    installed: InstalledCode,
    roots: InstalledRootLedger,
    diagnostic: String,
}

impl BeginClaimedDeploymentError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        TerminalComponentCandidate,
        InstalledCode,
        InstalledRootLedger,
    ) {
        (self.candidate, self.installed, self.roots)
    }
}

impl std::fmt::Display for BeginClaimedDeploymentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for BeginClaimedDeploymentError {}

/// Join a candidate to the exact empty ledger that already owns this
/// installed occurrence's one-shot registry claim. This path never attempts a
/// second claim; rejection returns all custody unchanged.
pub fn begin_terminal_component_deployment_with_claimed_registry(
    candidate: TerminalComponentCandidate,
    installed: InstalledCode,
    roots: InstalledRootLedger,
) -> Result<TerminalComponentDeploymentSession, Box<BeginClaimedDeploymentError>> {
    let reject = |candidate, installed, roots, diagnostic| {
        Err(Box::new(BeginClaimedDeploymentError {
            candidate,
            installed,
            roots,
            diagnostic,
        }))
    };
    if let Err(diagnostic) = preflight_terminal_component_deployment(&candidate, &installed) {
        return reject(candidate, installed, roots, diagnostic);
    }
    if !roots.binds_installed_code(&installed) {
        return reject(
            candidate,
            installed,
            roots,
            "claimed installation registry names a different installed-code occurrence".into(),
        );
    }
    if !roots.live_external_roots_are_empty() {
        return reject(
            candidate,
            installed,
            roots,
            "claimed installation registry still contains live external-root custody".into(),
        );
    }
    Ok(TerminalComponentDeploymentSession {
        candidate: candidate.into_parts(),
        installed,
        roots,
    })
}

#[derive(Debug)]
pub struct ProviderClosureError {
    session: TerminalComponentDeploymentSession,
    bindings: Vec<ProviderOccurrencePlanBinding>,
    diagnostic: String,
}

impl ProviderClosureError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        TerminalComponentDeploymentSession,
        Vec<ProviderOccurrencePlanBinding>,
    ) {
        (self.session, self.bindings)
    }
}

impl std::fmt::Display for ProviderClosureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ProviderClosureError {}

/// Claimed deployment after the complete selected provider closure is sealed.
#[derive(Debug)]
#[must_use = "provider-closed deployment retains installation custody"]
pub struct ProviderClosedTerminalComponentDeployment {
    session: TerminalComponentDeploymentSession,
}

impl ProviderClosedTerminalComponentDeployment {
    pub const fn installed(&self) -> &InstalledCode {
        &self.session.installed
    }

    /// Admit exact progress attestations and atomically close the candidate's
    /// manifest. Already admitted identical receipts replay idempotently, so a
    /// failed later row can be corrected and this returned session retried.
    pub fn close_progress(
        mut self,
        bindings: Vec<ComponentProgressAttestationBinding>,
    ) -> Result<ProgressClosedTerminalComponentDeployment, Box<ProgressClosureError>> {
        let manifest = self.session.candidate.component_progress.take();
        let Some(manifest) = manifest else {
            if bindings.is_empty() {
                return Ok(ProgressClosedTerminalComponentDeployment {
                    session: self.session,
                    progress: None,
                });
            }
            return Err(Box::new(ProgressClosureError {
                session: self,
                bindings,
                diagnostic:
                    "progress-free terminal component received progress establishment attestations"
                        .into(),
            }));
        };

        let mut admitted = Vec::with_capacity(bindings.len());
        for binding in &bindings {
            let receipt = match self
                .session
                .roots
                .admit_progress_profile_establishment(binding.attestation.clone())
            {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.session.candidate.component_progress = Some(manifest);
                    return Err(Box::new(ProgressClosureError {
                        session: self,
                        bindings,
                        diagnostic: error.diagnostic().0.clone(),
                    }));
                }
            };
            admitted.push(ComponentProgressReceiptBinding::new(
                binding.demand.clone(),
                receipt,
            ));
        }
        match self
            .session
            .roots
            .seal_component_progress(manifest, admitted)
        {
            Ok(progress) => Ok(ProgressClosedTerminalComponentDeployment {
                session: self.session,
                progress: Some(progress),
            }),
            Err(error) => {
                let diagnostic = error.diagnostic().0.clone();
                let (manifest, _) = error.into_parts();
                self.session.candidate.component_progress = Some(manifest);
                Err(Box::new(ProgressClosureError {
                    session: self,
                    bindings,
                    diagnostic,
                }))
            }
        }
    }
}

#[derive(Debug)]
pub struct ProgressClosureError {
    session: ProviderClosedTerminalComponentDeployment,
    bindings: Vec<ComponentProgressAttestationBinding>,
    diagnostic: String,
}

impl ProgressClosureError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ProviderClosedTerminalComponentDeployment,
        Vec<ComponentProgressAttestationBinding>,
    ) {
        (self.session, self.bindings)
    }
}

impl std::fmt::Display for ProgressClosureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ProgressClosureError {}

/// Fully admitted deployment inputs, still unpublished.
#[derive(Debug)]
#[must_use = "progress-closed deployment must be finalized or retained for retry"]
pub struct ProgressClosedTerminalComponentDeployment {
    session: TerminalComponentDeploymentSession,
    progress: Option<InstalledComponentProgressClosure>,
}

impl ProgressClosedTerminalComponentDeployment {
    pub const fn progress(&self) -> Option<&InstalledComponentProgressClosure> {
        self.progress.as_ref()
    }

    /// Canonically encode/decode the installation record, bind the complete
    /// object/image/code join, and retain the live root ledger in the runnable
    /// carrier. Rejection reconstructs this exact stage for retry.
    pub fn finalize(
        self,
        profile_decision: ProfileDecisionId,
    ) -> Result<InstalledRunnableComponent, Box<DeploymentFinalizationError>> {
        let Self {
            session:
                TerminalComponentDeploymentSession {
                    candidate,
                    installed,
                    roots,
                },
            progress,
        } = self;
        let selected_provider_plan_identities = candidate
            .selected_provider_plans
            .plans()
            .iter()
            .map(omega_effects::provider_plan::ProviderPlan::identity_fingerprint)
            .collect::<Vec<_>>();
        let record = match build_terminal_installation_record_with_selected_provider_plans_and_evidence(
            &candidate.image,
            profile_decision,
            selected_provider_plan_identities,
            &candidate.provider_executions,
            progress.as_ref().map(|value| value as &dyn omega_terminal_installation_evidence::TerminalComponentProgressAcceptanceEvidence),
        ) {
            Ok(record) => record,
            Err(error) => {
                return Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedTerminalComponentDeployment {
                        session: TerminalComponentDeploymentSession {
                            candidate,
                            installed,
                            roots,
                        },
                        progress,
                    },
                    diagnostic: format!("terminal installation construction failed: {error}"),
                }));
            }
        };
        let encoded = match encode_terminal_installation_record(&record) {
            Ok(encoded) => encoded,
            Err(error) => {
                return Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedTerminalComponentDeployment {
                        session: TerminalComponentDeploymentSession {
                            candidate,
                            installed,
                            roots,
                        },
                        progress,
                    },
                    diagnostic: format!("terminal installation encoding failed: {error}"),
                }));
            }
        };
        let record = match decode_terminal_installation_record(&encoded) {
            Ok(record) => record,
            Err(error) => {
                return Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedTerminalComponentDeployment {
                        session: TerminalComponentDeploymentSession {
                            candidate,
                            installed,
                            roots,
                        },
                        progress,
                    },
                    diagnostic: format!("terminal installation replay failed: {error}"),
                }));
            }
        };

        let TerminalComponentCandidateParts {
            target,
            entry_machine,
            semantic_bytes,
            proof_bytes,
            object,
            image,
            selected_provider_plans,
            provider_executions,
            component_progress,
        } = candidate;
        let artifact = match bind_installed_terminal_artifact(object, image, record, installed) {
            Ok(artifact) => artifact,
            Err(error) => {
                let diagnostic = error.diagnostic().to_owned();
                let (object, image, _, installed) = error.into_parts();
                return Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedTerminalComponentDeployment {
                        session: TerminalComponentDeploymentSession {
                            candidate: TerminalComponentCandidateParts {
                                target,
                                entry_machine,
                                semantic_bytes,
                                proof_bytes,
                                object,
                                image,
                                selected_provider_plans,
                                provider_executions,
                                component_progress,
                            },
                            installed,
                            roots,
                        },
                        progress,
                    },
                    diagnostic,
                }));
            }
        };
        match bind_installed_runnable_component(artifact, roots, progress) {
            Ok(runnable) => Ok(runnable),
            Err(error) => {
                let diagnostic = error.diagnostic().to_owned();
                let (artifact, roots, progress) = error.into_parts();
                let (object, image, _, installed) = artifact.into_parts();
                Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedTerminalComponentDeployment {
                        session: TerminalComponentDeploymentSession {
                            candidate: TerminalComponentCandidateParts {
                                target,
                                entry_machine,
                                semantic_bytes,
                                proof_bytes,
                                object,
                                image,
                                selected_provider_plans,
                                provider_executions,
                                component_progress,
                            },
                            installed,
                            roots,
                        },
                        progress,
                    },
                    diagnostic,
                }))
            }
        }
    }
}

#[derive(Debug)]
pub struct DeploymentFinalizationError {
    session: ProgressClosedTerminalComponentDeployment,
    diagnostic: String,
}

impl DeploymentFinalizationError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_session(self) -> ProgressClosedTerminalComponentDeployment {
        self.session
    }
}

impl std::fmt::Display for DeploymentFinalizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for DeploymentFinalizationError {}
