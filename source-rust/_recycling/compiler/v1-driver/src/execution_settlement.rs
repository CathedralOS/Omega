// Provider settlement follows actual invocations from the accepted entry.
// Choosing a whole provider adds its exact boundary-to-body associations, then
// the same call query substitutes actual arguments through those bodies. Code
// retained by a map or descriptor never becomes an independent entry root.

use crate::checked::CheckedCompilation;
use crate::checked::{PreparedProductTarget, PreparedRuntimeConsent};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use products::SelectedExecution;
use products::{
    BuildAdmission, CheckedCompilationInputs, CheckedCompileOutcome, CheckedProductSource,
    CompileFailure,
};
use source_compiler::provider_supply::{SourceProviderDemand, SourceProviderFacts};
use std::sync::Arc;

pub(crate) struct SelectedProviderCustody {
    pub(crate) optimizations: optimization_core::OptimizationSelections,
    pub(crate) terminal_publication: Arc<source_compiler::SourcePublication>,
    pub(crate) published_program: Arc<portable_publication::PublishedProgram>,
    pub(crate) source_completion_accounting:
        source_compiler::evaluation::SourceCompletionAccounting,
    pub(crate) execution: SelectedExecution,
    pub(crate) source_inspection: Arc<products::SelectedProviderSource>,
    pub(crate) build: Arc<products::ComputedBuildConfig>,
    pub(crate) build_admission: BuildAdmission,
    pub(crate) sources: Arc<package_compilation::CheckedSourceCustody>,
    pub(crate) timings: CompileTimings,
}

pub(crate) struct SelectedProductTarget {
    pub(crate) consent: Option<PreparedRuntimeConsent>,
    pub(crate) advisories: Option<Vec<Diagnostic>>,
    pub(crate) custody: SelectedProviderCustody,
}

impl SelectedProductTarget {
    pub(crate) fn select(
        source: &CheckedProductSource,
        prepared: PreparedProductTarget,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut timings = source.timings.clone();
        let (published_program, execution) =
            settle_execution(source, prepared.profile, prepared.demand, &mut timings)?;
        Ok(Self {
            consent: prepared.consent,
            advisories: prepared.advisories,
            custody: SelectedProviderCustody {
                optimizations: source.optimizations.clone(),
                terminal_publication: Arc::clone(&source.terminal_publication),
                published_program,
                source_completion_accounting: source.source_completion_accounting.clone(),
                source_inspection: Arc::new(products::SelectedProviderSource::new(
                    Arc::clone(&source.provider_inventory),
                    Arc::clone(execution.shared_plans()),
                )),
                execution,
                build: Arc::clone(&source.computed_build_config),
                build_admission: source.admission.clone(),
                sources: Arc::clone(&source.sources),
                timings,
            },
        })
    }

    // Checking one target on its own: the accepted program is produced, its
    // target-local conditions and task activations are checked, and the
    // checked compilation is sealed.
    pub(crate) fn finish_check(mut self) -> Result<CheckedCompilation, CompileFailure> {
        let checked = (|| -> Result<(), Vec<Diagnostic>> {
            let terminal = crate::terminal::check_terminal_conditions(&mut self.custody)?;
            self.check_declaration_requirements(terminal.module())
        })();
        if let Err(diagnostics) = checked {
            return Err(CompileFailure::source_completion(
                diagnostics,
                self.custody.source_completion_accounting,
            ));
        }
        self.into_checked_compilation()
    }

    pub(crate) fn check_declaration_requirements(
        &self,
        module: &terminal_psi::TerminalModule,
    ) -> Result<(), Vec<Diagnostic>> {
        let execution = &self.custody.execution;
        if module.task_activation_demands.demands.is_empty() {
            return Ok(());
        }
        let profile = execution.target_profile().ok_or_else(|| {
            vec![Diagnostic::error(
                "task activation requires an explicit deployment target",
            )]
        })?;
        task_plans::realize_task_activations(
            &module.task_activation_demands,
            &module.identities,
            execution.plans(),
            profile,
        )?;
        Ok(())
    }

    pub(crate) fn into_checked_compilation(self) -> Result<CheckedCompilation, CompileFailure> {
        let SelectedProviderCustody {
            optimizations: _,
            terminal_publication,
            published_program,
            source_completion_accounting,
            execution,
            source_inspection,
            build,
            build_admission,
            sources,
            timings,
        } = self.custody;
        CheckedCompilation::from_settled_execution(
            CheckedCompilationInputs {
                settled: execution.into_result(),
                source_inspection,
                build,
                build_admission,
            },
            sources,
            timings,
            terminal_publication,
        )
        .and_then(|checked| checked.with_published_program(published_program))
        .map_err(|diagnostics| {
            CompileFailure::source_completion(diagnostics, source_completion_accounting.clone())
        })
        .map(|checked| checked.with_source_completion_accounting(source_completion_accounting))
    }
}

fn settle_execution(
    source: &CheckedProductSource,
    profile: Option<target::TargetProfile>,
    demand: SourceProviderFacts<SourceProviderDemand>,
    timings: &mut CompileTimings,
) -> Result<
    (
        Arc<portable_publication::PublishedProgram>,
        SelectedExecution,
    ),
    Vec<Diagnostic>,
> {
    let demand = portable_publication::ProductCodeSource {
        publication: &source.terminal_publication,
        sources: &source.sources,
    }
    .publish_provider_demand(
        profile,
        demand,
        &mut crate::observations::PsiStageTimings::new(timings),
    )?;
    let program = source
        .terminal_publication
        .require_published(source.terminal_publication.optimizations())?;
    let producer = terminal_installation::HostProducerTrust::accept("in-process source compiler");
    let admitted = compilation_inputs::admit_published_provider_demand(
        Arc::clone(program.artifact()),
        Some(&producer),
        demand,
    )
    .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
    Ok((
        program,
        native_compiler::select_published_providers(admitted)?,
    ))
}

pub(crate) struct SelectedCompileOutcome {
    pub(crate) result: Result<SelectedProductTarget, CompileFailure>,
    pub(crate) independent_components: products::IndependentComponentDiscovery,
    // Separately selected test roots over the same checked source, in roster
    // order (wiki/spec/build/testing.md). Empty when no enabled group exists.
    pub(crate) tests: Vec<(
        products::TestRootIdentity,
        Result<SelectedProductTarget, CompileFailure>,
    )>,
    pub(crate) test_notes: Vec<products::TestRosterEntry>,
}

impl SelectedCompileOutcome {
    pub(crate) fn finish_check(self) -> CheckedCompileOutcome {
        CheckedCompileOutcome {
            result: self.result.and_then(SelectedProductTarget::finish_check),
            independent_components: self.independent_components,
        }
    }
}

pub(crate) fn select_product_target(
    source: &CheckedProductSource,
    prepared: Result<PreparedProductTarget, Vec<Diagnostic>>,
) -> SelectedCompileOutcome {
    SelectedCompileOutcome {
        result: prepared
            .and_then(|prepared| SelectedProductTarget::select(source, prepared))
            .map_err(|diagnostics| {
                CompileFailure::source_completion(
                    diagnostics,
                    source.source_completion_accounting.clone(),
                )
            }),
        independent_components: source.computed_build_config.independent_components(),
        tests: Vec::new(),
        test_notes: Vec::new(),
    }
}
