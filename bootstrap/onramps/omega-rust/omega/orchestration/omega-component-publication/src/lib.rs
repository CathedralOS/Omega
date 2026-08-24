#![forbid(unsafe_code)]

//! Authority-bearing publication of installed component eras.
//!
//! Native emission, executable installation, progress-profile admission, and
//! lifecycle publication are independent gates. This crate is the narrow
//! orchestration owner that joins their opaque results before an era may be
//! treated as runnable and retains those results until retirement succeeds.

use std::collections::BTreeMap;

use omega_effects::{
    ActiveComponentEraEntry, CoexistingExecutableTcbReport, ComponentEraCandidate,
    ComponentEraEntryLedger, ComponentEraEntryReceipt, ComponentEraEntryState,
    ComponentEraLeaveReceipt, ComponentEraPublicationReceipt, ComponentEraQuiescenceReceipt,
    ComponentEraRetirementReceipt, EraEntryError, EraLeaveError, EraQuiescenceError,
    ProgramLocalRootEpochLease, ProgramLocalRootEpochLeaseAcquisitionError,
    ProgramLocalRootEpochLeaseId, ProgramLocalRootEpochLeaseReleaseError,
};
use omega_executable_installation::{ArtifactId, InstalledCode, InstalledCodeId};
use omega_external_roots::InstalledComponentProgressClosure;
use omega_terminal_image_emission::InstalledTerminalArtifact;

/// Installed terminal artifact plus the concrete accepted progress closure
/// committed by its canonical installation record.
///
/// The progress acceptance is optional only when the record commits no
/// progress section. A report fingerprint can never construct this value.
#[derive(Debug)]
#[must_use = "installed runnable component evidence must be retained through era retirement"]
pub struct InstalledRunnableComponent {
    artifact: InstalledTerminalArtifact,
    progress: Option<InstalledComponentProgressClosure>,
}

impl InstalledRunnableComponent {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.artifact.installed_code()
    }

    pub fn artifact(&self) -> ArtifactId {
        self.artifact.artifact()
    }

    pub const fn progress(&self) -> Option<&InstalledComponentProgressClosure> {
        self.progress.as_ref()
    }

    pub const fn terminal_artifact(&self) -> &InstalledTerminalArtifact {
        &self.artifact
    }

    pub const fn installed(&self) -> &InstalledCode {
        self.artifact.installed()
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledTerminalArtifact,
        Option<InstalledComponentProgressClosure>,
    ) {
        (self.artifact, self.progress)
    }
}

#[derive(Debug)]
pub struct InstalledRunnableComponentBindingError {
    artifact: InstalledTerminalArtifact,
    progress: Option<InstalledComponentProgressClosure>,
    diagnostic: String,
}

impl InstalledRunnableComponentBindingError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledTerminalArtifact,
        Option<InstalledComponentProgressClosure>,
    ) {
        (self.artifact, self.progress)
    }
}

impl std::fmt::Display for InstalledRunnableComponentBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for InstalledRunnableComponentBindingError {}

/// Join an exact installed terminal artifact to the opaque progress
/// acceptance committed in its canonical installation record.
pub fn bind_installed_runnable_component(
    artifact: InstalledTerminalArtifact,
    progress: Option<InstalledComponentProgressClosure>,
) -> Result<InstalledRunnableComponent, Box<InstalledRunnableComponentBindingError>> {
    let reject = |artifact, progress, diagnostic: String| {
        Err(Box::new(InstalledRunnableComponentBindingError {
            artifact,
            progress,
            diagnostic,
        }))
    };

    let committed = artifact.installation().component_progress();
    let mismatch = match (committed, progress.as_ref()) {
        (None, None) => None,
        (None, Some(_)) => Some(
            "terminal installation record omits the supplied component-progress acceptance"
                .into(),
        ),
        (Some(_), None) => Some(
            "terminal installation record commits component progress but the opaque acceptance was not supplied"
                .into(),
        ),
        (Some(_), Some(progress)) if !progress.binds_installed_code(artifact.installed()) => Some(
            "component-progress acceptance names a different installed-code occurrence".into(),
        ),
        (Some(committed), Some(progress))
            if committed.manifest_identity() != progress.manifest().normalized_identity()
                || committed.acceptance_identity() != progress.fingerprint() =>
        {
            Some(
                "terminal installation record commits different component-progress identities"
                    .into(),
            )
        }
        (Some(_), Some(progress)) => {
            let installed_plans = artifact
                .installation()
                .selected_provider_plans()
                .iter()
                .map(|identity| identity.get())
                .collect::<Vec<_>>();
            (installed_plans != progress.selected_provider_plans()).then(|| {
                "terminal installation record and progress acceptance retain different selected provider-plan closures"
                    .into()
            })
        }
    };
    if let Some(diagnostic) = mismatch {
        return reject(artifact, progress, diagnostic);
    }

    Ok(InstalledRunnableComponent { artifact, progress })
}

/// Higher-level lifecycle owner that makes the progress acceptance impossible
/// to bypass or drop while its component era remains live.
#[derive(Debug)]
pub struct RunnableComponentEraLedger {
    lifecycle: ComponentEraEntryLedger,
    runnable: BTreeMap<u64, InstalledRunnableComponent>,
}

impl RunnableComponentEraLedger {
    pub fn new(lifecycle: ComponentEraEntryLedger) -> Self {
        Self {
            lifecycle,
            runnable: BTreeMap::new(),
        }
    }

    /// Read-only lifecycle access supports provider receipt construction while
    /// keeping publication and retirement behind the authority-bearing gate.
    pub const fn lifecycle(&self) -> &ComponentEraEntryLedger {
        &self.lifecycle
    }

