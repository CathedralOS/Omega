//! Capture occurrence-local Source inputs before the shared Product publication.
//!
//! A target chooses realization, never a pruned Source program. Each occurrence
//! retains its own augmented package authority and measured binding results;
//! sharing portable code cannot substitute another occurrence's admission.
//! Results stay deferred until their original settlement boundaries.

use super::checking::timed;
use artifacts::compile_timings::{CompileTimings, EXECUTION_SETTLEMENT};
use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::SettledTargetMachineDeclarations;

pub(crate) struct PreparedProductTarget {
    pub(crate) profile: Option<target::TargetProfile>,
    pub(crate) package_inputs: Option<PackageCompilationInputs>,
    pub(crate) declarations: Result<PreparedTargetDeclarations, Vec<Diagnostic>>,
    pub(crate) evaluated_bindings: Result<
        provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable,
        Vec<Diagnostic>,
    >,
    pub(crate) timings: CompileTimings,
}

pub(crate) struct PreparedTargetDeclarations {
    pub(crate) entry: Result<
        Option<provider_planning::program_entry::PreparedCompilerProgramEntry>,
        Vec<Diagnostic>,
    >,
    pub(crate) target_machines: SettledTargetMachineDeclarations,
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
        let (declarations, evaluated_bindings) = timed(&mut timings, EXECUTION_SETTLEMENT, |_| {
            // Ownership/default acquisition used to refuse before Calling, but
            // a formed entry's refusal belongs after provider selection.
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
                Ok(PreparedTargetDeclarations {
                    entry,
                    target_machines,
                })
            })();
            // Evaluate the same complete authored population under this actual
            // occurrence's fallback target. Do not filter inactive authored
            // leaves or execute an arbitrary catalog of unrequested profiles.
            // ConstEvaluable admission has no ambient host effects. Retain its
            // complete Result so Calling/publication failures still win.
            let evaluated_bindings =
                provider_planning::evaluated_via_bindings::evaluate_checked_via_bindings(
                    &source.program,
                    source::DependencyScope::Product,
                    profile.or_else(target::TargetProfile::host_if_supported),
                    package_inputs.as_ref(),
                );
            (declarations, evaluated_bindings)
        });
        Ok(Self {
            profile,
            package_inputs,
            declarations,
            evaluated_bindings,
            timings,
        })
    }
}
