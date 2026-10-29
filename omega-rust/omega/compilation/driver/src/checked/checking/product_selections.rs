//! Resolve inert source requests only after Build has joined Product sources.
//! Semantic admission belongs to Psi; this coordinator publishes its results.

mod additional_declarations;
mod exclusions;

use compilation_products::ComputedBuildConfig;
use diagnostics::Diagnostic;

pub(super) fn admit(
    source: &mut psi_compiler::SourceContinuation,
    computed: &mut ComputedBuildConfig,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    let package_providers = package_inputs
        .map(|inputs| inputs.rejoin_package_provider_selections(source))
        .transpose()?
        .unwrap_or_default();
    let providers = source.admit_product_provider_requests(
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
    )?;
    let typed = source.typed();
    let representations = representation_planning::collect_opaque_representation_selections(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
    )?;
    let native = build_time_evaluation::native_bindings::collect(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.native_binding_requests,
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
    computed.config.package_provider_selections = package_providers;
    computed.config.native_bindings = native
        .into_iter()
        .map(|selection| {
            let evaluation = computed
                .config
                .native_binding_evaluation
                .clone()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "native Build selection lost its completed evaluation",
                    )]
                })?;
            Ok(provider_planning::native_bindings::NativeBuildSupply {
                selection,
                evaluation,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    for bundle in package_inputs
        .into_iter()
        .flat_map(|inputs| inputs.product_dependency_native_handoffs())
    {
        let selections = build_time_evaluation::native_bindings::collect_captured(
            typed,
            bundle.native_declarations(),
            bundle.native_intents(),
        )?;
        for selection in selections {
            let evaluation = bundle.native_evaluation().cloned().ok_or_else(|| {
                vec![Diagnostic::error(
                    "native dependency effect lost its completed evaluation",
                )]
            })?;
            computed.config.native_bindings.push(
                provider_planning::native_bindings::NativeBuildSupply {
                    selection,
                    evaluation,
                },
            );
        }
    }
    additional_declarations::admit(typed, computed)?;
    computed.config.behavior_exclusions = exclusions::resolve(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
        &computed.behavior_exclusion_requests,
    )?;
    Ok(())
}
