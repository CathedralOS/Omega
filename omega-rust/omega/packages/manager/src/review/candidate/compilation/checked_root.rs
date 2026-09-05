//! Retain the final checked root after sponsored candidate staging is disposed.

use super::*;

/// CHECK accepts either project role and preserves the requested entry through
/// discovery and final binding. The returned value grants no native authority.
pub(crate) fn compile_resolved_package_candidate_for_check(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    entry_path: &Path,
) -> Result<compiler::CheckedCompilation, CompileResolvedPackageReviewsError> {
    let (reviews, checked) =
        compile_with_semantic_bindings(target_closure, build_root, entry_path, &[])?;
    let semantic_binding_inputs = candidate_semantic_binding_inputs(&reviews)?;
    if semantic_binding_inputs.is_empty() {
        return Ok(checked);
    }
    // Discovery is not the final checked product when bindings were proposed.
    drop((reviews, checked));
    compile_with_semantic_bindings(
        target_closure,
        build_root,
        entry_path,
        &semantic_binding_inputs,
    )
    .map(|(_, checked)| checked)
}

pub(super) fn compile_with_semantic_bindings(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    entry_path: &Path,
    semantic_binding_inputs: &[ConsumerScopedSemanticBindingReviewInput],
) -> Result<
    (CompilerIssuedPackageReviewSet, compiler::CheckedCompilation),
    CompileResolvedPackageReviewsError,
> {
    let closure = target_closure.source_closure();
    let semantic_bindings_by_consumer =
        semantic_bindings_by_consumer(closure, semantic_binding_inputs)?;
    let build_session = ReviewBuildSession::create(build_root)?;
    let result = compile_resolved_package_reviews_in_session(
        target_closure,
        build_session.root(),
        build_session.filesystem_sponsor(),
        build_session.evaluation_sponsor(),
        &semantic_bindings_by_consumer,
        Some(entry_path),
    );
    let compiled = build_session.dispose(result)?;
    let root = closure.graph().root();
    let checked = compiled.checked_root.ok_or_else(|| {
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
    Ok((compiled.reviews, checked))
}
