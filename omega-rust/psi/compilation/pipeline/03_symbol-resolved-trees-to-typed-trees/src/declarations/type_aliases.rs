//! Source alias headers survive typing independently of their expanded carriers.

use crate::lowerer::Lowerer;
use crate::lowerer::name::{lower_name, lower_names};
use crate::signatures::type_parameters::lower_type_parameters;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_type_alias_definition(
    lowerer: &mut Lowerer,
    alias: &resolved::type_aliases::TypeAliasDefinition,
) -> Result<typed::type_aliases::TypeAliasDefinition, Diagnostic> {
    let type_parameters = lower_type_parameters(lowerer, alias.type_parameters)?;
    let conformance_bounds =
        crate::signatures::evidence::lower_conformance_bounds(lowerer, &alias.conformance_bounds)?;
    let expansion = lower_type_reference_into_table(lowerer, &alias.expansion)?;
    Ok(typed::type_aliases::TypeAliasDefinition {
        symbol: alias.symbol,
        name: lower_name(&alias.name),
        is_public: alias.is_public,
        lifetime_parameters: lower_names(&alias.lifetime_parameters),
        type_parameters,
        conformance_bounds,
        expansion,
    })
}
