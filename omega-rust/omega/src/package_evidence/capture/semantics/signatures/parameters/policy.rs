//! Policy signature projection with an explicit containing static telescope.
use super::Projection;
use crate::package_evidence::PackageReviewInput;
use crate::package_evidence::capture::semantics::signatures::parameters::CallingContractScope;
use crate::package_evidence::capture::semantics::signatures::parameters::project_type_parameters_inner;
use crate::package_evidence::record::PackageReviewTypeParameter;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use symbols::SymbolHandle;

pub(crate) fn project_policy_type_parameters_after(
    typed: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    checked_source: &PackageReviewInput<'_>,
    parameters: &[symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameter],
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
    lifetime_binders: &[symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier],
    contract_scopes: &[CallingContractScope],
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    project_policy_type_parameters(
        typed,
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
    typed: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    checked_source: &PackageReviewInput<'_>,
    parameters: &[symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameter],
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
    lifetime_binders: &[symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier],
    substitutions: &[(
        SymbolHandle,
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    )],
    contract_scopes: &[CallingContractScope],
    public_nominals: bool,
    selection_exposure: AuthoredDeclarationSelectionExposure,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    project_type_parameters_inner(
        typed,
        checked_source,
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
            contract_scopes: Some(contract_scopes),
        },
    )
}
