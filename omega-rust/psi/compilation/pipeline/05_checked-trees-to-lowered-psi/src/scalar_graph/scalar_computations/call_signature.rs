//! The selected call signature owns custody independently of its mixed operand
//! evaluator. Boundary declarations and checked bodies retain their own identities.
use super::{CheckedTrees, LoweringError, unsupported};
use checked_trees::CheckedUnitStructuralParameterPlan;
use symbols::SymbolHandle;

pub(super) fn structural_parameters(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Result<&[CheckedUnitStructuralParameterPlan], LoweringError> {
    let mut boundaries = checked
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .filter(|boundary| boundary.machine == machine);
    if let Some(boundary) = boundaries.next() {
        if boundaries.next().is_some() || boundary.state != state {
            return unsupported("computed call has no unique boundary signature");
        }
        return Ok(&boundary.structural_parameters);
    }
    let target =
        crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
            checked, machine,
        )?;
    if target.entry_state()? != state || !target.entry_claims().is_empty() {
        return unsupported("computed call disagrees with its selected callee signature");
    }
    Ok(target.structural_parameters())
}
