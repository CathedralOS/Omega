// A product of one target reads two settled things: which target it is for
// and which checked plans or evaluated native bindings supply the boundary
// requirements its accepted Terminal module reaches. Its only
// constructor takes them from provider settlement, so a product cannot be
// formed for a target whose supply was never settled.
//
// It deliberately carries nothing a product can derive. The selected program
// entry, native provider declarations, calling applications, callback roots
// and review provenance belong to the products that use them and are formed
// there from the selected supplies and the accepted artifact. Native binding
// receipts remain here because their completed Build evaluations cannot be
// reconstructed from the Product's accepted artifact.

use diagnostics::Diagnostic;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    target_profile: Option<target::TargetProfile>,
    plans: Arc<effects::SelectedProviderPlanFacts>,
    native_bindings: Arc<provider_planning::target_supply::NormalizedNativeSupplies>,
}

impl SelectedExecution {
    /// Forms the selected execution from its settled supply. An absent profile
    /// retains unscoped checking, without selecting host providers. A chosen supply
    /// whose provider plan cannot be formed refuses here, by requirement.
    pub fn settle(
        target_profile: Option<target::TargetProfile>,
        supply: &provider_planning::target_supply::SettledTargetSupply,
        planning: provider_planning::target_supply::SupplyPlanning<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if target_profile.map(|profile| profile.target_name()) != supply.target() {
            return Err(vec![Diagnostic::error(
                "selected execution and settled provider supply name different deployments",
            )]);
        }
        let native_bindings = supply.native_bindings(planning.module)?;
        let plans =
            effects::SelectedProviderPlanFacts::from_selected_plans(supply.plans(planning)?)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "provider settlement for `{}`: {reason}",
                        supply.target().unwrap_or("<unscoped>")
                    ))]
                })?;
        let plans = planning.resolve_installation_reach(plans)?;
        Ok(Self {
            target_profile,
            plans: Arc::new(plans),
            native_bindings: Arc::new(native_bindings),
        })
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
        &self.plans
    }

    pub fn shared_plans(&self) -> &Arc<effects::SelectedProviderPlanFacts> {
        &self.plans
    }

    pub fn native_bindings(&self) -> &provider_planning::target_supply::NormalizedNativeSupplies {
        &self.native_bindings
    }

    /// The provider proposal this target's conditions are checked against:
    /// its settled plans joined to one produced Terminal artifact.
    ///
    /// Each plan's checked adapter rows are joined to the provider
    /// candidates the artifact states for the plan's provider declaration
    /// (`TerminalTargetProviderSelection::from_plans`); a row the artifact
    /// states no candidate for refuses the selection, and the refusal
    /// travels in the proposal like any other. A target with no plan
    /// proposes the empty selection of that artifact.
    pub fn provider_proposal(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_core::OptimizationSelectionIdentity,
    ) -> Option<backend_plan::TerminalTargetProviderProposal> {
        let target = self.target_profile?.target_name();
        let selection = backend_plan::TerminalTargetProviderSelection::from_plans(
            self.plans.plans().to_vec(),
            artifact,
        )
        .map(|mut selection| {
            selection.assumed_bindings = self.plans.assumed_bindings().to_vec();
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
