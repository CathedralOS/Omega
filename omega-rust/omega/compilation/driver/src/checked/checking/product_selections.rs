//! Resolve inert source requests only after Build has joined Product sources.
//! Semantic admission belongs to Psi; this coordinator publishes its results.

mod additional_declarations;
mod exclusions;
mod native_bindings;

use compilation_products::ComputedBuildConfig;
use diagnostics::Diagnostic;

pub(super) fn admit(
    source: &mut psi_compiler::SourceContinuation,
    computed: &mut ComputedBuildConfig,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    let typed = source.typed();
    let representations = representation_planning::collect_opaque_representation_selections(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
    )?;
    let providers =
        build_time_evaluation::provider_declarations::collect_product_provider_selections(
            build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
                typed,
                source::DependencyScope::Product,
            ),
            computed.selected_build_machine_symbol,
            &computed.declared_product_selections,
            &computed.product_selection_requests,
        )?;
    let grants = trust_model::collect_product_root_grants(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
    )?;
    computed.config.grants = grants;
    computed.config.opaque_representation_selections = representations;
    computed.config.provider_selections = providers;
    computed.config.package_provider_selections = package_inputs
        .map(|inputs| inputs.rejoin_package_provider_selections(typed))
        .transpose()?
        .unwrap_or_default();
    additional_declarations::admit(typed, computed)?;
    native_bindings::admit(source, computed)?;
    if let Some(inputs) = package_inputs {
        let retained = inputs.retained_package_native_bindings()?;
        let inherited = provider_planning::rejoin_package_native_bindings(source.typed(), retained)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        computed.config.admitted_native_bindings.extend(inherited);
    }
    provider_planning::unify_native_binding_identities(
        &mut computed.config.admitted_native_bindings,
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let typed = source.typed();
    computed.config.behavior_exclusions = exclusions::resolve(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
        &computed.behavior_exclusion_requests,
    )?;
    Ok(())
}
