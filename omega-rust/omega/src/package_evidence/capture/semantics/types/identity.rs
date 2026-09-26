use super::lifetimes::review_lifetime_topology_with_substitutions;
use super::validation::{missing_exact_toolchain_type_owner, validate_package_type_identity_input};
use crate::package_evidence::capture::PackageReviewInput;
use crate::package_evidence::capture::semantics::encoding::framed_identity;
use crate::package_evidence::record::PackageReviewTypeIdentity;
use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::type_identity::ExactOwnerTypeIdentityRequest;
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

pub(crate) fn review_type_identity_with_binders(
    compilation: &PackageReviewInput<'_>,
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let identity = compilation
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference,
            binders,
            substitutions: &[],
            exact_toolchain_sources: compilation.custody.exact_toolchain_sources(),
        })
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    Ok(PackageReviewTypeIdentity {
        canonical: identity.into_string(),
    })
}

pub(crate) fn review_type_identity_with_binders_and_substitutions(
    compilation: &PackageReviewInput<'_>,
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(
        SymbolHandle,
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    )],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let identity = compilation
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference,
            binders,
            substitutions,
            exact_toolchain_sources: compilation.custody.exact_toolchain_sources(),
        })
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    Ok(PackageReviewTypeIdentity {
        canonical: identity.into_string(),
    })
}

/// Public signature identity layers erased borrow-region relationships over
/// the ordinary package-qualified runtime type identity. General structural
/// type identity intentionally erases these tags; package compatibility may
/// not, because changing which input owns an output loan changes the callable
/// contract without changing layout or monomorphization.
pub(crate) fn review_signature_type_identity_with_binders(
    compilation: &PackageReviewInput<'_>,
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    review_signature_type_identity_with_binders_and_substitutions(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        &[],
    )
}

fn review_signature_type_identity_with_binders_and_substitutions(
    compilation: &PackageReviewInput<'_>,
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier],
    substitutions: &[(
        SymbolHandle,
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    )],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        substitutions,
        &[],
    )
}

pub(crate) fn review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
    compilation: &PackageReviewInput<'_>,
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier],
    substitutions: &[(
        SymbolHandle,
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    )],
    lifetime_substitutions: &[(
        symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier,
        symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier,
    )],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    signature_type_identity(
        &compilation.typed,
        compilation.custody.exact_toolchain_sources(),
        type_reference,
        binders,
        lifetime_binders,
        substitutions,
        lifetime_substitutions,
        false,
    )
}

/// Normalize types in either the checked input or projection-local trees.
/// Scratch trees retain the input's symbol/source identities; they do not carry
/// proof facts or selected-execution authority. Source commitments are borrowed
/// from the original compilation, never regenerated from temporary type nodes.
/// A const-argument root is permitted only after the caller establishes its
/// exact declared const telescope slot.
pub(crate) fn signature_type_identity(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier],
    substitutions: &[(
        SymbolHandle,
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    )],
    lifetime_substitutions: &[(
        symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier,
        symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier,
    )],
    const_argument: bool,
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    super::validation::validate_package_type_identity_input_inner(
        program,
        type_reference,
        binders,
        const_argument,
    )?;
    let runtime = program
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference,
            binders,
            substitutions,
            exact_toolchain_sources,
        })
        .ok_or_else(missing_exact_toolchain_type_owner)?
        .into_string();
    let lifetime = review_lifetime_topology_with_substitutions(
        program,
        exact_toolchain_sources,
        type_reference,
        lifetime_binders,
        substitutions,
        lifetime_substitutions,
        &mut Vec::new(),
    )?;
    Ok(PackageReviewTypeIdentity {
        canonical: framed_identity("signature-type", &[runtime, lifetime]),
    })
}
