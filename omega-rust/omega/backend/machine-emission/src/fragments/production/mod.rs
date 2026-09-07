//! Optimizer module role: executable entrance. Fragment assembly from current data.

mod ordinary_function;

use super::ResolvedFragmentEmissionError;
use machine_code::{FunctionFragmentEmissionPlan, ResolvedMachineProgram};
use optimization_core::FunctionFragmentEmissionIdentity;

pub(super) fn emit(
    program: &ResolvedMachineProgram,
) -> Result<FunctionFragmentEmissionPlan, ResolvedFragmentEmissionError> {
    let selected = &program.selected;
    let layout = &program.layout;
    if selected.target != layout.target || layout.selected != program.machine.selected {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let mut fragments = FunctionFragmentEmissionPlan {
        identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
        psi: selected.psi,
        fuel_schedule: selected.fuel_schedule,
        selected: layout.selected,
        target: selected.target,
        entry: selected.entry,
        functions: Vec::new(),
    };
    if selected.functions.len() != layout.functions.len() {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    for function in &selected.functions {
        let resolved = layout
            .functions
            .iter()
            .find(|row| row.machine == function.machine)
            .ok_or(ResolvedFragmentEmissionError::MissingFunction(
                function.machine,
            ))?;
        fragments
            .functions
            .push(ordinary_function::emit(function, resolved)?);
    }

    fragments.identity = fragments.recomputed_identity();
    Ok(fragments)
}
