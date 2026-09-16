//! Shared admission for cross-edge relocation: locate the named `member`
//! in one block's body, require that block to end in an unconditional
//! `Jump` whose semantic successor is the destination's block, and prove
//! the window the move crosses independent — no register or
//! condition-state hazard between the member and any crossed position, no
//! interference with the edge's register transports, no roster-carrying
//! member sharing the window with a second memory-access actor, no
//! barrier, call, hosted effect, or call-roster entry inside the window,
//! and no boundary settlement whose observed executed prefix changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockOrigin, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, SelectedValueTransport, VirtualRegisterId,
};

use super::EdgeRelocationError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The member's own block.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The destination block's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the target body: the
    /// member lands at this index. Naming the target's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
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

/// Whether the member can trade order with one crossed position. The three
/// hazards cover every position the move passes: the member defining a
/// location `crossed` reads (RAW) would starve the consumer or hand it a
/// different value once the member lands on the other side, the member
/// reading a location `crossed` writes (WAR) would hand the member the new
/// value, and a shared written location (WAW) would change which
/// definition later positions observe. Units and registers participate
/// identically. Crossed positions keep their relative order, so hazards
/// between them cannot change.
fn coupled(member: &SelectedInstruction, crossed: &SelectedInstruction) -> bool {
    writes_meet_reads(member, crossed)
        || writes_meet_reads(crossed, member)
        || register_writes(member)
            .any(|register| register_writes(crossed).any(|candidate| candidate == register))
        || member
            .implicit_defs
            .iter()
            .chain(member.clobbers.iter())
            .any(|unit| crossed.implicit_defs.contains(unit) || crossed.clobbers.contains(unit))
}

/// Calls, hosted effects, and terminator kinds are always barriers: they
/// can observe or expose reachable state regardless of roster rows, and a
/// terminator kind never belongs in a block body. Same boundary the
/// memory-motion rules enforce. The `Jump` instruction the member's own
/// block ends in is the crossed edge itself, not a window position, and is
/// audited separately.
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

/// An instruction without a roster row must be unable to reach any semantic
/// or place-backed storage: private-slot frame accesses touch
/// compiler-owned spill/boundary slots that no referent place aliases, and
/// pure register work has no memory side at all. Any other memory-capable
/// kind without a row is an unaccounted access whose reach the relocation
/// cannot prove.
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

/// Whether the roster accounts for the instruction's memory reach. Rows
/// name the instruction by identity, so the relocation retains them
/// unchanged.
fn has_memory_rows(function: &SelectedFunction, instruction: SelectedInstructionId) -> bool {
    function
        .memory_accesses
        .iter()
        .any(|access| access.instruction == instruction)
}

/// A call contract row makes the instruction an effect barrier even when
/// its kind survived the kind check.
fn has_call_contract(function: &SelectedFunction, instruction: SelectedInstructionId) -> bool {
    function
        .calls
        .iter()
        .any(|call| call.instruction == instruction)
}

/// Whether one body instruction may trade order with the member at all:
/// not a barrier kind, not named by the call roster, and either
/// roster-accounted or unable to reach storage the roster covers.
fn schedulable(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
) -> Result<bool, EdgeRelocationError> {
    if is_barrier(instruction) || has_call_contract(function, instruction.id) {
        return Err(EdgeRelocationError::UnsupportedInstruction);
    }
    let accounted = has_memory_rows(function, instruction.id);
    if !accounted && unaccounted_kind(instruction) {
        return Err(EdgeRelocationError::UnsupportedInstruction);
    }
    Ok(accounted)
}

/// Every successor edge a terminator names; `HostedExitProcess` and
/// `Return` name none.
fn terminator_successors(block: &SelectedBlock) -> Vec<&SelectedSuccessor> {
    match &block.terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::HostedExitProcess { .. } | SelectedTerminator::Return { .. } => {
            Vec::new()
        }
    }
}

