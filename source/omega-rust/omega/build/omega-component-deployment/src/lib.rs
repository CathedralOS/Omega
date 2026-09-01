#![forbid(unsafe_code)]

//! Transactional deployment composition above compilation.
//!
//! Compilation may produce a non-visible terminal component candidate, but it
//! cannot install code, bind provider occurrences, admit progress evidence, or
//! publish a runnable component. This owner joins those independently supplied
//! values without allowing a failed step to discard the one-shot installation
//! registry claim.

use omega_component_candidate::{
    ComponentCandidate, ComponentCandidateParts, NativeArtifact, NativeArtifactParts,
};
use omega_component_publication::{InstalledRunnableComponent, bind_installed_runnable_component};
use omega_executable_installation::InstalledCode;
use omega_external_roots::{
    ComponentProgressDemandIdentity, ComponentProgressReceiptBinding,
    InstalledComponentProgressClosure, InstalledRootLedger,
    ProgressProfileEstablishmentAttestation, ProviderOccurrencePlanBinding,
};
use omega_image_emission::{
    bind_installed_artifact, build_installation_record_with_selected_provider_plans_and_evidence,
    decode_installation_record, encode_installation_record, installation_fingerprint,
    validate_installation_record,
};
use psi_core::ProfileDecisionId;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FLAT_OUTPUT_STAGING_IDENTITY: AtomicU64 = AtomicU64::new(1);

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
pub struct ComponentDeploymentSession {
    candidate: ComponentCandidateParts,
    installed: InstalledCode,
    roots: InstalledRootLedger,
}

impl ComponentDeploymentSession {
    pub const fn candidate(&self) -> &ComponentCandidateParts {
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
    ) -> Result<ProviderClosedComponentDeployment, Box<ProviderClosureError>> {
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
        Ok(ProviderClosedComponentDeployment { session: self })
    }
}

#[derive(Debug)]
pub struct BeginDeploymentError {
    candidate: ComponentCandidate,
    installed: InstalledCode,
    diagnostic: String,
}

impl BeginDeploymentError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ComponentCandidate, InstalledCode) {
        (self.candidate, self.installed)
    }
}

impl std::fmt::Display for BeginDeploymentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for BeginDeploymentError {}

