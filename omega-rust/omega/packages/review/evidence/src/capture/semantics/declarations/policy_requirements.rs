//! Normalized policy coordinates remain separate from compiler replay names.

use super::{provider_requirement_identity, provider_requirement_schema};
use crate::capture::api::operators::project_operator_coordinate;
use crate::record::PackageReviewNominalIdentity;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use provider_planning::plans::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

pub(crate) fn policy_provider_requirement_identity(
    compilation: &CheckedCompilation,
    schema: ProviderSchemaDeclaration,
    requirement: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let declaration = provider_requirement_schema(compilation, schema, requirement)?;
    if !matches!(declaration, ProviderSchemaDeclaration::BoundaryOperator(_)) {
        return provider_requirement_identity(compilation, declaration, requirement);
    }
    let operator = typed_trees::operator::declaration_by_symbol(&compilation.typed, requirement)
        .filter(|operator| operator.is_boundary)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "operator policy has no exact boundary declaration",
            )]
        })?;
    let coordinate = project_operator_coordinate(compilation, operator)?;
    Ok(coordinate.policy_requirement_identity())
}
