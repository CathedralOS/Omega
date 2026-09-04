use omega_regalloc::{BlockLiveness, InstructionLiveness};

use super::{
    InstructionPairMatchError, InstructionPairPattern, MatchedPhysicalRead,
    model::ResolvedNamedUnitSet, registers, relations,
};

pub(super) fn match_boundary(
    pattern: &InstructionPairPattern,
    first: &InstructionLiveness,
    second: &InstructionLiveness,
    block: &BlockLiveness,
    named: &[ResolvedNamedUnitSet],
    first_operands: &[MatchedPhysicalRead],
    second_operands: &[MatchedPhysicalRead],
) -> Result<bool, InstructionPairMatchError> {
    let live_through = registers::units_for(pattern.live_through(), named);
    if first.unit_live_out != second.unit_live_in
        || !live_through
            .iter()
            .all(|unit| first.unit_live_out.contains(unit) && second.unit_uses.contains(unit))
    {
        return Err(InstructionPairMatchError::Liveness(second.instruction));
    }
    for coordinate in pattern.live_through_operands() {
        let Some(operand) = relations::operand(*coordinate, first_operands, second_operands) else {
            return Err(InstructionPairMatchError::Liveness(second.instruction));
        };
        if !operand.storage_units.iter().all(|unit| {
            first.unit_live_out.contains(unit)
                && second.unit_live_in.contains(unit)
                && second.unit_uses.contains(unit)
        }) {
            return Err(InstructionPairMatchError::Liveness(second.instruction));
        }
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
