// Provider settlement follows actual invocations from the accepted entry.
// Choosing a whole provider adds its exact boundary-to-body associations, then
// the same call query substitutes actual arguments through those bodies. Code
// retained by a map or descriptor never becomes an independent entry root.

use crate::checked::CheckedCompilation;
use crate::checked::PreparedProductTarget;
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use native_program_planning::SelectedExecution;
use products::{
    BuildAdmission, CheckedCompilationInputs, CheckedCompileOutcome, CheckedProductSource,
    CompileFailure,
};
use std::sync::Arc;

pub(crate) struct SelectedProviderCustody {
    pub(crate) optimizations: optimization_core::OptimizationSelections,
    pub(crate) terminal_publication: Arc<source_compiler::SourcePublication>,
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
    pub(crate) runtime_policy: Arc<review_evidence::permission_acceptance::PreparedRuntimePolicy>,
    pub(crate) custody: SelectedProviderCustody,
}

impl SelectedProductTarget {
    pub(crate) fn select(
        source: &CheckedProductSource,
        prepared: PreparedProductTarget,
    ) -> Result<Self, Vec<Diagnostic>> {
        let PreparedProductTarget { profile, entry } = prepared;
        let execution = settle_execution(source, profile, entry)?;
        Ok(Self {
            runtime_policy: Arc::new(
                review_evidence::permission_acceptance::PreparedRuntimePolicy::capture(source),
            ),
            custody: SelectedProviderCustody {
                optimizations: source.optimizations.clone(),
                terminal_publication: Arc::clone(&source.terminal_publication),
                source_completion_accounting: source.source_completion_accounting.clone(),
                source_inspection: Arc::new(products::SelectedProviderSource::new(
                    Arc::clone(&source.provider_inventory),
                    Arc::clone(execution.shared_plans()),
                )),
                execution,
                build: Arc::clone(&source.computed_build_config),
                build_admission: source.admission.clone(),
                sources: Arc::clone(&source.sources),
                timings: source.timings.clone(),
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
                settled: execution,
                source_inspection,
                build,
                build_admission,
            },
            sources,
            timings,
            terminal_publication,
        )
        .map_err(|diagnostics| {
            CompileFailure::source_completion(diagnostics, source_completion_accounting.clone())
        })
        .map(|checked| checked.with_source_completion_accounting(source_completion_accounting))
    }
}

fn settle_execution(
    source: &CheckedProductSource,
    profile: Option<target::TargetProfile>,
    entry: Option<semantic_vocabulary::MachineId>,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    use native_program_planning::NativeProgramPlanningError;
    use native_program_planning::target_supply::TargetSupplyRealizationError;
    let accepted = source.terminal_publication.accepted()?;
    let declared = accepted
        .boundary_requirements_declared()
        .map_err(|error| vec![error])?
        .into_iter()
        .map(
            |row| source_compiler::provider_supply::ReachedBoundaryRequirement {
                boundary: row.boundary,
                requirement: row.requirement,
                slot: row.slot,
                name: row.name,
            },
        )
        .collect::<Vec<_>>();
    let selected_requirements = source_compiler::provider_supply::explicit_build_requirements(
        &declared,
        &source.computed_build_config.config.provider_selections,
        profile.map(|profile| profile.target_name()),
    );
    let Some(target) = profile else {
        if !selected_requirements.is_empty() {
            return Err(declared
                .iter()
                .filter(|row| selected_requirements.binary_search(&row.boundary).is_ok())
                .map(|row| {
                    Diagnostic::error(format!(
                        "boundary requirement `{}` requires an explicit deployment target",
                        row.name,
                    ))
                })
                .collect());
        }
        let roots = source
            .terminal_publication
            .roster()?
            .entries
            .iter()
            .map(|symbol| {
                accepted.machine_of(*symbol).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "unscoped source lost its actual invocation root",
                    )]
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        return native_program_planning::plan_unscoped_execution(accepted.module(), &roots);
    };
    let closed = accepted.close_logical_provider_reach(
        target.target_name(),
        entry,
        &selected_requirements,
    )?;
    let reached = closed.requirements();
    let mut timings = source.timings.clone();
    let inputs = crate::terminal::prepare_provider_execution_inputs(
        &source.terminal_publication,
        &source.sources,
        &source.computed_build_config.config.native_bindings,
        target,
        reached,
        &mut crate::observations::PsiStageTimings::new(&mut timings),
    )?;
    native_program_planning::plan_target_execution(
        accepted.module(), target, closed.selection(), closed.bindings(), inputs,
    ).map_err(|error| match error {
            NativeProgramPlanningError::Supply(TargetSupplyRealizationError::MissingSupplies { target, boundaries }) => {
                let mut labels = declared.iter().collect::<Vec<_>>();
                labels.sort_unstable_by_key(|row| row.boundary);
                boundaries.iter().map(|boundary| {
                    let message = match labels.binary_search_by_key(boundary, |row| row.boundary) {
                        Ok(label_position) => format!(
                            "boundary requirement `{}` has no provider on target `{target}`: the Build selects none and the target package declares no default",
                            labels[label_position].name,
                        ),
                        Err(_) => format!(
                            "reached boundary {} has no normalized chosen provider or exact native leaf on target `{target}`",
                            boundary.get(),
                        ),
                    };
                    Diagnostic::error(message)
                })
                .collect()
            },
            NativeProgramPlanningError::Supply(error) => vec![Diagnostic::error(error.diagnostic(accepted.module()))],
            NativeProgramPlanningError::Admission(diagnostics) => diagnostics,
        })
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
