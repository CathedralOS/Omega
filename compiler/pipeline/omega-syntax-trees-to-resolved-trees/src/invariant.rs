use crate::program::Lowerer;
use crate::type_reference::lower_type_constraints;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_syntax_trees as syntax;
use omega_resolved_trees::invariant::InvariantDefinition;

pub(crate) fn lower_invariant_definition(
    lowerer: &mut Lowerer,
    invariant_definition: &syntax::item::InvariantDefinition,
) -> Result<InvariantDefinition, Diagnostic> {
    let constraints = lower_type_constraints(lowerer, &invariant_definition.constraints)?;

    Ok(InvariantDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&invariant_definition.name),
        constraints,
    })
}
