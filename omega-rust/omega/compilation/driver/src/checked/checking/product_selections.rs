//! Resolve inert source requests only after Build has joined Product sources.
//! Semantic admission belongs to Psi; this coordinator publishes its results.

use compilation_products::ComputedBuildConfig;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(super) fn admit(
    typed: &TypedTrees,
    computed: &mut ComputedBuildConfig,
) -> Result<(), Vec<Diagnostic>> {
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
    computed.config.opaque_representation_selections = representations;
    computed.config.provider_selections = providers;
    computed.config.behavior_exclusions = trust_model::harvest_behavior_exclusions(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
        &computed.behavior_exclusion_requests,
    )?;
    Ok(())
}
