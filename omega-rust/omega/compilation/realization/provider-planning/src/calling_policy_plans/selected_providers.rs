// We choose semantic helpers over the shared Terminal invocation graph for
// calling-policy evaluation (`CallingPolicyProviderInputs`). Calling uses
// neutral contracts because its ABI result does not yet exist; the ordinary
// canonical selector closes exact invocation demand through selected providers
// (`BuildOverride` or `TargetDefault`) without retaining Source body
// substitutions or a second helper-selection map.

use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use symbols::SymbolHandle;
use typed_trees::service_declarations::ServiceCatalog;

pub struct CallingPolicyProviderInputs<'program> {
    pub body_declarations: &'program DirectCallDeclarations<SymbolHandle>,
    pub origins: &'program [crate::SelectedTargetMachineOrigin],
    pub defaults: &'program [crate::TargetProviderDefaults],
    pub requested: &'program [crate::ProviderSelection],
    pub exact_toolchain_sources: &'program [(source::SourceId, [u8; 32])],
}

pub(super) fn prepare(
    source: &crate::PublishedProviderSource,
    services: ServiceCatalog<'_>,
    inputs: &CallingPolicyProviderInputs<'_>,
) -> Vec<(
    target::TargetProfile,
    Result<provider_selection::PreparedTargetChoices, Vec<Diagnostic>>,
)> {
    target::TargetProfile::ALL
        .into_iter()
        .map(|target| {
            let prepared = if target.native_realization().is_none() {
                Err(vec![Diagnostic::error(format!(
                    "target `{}` has no native Calling-policy realization",
                    target.target_name(),
                ))])
            } else {
                prepare_target(source, services, target, inputs)
            };
            (target, prepared)
        })
        .collect()
}

fn prepare_target(
    source: &crate::PublishedProviderSource,
    services: ServiceCatalog<'_>,
    target: target::TargetProfile,
    inputs: &CallingPolicyProviderInputs<'_>,
) -> Result<provider_selection::PreparedTargetChoices, Vec<Diagnostic>> {
    let defaults =
        crate::target_machines::settle_provider_defaults(inputs.origins, inputs.defaults, target);
    let inventory = crate::target_machines::AuthoredTargetProviders::unevaluated(
        services,
        inputs.body_declarations,
        &defaults.provider_defaults,
        target,
    )?;
    crate::target_machines::TargetProviderPreparationRequest {
        context: services.context(),
        source,
        exact_toolchain_sources: inputs.exact_toolchain_sources,
        application_roots: None,
        overrides: inputs.requested,
    }
    .prepare_target(target.target_name(), &inventory, &[])
    .map(|prepared| prepared.without_correspondence())
}

pub(super) fn select(
    target: target::TargetProfile,
    declarations: &[terminal_psi::ProofDeclarationId],
    code: &super::CallingProgram,
) -> Result<Vec<backend_plan::TerminalTargetProviderPlan>, Vec<Diagnostic>> {
    let prepared = code
        .source_preparation
        .helpers
        .iter()
        .find(|(profile, _)| *profile == target)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "Calling lost its retained target helper candidates",
            )]
        })?
        .1
        .as_ref()
        .map_err(Clone::clone)?
        .clone();
    let selected = prepared.select(
        code.artifact.view(),
        provider_selection::ProviderApplicationDemand::Invocation(declarations),
        &[],
    )?;
    for choice in &selected.choices {
        if !matches!(
            choice.selected_by,
            provider_selection::SelectionProvenance::BuildOverride(_)
                | provider_selection::SelectionProvenance::TargetDefault(_)
                | provider_selection::SelectionProvenance::TargetRealization(_)
        ) {
            return Err(vec![Diagnostic::error(
                "calling-policy helper requires an exact Build, target-default, or target-realized provider selection",
            )]);
        }
    }

    Ok(selected.selection.plans)
}
