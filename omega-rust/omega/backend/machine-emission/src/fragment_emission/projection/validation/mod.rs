//! Optimizer module role: executable entrance. Direct fragment projection checks.

mod control;
mod instruction;
mod ordinary;

use super::ResolvedFragmentEmissionError;
use machine_code::{FunctionFragmentEmissionPlan, ResolvedMachineProgram};

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
    ordinary::check(selected, layout, fragments)?;

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
