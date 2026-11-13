/*
Accepted compilation and the immutable source custody sealed against it.

The sealed `CheckedCompilation` record retains selected execution, the shared
evaluated Build, this compilation's Build admission, provider settlement, and
source custody. Its source inspection and accepted Terminal publication must
name the same frozen source owner. Consumers borrow the unit they read; the methods
here expose those units, reconstruct explicit root trust obligations, project
package build handoffs, and assemble the `TerminalNativeProposalRequest` that
Omega 00 joins against provider selection.
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
use provider_planning::ProviderSettlement;
use selected_dispatch::SelectedExecution;
use std::sync::Arc;
use terminal_psi_to_abstract_operations::{
    BoundaryApplicationSettlement, TerminalNativeProposalRequest,
};
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
    pub providers: ProviderSettlement,
    pub independent_components: Result<Vec<crate::SelectedIndependentComponent>, String>,
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

    pub const fn providers(&self) -> &ProviderSettlement {
        &self.execution.providers
    }

    pub fn into_independent_components(
        self,
    ) -> Result<Vec<crate::SelectedIndependentComponent>, String> {
        self.execution.independent_components
    }

    pub fn sources(&self) -> &CheckedSourceCustody {
        &self.sources
    }

    pub fn admit_trust(&self) -> Result<(), Vec<Diagnostic>> {
        let grants = self.build().config.root_grant_selectors();
        if grants.is_empty() {
            return Ok(());
        }
        trust_model::reconstruct_trust_obligations(
            &self.execution().trust_declarations,
            &grants,
            &self
                .providers()
                .provider_candidates
                .iter()
                .map(provider_planning::ProviderPlanCandidate::plan)
                .collect::<Vec<_>>(),
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
                self.execution.settled.selected_target_profile,
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

    pub fn terminal_native_proposal_request<'input>(
        &'input self,
        selections: &optimization_core::OptimizationSelections,
        exported_calling_applications: &[calling_conventions::BoundaryCallingApplication],
    ) -> Result<TerminalNativeProposalRequest<'input>, Vec<Diagnostic>> {
        let execution = self.execution();
        let providers = self.providers();
        let native_sources = execution
            .native_provider_sources
            .as_ref()
            .map_err(Clone::clone)?;
        if native_sources.declarations.len() != execution.selected_provider_provenance.len() {
            return Err(vec![Diagnostic::error(
                "native provider declarations lost selected-plan alignment",
            )]);
        }
        let selected_provider_provenance = execution
            .selected_provider_provenance
            .iter()
            .zip(native_sources.declarations.iter())
            .map(|(retained, declarations)| {
                if retained.plan != declarations.plan {
                    return Err(vec![Diagnostic::error(
                        "native provider declarations substituted a selected plan",
                    )]);
                }
                Ok(terminal_psi_to_abstract_operations::provider_admission::CompilerIntrinsicPlanProvenance {
                    plan: &retained.plan,
                    row_executions: &retained.row_compiler_intrinsic_executions,
                    semantic_schema: &retained.plan.schema,
                    row_numeric_realizations: &declarations.row_numeric_realizations,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let (
            package_terminal_authority_permissions,
            package_terminal_authority_permission_identities,
        ) = execution.terminal_authority_permissions();
        Ok(TerminalNativeProposalRequest {
            evaluated_binding_identities: providers.evaluated_via_bindings.identities(),
            direct_boundary_leaves: execution.direct_boundary_leaves.iter().collect(),
            provider_declarations: &native_sources.declarations,
            family_realizations: &native_sources.family_realizations,
            target_profile: execution.selected_target_profile,
            boundaries: BoundaryApplicationSettlement {
                selected_provider_plans: execution.plans(),
                selected_provider_provenance,
                calling_applications: providers
                    .calling_applications(exported_calling_applications)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
            },
            native_target: execution.selected_native_target(),
            program_entry: execution.selected_program_entry.as_ref().map(|entry| {
                terminal_psi_to_abstract_operations::NativeProgramEntrySettlement::new(
                    entry.source_signature(),
                    entry.calling_plans().map(|plans| {
                        (
                            &plans.semantic_calling_application,
                            &plans.physical_calling_application,
                            &plans.storage_entry,
                        )
                    }),
                )
            }),
            application: self.build_admission().native_application_metadata(
                &self.build().config.settings,
                execution.selected_target_profile,
            ),
            optimizations: selections.project_post_terminal(),
            external_bindings: &providers.external_binding_rows,
            behavior_exclusions: self.build().config.behavior_exclusions.as_slice(),
            package_terminal_authority_permissions,
            package_terminal_authority_permission_identities,
            x86_scalar_fma_provider: execution.x86_scalar_fma_provider,
        })
    }
}

impl std::ops::Deref for CheckedCompilation {
    type Target = FrozenTypedTrees;

    fn deref(&self) -> &Self::Target {
        self.source_inspection().program()
    }
}
