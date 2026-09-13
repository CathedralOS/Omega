//! Candidate review workflow: select bindings, compile, then retain the requested product.
//!
//! All products share discovery and final checking. Each pass owns a disposable
//! sponsored session; package_pass owns dependency-order compilation and projection.

mod ledger;
mod package_pass;
mod policy;
mod session;
mod session_accounting;
#[cfg(test)]
mod tests;

use super::semantic_bindings::{candidate_semantic_binding_inputs, semantic_bindings_by_consumer};
use super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet,
    ConsumerScopedSemanticBindingReviewInput, ReviewedPackageProductionCandidate,
};
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use package_pass::{CompiledPackageReviews, PackageSourcePreparation};
use session::ReviewBuildSession;
use std::path::Path;

/// How this invocation obtains consumer-scoped semantic bindings.
/// Explicit input is checked as supplied; it does not trigger discovery or admission.
#[derive(Clone, Copy)]
pub enum SemanticBindingReview<'a> {
    /// Discover package-owned surfaces, then recheck with the proposed bindings.
    Discover,
    /// Compile once with exactly these bindings, including an explicitly empty set.
    Explicit(&'a [ConsumerScopedSemanticBindingReviewInput]),
}

/// Compile the exact dependency closure and return its compiler-issued reviews.
/// No review escapes its disposable session. Binding discovery is not acceptance.
pub fn compile_resolved_package_reviews(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: SemanticBindingReview<'_>,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    compile_candidate(target_closure, build_root, bindings, None).map(|compiled| compiled.reviews)
}

/// Retain the application root from the same final pass that produced its reviews.
/// This checked product grants no native authority.
pub fn compile_resolved_package_candidate_for_production(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: SemanticBindingReview<'_>,
) -> Result<ReviewedPackageProductionCandidate, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let root = closure.graph().root().clone();
    if closure.root_role() != package_compilation::BuildDeclarationKind::Application {
        return Err(
            CompileResolvedPackageReviewsError::InvalidProductionRootRole {
                package: root,
                role: closure.root_role(),
            },
        );
    }
    let root_path = closure
        .source_root(&root)
        .expect("validated source closure retains its root custody")
        .join("main.omg");
    let compiled = compile_candidate(target_closure, build_root, bindings, Some(&root_path))?;
    let checked_root = compiled.checked_root.ok_or_else(|| {
        CompileResolvedPackageReviewsError::IdentityMismatch {
            package: root.clone(),
        }
    })?;
    Ok(ReviewedPackageProductionCandidate {
        reviews: compiled.reviews,
        root,
        root_path,
        root_role: closure.root_role(),
        target_profile: target_closure.target_profile(),
        checked_root,
    })
}

/// CHECK accepts either project role and preserves the requested entry through
/// discovery and final binding, without entering production.
pub(crate) fn compile_resolved_package_candidate_for_check(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    entry_path: &Path,
) -> Result<compiler::CheckedCompilation, CompileResolvedPackageReviewsError> {
    let compiled = compile_candidate(
        target_closure,
        build_root,
        SemanticBindingReview::Discover,
        Some(entry_path),
    )?;
    compiled
        .checked_root
        .map(|checked| *checked)
        .ok_or_else(|| CompileResolvedPackageReviewsError::IdentityMismatch {
            package: target_closure.source_closure().graph().root().clone(),
        })
}

fn compile_candidate(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: SemanticBindingReview<'_>,
    retained_root_entry: Option<&Path>,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    if let SemanticBindingReview::Explicit(inputs) = bindings {
        return compile_pass(
            target_closure,
            build_root,
            inputs,
            retained_root_entry,
            PackageSourcePreparation::Independent,
        );
    }
    let mut prepared_sources = vec![None; target_closure.source_closure().graph().packages().len()];
    let preliminary = compile_pass(
        target_closure,
        build_root,
        &[],
        retained_root_entry,
        PackageSourcePreparation::Retain(&mut prepared_sources),
    )?;
    let discovered = candidate_semantic_binding_inputs(&preliminary.reviews)?;
    if discovered.is_empty() {
        return Ok(preliminary);
    }
    // Discovery cannot survive as a checked product when it proposed bindings.
    // Only source preparation is reused; custody, builds and checking run again.
    drop(preliminary);
    compile_pass(
        target_closure,
        build_root,
        &discovered,
        retained_root_entry,
        PackageSourcePreparation::Consume(&mut prepared_sources),
    )
}

fn compile_pass(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: &[ConsumerScopedSemanticBindingReviewInput],
    retained_root_entry: Option<&Path>,
    source_preparation: PackageSourcePreparation<'_>,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let bindings = semantic_bindings_by_consumer(closure, bindings)?;
    let session = ReviewBuildSession::create(build_root)?;
    let result = package_pass::compile_dependency_closure(
        target_closure,
        session.root(),
        session.filesystem_sponsor(),
        session.evaluation_sponsor(),
        &bindings,
        retained_root_entry,
        source_preparation,
    );
    let compiled = session.dispose(result)?;
    if retained_root_entry.is_some() {
        let root = closure.graph().root();
        let checked = compiled.checked_root.as_ref().ok_or_else(|| {
            CompileResolvedPackageReviewsError::IdentityMismatch {
                package: root.clone(),
            }
        })?;
        let subject = checked.package_compilation_subject();
        if subject.map(|subject| subject.root()) != Some(root.identity())
            || subject.map(|subject| subject.root_role()) != Some(closure.root_role())
            || checked.selected_target_profile() != Some(target_closure.target_profile())
        {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch {
                package: root.clone(),
            });
        }
    }
    Ok(compiled)
}
