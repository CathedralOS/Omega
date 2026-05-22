use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::domain::DomainDefinition;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    domain: &syntax::item::DomainDefinition,
) -> Result<DomainDefinition, Diagnostic> {
    Ok(DomainDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&domain.name),
        target_type: lower_type_reference_handle(lowerer, syntax_trees, domain.target_type)?,
        body_token_count: domain.body_token_count,
    })
}