fn preflight_component_deployment(
    candidate: &ComponentCandidate,
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
pub fn begin_component_deployment(
    candidate: ComponentCandidate,
    mut installed: InstalledCode,
) -> Result<ComponentDeploymentSession, Box<BeginDeploymentError>> {
    let reject = |candidate, installed, diagnostic| {
        Err(Box::new(BeginDeploymentError {
            candidate,
            installed,
            diagnostic,
        }))
    };
    if let Err(diagnostic) = preflight_component_deployment(&candidate, &installed) {
        return reject(candidate, installed, diagnostic);
    }
    let roots = match InstalledRootLedger::claim(&mut installed) {
        Ok(roots) => roots,
        Err(diagnostic) => return reject(candidate, installed, diagnostic.0),
    };
    Ok(ComponentDeploymentSession {
        candidate: candidate.into_parts(),
        installed,
        roots,
    })
}

/// Rejected handoff of an already-claimed, completely torn-down root ledger.
/// All three linear inputs remain available for an exact retry.
#[derive(Debug)]
pub struct BeginClaimedDeploymentError {
    candidate: ComponentCandidate,
    installed: InstalledCode,
    roots: InstalledRootLedger,
    diagnostic: String,
}

impl BeginClaimedDeploymentError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ComponentCandidate, InstalledCode, InstalledRootLedger) {
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
pub fn begin_component_deployment_with_claimed_registry(
    candidate: ComponentCandidate,
    installed: InstalledCode,
    roots: InstalledRootLedger,
) -> Result<ComponentDeploymentSession, Box<BeginClaimedDeploymentError>> {
    let reject = |candidate, installed, roots, diagnostic| {
        Err(Box::new(BeginClaimedDeploymentError {
            candidate,
            installed,
            roots,
            diagnostic,
        }))
    };
    if let Err(diagnostic) = preflight_component_deployment(&candidate, &installed) {
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
    Ok(ComponentDeploymentSession {
        candidate: candidate.into_parts(),
        installed,
        roots,
    })
}

#[derive(Debug)]
pub struct ProviderClosureError {
    session: ComponentDeploymentSession,
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
        ComponentDeploymentSession,
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
pub struct ProviderClosedComponentDeployment {
    session: ComponentDeploymentSession,
}

impl ProviderClosedComponentDeployment {
    pub const fn installed(&self) -> &InstalledCode {
        &self.session.installed
    }

    /// Admit exact progress attestations and atomically close the candidate's
    /// manifest. Already admitted identical receipts replay idempotently, so a
    /// failed later row can be corrected and this returned session retried.
    pub fn close_progress(
        mut self,
        bindings: Vec<ComponentProgressAttestationBinding>,
    ) -> Result<ProgressClosedComponentDeployment, Box<ProgressClosureError>> {
        let manifest = self.session.candidate.component_progress.take();
        let Some(manifest) = manifest else {
            if bindings.is_empty() {
                return Ok(ProgressClosedComponentDeployment {
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
            Ok(progress) => Ok(ProgressClosedComponentDeployment {
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
    session: ProviderClosedComponentDeployment,
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
        ProviderClosedComponentDeployment,
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
pub struct ProgressClosedComponentDeployment {
    session: ComponentDeploymentSession,
    progress: Option<InstalledComponentProgressClosure>,
}

impl ProgressClosedComponentDeployment {
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
                ComponentDeploymentSession {
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
            .map(omega_effects::provider_plan::ProviderPlan::report_fingerprint)
            .collect::<Vec<_>>();
        let record = match build_installation_record_with_selected_provider_plans_and_evidence(
            candidate.native_artifact.image(),
            profile_decision,
            selected_provider_plan_identities,
            candidate.native_artifact.provider_executions(),
            progress.as_ref().map(|value| {
                value as &dyn omega_installation_evidence::ComponentProgressAcceptanceEvidence
            }),
        ) {
            Ok(record) => record,
            Err(error) => {
                return Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedComponentDeployment {
                        session: ComponentDeploymentSession {
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
        let encoded = match encode_installation_record(&record) {
            Ok(encoded) => encoded,
            Err(error) => {
                return Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedComponentDeployment {
                        session: ComponentDeploymentSession {
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
        let record = match decode_installation_record(&encoded) {
            Ok(record) => record,
            Err(error) => {
                return Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedComponentDeployment {
                        session: ComponentDeploymentSession {
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

        let ComponentCandidateParts {
            native_artifact,
            entry_machine,
            selected_provider_plans,
            component_progress,
            stack_demand,
        } = candidate;
        let NativeArtifactParts {
            target,
            psi_artifact,
            object,
            image,
            selected_provider_closure_report_identity,
            selected_provider_closure_digest,
            selected_provider_plans: native_selected_provider_plans,
            provider_executions,
            physical_evidence_scope,
            physical_evidence,
        } = native_artifact.into_parts();
        let artifact = match bind_installed_artifact(object, image, record, installed) {
            Ok(artifact) => artifact,
            Err(error) => {
                let diagnostic = error.diagnostic().to_owned();
                let (object, image, _, installed) = error.into_parts();
                return Err(Box::new(DeploymentFinalizationError {
                    session: ProgressClosedComponentDeployment {
                        session: ComponentDeploymentSession {
                            candidate: ComponentCandidateParts {
                                native_artifact: NativeArtifact::from_replayed_parts(
                                    NativeArtifactParts {
                                        target,
                                        psi_artifact,
                                        object,
                                        image,
                                        selected_provider_closure_report_identity,
                                        selected_provider_closure_digest,
                                        selected_provider_plans: native_selected_provider_plans,
                                        provider_executions,
                                        physical_evidence_scope,
                                        physical_evidence,
                                    },
                                )
                                .expect("failed installation must return the validated native artifact unchanged"),
                                entry_machine,
                                selected_provider_plans,
                                component_progress,
                                stack_demand,
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
                    session: ProgressClosedComponentDeployment {
                        session: ComponentDeploymentSession {
                            candidate: ComponentCandidateParts {
                                native_artifact: NativeArtifact::from_replayed_parts(
                                    NativeArtifactParts {
                                        target,
                                        psi_artifact,
                                        object,
                                        image,
                                        selected_provider_closure_report_identity,
                                        selected_provider_closure_digest,
                                        selected_provider_plans: native_selected_provider_plans,
                                        provider_executions,
                                        physical_evidence_scope,
                                        physical_evidence,
                                    },
                                )
                                .expect("failed runnable binding must return the validated native artifact unchanged"),
                                entry_machine,
                                selected_provider_plans,
                                component_progress,
                                stack_demand,
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
    session: ProgressClosedComponentDeployment,
    diagnostic: String,
}

impl DeploymentFinalizationError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_session(self) -> ProgressClosedComponentDeployment {
        self.session
    }
}

impl std::fmt::Display for DeploymentFinalizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for DeploymentFinalizationError {}

/// Exact filesystem-publication receipt for one deployed terminal component.
///
/// The receipt is deliberately non-clonable. Its installation fingerprint
/// commits the canonical manifest and acceptance identities, while its image
/// fingerprint commits the exact bytes replayed at `output_path`.
#[derive(Debug)]
#[must_use = "terminal component output publication receipts must remain with deployment custody"]
pub struct ComponentFlatOutputReceipt {
    output_path: PathBuf,
    installation_fingerprint: omega_image_emission::InstallationFingerprint,
    image_fingerprint: omega_image_emission::ImageFingerprint,
    byte_count: usize,
}

impl ComponentFlatOutputReceipt {
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub const fn installation_fingerprint(&self) -> omega_image_emission::InstallationFingerprint {
        self.installation_fingerprint
    }

    pub const fn image_fingerprint(&self) -> omega_image_emission::ImageFingerprint {
        self.image_fingerprint
    }

    pub const fn byte_count(&self) -> usize {
        self.byte_count
    }

    /// Replay this receipt against both the retained deployment evidence and
    /// the currently visible file.
    pub fn validate(
        &self,
        runnable: &InstalledRunnableComponent,
    ) -> Result<(), ComponentFlatOutputValidationError> {
        validate_terminal_flat_output_receipt(self, runnable)
            .map_err(ComponentFlatOutputValidationError)
    }
}

/// A visible flat executable that still owns the complete runnable component.
/// Later era publication can recover both values explicitly; a filesystem
/// receipt never decomposes or substitutes for installation custody.
#[derive(Debug)]
#[must_use = "published terminal output retains runnable installation custody"]
pub struct PublishedComponentFlatOutput {
    runnable: InstalledRunnableComponent,
    receipt: ComponentFlatOutputReceipt,
}

impl PublishedComponentFlatOutput {
    pub const fn runnable(&self) -> &InstalledRunnableComponent {
        &self.runnable
    }

    pub const fn receipt(&self) -> &ComponentFlatOutputReceipt {
        &self.receipt
    }

    pub fn validate(&self) -> Result<(), ComponentFlatOutputValidationError> {
        self.receipt.validate(&self.runnable)
    }

    /// Recover both the live runnable carrier and its filesystem receipt for
    /// transfer to the next deployment/era owner.
    pub fn into_parts(self) -> (InstalledRunnableComponent, ComponentFlatOutputReceipt) {
        (self.runnable, self.receipt)
    }
}

#[derive(Debug)]
pub struct ComponentFlatOutputPublicationError {
    runnable: InstalledRunnableComponent,
    requested_path: PathBuf,
    diagnostic: String,
}

impl ComponentFlatOutputPublicationError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn requested_path(&self) -> &Path {
        &self.requested_path
    }

    /// Recover the exact runnable custody and caller-requested path after any
    /// rejected or failed publication attempt.
    pub fn into_parts(self) -> (InstalledRunnableComponent, PathBuf) {
        (self.runnable, self.requested_path)
    }
}

impl std::fmt::Display for ComponentFlatOutputPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ComponentFlatOutputPublicationError {}

#[derive(Debug, PartialEq, Eq)]
pub struct ComponentFlatOutputValidationError(String);

impl ComponentFlatOutputValidationError {
    pub fn diagnostic(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentFlatOutputValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ComponentFlatOutputValidationError {}

/// Publish the exact executable image retained by a finalized deployment.
///
/// The requested filename must equal the compiler-sealed image filename. The
/// file is staged beside its destination, replayed byte-for-byte with its
/// executable mode in place, atomically renamed, and replayed again before a
/// receipt exists. Any failure returns the complete runnable carrier.
pub fn publish_component_flat_output(
    runnable: InstalledRunnableComponent,
    requested_path: PathBuf,
) -> Result<PublishedComponentFlatOutput, Box<ComponentFlatOutputPublicationError>> {
    let result = publish_component_flat_output_inner(&runnable, &requested_path);
    match result {
        Ok(receipt) => Ok(PublishedComponentFlatOutput { runnable, receipt }),
        Err(diagnostic) => Err(Box::new(ComponentFlatOutputPublicationError {
            runnable,
            requested_path,
            diagnostic,
        })),
    }
}

fn publish_component_flat_output_inner(
    runnable: &InstalledRunnableComponent,
    output_path: &Path,
) -> Result<ComponentFlatOutputReceipt, String> {
    let terminal = runnable.installed_artifact();
    let image = terminal.image();
    let installation = terminal.installation();
    validate_installation_record(installation, image)
        .map_err(|error| format!("terminal output installation replay failed: {error}"))?;
    let installation_fingerprint = installation_fingerprint(installation)
        .map_err(|error| format!("terminal output installation fingerprint failed: {error}"))?;
    let output = image.output();
    if output_path.file_name() != Some(std::ffi::OsStr::new(&output.file_name)) {
        return Err(format!(
            "terminal output path must retain sealed executable filename `{}`",
            output.file_name
        ));
    }
    if output.bytes.is_empty() {
        return Err("terminal output image cannot publish empty executable bytes".into());
    }

    write_atomic_executable(output_path, &output.bytes)?;
    let receipt = ComponentFlatOutputReceipt {
        output_path: output_path.to_path_buf(),
        installation_fingerprint,
        image_fingerprint: installation.image(),
        byte_count: output.bytes.len(),
    };
    if let Err(diagnostic) = validate_terminal_flat_output_receipt(&receipt, runnable) {
        let _ = std::fs::remove_file(output_path);
        return Err(diagnostic);
    }
    Ok(receipt)
}

fn validate_terminal_flat_output_receipt(
    receipt: &ComponentFlatOutputReceipt,
    runnable: &InstalledRunnableComponent,
) -> Result<(), String> {
    let terminal = runnable.installed_artifact();
    validate_installation_record(terminal.installation(), terminal.image())
        .map_err(|error| format!("terminal output installation replay failed: {error}"))?;
    let installation_fingerprint = installation_fingerprint(terminal.installation())
        .map_err(|error| format!("terminal output installation fingerprint failed: {error}"))?;
    if receipt.installation_fingerprint != installation_fingerprint
        || receipt.image_fingerprint != terminal.installation().image()
        || receipt.byte_count != terminal.image().output().bytes.len()
        || receipt.output_path.file_name()
            != Some(std::ffi::OsStr::new(&terminal.image().output().file_name))
    {
        return Err(
            "terminal output receipt does not bind the exact runnable installation and image"
                .into(),
        );
    }
    validate_published_executable(&receipt.output_path, &terminal.image().output().bytes)
}

fn write_atomic_executable(output_path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create terminal output directory {}: {error}",
            parent.display()
        )
    })?;
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "terminal output path has no UTF-8 executable filename".to_owned())?;
    let staging_identity = NEXT_FLAT_OUTPUT_STAGING_IDENTITY.fetch_add(1, Ordering::Relaxed);
    let staged_path = output_path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        staging_identity
    ));
    let mut staged = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged_path)
        .map_err(|error| {
            format!(
                "failed to create staged terminal output {}: {error}",
                staged_path.display()
            )
        })?;
    if let Err(error) = staged.write_all(bytes) {
        drop(staged);
        let _ = std::fs::remove_file(&staged_path);
        return Err(format!(
            "failed to write staged terminal output {}: {error}",
            staged_path.display()
        ));
    }
    if let Err(error) = staged.sync_all() {
        drop(staged);
        let _ = std::fs::remove_file(&staged_path);
        return Err(format!(
            "failed to synchronize staged terminal output {}: {error}",
            staged_path.display()
        ));
    }
    drop(staged);
    if let Err(diagnostic) = mark_executable(&staged_path)
        .and_then(|()| validate_published_executable(&staged_path, bytes))
    {
        let _ = std::fs::remove_file(&staged_path);
        return Err(diagnostic);
    }
    if let Err(error) = std::fs::rename(&staged_path, output_path) {
        let _ = std::fs::remove_file(&staged_path);
        return Err(format!(
            "failed to install terminal output {}: {error}",
            output_path.display()
        ));
    }
    if let Err(diagnostic) = validate_published_executable(output_path, bytes) {
        let _ = std::fs::remove_file(output_path);
        return Err(diagnostic);
    }
    Ok(())
}

fn validate_published_executable(path: &Path, expected: &[u8]) -> Result<(), String> {
    let actual = std::fs::read(path).map_err(|error| {
        format!(
            "failed to replay terminal output {}: {error}",
            path.display()
        )
    })?;
    if actual != expected {
        return Err("published terminal output bytes differ from the deployed image".into());
    }
    validate_executable_mode(path)
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| format!("failed to read {} permissions: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to mark {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn mark_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn validate_executable_mode(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::metadata(path)
        .map_err(|error| format!("failed to read {} permissions: {error}", path.display()))?
        .permissions()
        .mode();
    if mode & 0o777 != 0o755 {
        return Err("published terminal output does not retain exact executable mode 0755".into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_executable_mode(_path: &Path) -> Result<(), String> {
    Ok(())
}
