//! Policy signature projection with an explicit containing static telescope.

use super::*;

pub(crate) fn project_policy_type_parameters_after(
    compilation: &CheckedCompilation,
    checked_source: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    contract_scopes: &[CallingContractScope],
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    project_policy_type_parameters(
        compilation,
        checked_source,
        parameters,
        declaration_path,
        preceding_binders,
        ordinal_offset,
        lifetime_binders,
        &[],
        contract_scopes,
        true,
        AuthoredDeclarationSelectionExposure::PublicInterface,
    )
}

pub(crate) fn project_policy_type_parameters(
    compilation: &CheckedCompilation,
    checked_source: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    contract_scopes: &[CallingContractScope],
    public_nominals: bool,
    selection_exposure: AuthoredDeclarationSelectionExposure,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    project_type_parameters_inner(
        compilation,
        parameters,
        "public policy",
        declaration_path,
        preceding_binders,
        ordinal_offset,
        lifetime_binders,
        0,
        Projection {
            public_nominals,
            policy_crash_guards: true,
            selection_exposure,
            substitutions,
            checked_source: Some(checked_source),
            contract_scopes,
        },
    )
}
