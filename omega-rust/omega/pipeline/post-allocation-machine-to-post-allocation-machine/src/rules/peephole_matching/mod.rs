//! Optimizer module role: executable entrance. Declarative symbolic peephole matching.
//!
//! A rule supplies a bounded [`InstructionPairPattern`]. This entrance resolves named
//! register sets, matches each instruction footprint, and then checks the
//! cross-instruction liveness contract for the descriptor's exact topology. It
//! proposes matches only; independent
//! rule validators retain replay authority.

mod instruction;
mod liveness;
mod model;
mod registers;
mod relations;

pub(crate) use model::{
    ControlPattern, FixedViewPattern, InstructionPairMatch, InstructionPairMatchError,
    InstructionPairPattern, InstructionPairPatternId, InstructionPairTopology, InstructionPattern,
    MatchedPhysicalRead, OperandCoordinate, OperandPattern, OperandReadPattern, OperandRelation,
    OperandWritePattern, PairInstruction, UnitSetPattern, ViewPattern,
};

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::SelectedInstruction;
use selected_instructions_to_register_homes::{BlockLiveness, InstructionLiveness};

use physical_instructions::PostAllocationMachineInstruction;

#[allow(clippy::too_many_arguments)]
pub(crate) fn match_instruction_pair(
    pattern: &InstructionPairPattern,
    topology: InstructionPairTopology,
    first: &SelectedInstruction,
    second: &SelectedInstruction,
    machine_first: &PostAllocationMachineInstruction,
    machine_second: &PostAllocationMachineInstruction,
    live_first: &InstructionLiveness,
    live_second: &InstructionLiveness,
    live_block: &BlockLiveness,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<InstructionPairMatch, InstructionPairMatchError> {
    if pattern.topology() != topology {
        return Err(InstructionPairMatchError::Topology);
    }
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
    let second_reads = instruction::match_instruction(
        pattern.second(),
        second,
        machine_second,
        live_second,
        physical,
        &named,
        false,
    )?;
    let failed_relations = relations::failed_relations(pattern, &first_reads, &second_reads);
    let dead_sets_live_out = liveness::match_boundary(
        pattern,
        live_first,
        live_second,
        live_block,
        &named,
        &first_reads,
        &second_reads,
    )?;
    Ok(InstructionPairMatch::new(
        first_reads,
        second_reads,
        failed_relations,
        dead_sets_live_out,
    ))
}
