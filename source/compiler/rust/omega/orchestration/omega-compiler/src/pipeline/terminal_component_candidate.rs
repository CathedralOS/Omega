pub use omega_terminal_component_candidate::{
    TerminalComponentCandidate, TerminalComponentCandidateParts, TerminalComponentProviderExecution,
};
use psi_diagnostics::Diagnostic;

/// Compatibility name for component callers. The supplied evidence belongs to
/// native realization; component staging only layers entry/progress policy.
pub type TerminalComponentProviderSettlement<'execution> =
    super::terminal_native_artifact::TerminalNativeProviderSettlement<'execution>;

/// Layer component-specific entry and progress identity over the universal
/// Terminal-native artifact. Runtime installation and publication deliberately
/// do not occur here.
pub fn stage_terminal_component(
    terminal_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    entry_machine: &str,
    target: omega_target::NativeTarget,
    subsystem: u16,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::OptimizationSelections,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    component_progress: Option<&omega_effects::ComponentProgressManifest>,
    settlements: &[TerminalComponentProviderSettlement<'_>],
) -> Result<TerminalComponentCandidate, Vec<Diagnostic>> {
    let native_artifact = super::terminal_native_artifact::realize_terminal_native_artifact(
        terminal_artifact,
        target,
        subsystem,
        profile,
        optimization_selections,
        selected_provider_plans,
        settlements,
    )?;
    TerminalComponentCandidate::checked(TerminalComponentCandidateParts {
        native_artifact,
        entry_machine: entry_machine.to_owned(),
        selected_provider_plans: selected_provider_plans.clone(),
        component_progress: component_progress
            .filter(|manifest| !manifest.pending().is_empty())
            .cloned(),
    })
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "Terminal component policy replay failed: {error}"
        ))]
    })
}
