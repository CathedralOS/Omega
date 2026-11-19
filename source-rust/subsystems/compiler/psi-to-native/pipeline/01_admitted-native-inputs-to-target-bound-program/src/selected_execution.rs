//! The driver finishes selection and folding, then hands the resulting plans,
//! entry nomination and review evidence to a checked compilation.

use crate::target_supply::{RealizedTargetSupply, SelectedProviderRealization};
use diagnostics::Diagnostic;
use std::sync::Arc;

/// The driver owns the remaining target-dependent fold while building a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    result: SelectedProviderResult,
    folded_reach: Option<Result<crate::folded_entry::FoldedEntryReach, String>>,
}

/// Selected plans, realized rows, and entry nomination for this publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderResult {
    original: terminal_psi::TerminalPsiIdentity,
    target_profile: Option<target::TargetProfile>,
    folded: Option<Arc<crate::FoldedTargetProgram>>,
    realization: Arc<SelectedProviderRealization>,
    package_review: Option<Arc<crate::package_review::PackageTargetReview>>,
    entry: Result<crate::program_entry::PublishedEntryNomination, Vec<Diagnostic>>,
}

impl std::ops::Deref for SelectedExecution {
    type Target = SelectedProviderResult;

    fn deref(&self) -> &Self::Target {
        self.result()
    }
}

impl SelectedExecution {
    pub const fn result(&self) -> &SelectedProviderResult {
        &self.result
    }

    pub fn into_result(self) -> SelectedProviderResult {
        self.result
    }

    /// Retains the provider realization already formed for this exact target.
    pub(super) fn from_realization(
        module: &terminal_psi::TerminalModule,
        target_profile: Option<target::TargetProfile>,
        realization: RealizedTargetSupply,
    ) -> Result<Self, Vec<Diagnostic>> {
        if realization.target() != target_profile.map(|profile| profile.target_name()) {
            return Err(vec![Diagnostic::error(
                "selected execution and realized provider supply name different targets",
            )]);
        }
        let entry = crate::program_entry::PublishedEntryNomination::from_module(
            module,
            realization.program(),
            target_profile,
        );
        Ok(Self {
            result: SelectedProviderResult {
                original: realization.program(),
                target_profile,
                realization: Arc::new(realization.into_result()),
                package_review: None,
                folded: None,
                entry,
            },
            folded_reach: None,
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
            self.result.package_review =
                Some(Arc::new(review.finish(module, target, &self.realization)));
        }
        self
    }

    pub(super) fn with_folded_entry_reach(
        mut self,
        reach: Result<crate::folded_entry::FoldedEntryReach, String>,
    ) -> Self {
        self.folded_reach = Some(reach);
        self
    }

    /// Target commitments depend on the admitted native calling contracts. Only
    /// the closed portable reach may request them, and the source artifact is immutable.
    pub fn bind_folded_entry(
        &mut self,
        original: &terminal_codec::CanonicalTerminalArtifact,
        accepted_evidence: Option<&terminal_codec::EvidenceSeals>,
    ) -> Result<(), String> {
        self.target_profile
            .ok_or("executable entry metadata requires an explicit deployment target")?;
        self.realization
            .require_program(original.view().identity())?;
        let reach = self
            .folded_reach
            .as_ref()
            .ok_or("executable entry has no published scalar provider reach")?
            .as_ref()
            .map_err(Clone::clone)?;
        if let Some((realization, folded)) = RealizedTargetSupply::close_folded_entry(
            &self.realization,
            original,
            accepted_evidence,
            reach,
        )? {
            self.result.realization = Arc::new(realization);
            self.result.folded = Some(Arc::new(folded));
            self.folded_reach = None;
        }
        Ok(())
    }

    pub fn with_folded_program(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        Ok(Self {
            result: SelectedProviderResult {
                realization: Arc::new(RealizedTargetSupply::replay_folded_program(
                    &self.realization,
                    artifact,
                )?),
                folded: None,
                ..self.result.clone()
            },
            folded_reach: None,
        })
    }

    /// Rejoin the actual selected declarations to this canonical artifact.
    pub fn provider_proposal(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_selections::OptimizationSelectionIdentity,
    ) -> Option<target_provider_admission::TerminalTargetProviderProposal> {
        let target = self.target_profile?.target_name();
        let selection = self
            .realization
            .require_program(artifact.identity())
            .and_then(|()| {
                target_provider_admission::TerminalTargetProviderSelection::from_plans(
                    self.realization.provider_declarations().to_vec(),
                    artifact,
                )
            })
            .map(|mut selection| {
                selection.assumed_bindings = self.plans().assumed_bindings().to_vec();
                selection
            })
            .map_err(|reason| vec![format!("provider proposal for `{target}`: {reason}")]);
        Some(target_provider_admission::TerminalTargetProviderProposal {
            target: target.to_owned(),
            terminal_psi: artifact.identity(),
            optimization_selection,
            application_roots: None,
            application_required: true,
            selection,
        })
    }
}

impl SelectedProviderResult {
    /// The admitted source program whose selected providers produced the effective artifact.
    pub fn original_program(&self) -> terminal_psi::TerminalPsiIdentity {
        self.original
    }

    pub fn entry_nomination(
        &self,
    ) -> Result<&crate::program_entry::PublishedEntryNomination, Vec<Diagnostic>> {
        self.entry.as_ref().map_err(Clone::clone)
    }

    pub fn package_review(&self) -> Option<&crate::package_review::PackageTargetReview> {
        self.package_review.as_deref()
    }

    pub fn realization(&self) -> &SelectedProviderRealization {
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

    pub fn plans(&self) -> &selected_provider_plans::SelectedProviderPlanFacts {
        self.realization.plans()
    }

    pub fn shared_plans(&self) -> &Arc<selected_provider_plans::SelectedProviderPlanFacts> {
        self.realization.shared_plans()
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
            self.realization
                .require_program(original.view().identity())?;
            Ok(original)
        }
    }
}