/// The instruction a terminator carries — the `Jump`'s own row is a
/// position the move crosses; conditional and return forms are refused
/// before this is reached.
fn terminator_instruction(block: &SelectedBlock) -> &SelectedInstruction {
    match &block.terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, EdgeRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(EdgeRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(EdgeRelocationError::SourceMismatch)?;
    let (block_index, member_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(EdgeRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // Only an unconditional semantic jump edge keeps the member's execution
    // count: every traversal of the member's block leaves through it, and —
    // with the single-predecessor rule below — every traversal of the
    // destination block arrives through it. A conditional terminator keeps
    // a second exit the member would still execute on after relocating.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &block.terminator
    else {
        return Err(EdgeRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if successor.role != SelectedSuccessorRole::Semantic
        || successor.structural_case.is_some()
        || !successor.fuel.is_empty()
        || successor.structural_bindings.iter().any(|binding| {
            binding.transport != selected_instructions::SelectedStructuralTransport::Unused
        })
    {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    let target_index = function
        .blocks
        .iter()
        .position(|candidate| candidate.id == successor.block)
        .ok_or(EdgeRelocationError::SourceMismatch)?;
    let target = &function.blocks[target_index];
    // A self-edge is the in-block family's case with a back-edge transport
    // reading, not a cross-edge window. The entry block is reached with no
    // predecessor at all, and an implementation block's origin carries edge
    // or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    // Every edge into the destination block must be this one: a second
    // predecessor gives the block a path the member would newly execute on.
    if function
        .blocks
        .iter()
        .flat_map(terminator_successors)
        .filter(|edge| edge.block == target.id)
        .count()
        != 1
    {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    // The destination names a position in the target body — the member
    // lands at its index — or the target's terminator-carried instruction,
    // landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(target).id == destination).then_some(target.instructions.len())
        })
        .ok_or(EdgeRelocationError::UnsupportedPair)?;
    let member_accounted = schedulable(function, member_instruction)?;
    // The edge's register transports sit between the member's old and new
    // positions: a member defining the transported argument would hand the
    // binding a stale value, a member defining the parameter would be
    // overwritten by it, and a member reading the parameter would observe
    // the transported value only after the move. Reading the argument is
    // harmless — the binding never writes it.
    for binding in &successor.bindings {
        if let SelectedValueTransport::Registers {
            argument,
            parameter,
        } = binding.transport
            && (register_writes(member_instruction)
                .any(|register| register == argument || register == parameter)
                || register_reads(member_instruction).any(|register| register == parameter))
        {
            return Err(EdgeRelocationError::UnsupportedPair);
        }
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the hazard, call, or
    // memory accounting. Rows the roster records with the edge's own origin
    // count as the edge position's memory surface.
    if has_call_contract(function, terminator.id) {
        return Err(EdgeRelocationError::UnsupportedInstruction);
    }
    let edge_accounted = has_memory_rows(function, terminator.id)
        || function.memory_accesses.iter().any(|access| {
            access.origin
                == selected_instructions::SelectedMemoryAccessOrigin::Edge(successor.psi_edge)
        });
    if member_accounted && edge_accounted {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    if coupled(member_instruction, terminator) {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    // The member trades order with the positions behind it in its own body
    // and the positions before the landing index in the destination body.
    // Every other position keeps the member on the side it always had.
    for crossed in block.instructions[member_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        let crossed_accounted = schedulable(function, crossed)?;
        if member_accounted && crossed_accounted {
            return Err(EdgeRelocationError::UnsupportedPair);
        }
        if coupled(member_instruction, crossed) {
            return Err(EdgeRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the source block's prefix; a settlement positioned past the landing
    // index observes it inside the destination's. Both refuse; positions at
    // or before either boundary keep the executed set they always had.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member and count predecessor edges; the window audit walks
    // the member's surface against each crossed position's, plus the
    // function's three rosters and the edge's binding roster.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, candidate| {
                candidate
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminator_instruction(candidate)))
                    .try_fold(total, |total, _| total.checked_add(1))
            })
        })
        .and_then(|total| {
            block.instructions[member_index + 1..]
                .iter()
                .chain(target.instructions[..landing_index].iter())
                .chain(std::iter::once(terminator))
                .try_fold(total, |total, crossed| {
                    total
                        .checked_add(surface(member_instruction))?
                        .checked_add(surface(crossed))
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())?
                .checked_add(successor.bindings.len())
        })
        .ok_or(EdgeRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| EdgeRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(EdgeRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}

/// One instruction's hazard-audit surface: the operand, implicit-use,
/// implicit-definition, and clobber rows `coupled` walks.
fn surface(instruction: &SelectedInstruction) -> usize {
    instruction.operands.len()
        + instruction.implicit_uses.len()
        + instruction.implicit_defs.len()
        + instruction.clobbers.len()
}
