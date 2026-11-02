//! Choose semantic helpers over the shared Terminal invocation graph.
//!
//! Calling uses neutral contracts: its ABI result does not yet exist. The
//! ordinary canonical selector closes exact invocation demand through selected
//! providers. Invocation admission consumes the same canonical selected graph;
//! no Source body substitutions or second helper-selection map are retained.
//! Neutral commitments never become final native calling identities.

use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use symbols::SymbolHandle;

pub struct CallingPolicyProviderInputs<'program> {
    pub body_declarations: &'program DirectCallDeclarations<SymbolHandle>,
    pub defaults: &'program [crate::ProviderSelection],
    pub requested: &'program [crate::ProviderSelection],
    pub exact_toolchain_sources: &'program [(source::SourceId, [u8; 32])],
}

pub(super) struct SelectedPolicyProviders {
    pub declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
}

pub(super) fn prepare(
    services: ServiceCatalog<'_>,
    target: target::TargetProfile,
    declarations: &[terminal_psi::ProofDeclarationId],
    inputs: &CallingPolicyProviderInputs<'_>,
    code: &super::CallingProgram,
) -> Result<SelectedPolicyProviders, Vec<Diagnostic>> {
    let failure = |reason: String| vec![Diagnostic::error(reason)];
    let artifact = code.artifact.view();
    let inventory = crate::target_machines::AuthoredTargetProviders::unevaluated(
        services,
        inputs.body_declarations,
        inputs.defaults,
        target,
    )?;
    let prepared = crate::target_machines::TargetProviderPreparationRequest {
        context: services.context(),
        exact_toolchain_sources: inputs.exact_toolchain_sources,
        application_roots: None,
        application_scopes: &[],
        overrides: inputs.requested,
        assumed_bindings: &[],
        independent_components: &[],
        selected: None,
        targets: &[],
    }
    .prepare_target(target.target_name(), &inventory, &[])?;
    let selected = prepared.select(
        artifact,
        provider_selection::ProviderApplicationDemand::Invocation(declarations),
        &[],
    )?;
    for choice in &selected.choices {
        if !matches!(
            choice.selected_by,
            provider_selection::SelectionProvenance::BuildOverride(_)
                | provider_selection::SelectionProvenance::TargetDefault(_)
        ) {
            return Err(failure("calling-policy helper requires an exact Build or target-default provider selection".into()));
        }
    }

    Ok(SelectedPolicyProviders {
        declarations: selected.selection.plans,
    })
}
