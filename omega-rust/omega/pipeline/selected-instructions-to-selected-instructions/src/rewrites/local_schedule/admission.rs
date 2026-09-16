//! Shared admission for in-block interchange: locate the named earlier
//! instruction inside one block's body, locate the named later instruction
//! after it in the same block, and prove the window they bound independent —
//! no register or condition-state hazard between either member and the
//! instructions between them, at most one roster-carrying memory actor among
//! the crossed positions, no call, hosted-effect, or terminator barrier
//! anywhere in the window, and no boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
};

use super::LocalScheduleError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    /// The earlier member's index in the block body.
    pub earlier_index: usize,
    /// The later member's index: the window the interchange crosses is
    /// `earlier_index..=later_index`, a single position when the pair is
    /// adjacent.
    pub later_index: usize,
}

/// Register locations an operand reads or writes. `UseDef` participates in
/// both directions, matching the access marks the constraint rows publish.
fn reads(operand_access: RegisterOperandAccess) -> bool {
    matches!(
        operand_access,
        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
    )
}

fn writes(operand_access: RegisterOperandAccess) -> bool {
    matches!(
        operand_access,
        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
    )
}

fn register_reads(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| reads(operand.access))
        .map(|operand| operand.virtual_register)
}

fn register_writes(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| writes(operand.access))
        .map(|operand| operand.virtual_register)
}

/// Whether `writer`'s writes meet `reader`'s reads on a register or a
/// condition-state unit. Implicit uses read units; implicit definitions and
/// clobbers write them, so flag publishers and flag consumers couple the
/// same way explicit operands do.
fn writes_meet_reads(writer: &SelectedInstruction, reader: &SelectedInstruction) -> bool {
    register_writes(writer)
        .any(|register| register_reads(reader).any(|candidate| candidate == register))
        || writer
            .implicit_defs
            .iter()
            .chain(writer.clobbers.iter())
            .any(|unit| reader.implicit_uses.contains(unit))
}

/// Whether two instructions can exchange order. The three hazards cover the
/// named members and every crossed position: `earlier` defining a location
/// `later` reads (RAW) would starve the consumer, `earlier` reading a
/// location `later` writes (WAR) would hand it the new value, and a shared
/// written location (WAW) would change which definition later positions
/// observe. Units and registers participate identically.
fn coupled(earlier: &SelectedInstruction, later: &SelectedInstruction) -> bool {
    writes_meet_reads(earlier, later)
        || writes_meet_reads(later, earlier)
        || register_writes(earlier)
            .any(|register| register_writes(later).any(|candidate| candidate == register))
        || earlier
            .implicit_defs
            .iter()
            .chain(earlier.clobbers.iter())
            .any(|unit| later.implicit_defs.contains(unit) || later.clobbers.contains(unit))
}

/// Calls, hosted effects, and terminator kinds are always barriers: they
/// can observe or expose reachable state regardless of roster rows, and a
/// terminator kind never belongs in a block body. Same boundary the
/// memory-motion rules enforce.
fn is_barrier(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    matches!(
        instruction.kind,
        CallUnit { .. }
            | CallScalar { .. }
            | CallAggregate { .. }
            | HostedReadByte { .. }
            | HostedWriteByteI32 { .. }
            | HostedExitProcessI32
            | ReturnScalar
            | ReturnAggregate { .. }
            | ReturnUnit
            | Jump
            | ConditionalBranchNonZero
            | ConditionalBranchU64LessThan
            | ConditionalBranchI64LessThan
    )
}

/// A member without a roster row must be unable to reach any semantic or
/// place-backed storage: private-slot frame accesses touch compiler-owned
/// spill/boundary slots that no referent place aliases, and pure register
/// work has no memory side at all. Any other memory-capable kind without a
/// row is an unaccounted access whose reach the interchange cannot prove.
fn unaccounted_kind(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    !matches!(
        instruction.kind,
        Store64 {
            slot: FrameStorageSlotId::Local(
                LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. },
            ),
            ..
        } | Load8 { .. }
            | Load16 { .. }
            | Load32 { .. }
            | Load64 { .. }
            | Load8Indexed
            | LoadPacked { .. }
            | FrameAddress { .. }
            | AddressOffset { .. }
            | ByteViewAddress
            | CopyI64
            | MaterializeI64 { .. }
            | CompareI64
            | CompareI64Zero
            | CompareI64Immediate { .. }
            | ExactAddI64 { .. }
            | ExactSubtractI64 { .. }
            | ExactAddI64Immediate { .. }
            | ExactSubtractI64Immediate { .. }
            | SaturatingAdd { .. }
            | WrappingAddI64
            | SaturatingSubtract { .. }
            | SaturatingDivide { .. }
            | ExactDivideU64 { .. }
            | WrappingRemainderI64 { .. }
            | BitwiseAndI64
            | BitwiseXorI64
            | ZeroExtendU8
            | ZeroExtendU16
            | ZeroExtendU32
            | SignExtendI8
            | SignExtendI16
            | SignExtendI32
            | Float32ToBits
            | Float64ToBits
            | BitsToFloat32
            | BitsToFloat64
            | MaterializeBooleanEqual
            | MaterializeBooleanU64LessThan
            | MaterializeBooleanI64LessThan
            | MaterializeBooleanU64LessOrEqual
            | MaterializeBooleanI64LessOrEqual
    )
}

