//! Publish Product choices after package authentication and Psi source admission.
//! Evaluation receipts remain Build custody; semantic choices come back owned.

use compilation_products::ComputedBuildConfig;
use diagnostics::Diagnostic;

pub(super) fn admit(
    source: &mut psi_compiler::SourceContinuation,
    computed: &mut ComputedBuildConfig,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
    handoffs: &package_compilation::BuildHandoffInventory,
) -> Result<(), Vec<Diagnostic>> {
    let mut package_providers = Vec::new();
    for selection in
        handoffs.admitted_provider_selections(package_inputs, source::DependencyScope::Product)?
    {
        package_providers.push(
            source
                .rejoin_product_provider_selection(&selection)
                .map_err(|error| vec![error])?,
        );
    }
    let admitted = source.admit_product_policy(psi_compiler::ProductPolicyRequest {
        activation: computed.selected_build_machine_symbol,
        declared: &computed.declared_product_selections,
        executed: &computed.product_selection_requests,
        native_bindings: &computed.native_binding_requests,
        exclusions: &computed.behavior_exclusion_requests,
    })?;

    // Root source formation completes before receipt attachment or dependency
    // resolution. These independent custody checks cannot change source meaning.
    let mut native_bindings = Vec::new();
    for selection in admitted.native_bindings {
        let evaluation = computed
            .config
            .native_binding_evaluation
            .clone()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "native Build selection lost its completed evaluation",
                )]
            })?;
        native_bindings.push(provider_planning::native_bindings::NativeBuildSupply {
            selection,
            evaluation,
        });
    }
    for (_, bundle) in handoffs
        .handoffs()
        .filter(|(scope, _)| *scope == build_declarations::DependencyPurpose::Product)
    {
        let selections = source.admit_captured_native_bindings(
            bundle.native_declarations(),
            bundle.native_intents(),
        )?;
        for selection in selections {
            let evaluation = bundle.native_evaluation().cloned().ok_or_else(|| {
                vec![Diagnostic::error(
                    "native dependency effect lost its completed evaluation",
                )]
            })?;
            native_bindings.push(provider_planning::native_bindings::NativeBuildSupply {
                selection,
                evaluation,
            });
        }
    }

    computed.config.grants = admitted.grants;
    computed.config.opaque_representation_selections = admitted.representations;
    computed.config.provider_selections = admitted.providers;
    computed.config.package_provider_selections = package_providers;
    computed.config.native_bindings = native_bindings;
    computed.config.service_audits = admitted.declarations.service_audits;
    computed.config.assumed_bindings = admitted.declarations.assumed_bindings;
    computed.config.wire_compatibility_demands = admitted.declarations.wire_demands;
    computed.config.behavior_exclusions = admitted.exclusions;
    Ok(())
}
