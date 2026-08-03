use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_constraints;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(crate) fn lower_invariant_definition(
    lowerer: &mut Lowerer,
    invariant_definition: &resolved::invariant::InvariantDefinition,
) -> Result<typed::invariant::InvariantDefinition, Diagnostic> {
    let constraints = lower_type_constraints(lowerer, invariant_definition.constraints)?;

    Ok(typed::invariant::InvariantDefinition {
        symbol: invariant_definition.symbol,
        name: crate::name::lower_name(&invariant_definition.name),
        constraints,
    })
}
