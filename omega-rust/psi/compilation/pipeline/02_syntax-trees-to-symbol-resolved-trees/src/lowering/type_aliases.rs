//! Source alias headers retain their own telescope and expansion custody.

use crate::lowering::data::lower_type_parameters;
use crate::lowering::machine::lower_generic_conformance_bounds;
use crate::lowering::name::{lower_name, lower_names};
use crate::lowering::type_reference::lower_type_reference_handle;
use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees::type_aliases::TypeAliasDefinition;
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_type_alias_definition(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    alias: &syntax::item::TypeAliasDefinition,
) -> Result<TypeAliasDefinition, Diagnostic> {
    let type_parameters = lower_type_parameters(lowerer, syntax, alias.type_parameters)?;
    let conformance_bounds =
        lower_generic_conformance_bounds(lowerer, syntax, &alias.conformance_bounds)?;
    let expansion = lower_type_reference_handle(lowerer, syntax, alias.expansion)?;
    Ok(TypeAliasDefinition {
        symbol: SymbolHandle::invalid(),
        name: lower_name(syntax, &alias.name),
        is_public: alias.is_public,
        lifetime_parameters: lower_names(syntax, &alias.lifetime_parameters),
        type_parameters,
        conformance_bounds,
        expansion,
    })
}
