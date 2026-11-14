// We check a completed `PackageBuild` and publish its `PackageReviewProduct`.
// Package review receives inert evidence (`PackageReviewEvidence`) and an
// opaque `PackageReviewSource` handle for review reuse and demand-driven
// component publication (`publish_component`), never a raw checked program.

use super::{CompilationSession, PackageBuild};
use compilation_products::{CheckedCompilation, CompileFailure};
use diagnostics::Diagnostic;
use package_compilation::IndependentComponentDescription;
use package_evidence::record::{
    PackageReviewEvidence, PackageReviewSourceFacts, SemanticBindingReviewCandidate,
};
use package_evidence::{PackageReviewProjectionError, SemanticBindingProjectionError};
use std::sync::Arc;

pub struct PackageReviewProduct {
    pub evidence: PackageReviewEvidence,
    pub source: PackageReviewSource,
}

#[derive(Debug)]
pub enum PackageReviewFailure {
    Compilation(CompileFailure),
    Projection(PackageReviewProjectionError),
}

impl From<Vec<Diagnostic>> for PackageReviewFailure {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Compilation(CompileFailure::Diagnostics(diagnostics))
    }
}

#[derive(Debug, Clone)]
pub struct PackageReviewSource {
    retained: Arc<RetainedReviewSource>,
}

#[derive(Debug)]
struct RetainedReviewSource {
    checked: CheckedCompilation,
    facts: PackageReviewSourceFacts,
    target_entry: Result<Option<SemanticBindingReviewCandidate>, SemanticBindingProjectionError>, // evaluated lazily when discovery or cache capture requests it
}

impl PackageReviewSource {
    pub fn facts(&self) -> &PackageReviewSourceFacts {
        &self.retained.facts
    }

    pub fn target_entry_candidate(
        &self,
    ) -> Result<Option<SemanticBindingReviewCandidate>, SemanticBindingProjectionError> {
        self.retained.target_entry.clone()
    }

    pub fn publish_component(&self) -> Result<IndependentComponentDescription, Vec<Diagnostic>> {
        super::run_on_compile_thread(|| {
            let request =
                crate::product_execution::ProductRequest::Component(self.retained.checked.clone());
            let ((), output) = crate::target_batch::produce_products(vec![((), Ok(request))])
                .into_iter()
                .next()
                .ok_or_else(|| vec![Diagnostic::error("component publication lost its outcome")])?;
            output
                .and_then(crate::CompilationOutput::into_component)
                .map(|product| product.description().clone())
                .map_err(crate::CompilationProductError::into_diagnostics)
        })
    }
}

impl CompilationSession {
    pub fn review_package_build(
        &mut self,
        build: PackageBuild,
        maximum_policy_canonical_bytes: usize,
    ) -> Result<PackageReviewProduct, PackageReviewFailure> {
        super::run_on_compile_thread(|| {
            let package = build.package_inputs().root();
            let target = build.build_handoff().target();
            let checked = self
                .check_package_build(build)
                .map_err(PackageReviewFailure::Compilation)?;
            let evidence = package_evidence::project_package_review(
                &checked,
                package,
                target,
                maximum_policy_canonical_bytes,
            )
            .map_err(PackageReviewFailure::Projection)?;
            let facts = PackageReviewSourceFacts {
                consumed_units: checked
                    .sources()
                    .package_subject()
                    .map(|subject| subject.consumed_units().to_vec()),
            };
            let target_entry =
                package_evidence::candidate_target_entry_binding(&checked, package, target);
            Ok(PackageReviewProduct {
                evidence,
                source: PackageReviewSource {
                    retained: Arc::new(RetainedReviewSource {
                        checked,
                        facts,
                        target_entry,
                    }),
                },
            })
        })
    }
}
