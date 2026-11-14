// Provider settlement chooses, for one target, the provider of every boundary
// requirement the Product's accepted Terminal module reaches
// (`provider_planning::target_supply`): a requirement with no declared supply
// or with competing supplies rejects the program, and a module that reaches
// none settles to no plan.
//
// What a product of that target then reads is small. `SelectedExecution` is
// the target and its settled plans; `SelectedProviderCustody` joins it to the
// accepted Terminal publication and to the source and Build custody the
// checked source already owns. Nothing here is a retained copy of a later
// product's inputs: a product derives what it needs from the plans and the
// accepted artifact where it uses them.

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
    pub(crate) provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
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
        // Settlement rejects a demanded requirement with no supply or with
        // competing supplies, then forms the plans of the chosen ones. A
        // module that demands nothing settles to no plan.
        let PreparedProductTarget { profile, supply } = prepared;
        let execution = SelectedExecution::settle(profile, &supply.settle()?)?;
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
                provider_defaults: Arc::clone(&source.provider_defaults),
                sources: Arc::clone(&source.sources),
                timings: source.timings.clone(),
            },
        })
    }

    // Checking one target on its own: the accepted program is produced, its
    // target-local conditions and task activations are checked, and the
    // checked compilation is sealed.
    pub(crate) fn finish_check(mut self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let terminal = crate::terminal::check_terminal_conditions(&mut self.custody)?;
        self.check_declaration_requirements(terminal.module())?;
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

    pub(crate) fn into_checked_compilation(self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let SelectedProviderCustody {
            terminal_publication,
            source_completion_accounting,
            execution,
            source_inspection,
            build,
            build_admission,
            provider_defaults,
            sources,
            timings,
        } = self.custody;
        CheckedCompilation::from_settled_execution(
            CheckedCompilationInputs {
                settled: execution,
                source_inspection,
                build,
                build_admission,
                provider_defaults,
            },
            sources,
            timings,
            terminal_publication,
        )
        .map(|checked| checked.with_source_completion_accounting(source_completion_accounting))
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
            result: self
                .result
                .and_then(|selected| selected.finish_check().map_err(CompileFailure::from)),
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
            .map_err(CompileFailure::from),
        independent_components: source.computed_build_config.independent_components(),
        tests: Vec::new(),
        test_notes: Vec::new(),
    }
}
