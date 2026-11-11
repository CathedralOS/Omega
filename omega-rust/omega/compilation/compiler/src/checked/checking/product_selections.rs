// We publish Product policy choices (`admit`) after package authentication and
// Psi source admission (`source.admit_product_policy`). Evaluation receipts
// remain Build custody while semantic selections (grants, opaque
// representations, provider selections, native bindings, service audits,
// assumed bindings, wire demands, and behavior exclusions) populate
// `ComputedBuildConfig`.

use compilation_products::ComputedBuildConfig;
use diagnostics::Diagnostic;

pub(super) fn admit(
    source: &mut psi_compiler::SourceContinuation,
    computed: &mut ComputedBuildConfig,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    let mut default_providers = package_inputs
        .map(|inputs| inputs.rejoin_package_provider_selections(source))
        .transpose()?
        .unwrap_or_default();
    if let Some(defaults) = &computed.toolchain_provider_defaults {
        let capture = defaults.toolchain_capture().ok_or_else(|| {
            vec![Diagnostic::error(
                "core defaults have a different supplier kind",
            )]
        })?;
        default_providers.extend(defaults.rejoin(
            source,
            package_compilation::ProviderDefaultsAdmission::CapturedToolchain(capture),
        )?);
    }
    let admitted = source.admit_product_policy(psi_compiler::ProductPolicyRequest {
        activation: computed.selected_build_machine_symbol,
        declared: &computed.declared_product_selections,
        executed: &computed.product_selection_requests,
        native_bindings: &computed.native_binding_requests,
        exclusions: &computed.behavior_exclusion_requests,
    })?;

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
    for (inputs, bundle) in package_inputs.into_iter().flat_map(|inputs| {
        inputs
            .product_dependency_native_handoffs()
            .map(move |bundle| (inputs, bundle))
    }) {
        let declarations = inputs.admit_package_native_declarations(bundle)?;
        let selections =
            source.admit_captured_native_bindings(&declarations, bundle.native_intents())?;
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
    computed.config.default_provider_selections = default_providers;
    computed.config.native_bindings = native_bindings;
    computed.config.service_audits = admitted.declarations.service_audits;
    computed.config.assumed_bindings = admitted.declarations.assumed_bindings;
    computed.config.wire_compatibility_demands = admitted.declarations.wire_demands;
    computed.config.test_groups = admitted.declarations.test_groups;
    computed.config.behavior_exclusions = admitted.exclusions;
    Ok(())
}
