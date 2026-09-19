//! Shared admission for copy removal: locate the named `CopyI64`, prove its
//! clean `[use source, def destination]` shape against the target's own
//! constraint row, then exhaustively classify every mention of the
//! destination register in the function.
//!
//! Every `VirtualRegisterId` surface is audited: body and terminator-carried
//! instruction operands, successor `bindings`/`structural_bindings`/
//! `structural_case` transports, `local_storage_slots`, instruction-carried
//! frame slots, `memory_accesses` slot roles, call contracts, and other
//! registers' origins. A mention outside the admitted set — any use that is
//! not a plain `Use` operand later in the copy's own block, and any second
//! definition anywhere — refuses the rewrite rather than guessing which
//! surface semantics a substitution would disturb.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId,
    SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedMemoryAccessRole, SelectedStructuralTransport,
    SelectedSuccessor, SelectedTerminator, SelectedValueTransport, VirtualRegisterId,
    VirtualRegisterOrigin,
};

use super::CopyRemovalError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{block_instructions, terminator_successors};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub copy_index: usize,
    /// The surviving register the admitted uses rebind to.
    pub input: VirtualRegisterId,
    /// The destination row's position in `virtual_registers`; removal drops
    /// it at this index.
    pub register_index: usize,
    /// Operands rebound to `input`, in source ordinals. A position below the
    /// block's body length indexes `instructions`; the body length itself
    /// indexes the terminator's carried instruction.
    pub uses: Vec<CopyUse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CopyUse {
    pub position: usize,
    pub operand: usize,
}

