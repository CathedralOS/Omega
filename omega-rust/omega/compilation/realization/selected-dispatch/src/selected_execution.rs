// A product of one target reads two settled things: which target it is for
// and which provider plans supply the boundary requirements its accepted
// Terminal module reaches. This record holds exactly those, and its only
// constructor takes them from provider settlement, so a product cannot be
// formed for a target whose supply was never settled.
//
// It deliberately carries nothing a product can derive. The selected program
// entry, native provider declarations, calling applications, callback roots
// and review provenance belong to the products that use them and are formed
// there from these plans and the accepted artifact; a product whose inputs
// have no producer yet refuses by name instead of reading a retained copy.

use diagnostics::Diagnostic;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    target_profile: Option<target::TargetProfile>,
    plans: Arc<effects::SelectedProviderPlanFacts>,
}

impl SelectedExecution {
    /// Forms the selected execution of `target_profile` (the host when the
    /// request named no target) from its settled supply. A chosen supply
    /// whose provider plan cannot be formed refuses here, by requirement.
    pub fn settle(
        target_profile: Option<target::TargetProfile>,
        supply: &provider_planning::target_supply::SettledTargetSupply,
    ) -> Result<Self, Vec<Diagnostic>> {
        let plans = effects::SelectedProviderPlanFacts::from_selected_plans(supply.plans()?)
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "provider settlement for `{}`: {reason}",
                    supply.target()
                ))]
            })?;
        Ok(Self {
            target_profile,
            plans: Arc::new(plans),
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

    /// The provider proposal this target's conditions are checked against:
    /// its settled plans joined to one produced Terminal artifact.
    ///
    /// A settled plan becomes a Terminal provider plan only with the
    /// declaration and numeric realizations of its rows. Nothing on the
    /// lowered route produces those yet, so each plan is refused by name and
    /// the refusal travels in the proposal like any other; a target with no
    /// plan proposes the empty selection of that artifact.
    pub fn provider_proposal(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_core::OptimizationSelectionIdentity,
    ) -> backend_plan::TerminalTargetProviderProposal {
        let target = self
            .target_profile
            .unwrap_or_else(target::TargetProfile::host)
            .target_name();
        let refusals = self
            .plans
            .plans()
            .iter()
            .map(|plan| {
                format!(
                    "no product route yet: provider proposal for `{target}`: provider plan `{}` has no Terminal declaration or numeric realization on this route",
                    plan.name
                )
            })
            .collect::<Vec<_>>();
        let selection = if refusals.is_empty() {
            backend_plan::TerminalTargetProviderSelection::from_plans(Vec::new(), artifact)
                .map(|mut selection| {
                    selection.assumed_bindings = self.plans.assumed_bindings().to_vec();
                    selection
                })
                .map_err(|reason| vec![reason])
        } else {
            Err(refusals)
        };
        backend_plan::TerminalTargetProviderProposal {
            target: target.to_owned(),
            terminal_psi: artifact.identity(),
            optimization_selection,
            application_roots: None,
            application_required: true,
            selection,
        }
    }
}
