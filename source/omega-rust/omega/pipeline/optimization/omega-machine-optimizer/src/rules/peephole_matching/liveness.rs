use omega_regalloc::{BlockLiveness, InstructionLiveness};

use super::{TerminalPairMatchError, TerminalPairPattern, model::ResolvedNamedUnitSet, registers};

pub(super) fn match_boundary(
    pattern: &TerminalPairPattern,
    first: &InstructionLiveness,
    second: &InstructionLiveness,
    block: &BlockLiveness,
    named: &[ResolvedNamedUnitSet],
) -> Result<bool, TerminalPairMatchError> {
    let live_through = registers::units_for(pattern.live_through(), named);
    if first.unit_live_out != second.unit_live_in
        || !live_through
            .iter()
            .all(|unit| first.unit_live_out.contains(unit) && second.unit_uses.contains(unit))
    {
        return Err(TerminalPairMatchError::Liveness(second.instruction));
    }
    let dead_after = registers::units_for(pattern.dead_after(), named);
    Ok(second
        .unit_live_out
        .iter()
        .chain(&block.unit_live_out)
        .chain(
            block
                .successors
                .iter()
                .flat_map(|successor| &successor.unit_live),
        )
        .any(|unit| dead_after.contains(unit)))
}
