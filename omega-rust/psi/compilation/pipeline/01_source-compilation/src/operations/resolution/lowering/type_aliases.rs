/* Source alias headers retain their own telescope and expansion custody. */

use crate::resolution::lowering::data::lower_type_parameters;
use crate::resolution::lowering::lowerer::Lowerer;
use crate::resolution::lowering::machine::lower_generic_conformance_bounds;
use crate::resolution::lowering::name::{lower_name, lower_names};
use crate::resolution::lowering::type_reference::lower_type_reference_handle;
use crate::source_resolved::type_aliases::TypeAliasDefinition;
use diagnostics::Diagnostic;
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
    let previous = lowerer.structural_type_equations_pending;
    lowerer.structural_type_equations_pending = true;
    let expansion = lower_type_reference_handle(lowerer, syntax, alias.expansion);
    lowerer.structural_type_equations_pending = previous;
    let expansion = expansion?;
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
