use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    domain: &resolved::domain::DomainDefinition,
) -> Result<typed::domain::DomainDefinition, Diagnostic> {
    Ok(typed::domain::DomainDefinition {
        symbol: domain.symbol,
        name: crate::name::lower_name(&domain.name),
        target_type: lower_type_reference_into_table(lowerer, &domain.target_type)?,
        body_token_count: domain.body_token_count,
    })
}
