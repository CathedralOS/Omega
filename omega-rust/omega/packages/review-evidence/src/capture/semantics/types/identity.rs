/*
We compute canonical package-review runtime and signature type identities.

Public signature identity layers erased borrow-region relationships over the
ordinary package-qualified runtime type identity. General structural type
identity intentionally erases these tags, but package compatibility cannot
because changing which input owns an output loan changes the callable contract
without changing layout or monomorphization.

When normalizing types in projection-local scratch trees, the scratch trees
retain the input's symbol and source identities without carrying proof facts or
selected-execution authority; source commitments are borrowed from the original
compilation rather than regenerated from temporary type nodes. A const-argument
root is permitted only after the caller establishes its exact declared const
telescope slot.
*/

use super::lifetimes::review_lifetime_topology_with_substitutions;
use super::validation::{missing_exact_toolchain_type_owner, validate_package_type_identity_input};
use crate::capture::SourceDeclarationInput;
use crate::capture::semantics::encoding::framed_identity;
use crate::record::PackageReviewTypeIdentity;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::type_identity::ExactOwnerTypeIdentityRequest;

pub(crate) fn review_type_identity_with_binders(
    compilation: &SourceDeclarationInput<'_>,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    review_type_identity_with_binders_and_substitutions(compilation, type_reference, binders, &[])
}

pub(crate) fn review_type_identity_with_binders_and_substitutions(
    compilation: &SourceDeclarationInput<'_>,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(
        compilation,
        compilation.checking_input,
        type_reference,
        binders,
    )?;
    let identity = compilation
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference,
            binders,
            substitutions,
            exact_toolchain_sources: compilation.exact_toolchain_sources,
        })
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    Ok(PackageReviewTypeIdentity {
        canonical: compilation
            .checking_input
            .typed_program()
            .type_identity_text(identity)
            .unwrap_or_default(),
    })
}

pub(crate) fn review_signature_type_identity_with_binders(
    compilation: &SourceDeclarationInput<'_>,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[typed_trees::name::Identifier],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        &[],
        &[],
    )
}

pub(crate) fn review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
    compilation: &SourceDeclarationInput<'_>,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(typed_trees::name::Identifier, typed_trees::name::Identifier)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    signature_type_identity(
        compilation,
        compilation.checking_input,
        compilation.exact_toolchain_sources,
        type_reference,
        binders,
        lifetime_binders,
        substitutions,
        lifetime_substitutions,
        false,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    )
}

pub(crate) fn signature_type_identity(
    compilation: &SourceDeclarationInput<'_>,
    program: &dyn ::validation::CheckingInput,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(typed_trees::name::Identifier, typed_trees::name::Identifier)],
    const_argument: bool,
    selection_exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    super::validation::validate_package_type_identity_input_inner(
        compilation,
        program,
        type_reference,
        binders,
        const_argument,
        Some(selection_exposure),
    )?;
    let runtime = program
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference,
            binders,
            substitutions,
            exact_toolchain_sources,
        })
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    let runtime = program.type_identity_text(runtime).unwrap_or_default();
    let lifetime = review_lifetime_topology_with_substitutions(
        program,
        exact_toolchain_sources,
        type_reference,
        binders,
        lifetime_binders,
        substitutions,
        lifetime_substitutions,
        &mut Vec::new(),
    )?;
    Ok(PackageReviewTypeIdentity {
        canonical: framed_identity("signature-type", &[runtime, lifetime]),
    })
}
