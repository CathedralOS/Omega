// Selected execution owns one realized provider closure. Native and Psi
// consumers borrow its canonical plan roster and exact row execution data;
// choosing a provider never reconstructs declarations from source later.

use diagnostics::Diagnostic;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    target_profile: Option<target::TargetProfile>,
    realization: Arc<provider_planning::target_supply::RealizedTargetSupply>,
}

impl SelectedExecution {
    /// Retains the provider realization already formed for this exact target.
    pub fn from_realization(
        target_profile: Option<target::TargetProfile>,
        realization: provider_planning::target_supply::RealizedTargetSupply,
    ) -> Result<Self, Vec<Diagnostic>> {
        if realization.target() != target_profile.map(|profile| profile.target_name()) {
            return Err(vec![Diagnostic::error(
                "selected execution and realized provider supply name different targets",
            )]);
        }
        Ok(Self {
            target_profile,
            realization: Arc::new(realization),
        })
    }

    pub fn settle(
        target_profile: Option<target::TargetProfile>,
        supply: &provider_planning::target_supply::SettledTargetSupply,
    ) -> Result<Self, Vec<Diagnostic>> {
        if target_profile.map(|profile| profile.target_name()) != supply.target() {
            return Err(vec![Diagnostic::error(
                "selected execution and settled provider supply name different deployments",
            )]);
        }
        Self::from_realization(target_profile, supply.realize_no_demand()?)
    }

    pub fn realization(&self) -> &provider_planning::target_supply::RealizedTargetSupply {
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

    pub fn plans(&self) -> &effects::SelectedProviderPlanFacts {
        self.realization.plans()
    }

    pub fn shared_plans(&self) -> &Arc<effects::SelectedProviderPlanFacts> {
        self.realization.shared_plans()
    }

    /// Rejoin the actual selected declarations to this canonical artifact.
    pub fn provider_proposal(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_core::OptimizationSelectionIdentity,
    ) -> Option<backend_plan::TerminalTargetProviderProposal> {
        let target = self.target_profile?.target_name();
        let selection = backend_plan::TerminalTargetProviderSelection::from_plans(
            self.realization.provider_declarations(),
            artifact,
        )
        .map(|mut selection| {
            selection.assumed_bindings = self.plans().assumed_bindings().to_vec();
            selection
        })
        .map_err(|reason| vec![reason]);
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
