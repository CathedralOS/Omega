//! Optimizer module role: executable entrance. Direct fragment projection checks.

mod control;
mod instruction;
mod ordinary;
mod structural;

use super::ResolvedFragmentEmissionError;
use omega_machine_code::{FunctionFragmentEmissionPlan, ResolvedMachineProgram};

pub(super) fn check(
    program: &ResolvedMachineProgram,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), ResolvedFragmentEmissionError> {
    let selected = &program.selected;
    let layout = &program.layout;
    require(
        fragments.psi == selected.psi
            && fragments.fuel_schedule == selected.fuel_schedule
            && fragments.selected == layout.selected
            && layout.selected == program.machine.selected
            && fragments.target == selected.target
            && layout.target == selected.target
            && fragments.entry == selected.entry,
    )?;
    if selected.structural_unit_functions.is_empty() {
        require(
            fragments.structural_unit_functions.is_empty()
                && layout.structural_unit_functions.is_empty(),
        )?;
        ordinary::check(selected, layout, fragments)?;
    } else {
        require(
            selected.functions.is_empty()
                && layout.functions.is_empty()
                && fragments.functions.is_empty(),
        )?;
        structural::check(selected, layout, fragments)?;
    }
    require(fragments.identity == fragments.recomputed_identity())
}

fn require(condition: bool) -> Result<(), ResolvedFragmentEmissionError> {
    if condition {
        Ok(())
    } else {
        Err(ResolvedFragmentEmissionError::ArtifactMismatch)
    }
}

fn byte_span(
    bytes: &[u8],
    offset: u64,
    content: &[u8],
) -> Result<(), ResolvedFragmentEmissionError> {
    let start =
        usize::try_from(offset).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    let end = start
        .checked_add(content.len())
        .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?;
    require(bytes.get(start..end) == Some(content))
}
