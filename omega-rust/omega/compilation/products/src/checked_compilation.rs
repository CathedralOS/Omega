/*
Accepted compilation and the immutable source custody sealed against it.

The sealed `CheckedCompilation` record retains the selected providers (the
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

use crate::SelectedProviderResult;
use crate::{BuildAdmission, ComputedBuildConfig};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::CheckedSourceCustody;
use source_compiler::source_state::FrozenTypedTrees;
use std::sync::Arc;

#[path = "checked_compilation_report.rs"]
mod report;

pub use report::{
    CheckedProductReport, CompileReportSettlement, ProductReportPlan, ProductReportRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompilationInputs {
    pub settled: SelectedProviderResult,
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
    source_completion_accounting: source_compiler::evaluation::SourceCompletionAccounting,
    root_grants: Result<Arc<[provider_correspondence::trust::TrustAdmission]>, Vec<Diagnostic>>,
    execution: CheckedCompilationInputs,
    sources: Arc<CheckedSourceCustody>,
    timings: CompileTimings,
    terminal_publication: Arc<source_compiler::SourcePublication>,
    published_program: Option<Arc<crate::PublishedProgram>>,
}

impl PartialEq for CheckedCompilation {
    fn eq(&self, other: &Self) -> bool {
        self.execution == other.execution && self.sources == other.sources
    }
}

impl Eq for CheckedCompilation {}

fn admit_published_root_grants(
    execution: &CheckedCompilationInputs,
    sources: &CheckedSourceCustody,
    publication: &source_compiler::SourcePublication,
) -> Result<Arc<[provider_correspondence::trust::TrustAdmission]>, Vec<Diagnostic>> {
    use declaration_contracts::selection::RootGrantSubject;
    use provider_correspondence::trust::RootTrustPolicy;

    let grants = &execution.build.config.grants;
    if grants.is_empty() {
        return Ok(Arc::from([]));
    }
    let declarations = execution
        .source_inspection
        .inventory()
        .published_trust_declarations()
        .map_err(|error| vec![error])?;
    let policy = match sources.package_subject() {
        None => RootTrustPolicy::Unpackaged,
        Some(subject)
            if subject.root_role() == build_declarations::BuildDeclarationKind::Application =>
        {
            RootTrustPolicy::Application(subject.root().digest())
        }
        Some(_) => RootTrustPolicy::PackageReview,
    };
    let needs_native_program = grants
        .iter()
        .any(|grant| matches!(grant.subject, RootGrantSubject::BoundaryRequirement { .. }));
    let program = needs_native_program
        .then(|| publication.require_published(publication.optimizations()))
        .transpose()?;
    let artifact = program
        .as_ref()
        .map(|program| execution.settled.effective_artifact(program.artifact()))
        .transpose()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    execution
        .settled
        .trust_evidence()
        .admit_root_grants(
            declarations,
            grants,
            policy,
            artifact.map(|artifact| artifact.view()),
        )
        .map(Arc::from)
}

impl CheckedCompilation {
    pub fn from_settled_execution(
        execution: CheckedCompilationInputs,
        sources: Arc<CheckedSourceCustody>,
        timings: CompileTimings,
        terminal_publication: Arc<source_compiler::SourcePublication>,
    ) -> Result<Self, Vec<Diagnostic>> {
        sources
            .require_owner(execution.source_inspection.program())
            .map_err(|error| vec![error])?;
        terminal_publication
            .require_source_owner(execution.source_inspection.program())
            .map_err(|error| vec![error])?;
        // A fresh judgment of this source can be checked before it publishes code.
        let original_program = match terminal_publication.published() {
            Some(program) => program.artifact().view().identity(),
            None => {
                terminal_codec::terminal_psi_identity(terminal_publication.accepted()?.module())
                    .map_err(|error| vec![Diagnostic::error(error.to_string())])?
            }
        };
        if execution.settled.original_program() != original_program
            || !execution
                .settled
                .publisher()
                .matches_source(sources.source_identity())
        {
            return Err(vec![Diagnostic::error(
                "selected providers belong to a different accepted program",
            )]);
        }
        let root_grants = admit_published_root_grants(&execution, &sources, &terminal_publication);
        Ok(Self {
            source_completion_accounting: Default::default(),
            root_grants,
            execution,
            sources,
            timings,
            terminal_publication,
            published_program: None,
        })
    }

    /// Retain the source-free publication projected before provider planning.
    pub fn with_published_program(
        mut self,
        published: Arc<crate::PublishedProgram>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let source = self
            .terminal_publication
            .require_published(self.terminal_publication.optimizations())?;
        if !published.belongs_to(&source) {
            return Err(vec![Diagnostic::error(
                "published code belongs to another checked source",
            )]);
        }
        self.published_program = Some(published);
        Ok(self)
    }

    /// Consume source custody at the executable-planning boundary. Only
    /// the published code and immutable inputs needed by that phase survive.
    /// Checking/review callers retain source explicitly in CheckedCompilation.
    pub fn into_published(self) -> Result<crate::PublishedCompilation, Vec<Diagnostic>> {
        let source = match &self.published_program {
            Some(source) => Arc::clone(source),
            None => {
                let program = self
                    .terminal_publication
                    .require_published(self.terminal_publication.optimizations())?;
                Arc::new(crate::PublishedProgram::from_source(&program))
            }
        };
        let program =
            crate::PublishedExecutableProgram::for_execution(&source, &self.execution.settled)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let application = self.native_application_metadata();
        let behavior_exclusions = self.build().config.behavior_exclusions.clone();
        let x86_deployment_claim = self.build().config.settings.x86_deployment_claim;
        Ok(crate::PublishedCompilation {
            execution: self.execution.settled,
            root_grants: self.root_grants?,
            package_inputs: crate::ExecutablePackageInputs::from_checked(&self.sources),
            program: Arc::new(program),
            application,
            behavior_exclusions,
            x86_deployment_claim,
        })
    }

    pub fn source_inspection(&self) -> &crate::SelectedProviderSource {
        &self.execution.source_inspection
    }

    /// The portable program already issued for this checked product.
    pub fn published_program(&self) -> Option<&crate::PublishedProgram> {
        self.published_program.as_deref()
    }

    /// The canonical artifact already issued for this product or its explicit source judgment.
    pub fn published_artifact(&self) -> Option<&Arc<terminal_codec::CanonicalTerminalArtifact>> {
        self.published_program()
            .map(crate::PublishedProgram::artifact)
            .or_else(|| {
                self.terminal_publication
                    .published()
                    .map(|program| program.artifact())
            })
    }

    pub fn terminal_publication(&self) -> &source_compiler::SourcePublication {
        &self.terminal_publication
    }

    pub fn with_source_completion_accounting(
        mut self,
        accounting: source_compiler::evaluation::SourceCompletionAccounting,
    ) -> Self {
        self.source_completion_accounting = accounting;
        self
    }

    pub fn source_completion_accounting(
        &self,
    ) -> &source_compiler::evaluation::SourceCompletionAccounting {
        &self.source_completion_accounting
    }

    /// The selected target, plans and review evidence for the checked source.
    ///
    /// ```compile_fail,E0599
    /// fn advance(checked: &products::CheckedCompilation, original: &terminal_codec::CanonicalTerminalArtifact) {
    ///     let mut selected = checked.execution().clone();
    ///     let _ = selected.bind_folded_entry(original, None);
    /// }
    /// ```
    pub const fn execution(&self) -> &SelectedProviderResult {
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
        self.root_grants.as_ref().map(|_| ()).map_err(Clone::clone)
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
            source_compiler::evaluation::native_bindings::capture_package_intents(
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
