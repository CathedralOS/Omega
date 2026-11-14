// Provider settlement follows actual invocations from the accepted entry.
// Choosing a whole provider adds its exact boundary-to-body associations, then
// the same call query substitutes actual arguments through those bodies. Code
// retained by a map or descriptor never becomes an independent entry root.

use crate::checked::CheckedCompilation;
use crate::checked::PreparedProductTarget;
use artifacts::compile_timings::CompileTimings;
use compilation_products::{
    BuildAdmission, CheckedCompilationInputs, CheckedCompileOutcome, CheckedProductSource,
    CompileFailure,
};
use diagnostics::Diagnostic;
use selected_dispatch::SelectedExecution;
use std::sync::Arc;

pub(crate) struct SelectedProviderCustody {
    pub(crate) terminal_publication: Arc<compilation_products::ProductTerminalPublication>,
    pub(crate) source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    pub(crate) execution: SelectedExecution,
    pub(crate) source_inspection: Arc<compilation_products::SelectedProviderSource>,
    pub(crate) build: Arc<compilation_products::ComputedBuildConfig>,
    pub(crate) build_admission: BuildAdmission,
    pub(crate) sources: Arc<package_compilation::CheckedSourceCustody>,
    pub(crate) timings: CompileTimings,
}

pub(crate) struct SelectedProductTarget {
    pub(crate) runtime_policy: Arc<package_evidence::permission_acceptance::PreparedRuntimePolicy>,
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
                package_evidence::permission_acceptance::PreparedRuntimePolicy::capture(source),
            ),
            custody: SelectedProviderCustody {
                terminal_publication: Arc::clone(&source.terminal_publication),
                source_completion_accounting: source.source_completion_accounting.clone(),
                source_inspection: Arc::new(compilation_products::SelectedProviderSource::new(
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

/// Grow only the body bindings of actually chosen maps. Registering their
/// other rows makes those bodies available at a call; it does not invoke them.
fn selected_boundary_implementations(
    module: &terminal_psi::TerminalModule,
    realization: &provider_planning::target_supply::RealizedTargetSupply,
) -> Result<Vec<psi_compiler::CallableBoundaryImplementation>, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    for declaration in realization.provider_declarations() {
        if !declaration.plan.rows.iter().any(|row| {
            matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        let owner = declaration.provider_declaration.as_deref().ok_or_else(|| {
            vec![Diagnostic::error(
                "chosen provider has no exact nominal declaration",
            )]
        })?;
        let selected =
            backend_plan::selected_provider_candidate_rows(&declaration.plan, owner, module)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        for row in selected {
            for position in row.candidate_positions {
                let candidate = module.provider_candidates.get(position).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "chosen provider lost its authenticated candidate coordinate",
                    )]
                })?;
                bindings.push(psi_compiler::CallableBoundaryImplementation {
                    boundary: candidate.boundary,
                    machine: candidate.candidate,
                });
            }
        }
    }
    bindings.sort_unstable();
    bindings.dedup();
    if bindings
        .windows(2)
        .any(|pair| pair[0].boundary == pair[1].boundary)
    {
        return Err(vec![Diagnostic::error(
            "chosen provider has competing exact bodies for one boundary application",
        )]);
    }
    Ok(bindings)
}

fn settle_execution(
    source: &CheckedProductSource,
    profile: Option<target::TargetProfile>,
    entry: Option<semantic_vocabulary::MachineId>,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    use provider_planning::target_supply::{TargetSupplyRealizationRequest, realize_target_supply};
    let accepted = source.terminal_publication.accepted()?;
    let Some(target) = profile else {
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
        let realization = provider_planning::target_supply::RealizedTargetSupply::unscoped(
            accepted.module(),
            &roots,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        return SelectedExecution::from_realization(None, realization);
    };
    let mut bindings = Vec::new();
    let mut execution_inputs = None::<(
        Vec<semantic_vocabulary::BoundaryMachineId>,
        provider_planning::target_supply::TargetSupplyExecutionInputs,
    )>;
    loop {
        let reached = match entry {
            Some(entry) => accepted
                .boundary_calls_reached(entry, &bindings)
                .map_err(|error| vec![error])?,
            None => Vec::new(),
        };
        let inputs = match &execution_inputs {
            Some((previous, inputs)) if *previous == reached => inputs.clone(),
            _ => {
                let inputs =
                    crate::terminal::prepare_provider_execution_inputs(source, target, &reached)?;
                execution_inputs = Some((reached.clone(), inputs.clone()));
                inputs
            }
        };
        let realization =
            realize_target_supply(TargetSupplyRealizationRequest::from_execution_inputs(
                accepted.module(),
                target.target_name(),
                &reached,
                inputs,
            ))
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let next = selected_boundary_implementations(accepted.module(), &realization)?;
        if next == bindings {
            return SelectedExecution::from_realization(profile, realization);
        }
        if bindings
            .iter()
            .any(|previous| next.binary_search(previous).is_err())
        {
            return Err(vec![Diagnostic::error(
                "provider demand changed an already selected exact boundary body",
            )]);
        }
        bindings = next;
    }
}

pub(crate) struct SelectedCompileOutcome {
    pub(crate) result: Result<SelectedProductTarget, CompileFailure>,
    pub(crate) independent_components: compilation_products::IndependentComponentDiscovery,
    // Separately selected test roots over the same checked source, in roster
    // order (wiki/spec/build/testing.md). Empty when no enabled group exists.
    pub(crate) tests: Vec<(
        compilation_products::TestRootIdentity,
        Result<SelectedProductTarget, CompileFailure>,
    )>,
    pub(crate) test_notes: Vec<compilation_products::TestRosterEntry>,
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
