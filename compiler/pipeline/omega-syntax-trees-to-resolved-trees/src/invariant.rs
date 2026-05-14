use crate::program::Lowerer;
use crate::type_reference::lower_type_constraint_handles;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_syntax_trees::{self as syntax, SyntaxTrees};
use omega_resolved_trees::invariant::{InvariantDefinition, InvariantDefinitionStorage};

pub(crate) fn lower_invariant_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    invariant_definition: &syntax::item::InvariantDefinition,
) -> Result<InvariantDefinition, Diagnostic> {
    let constraints =
        lower_type_constraint_handles(lowerer, syntax_trees, invariant_definition.constraints)?;

    Ok(InvariantDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&invariant_definition.name),
        storage: InvariantDefinitionStorage { constraints },
    })
}
