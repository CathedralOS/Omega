//! Shared construction rows for object-artifact custody fixtures.

/// The structural declaration every fixture starts from: no quotient carrier,
/// no linear claim, no affine cleanup, not copyable, no descriptor cleanup and
/// no scalar restriction. Each caller states only the identity and shape it
/// observes, so a new attributed field on the carrier cannot be forgotten by a
/// fixture that meant to pin it.
pub(crate) fn structural_declaration(
    id: semantic_vocabulary::StructuralTypeId,
    identity: terminal_psi::TerminalIdentityIndex,
    shape: terminal_psi::StructuralTypeShape,
) -> terminal_psi::StructuralTypeDeclaration {
    terminal_psi::StructuralTypeDeclaration {
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        copyable: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id,
        identity,
        shape,
    }
}