/// Whether the roster accounts for the member's memory reach. Rows name the
/// instruction by identity, so the interchange retains them unchanged.
fn has_memory_rows(function: &SelectedFunction, instruction: SelectedInstructionId) -> bool {
    function
        .memory_accesses
        .iter()
        .any(|access| access.instruction == instruction)
}

/// A call contract row makes the member an effect barrier even when its
/// kind survived the kind check.
fn has_call_contract(function: &SelectedFunction, instruction: SelectedInstructionId) -> bool {
    function
        .calls
        .iter()
        .any(|call| call.instruction == instruction)
}

/// A boundary settlement at `position` sits before that body ordinal: any
/// position after the earlier member through the later member's own index
/// observes a different executed set once the pair trades places, while
/// positions at or outside the window's span see the same executed set on
/// either order.
fn interior_settlement(
    function: &SelectedFunction,
    block: SelectedBlockId,
    window: std::ops::RangeInclusive<usize>,
) -> bool {
    function.boundary_settlements.iter().any(|settlement| {
        settlement.block == block && window.contains(&(settlement.instruction_index as usize))
    })
}

/// Whether one instruction may trade order with a crossed instruction at
/// all: not a barrier kind, not named by the call roster, and either
/// roster-accounted or unable to reach storage the roster covers.
fn schedulable(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
) -> Result<bool, LocalScheduleError> {
    if is_barrier(instruction) || has_call_contract(function, instruction.id) {
        return Err(LocalScheduleError::UnsupportedInstruction);
    }
    let accounted = has_memory_rows(function, instruction.id);
    if !accounted && unaccounted_kind(instruction) {
        return Err(LocalScheduleError::UnsupportedInstruction);
    }
    Ok(accounted)
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, LocalScheduleError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LocalScheduleError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LocalScheduleError::SourceMismatch)?;
    let (block_index, earlier_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == earlier)
                .map(|earlier_index| (block_index, earlier_index))
        })
        .ok_or(LocalScheduleError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The pair is the two named instructions in the named order inside this
    // block; every instruction between them belongs to the window the
    // interchange crosses.
    let later_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later)
        .filter(|position| *position > earlier_index)
        .ok_or(LocalScheduleError::UnsupportedPair)?;
    let earlier_instruction = &block.instructions[earlier_index];
    let later_instruction = &block.instructions[later_index];
    let earlier_accounted = schedulable(function, earlier_instruction)?;
    let later_accounted = schedulable(function, later_instruction)?;
    // A roster-carrying access may only cross instructions that cannot
    // observe memory: a second accounted actor anywhere in the window would
    // need a place-alias decision this step does not take.
    if earlier_accounted && later_accounted {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    let member_accounted = earlier_accounted || later_accounted;
    let window = &block.instructions[earlier_index..=later_index];
    for interior in &window[1..window.len() - 1] {
        // Every crossed instruction meets the same schedulable bar as the
        // named members: no barrier kind, no call contract, and no
        // unaccounted memory reach. Its roster rows may stay only while
        // neither member carries any — the interior's recorded accesses
        // then keep their position while two memory-inert instructions
        // trade places around them.
        let interior_accounted = schedulable(function, interior)?;
        if interior_accounted && member_accounted {
            return Err(LocalScheduleError::UnsupportedPair);
        }
        // The interior keeps its position but trades order with both
        // members, so each direction of every hazard applies against each.
        if coupled(earlier_instruction, interior) || coupled(later_instruction, interior) {
            return Err(LocalScheduleError::UnsupportedPair);
        }
    }
    if interior_settlement(function, block.id, earlier_index + 1..=later_index) {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    if coupled(earlier_instruction, later_instruction) {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every crossed member's operand and unit lists
    // plus the function's three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            window.iter().try_fold(total, |total, instruction| {
                total
                    .checked_add(instruction.operands.len())?
                    .checked_add(instruction.implicit_uses.len())?
                    .checked_add(instruction.implicit_defs.len())?
                    .checked_add(instruction.clobbers.len())
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(LocalScheduleError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LocalScheduleError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LocalScheduleError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        earlier_index,
        later_index,
    })
}
