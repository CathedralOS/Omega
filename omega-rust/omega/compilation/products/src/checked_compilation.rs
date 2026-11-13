/*
Accepted compilation and the immutable source custody sealed against it.

The sealed `CheckedCompilation` record retains the selected execution (the
target and its settled provider plans), the shared evaluated Build, this
compilation's Build admission, and source custody. Its source inspection and
accepted Terminal publication must name the same frozen source owner.
Consumers borrow the unit they read; the methods here expose those units,
reconstruct explicit root trust obligations and project package build
handoffs.
*/

/*
Measurements, a retained judgment, and invocation-local derived code are not a
separate checked semantic identity: `PartialEq` compares only `execution` and
`sources` so populating lazy Terminal publication cannot alter equality.
*/

use crate::{BuildAdmission, ComputedBuildConfig};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::CheckedSourceCustody;
use selected_dispatch::SelectedExecution;
use std::sync::Arc;
use typed_trees::FrozenTypedTrees;

mod report;

pub use report::{
    CheckedProductReport, CompileReportSettlement, ProductReportPlan, ProductReportRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompilationInputs {
    pub settled: SelectedExecution,
    pub source_inspection: Arc<crate::SelectedProviderSource>, // retained only for explicit inspection and reference interpretation
    pub build: Arc<ComputedBuildConfig>,
    pub build_admission: BuildAdmission,
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
}

/// ```compile_fail
/// fn replace_program(checked: &mut compilation_products::CheckedCompilation) {
///     let _ = &mut checked.execution;
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CheckedCompilation {
    source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    execution: CheckedCompilationInputs,
    sources: Arc<CheckedSourceCustody>,
    timings: CompileTimings,
    terminal_publication: Arc<crate::ProductTerminalPublication>,
}

impl PartialEq for CheckedCompilation {
    fn eq(&self, other: &Self) -> bool {
        self.execution == other.execution && self.sources == other.sources
    }
}

impl Eq for CheckedCompilation {}

impl CheckedCompilation {
    pub fn from_settled_execution(
        execution: CheckedCompilationInputs,
        sources: Arc<CheckedSourceCustody>,
        timings: CompileTimings,
        terminal_publication: Arc<crate::ProductTerminalPublication>,
    ) -> Result<Self, Vec<Diagnostic>> {
        sources
            .require_owner(execution.source_inspection.program())
            .map_err(|error| vec![error])?;
        terminal_publication
            .require_source_owner(execution.source_inspection.program())
            .map_err(|error| vec![error])?;
        Ok(Self {
            source_completion_accounting: Default::default(),
            execution,
            sources,
            timings,
            terminal_publication,
        })
    }

    pub fn source_inspection(&self) -> &crate::SelectedProviderSource {
        &self.execution.source_inspection
    }

    pub fn terminal_publication(&self) -> &crate::ProductTerminalPublication {
        &self.terminal_publication
    }

    pub fn with_source_completion_accounting(
        mut self,
        accounting: build_time_evaluation::SourceCompletionAccounting,
    ) -> Self {
        self.source_completion_accounting = accounting;
        self
    }

    pub fn source_completion_accounting(
        &self,
    ) -> &build_time_evaluation::SourceCompletionAccounting {
        &self.source_completion_accounting
    }

    pub const fn execution(&self) -> &SelectedExecution {
        &self.execution.settled
    }

    pub fn build(&self) -> &ComputedBuildConfig {
        &self.execution.build
    }

    pub const fn build_admission(&self) -> &BuildAdmission {
        &self.execution.build_admission
    }

    pub fn provider_defaults(
        &self,
    ) -> &[build_time_evaluation::provider_declarations::TargetProviderDefaults] {
        &self.execution.provider_defaults
    }

    pub fn sources(&self) -> &CheckedSourceCustody {
        &self.sources
    }

    pub fn admit_trust(&self) -> Result<(), Vec<Diagnostic>> {
        let grants = self.build().config.root_grant_selectors();
        if grants.is_empty() {
            return Ok(());
        }
        // A root grant names a provider this target selected, so the settled
        // plans are the whole roster a grant can resolve against; the trust
        // claims come from the source census the inventory already holds.
        let declarations = self
            .source_inspection()
            .inventory()
            .trust_declarations()
            .map_err(|error| vec![error])?;
        trust_model::reconstruct_trust_obligations(
            declarations,
            &grants,
            &self.execution().plans().plans().iter().collect::<Vec<_>>(),
            self.execution().plans(),
            self.sources.package_subject().is_some(),
        )
        .map(drop)
    }

    pub fn package_build_handoff(
        &self,
    ) -> Result<package_compilation::PackageBuildHandoff, String> {
        let handoff = self
            .sources
            .package_build_handoff(
                self.execution.settled.target_profile(),
                self.execution.build.generated_sources.clone(),
            )
            .map_err(str::to_owned)?
            .with_checked_provider_selections(
                self,
                &self.build().config.provider_selections,
                self.build().observation_summary.as_ref(),
            )?;
        let (declarations, intents) =
            build_time_evaluation::native_bindings::capture_package_intents(
                self,
                handoff.package(),
                &self.build().native_formations,
                &self.build().native_binding_requests,
            )
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        handoff.with_native_binding_outputs(
            declarations,
            intents,
            self.build().config.native_binding_evaluation.clone(),
        )
    }

    pub const fn timings(&self) -> &CompileTimings {
        &self.timings
    }

    pub fn timings_mut(&mut self) -> &mut CompileTimings {
        &mut self.timings
    }
}

impl std::ops::Deref for CheckedCompilation {
    type Target = FrozenTypedTrees;

    fn deref(&self) -> &Self::Target {
        self.source_inspection().program()
    }
}
