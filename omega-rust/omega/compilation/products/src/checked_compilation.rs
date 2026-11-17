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

#[path = "checked_compilation_report.rs"]
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
}

/// ```compile_fail
/// fn replace_program(checked: &mut products::CheckedCompilation) {
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

    pub fn native_application_metadata(&self) -> target::NativeApplicationMetadata {
        self.build_admission().native_application_metadata(
            &self.build().config.settings,
            self.execution().target_profile(),
        )
    }

    pub fn sources(&self) -> &CheckedSourceCustody {
        &self.sources
    }

    pub fn admit_trust(&self) -> Result<(), Vec<Diagnostic>> {
        let grants = &self.build().config.grants;
        if grants.is_empty() {
            return Ok(());
        }
        // Grants retain their resolved subject. Settlement joins that exact
        // provider application or native requirement to this target's supply;
        // accepted-machine claims use the existing source-owned census.
        let declarations = self
            .source_inspection()
            .inventory()
            .trust_declarations()
            .map_err(|error| vec![error])?;
        trust_model::reconstruct_trust_obligations(
            declarations,
            grants,
            &self.execution().plans().plans().iter().collect::<Vec<_>>(),
            self.execution().plans(),
            &self.native_grant_evidence(grants)?,
            match self.sources.package_subject() {
                None => trust_model::RootTrustPolicy::Unpackaged,
                Some(subject)
                    if subject.root_role()
                        == build_declarations::BuildDeclarationKind::Application =>
                {
                    trust_model::RootTrustPolicy::Application(subject.root().digest())
                }
                Some(_) => trust_model::RootTrustPolicy::PackageReview,
            },
        )
        .map(drop)
    }

    fn native_grant_evidence(
        &self,
        grants: &[language_semantics::declaration_selection::AuthoredRootGrant],
    ) -> Result<Vec<Result<backend_plan::NativeRequirementTrust, String>>, Vec<Diagnostic>> {
        use language_semantics::declaration_selection::RootGrantSubject;
        let requests = grants
            .iter()
            .filter_map(|grant| {
                let RootGrantSubject::BoundaryRequirement {
                    declaration,
                    callable,
                } = &grant.subject
                else {
                    return None;
                };
                Some((grant, (declaration.as_str(), callable.as_str())))
            })
            .collect::<Vec<_>>();
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let realization = self.execution().realization();
        let mut leaves = realization
            .direct_boundary_leaves()
            .iter()
            .collect::<Vec<_>>();
        fn key(leaf: &backend_plan::DirectBoundaryLeaf) -> (&str, &str) {
            (&leaf.declaration_identity, &leaf.callable_identity)
        }
        leaves.sort_unstable_by(|left, right| key(left).cmp(&key(right)));
        let publication = self.terminal_publication().accepted()?;
        let module = &publication.closure().elaboration.module;
        Ok(requests.into_iter().map(|(grant, selected)| {
            let first = leaves.partition_point(|leaf| key(leaf) < selected);
            let last = leaves.partition_point(|leaf| key(leaf) <= selected);
            let [leaf] = &leaves[first..last] else {
                return Err(format!(
                    "root grant `{}` has no unique selected native requirement with its exact declaration and callable application",
                    grant.selector
                ));
            };
            leaf.prepare_native_trust(grant, realization.target(), module,
                realization.evaluated_native_bindings(), realization.evaluated_binding_identities())
        }).collect())
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
}

impl std::ops::Deref for CheckedCompilation {
    type Target = FrozenTypedTrees;

    fn deref(&self) -> &Self::Target {
        self.source_inspection().program()
    }
}

#[cfg(test)]
#[path = "checked_compilation_trust_tests.rs"]
mod trust_tests;
