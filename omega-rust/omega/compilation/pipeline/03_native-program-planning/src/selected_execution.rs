// Selected execution owns one realized provider closure. Native and Psi
// consumers borrow its canonical plan roster and exact row execution data;
// choosing a provider never reconstructs declarations from source later.

use diagnostics::Diagnostic;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    target_profile: Option<target::TargetProfile>,
    folded_reach: Option<Result<backend_plan::FoldedEntryReach, String>>,
    folded: Option<Arc<crate::FoldedTargetProgram>>,
    realization: Arc<crate::target_supply::RealizedTargetSupply>,
    package_review: Option<Arc<backend_plan::package_review::PackageTargetReview>>,
}

impl SelectedExecution {
    /// Retains the provider realization already formed for this exact target.
    pub(super) fn from_realization(
        target_profile: Option<target::TargetProfile>,
        realization: crate::target_supply::RealizedTargetSupply,
    ) -> Result<Self, Vec<Diagnostic>> {
        if realization.target() != target_profile.map(|profile| profile.target_name()) {
            return Err(vec![Diagnostic::error(
                "selected execution and realized provider supply name different targets",
            )]);
        }
        Ok(Self {
            target_profile,
            realization: Arc::new(realization),
            package_review: None,
            folded_reach: None,
            folded: None,
        })
    }

    /// Capture a package's independent review inventory for this deployment.
    /// A refusal belongs to the requested review and cannot change execution.
    pub(super) fn with_package_review(
        mut self,
        module: &terminal_psi::TerminalModule,
        review: crate::package_review::PreparedReview,
    ) -> Self {
        if let Some(target) = self.target_profile {
            self.package_review = Some(Arc::new(review.finish(module, target, &self.realization)));
        }
        self
    }

    pub fn package_review(&self) -> Option<&backend_plan::package_review::PackageTargetReview> {
        self.package_review.as_deref()
    }

    pub fn realization(&self) -> &crate::target_supply::RealizedTargetSupply {
        &self.realization
    }

    /// The deployment profile the request named. Profiles that share a
    /// native ABI (such as windows_x64 and uefi_x64) stay distinct here.
    pub fn target_profile(&self) -> Option<target::TargetProfile> {
        self.target_profile
    }

    pub fn native_target(&self) -> Option<target::NativeTarget> {
        self.target_profile
            .map(target::TargetProfile::native_target)
    }

    /// The evidence settled for this exact program and target. Grant checking reads it independently.
    pub fn trust_evidence(&self) -> provider_correspondence::trust::SelectedTrustEvidence<'_> {
        let realized = self.realization();
        provider_correspondence::trust::SelectedTrustEvidence {
            program: realized.program(),
            target: realized.target(),
            plans: realized.plans(),
            leaves: realized.direct_boundary_leaves(),
            bindings: realized.evaluated_native_bindings(),
            identities: realized.evaluated_binding_identities(),
        }
    }

    pub fn plans(&self) -> &effects::SelectedProviderPlanFacts {
        self.realization.plans()
    }

    pub fn shared_plans(&self) -> &Arc<effects::SelectedProviderPlanFacts> {
        self.realization.shared_plans()
    }

    pub(super) fn with_folded_entry_reach(
        mut self,
        reach: Result<backend_plan::FoldedEntryReach, String>,
    ) -> Self {
        self.folded_reach = Some(reach);
        self
    }

    pub fn folded_program(&self) -> Option<&crate::FoldedTargetProgram> {
        self.folded.as_deref()
    }

    /// Return the exact executable code selected for a published source artifact.
    pub fn effective_artifact<'a>(
        &'a self,
        original: &'a Arc<terminal_codec::CanonicalTerminalArtifact>,
    ) -> Result<&'a Arc<terminal_codec::CanonicalTerminalArtifact>, String> {
        if let Some(folded) = self.folded_program() {
            if folded.original() != original.view().identity() {
                return Err("folded constant product belongs to another accepted program".into());
            }
            Ok(folded.artifact())
        } else {
            self.realization.require_program(original.view().identity())?;
            Ok(original)
        }
    }

    /// Target commitments depend on the admitted native calling contracts. Only
    /// the closed portable reach may request them, and the source artifact is immutable.
    pub fn bind_folded_entry(
        &mut self,
        original: &terminal_codec::CanonicalTerminalArtifact,
        accepted_evidence: Option<&terminal_codec::EvidenceSeals>,
    ) -> Result<(), String> {
        let profile = self.target_profile.ok_or("executable entry metadata requires an explicit deployment target")?;
        self.realization.require_program(original.view().identity())?;
        let reach = self.folded_reach.as_ref()
            .ok_or("executable entry has no published scalar provider reach")?
            .as_ref().map_err(Clone::clone)?;
        let closure = reach.bind(original.view(), profile.target_name(), self.plans().plans())?;
        if closure.dependencies.is_empty() { return Ok(()); }
        let folded = crate::FoldedTargetProgram::close(original, accepted_evidence, closure)?;
        self.realization = Arc::new(self.realization.bind_folded_artifact(&folded)?);
        self.folded = Some(Arc::new(folded));
        self.folded_reach = None;
        Ok(())
    }

    pub fn with_folded_program(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        Ok(Self {
            target_profile: self.target_profile,
            realization: Arc::new(self.realization.with_folded_program(artifact)?),
            package_review: self.package_review.clone(),
            folded_reach: None,
            folded: None,
        })
    }

    /// Rejoin the actual selected declarations to this canonical artifact.
    pub fn provider_proposal(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_core::OptimizationSelectionIdentity,
    ) -> Option<backend_plan::TerminalTargetProviderProposal> {
        let target = self.target_profile?.target_name();
        let selection = self
            .realization
            .require_program(artifact.identity())
            .and_then(|()| {
                backend_plan::TerminalTargetProviderSelection::from_plans(
                    self.realization.provider_declarations().to_vec(),
                    artifact,
                )
            })
            .map(|mut selection| {
                selection.assumed_bindings = self.plans().assumed_bindings().to_vec();
                selection
            })
            .map_err(|reason| vec![format!("provider proposal for `{target}`: {reason}")]);
        Some(backend_plan::TerminalTargetProviderProposal {
            target: target.to_owned(),
            terminal_psi: artifact.identity(),
            optimization_selection,
            application_roots: None,
            application_required: true,
            selection,
        })
    }
}
