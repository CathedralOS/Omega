// We capture everything a requested target needs (`PreparedProductTarget`) from
// `CheckedProductSource` before shared Terminal code is published. Targets keep
// their own build settings and package permissions, and declaration errors are
// retained inside `Result` fields so preparing an unused declaration never
// rejects a valid program before target provider selection runs.

use super::checking::timed;
use artifacts::compile_timings::{CompileTimings, EXECUTION_SETTLEMENT};
use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::target_machines::{PreparedProductProviders, ProductProviderPreparation};

pub(crate) struct PreparedProductTarget {
    pub(crate) runtime_policy:
        std::sync::Arc<package_evidence::permission_acceptance::PreparedRuntimePolicy>,
    pub(crate) profile: Option<target::TargetProfile>,
    pub(crate) package_inputs: Option<PackageCompilationInputs>,
    pub(crate) declarations: Result<PreparedTargetDeclarations, Vec<Diagnostic>>,
    pub(crate) deployment: Result<(), Vec<Diagnostic>>,
    pub(crate) timings: CompileTimings,
    pub(crate) binding_declarations: std::sync::Arc<selected_dispatch::PreparedBindingDeclarations>,
    pub(crate) direct_calls: Result<
        std::sync::Arc<
            language_semantics::direct_calls::DirectCallDeclarations<symbols::SymbolHandle>,
        >,
        Diagnostic,
    >,
}

pub(crate) struct PreparedTargetDeclarations {
    pub(crate) entry: Result<
        Option<provider_planning::program_entry::PreparedCompilerProgramEntry>,
        Vec<Diagnostic>,
    >,
    pub(crate) providers: Result<PreparedProductProviders, Vec<Diagnostic>>,
    pub(crate) entry_service_requirements: Result<Vec<symbols::SymbolHandle>, Vec<Diagnostic>>,
}

impl PreparedProductTarget {
    pub(crate) fn capture(
        source: &CheckedProductSource,
        profile: Option<target::TargetProfile>,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut package_inputs = package_inputs.cloned();
        if let Some(inputs) = package_inputs.as_ref()
            && let Some(binding) =
                provider_planning::program_entry::discover_explicit_entry_binding(
                    source
                        .provider_inventory
                        .services_for(&source.program)
                        .map_err(|error| vec![error])?,
                    &source.computed_build_config.config.root_bindings,
                    profile,
                    &source.computed_build_config.config.provider_selections,
                    inputs,
                )?
        {
            let mut bindings = inputs
                .accepted_semantic_bindings()
                .cloned()
                .collect::<Vec<_>>();
            bindings.push(binding);
            package_inputs = Some(
                inputs
                    .clone()
                    .with_accepted_semantic_bindings(bindings)
                    .map_err(|errors| {
                        errors
                            .into_iter()
                            .map(|error| {
                                Diagnostic::error(format!("entry candidate inputs: {error:?}"))
                            })
                            .collect::<Vec<_>>()
                    })?,
            );
        }
        let mut timings = if source.timings.is_enabled() {
            CompileTimings::enabled()
        } else {
            CompileTimings::default()
        };
        let deployment = source.computed_build_config.config.provider_deployment();
        let declarations = timed(&mut timings, EXECUTION_SETTLEMENT, |_| {
            let declarations = (|| {
                let services = source
                    .provider_inventory
                    .services_for(&source.program)
                    .map_err(|error| vec![error])?;
                let entry = provider_planning::program_entry::prepare_compiler_program_entry(
                    services,
                    &source.computed_build_config.config.root_bindings,
                    profile,
                    package_inputs.as_ref(),
                );
                let origins = source
                    .provider_inventory
                    .origins_for(&source.program)
                    .map_err(|error| vec![error])?;
                let target_machines = provider_planning::target_machines::settle_provider_defaults(
                    origins,
                    &source.provider_defaults,
                    profile.unwrap_or_else(target::TargetProfile::host),
                );
                Ok((entry, target_machines))
            })();
            let evaluated_bindings = build_settings::evaluate_via_bindings(
                provider_planning::ProviderPlanningContext::new(
                    &source.program,
                    source::DependencyScope::Product,
                ),
                profile.or_else(target::TargetProfile::host_if_supported),
                package_inputs.as_ref(),
            );
            declarations.map(|(entry, target_machines)| {
                let providers = (|| {
                    let deployment = deployment.as_ref().map_err(Clone::clone)?;
                    ProductProviderPreparation {
                        source: &source.program,
                        services: source
                            .provider_inventory
                            .services_for(&source.program)
                            .map_err(|error| vec![error])?,
                        body_declarations: source
                            .provider_inventory
                            .direct_calls_for(&source.program)
                            .map_err(|error| vec![error])?,
                        origins: source.provider_inventory.origins(),
                        defaults: &source.provider_defaults,
                        target_machines,
                        selected_target_profile: profile,
                        evaluated_bindings: evaluated_bindings?,
                        application_roots: deployment.roots.as_deref(),
                        overrides: &source.computed_build_config.config.provider_selections,
                        exact_toolchain_sources: source.sources.exact_toolchain_sources(),
                        native_bindings: &source.native_bindings,
                        native_supplies: &source.computed_build_config.config.native_bindings,
                        assumed_bindings: &source.computed_build_config.config.assumed_bindings,
                    }
                    .capture()
                })();
                let entry_service_requirements = match &entry {
                    Ok(Some(entry)) => entry.service_requirements(&source.program),
                    Ok(None) => Ok(Vec::new()),
                    Err(diagnostics) => Err(diagnostics.clone()),
                };
                PreparedTargetDeclarations {
                    entry,
                    providers,
                    entry_service_requirements,
                }
            })
        });
        let binding_declarations =
            std::sync::Arc::clone(source.binding_declarations.get_or_init(|| {
                std::sync::Arc::new(selected_dispatch::PreparedBindingDeclarations::capture(
                    &source.program,
                    &source.provider_inventory,
                    source.sources.exact_toolchain_sources(),
                ))
            }));
        let direct_calls = source
            .provider_inventory
            .retained_direct_calls_for(&source.program);
        let runtime_policy = std::sync::Arc::new(
            package_evidence::permission_acceptance::PreparedRuntimePolicy::capture(source),
        );
        Ok(Self {
            runtime_policy,
            binding_declarations,
            direct_calls,
            profile,
            package_inputs,
            declarations,
            deployment: deployment.map(|_| ()),
            timings,
        })
    }
}