    pub const fn current_era(&self) -> Option<u64> {
        self.lifecycle.current_era()
    }

    pub fn live_eras(&self) -> impl Iterator<Item = (u64, ComponentEraEntryState, usize)> + '_ {
        self.lifecycle.live_eras()
    }

    pub fn live_executable_tcb_report(&self) -> CoexistingExecutableTcbReport {
        self.lifecycle.live_executable_tcb_report()
    }

    pub fn program_local_root_authority_holds(&self, era_identity: u64) -> Option<usize> {
        self.lifecycle
            .program_local_root_authority_holds(era_identity)
    }

    pub fn retained_component(&self, era_identity: u64) -> Option<&InstalledRunnableComponent> {
        self.runnable.get(&era_identity)
    }

    pub fn publish(
        &mut self,
        candidate: ComponentEraCandidate,
        receipt: ComponentEraPublicationReceipt,
        runnable: InstalledRunnableComponent,
    ) -> Result<(), Box<RunnableEraPublicationError>> {
        if candidate.artifact_instance_identity != runnable.installed_code().normalized_identity() {
            return Err(Box::new(RunnableEraPublicationError {
                candidate,
                receipt,
                runnable,
                diagnostic:
                    "component era candidate names a different installed artifact occurrence".into(),
            }));
        }
        if self.runnable.contains_key(&candidate.era_identity) {
            return Err(Box::new(RunnableEraPublicationError {
                candidate,
                receipt,
                runnable,
                diagnostic: "component era already retains runnable installation evidence".into(),
            }));
        }
        let era_identity = candidate.era_identity;
        if let Err(error) = self.lifecycle.publish(candidate, receipt) {
            let diagnostic = error.diagnostic().to_owned();
            let (candidate, receipt) = error.into_parts();
            return Err(Box::new(RunnableEraPublicationError {
                candidate,
                receipt,
                runnable,
                diagnostic,
            }));
        }
        let previous = self.runnable.insert(era_identity, runnable);
        debug_assert!(
            previous.is_none(),
            "publication checked unique era evidence"
        );
        Ok(())
    }

    pub fn acquire_program_local_root_epoch_lease(
        &mut self,
        identity: ProgramLocalRootEpochLeaseId,
        era_identity: u64,
        entry_contract_identity: &str,
    ) -> Result<ProgramLocalRootEpochLease, ProgramLocalRootEpochLeaseAcquisitionError> {
        self.lifecycle.acquire_program_local_root_epoch_lease(
            identity,
            era_identity,
            entry_contract_identity,
        )
    }

    pub fn validate_program_local_root_epoch_lease(
        &self,
        lease: &ProgramLocalRootEpochLease,
    ) -> Result<(), String> {
        self.lifecycle
            .validate_program_local_root_epoch_lease(lease)
    }

    pub fn release_program_local_root_epoch_lease(
        &mut self,
        lease: ProgramLocalRootEpochLease,
    ) -> Result<(), ProgramLocalRootEpochLeaseReleaseError> {
        self.lifecycle.release_program_local_root_epoch_lease(lease)
    }

    pub fn enter(
        &mut self,
        receipt: ComponentEraEntryReceipt,
    ) -> Result<ActiveComponentEraEntry, EraEntryError> {
        self.lifecycle.enter(receipt)
    }

    pub fn leave(
        &mut self,
        entry: ActiveComponentEraEntry,
        receipt: ComponentEraLeaveReceipt,
    ) -> Result<(), Box<EraLeaveError>> {
        self.lifecycle.leave(entry, receipt)
    }

    pub fn establish_quiescence(
        &mut self,
        receipt: ComponentEraQuiescenceReceipt,
    ) -> Result<(), EraQuiescenceError> {
        self.lifecycle.establish_quiescence(receipt)
    }

    /// Successful retirement returns the retained opaque installation and
    /// progress evidence. Rejection leaves it in the live-era ledger.
    pub fn retire(
        &mut self,
        receipt: ComponentEraRetirementReceipt,
    ) -> Result<InstalledRunnableComponent, RunnableEraRetirementError> {
        let era_identity = receipt.era_identity();
        if !self.runnable.contains_key(&era_identity) {
            return Err(RunnableEraRetirementError {
                receipt,
                diagnostic: "component era has no retained runnable installation evidence".into(),
            });
        }
        if let Err(error) = self.lifecycle.retire(receipt) {
            return Err(RunnableEraRetirementError {
                diagnostic: error.diagnostic().to_owned(),
                receipt: error.into_value(),
            });
        }
        Ok(self
            .runnable
            .remove(&era_identity)
            .expect("live lifecycle era retained runnable evidence"))
    }
}

#[derive(Debug)]
pub struct RunnableEraPublicationError {
    candidate: ComponentEraCandidate,
    receipt: ComponentEraPublicationReceipt,
    runnable: InstalledRunnableComponent,
    diagnostic: String,
}

impl RunnableEraPublicationError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ComponentEraCandidate,
        ComponentEraPublicationReceipt,
        InstalledRunnableComponent,
    ) {
        (self.candidate, self.receipt, self.runnable)
    }
}

impl std::fmt::Display for RunnableEraPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for RunnableEraPublicationError {}

#[derive(Debug)]
pub struct RunnableEraRetirementError {
    receipt: ComponentEraRetirementReceipt,
    diagnostic: String,
}

impl RunnableEraRetirementError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_receipt(self) -> ComponentEraRetirementReceipt {
        self.receipt
    }
}

impl std::fmt::Display for RunnableEraRetirementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for RunnableEraRetirementError {}

#[cfg(test)]
mod tests;