fn terminator_instruction_mut(terminator: &mut SelectedTerminator) -> &mut SelectedInstruction {
    match terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

/// The instruction at a source-ordinal position: a body index, or the
/// terminator's carried instruction at the position equal to the body length.
fn block_instruction_mut(
    block: &mut SelectedBlock,
    position: usize,
) -> Option<&mut SelectedInstruction> {
    if position == block.instructions.len() {
        Some(terminator_instruction_mut(&mut block.terminator))
    } else {
        block.instructions.get_mut(position)
    }
}

fn local_slot_mentions(slot: LocalStorageSlotId, register: VirtualRegisterId) -> bool {
    matches!(slot, LocalStorageSlotId::Spill { register: subject } if subject == register)
}

fn frame_slot_mentions(slot: FrameStorageSlotId, register: VirtualRegisterId) -> bool {
    match slot {
        FrameStorageSlotId::Local(slot) => local_slot_mentions(slot, register),
        _ => false,
    }
}

/// Register mentions reachable through one instruction's kind payload: the
/// frame and hosted-access slot identities, the only kind fields that name
/// a virtual register indirectly.
fn instruction_slot_mentions(
    instruction: &SelectedInstruction,
    register: VirtualRegisterId,
) -> bool {
    match instruction.kind {
        SelectedInstructionKind::Store64 { slot, .. }
        | SelectedInstructionKind::FrameAddress { slot, .. } => frame_slot_mentions(slot, register),
        SelectedInstructionKind::HostedReadByte { slot }
        | SelectedInstructionKind::HostedWriteByteI32 { slot } => {
            local_slot_mentions(slot, register)
        }
        _ => false,
    }
}

/// A register origin that names the removed instruction or register. Origins
/// are the roster's definition claims: another register produced by the copy
/// or bound to the destination's spill storage cannot survive the removal.
fn origin_mentions(
    origin: VirtualRegisterOrigin,
    instruction: SelectedInstructionId,
    register: VirtualRegisterId,
) -> bool {
    use VirtualRegisterOrigin::*;
    match origin {
        InstructionScratch {
            instruction: id, ..
        }
        | StructuralObservation {
            instruction: id, ..
        }
        | ScalarAbiAddress {
            instruction: id, ..
        }
        | AbiTransport {
            instruction: id, ..
        }
        | InstructionResult {
            instruction: id, ..
        } => id == instruction,
        SpillAddress {
            instruction: id,
            register: subject,
        } => id == instruction || subject == register,
        StructuralParameter { .. } | EntryParameter { .. } | BlockParameter { .. } => false,
    }
}

/// Whether one successor edge mentions `register` in any transport field or
/// slot destination. Arguments read the register on the edge; parameters
/// define it there. Neither belongs to the same-block substitution surface.
fn successor_mentions(successor: &SelectedSuccessor, register: VirtualRegisterId) -> bool {
    let binding_mentions = successor
        .bindings
        .iter()
        .any(|binding| match binding.transport {
            SelectedValueTransport::Registers {
                argument,
                parameter,
            } => argument == register || parameter == register,
            SelectedValueTransport::Unused => false,
        });
    let structural_mentions =
        successor
            .structural_bindings
            .iter()
            .any(|binding| match binding.transport {
                SelectedStructuralTransport::WholeValue {
                    argument,
                    destination,
                    ..
                }
                | SelectedStructuralTransport::Descriptor {
                    argument,
                    destination,
                } => argument == register || local_slot_mentions(destination, register),
                SelectedStructuralTransport::Unused => false,
            });
    let case_mentions = successor.structural_case.as_ref().is_some_and(|case| {
        local_slot_mentions(case.slot, register)
            || case.payloads.iter().any(|payload| match payload.transport {
                SelectedCasePayloadTransport::Unmaterialized { parameter } => parameter == register,
                SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } => argument == register || parameter == register,
                SelectedCasePayloadTransport::Unused => false,
            })
    });
    binding_mentions || structural_mentions || case_mentions
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    copy: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, CopyRemovalError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CopyRemovalError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CopyRemovalError::SourceMismatch)?;
    let (block_index, copy_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == copy)
                .map(|copy_index| (block_index, copy_index))
        })
        .ok_or(CopyRemovalError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let copy_instruction = &block.instructions[copy_index];
    // The removed instruction must be the target's plain register copy: a
    // single read rebinding a single definition, with no implicit unit
    // traffic or operand constraints the removal would silently drop.
    if copy_instruction.kind != SelectedInstructionKind::CopyI64
        || copy_instruction.operands.len() != 2
        || !copy_instruction.implicit_uses.is_empty()
        || !copy_instruction.implicit_defs.is_empty()
        || !copy_instruction.clobbers.is_empty()
    {
        return Err(CopyRemovalError::UnsupportedInstruction);
    }
    let input_operand = &copy_instruction.operands[0];
    let output_operand = &copy_instruction.operands[1];
    if input_operand.operand != 0
        || input_operand.access != RegisterOperandAccess::Use
        || output_operand.operand != 1
        || output_operand.access != RegisterOperandAccess::Def
        || copy_instruction.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(CopyRemovalError::UnsupportedInstruction);
    }
    let input = input_operand.virtual_register;
    let output = output_operand.virtual_register;
    if input == output {
        return Err(CopyRemovalError::UnsupportedRegister);
    }
    let register_index = function
        .virtual_registers
        .iter()
        .position(|register| register.id == output)
        .ok_or(CopyRemovalError::UnsupportedRegister)?;
    let output_register = &function.virtual_registers[register_index];
    let input_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == input)
        .ok_or(CopyRemovalError::UnsupportedRegister)?;
    // The roster must agree this copy defines the destination: an origin
    // naming another surface keeps a definition the removal cannot honor,
    // and an ABI live-in view cannot disappear with the row.
    let copy_defined = match output_register.origin {
        VirtualRegisterOrigin::InstructionResult { instruction, .. } => instruction == copy,
        VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand,
        } => instruction == copy && operand == 1,
        _ => false,
    };
    if !copy_defined
        || output_register.entry_fixed_view.is_some()
        || output_register.scalar_type != input_register.scalar_type
        || output_register.class != input_register.class
    {
        return Err(CopyRemovalError::UnsupportedRegister);
    }
    // The copy's own constraint row must declare the same `[use, def]` shape
    // at the classes the roster assigns; a different row would change what
    // removing the instruction means.
    let row = environment
        .constraint(copy_instruction.constraint)
        .ok_or(CopyRemovalError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != input_register.class
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Def
        || row.operands[1].class != output_register.class
    {
        return Err(CopyRemovalError::ConstraintMismatch);
    }
    // Removing the copy orphans anything that names it outside the admitted
    // roster row: call contracts and memory-access rows binding the
    // instruction, and the destination's spill storage slot.
    if function.calls.iter().any(|call| call.instruction == copy)
        || function
            .memory_accesses
            .iter()
            .any(|access| access.instruction == copy)
    {
        return Err(CopyRemovalError::UnsupportedInstruction);
    }
    if function
        .local_storage_slots
        .iter()
        .any(|slot| local_slot_mentions(slot.id, output))
        || function
            .memory_accesses
            .iter()
            .any(|access| match access.role {
                SelectedMemoryAccessRole::WriteLocal { slot }
                | SelectedMemoryAccessRole::AddressLocal { slot } => {
                    local_slot_mentions(slot, output)
                }
                _ => false,
            })
        || function
            .virtual_registers
            .iter()
            .enumerate()
            .any(|(index, register)| {
                index != register_index && origin_mentions(register.origin, copy, output)
            })
    {
        return Err(CopyRemovalError::UnsupportedUse);
    }
    // Classify every destination mention in the function. Each must be a
    // plain `Use` operand later in the copy's own block — body or
    // terminator-carried — with the source register's class and no operand
    // constraints; anything else is a second definition or a surface the
    // substitution does not cover.
    let mut uses = Vec::new();
    for (current_index, current) in function.blocks.iter().enumerate() {
        for (position, instruction) in block_instructions(current).enumerate() {
            if instruction_slot_mentions(instruction, output) {
                return Err(CopyRemovalError::UnsupportedUse);
            }
            for (operand_index, operand) in instruction.operands.iter().enumerate() {
                if operand.virtual_register != output {
                    continue;
                }
                if instruction.id == copy {
                    continue;
                }
                if current_index != block_index
                    || position <= copy_index
                    || operand.access != RegisterOperandAccess::Use
                    || operand.fixed_view.is_some()
                    || operand.tied_to.is_some()
                    || operand.early_clobber
                    || operand.class != input_register.class
                {
                    return Err(CopyRemovalError::UnsupportedUse);
                }
                uses.push(CopyUse {
                    position,
                    operand: operand_index,
                });
            }
        }
        if terminator_successors(&current.terminator)
            .into_iter()
            .any(|successor| successor_mentions(successor, output))
        {
            return Err(CopyRemovalError::UnsupportedUse);
        }
    }
    // A use-free copy is dead code, not a substitution candidate; that
    // removal belongs to a dead-instruction family with its own admission.
    if uses.is_empty() {
        return Err(CopyRemovalError::UnsupportedUse);
    }
    // Between the copy and its last admitted use, nothing may redefine the
    // source: an intervening `Def`/`UseDef` would leave later uses reading
    // the new value instead of the copied one. The interval stops before
    // the last use's own instruction — operand uses read pre-definition, so
    // even a definition on that instruction cannot disturb the read.
    let last = uses.last().map(|site| site.position).unwrap_or(0);
    for (position, instruction) in block_instructions(block).enumerate() {
        if position <= copy_index {
            continue;
        }
        if position >= last {
            break;
        }
        if instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == input
        }) {
            return Err(CopyRemovalError::UnsupportedUse);
        }
    }
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            // A second scan of this function audits the destination's
            // mentions across instructions, transports, slots, and the
            // register roster.
            function.blocks.iter().try_fold(total, |total, block| {
                total
                    .checked_add(block.instructions.len())?
                    .checked_add(1)?
                    .checked_add(
                        terminator_successors(&block.terminator)
                            .into_iter()
                            .map(|successor| {
                                successor.bindings.len()
                                    + successor.structural_bindings.len()
                                    + successor
                                        .structural_case
                                        .as_ref()
                                        .map_or(0, |case| case.payloads.len() + 1)
                            })
                            .sum::<usize>(),
                    )
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.virtual_registers.len())?
                .checked_add(function.local_storage_slots.len())?
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())
        })
        .ok_or(CopyRemovalError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CopyRemovalError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CopyRemovalError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        copy_index,
        input,
        register_index,
        uses,
    })
}

