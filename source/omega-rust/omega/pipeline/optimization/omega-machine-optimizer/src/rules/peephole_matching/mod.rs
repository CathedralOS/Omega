//! Optimizer module role: executable entrance. Declarative symbolic peephole matching.
//!
//! A rule supplies a [`TerminalPairPattern`]. This entrance resolves named
//! register sets, matches each instruction footprint, and then checks the
//! cross-instruction liveness contract. It proposes matches only; independent
//! rule validators retain replay authority.

mod instruction;
mod liveness;
mod model;
mod registers;

pub(crate) use model::{
    InstructionPattern, MatchedPhysicalRead, OperandPattern, TerminalPairMatch,
    TerminalPairMatchError, TerminalPairPattern, TerminalPairPatternId, UnitSetPattern,
    ViewPattern,
};

use omega_regalloc::{BlockLiveness, InstructionLiveness};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::SelectedInstruction;

use crate::PostAllocationMachineInstruction;

#[allow(clippy::too_many_arguments)]
pub(crate) fn match_terminal_pair(
    pattern: &TerminalPairPattern,
    first: &SelectedInstruction,
    second: &SelectedInstruction,
    machine_first: &PostAllocationMachineInstruction,
    machine_second: &PostAllocationMachineInstruction,
    live_first: &InstructionLiveness,
    live_second: &InstructionLiveness,
    live_block: &BlockLiveness,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<TerminalPairMatch, TerminalPairMatchError> {
    let named = registers::resolve_named_sets(pattern, physical)?;
    let first_reads = instruction::match_instruction(
        pattern.first(),
        first,
        machine_first,
        live_first,
        physical,
        &named,
        true,
    )?;
    instruction::match_instruction(
        pattern.second(),
        second,
        machine_second,
        live_second,
        physical,
        &named,
        false,
    )?;
    let dead_sets_live_out =
        liveness::match_boundary(pattern, live_first, live_second, live_block, &named)?;
    Ok(TerminalPairMatch::new(first_reads, dead_sets_live_out))
}
