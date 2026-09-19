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
use crate::resolution::graph::{ExactTargetPackageSourceClosure, ResolvedPackageSourceClosure};
use package_pass::{CompiledPackageReviews, TargetEntryDiscovery};
use session::ReviewBuildSession;
use std::path::Path;

/// Target-independent prepared package sources retained across candidate
/// reviews of one resolved source closure.
///
/// Slots follow the resolver's package positions, each keyed by the entry
/// root it was prepared for. A populated slot supplies only that package's
/// binding-independent parse frontier to a later pass or candidate: custody
/// verification, build execution, and checking always run again, and each
/// child's own root-path and source-input validation rejects a checkpoint
/// that no longer names its package's prepared sources. The store holds no
/// checked result, binding decision, review row, or build output, so nothing
/// verified can be replayed merely because source bytes match.
///
/// One store belongs to one resolved closure. Callers reviewing several exact
/// targets of the same closure share it; a differently shaped closure drops
/// stale slots rather than risk a misplaced checkpoint, and a slot whose
/// recorded entry root differs from the request's is prepared fresh.
#[derive(Default)]
pub struct CandidateSourcePreparation {
    slots: Vec<Option<PreparedPackageSource>>,
    /// Fresh preparations performed through this store; witnesses that a
    /// repeated or cross-target candidate prepared each package once.
    fresh_preparations: usize,
}

/// One package's retained parse frontier and the entry root it belongs to.
/// The root is the exact identity the child request validates, so a same
/// count but differently resolved closure cannot consume a misplaced slot.
struct PreparedPackageSource {
    entry_root: std::path::PathBuf,
    prepared: compiler::PreparedCheckedSource,
}

impl CandidateSourcePreparation {
    /// An empty store; the first candidate sizes it to that closure's graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Slots pre-sized for this closure's package graph.
    pub fn for_closure(closure: &ResolvedPackageSourceClosure) -> Self {
        Self {
            slots: empty_slots(closure.graph().packages().len()),
            fresh_preparations: 0,
        }
    }

    /// Size slots to this closure's package positions. A differently shaped
    /// closure starts over empty rather than risk a misplaced checkpoint.
    fn size_for(&mut self, closure: &ResolvedPackageSourceClosure) {
        let package_count = closure.graph().packages().len();
        if self.slots.len() != package_count {
            self.slots = empty_slots(package_count);
        }
    }

    /// Fresh source preparations performed through this store.
    #[cfg(test)]
    fn fresh_preparation_count(&self) -> usize {
        self.fresh_preparations
    }
}

fn empty_slots(count: usize) -> Vec<Option<PreparedPackageSource>> {
    std::iter::repeat_with(|| None).take(count).collect()
}

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
    compile_resolved_package_reviews_reusing(
        target_closure,
        build_root,
        bindings,
        &mut CandidateSourcePreparation::for_closure(target_closure.source_closure()),
    )
}

/// The same candidate review, retaining binding-independent source preparation
/// in the caller's store. A command reviewing several targets of one resolved
/// closure prepares each package once; changed sources and selections still
/// reject through the ordinary custody and checkpoint checks.
pub fn compile_resolved_package_reviews_reusing(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: SemanticBindingReview<'_>,
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    compile_candidate(target_closure, build_root, bindings, None, preparation)
        .map(|compiled| compiled.reviews)
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
    let compiled = compile_candidate(
        target_closure,
        build_root,
        bindings,
        Some(&root_path),
        &mut CandidateSourcePreparation::for_closure(closure),
    )?;
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
        &mut CandidateSourcePreparation::for_closure(target_closure.source_closure()),
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
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    preparation.size_for(target_closure.source_closure());
    if let SemanticBindingReview::Explicit(inputs) = bindings {
        return compile_pass(
            target_closure,
            build_root,
            inputs,
            retained_root_entry,
            TargetEntryDiscovery::Disabled,
            preparation,
        );
    }
    let preliminary = compile_pass(
        target_closure,
        build_root,
        &[],
        retained_root_entry,
        TargetEntryDiscovery::Dependencies,
        preparation,
    )?;
    let discovered = candidate_semantic_binding_inputs(
        &preliminary.reviews,
        target_closure.source_closure().graph().root(),
    )?;
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
        TargetEntryDiscovery::Disabled,
        preparation,
    )
}

fn compile_pass(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: &[ConsumerScopedSemanticBindingReviewInput],
    retained_root_entry: Option<&Path>,
    discovery: TargetEntryDiscovery,
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let execution_profile = target::TargetProfile::host_if_supported();
    package_pass::validate_nested_build_activations(target_closure, execution_profile)?;
    let bindings = semantic_bindings_by_consumer(closure, bindings)?;
    let session = ReviewBuildSession::create(build_root)?;
    let result = package_pass::compile_dependency_closure(
        target_closure,
        execution_profile,
        session.root(),
        session.filesystem_sponsor(),
        session.evaluation_sponsor(),
        &bindings,
        retained_root_entry,
        discovery,
        preparation,
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