/// The one-function transformation shared by proposal and replay: rebind
/// every admitted use to the source register, remove the copy and the
/// destination's roster row, and shift boundary settlements over the removed
/// ordinal. Both sides compute it from the source, never from each other.
pub(super) fn apply(
    admitted: &Admission<'_>,
    function: &mut SelectedFunction,
) -> Result<(), CopyRemovalError> {
    let block = function
        .blocks
        .get_mut(admitted.block_index)
        .ok_or(CopyRemovalError::SourceMismatch)?;
    if block.id != admitted.block {
        return Err(CopyRemovalError::SourceMismatch);
    }
    for site in &admitted.uses {
        block_instruction_mut(block, site.position)
            .and_then(|instruction| instruction.operands.get_mut(site.operand))
            .ok_or(CopyRemovalError::SourceMismatch)?
            .virtual_register = admitted.input;
    }
    function.boundary_settlements =
        shifted_boundary_settlements(admitted.function, admitted.block, admitted.copy_index)?;
    block.instructions.remove(admitted.copy_index);
    function.virtual_registers.remove(admitted.register_index);
    Ok(())
}

/// The block's boundary settlements after removing the instruction at
/// `removed`: positions at or before it name instructions that stay put, and
/// every later position — including the after-body position — shifts one
/// ordinal earlier. Register substitution needs no settlement change:
/// payloads name source operations and values, never registers.
pub(super) fn shifted_boundary_settlements(
    function: &SelectedFunction,
    block: SelectedBlockId,
    removed: usize,
) -> Result<Vec<selected_instructions::SelectedBoundarySettlement>, CopyRemovalError> {
    let body = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .ok_or(CopyRemovalError::SourceMismatch)?
        .instructions
        .len();
    let mut shifted = function.boundary_settlements.clone();
    for settlement in &mut shifted {
        if settlement.block != block {
            continue;
        }
        let position = settlement.instruction_index as usize;
        if position > body {
            return Err(CopyRemovalError::SourceMismatch);
        }
        if position > removed {
            settlement.instruction_index =
                u32::try_from(position - 1).map_err(|_| CopyRemovalError::IdentityOverflow)?;
        }
    }
    Ok(shifted)
}
