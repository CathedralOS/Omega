use super::lifetimes::review_lifetime_topology_with_substitutions;
use super::validation::{missing_exact_toolchain_type_owner, validate_package_type_identity_input};
use crate::capture::semantics::encoding::framed_identity;
use crate::record::PackageReviewTypeIdentity;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn review_type_identity_with_binders(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let identity = compilation
        .package_qualified_type_identity_with_binders_and_toolchain_sources(
            type_reference,
            binders,
            compilation.exact_toolchain_sources(),
        )
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    Ok(PackageReviewTypeIdentity {
        canonical: identity.into_string(),
    })
}

pub(crate) fn review_type_identity_with_binders_and_substitutions(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let identity = compilation
        .package_qualified_type_identity_with_binders_substitutions_and_toolchain_sources(
            type_reference,
            binders,
            substitutions,
            compilation.exact_toolchain_sources(),
        )
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
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
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
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
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
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    signature_type_identity(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        substitutions,
        lifetime_substitutions,
        false,
    )
}

/// Only callers already holding the exact declared const telescope slot may
/// admit the compiler's unnameable value atoms at this root position.
pub(crate) fn review_signature_const_argument_identity(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    signature_type_identity(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        substitutions,
        &[],
        true,
    )
}

fn signature_type_identity(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
    const_argument: bool,
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    super::validation::validate_package_type_identity_input_inner(
        &compilation.typed,
        type_reference,
        binders,
        const_argument,
    )?;
    let runtime = compilation
        .package_qualified_type_identity_with_binders_substitutions_and_toolchain_sources(
            type_reference,
            binders,
            substitutions,
            compilation.exact_toolchain_sources(),
        )
        .ok_or_else(missing_exact_toolchain_type_owner)?
        .into_string();
    let lifetime = review_lifetime_topology_with_substitutions(
        compilation,
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
