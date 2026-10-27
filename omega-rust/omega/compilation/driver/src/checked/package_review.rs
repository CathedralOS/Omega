//! Check one completed Build and publish its review product.
//!
//! The package consumer gets inert evidence and an opaque source owner, never
//! checked trees. Component publication remains demand-driven; review does not
//! lower every dependency to Terminal merely because it might be used later.

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

/// Shared private custody for review reuse and later component demands.
/// No constructor, mutable access or checked-source extraction is exposed.
#[derive(Debug, Clone)]
pub struct PackageReviewSource {
    retained: Arc<RetainedReviewSource>,
}

#[derive(Debug)]
struct RetainedReviewSource {
    checked: CheckedCompilation,
    facts: PackageReviewSourceFacts,
    // A target-entry projection failure matters only when discovery or cache
    // capture actually asks for it; explicit binding review does not require it.
    target_entry: Result<Option<SemanticBindingReviewCandidate>, SemanticBindingProjectionError>,
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
            let ((), output) = crate::compiler::produce_products(vec![((), Ok(request))])
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
    /// Complete review without replaying Build or exposing its checked product.
    /// The aggregate owner supplies its remaining policy budget.
    pub fn review_package_build(
        &mut self,
        build: PackageBuild,
        maximum_policy_canonical_bytes: usize,
    ) -> Result<PackageReviewProduct, PackageReviewFailure> {
        super::run_on_compile_thread(|| {
            let package = build.package_inputs().root();
            let target = build.generated_source_bundle().target();
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
