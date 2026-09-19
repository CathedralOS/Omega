//! Independent validation of copy removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! `CopyI64` shape against the bound constraint row, the destination's
//! copy-claimed roster origin, every destination mention across body,
//! terminator, transport, storage, access, call, and roster surfaces, and
//! the source register's freedom from intervening redefinition — then
//! rebuilds the function the contract demands and requires the proposal to
//! equal it. Restoring the removed copy, the roster row, and the source
//! settlements must reproduce the complete source by content. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedCasePayloadTransport,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedMemoryAccessRole, SelectedStructuralTransport,
    SelectedSuccessor, SelectedTerminator, SelectedValueTransport, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{CopyRemovalError, CopyRemovalReceipt, ValidatedCopyRemoval};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{block_instructions, terminator_successors};

/// The validator's own reconstruction of the removal the contract permits:
/// the admitted copy's coordinates, its surviving source register, the
/// removed destination and roster position, and the operand sites that must
/// rebind. It shares no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    block: selected_instructions::SelectedBlockId,
    copy_index: usize,
    input: VirtualRegisterId,
    output: VirtualRegisterId,
    register_index: usize,
    uses: Vec<ReboundUse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReboundUse {
    position: usize,
    operand: usize,
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
fn instruction_at_mut(
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

/// Reconstruct the legality of removing `copy` from first principles: locate
/// the instruction, verify the target's plain `[use, def]` copy shape against
/// the bound constraint row and the roster, audit every mention of the
/// destination across the function's surfaces, and check that the copied
/// source is not redefined inside the substitution interval. Nothing in this
/// audit reads the producer's admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    copy: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, CopyRemovalError> {
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
    // Every destination mention must be a plain `Use` operand later in the
    // copy's own block — body or terminator-carried — at the source register's
    // class and free of operand constraints. The validator collects the same
    // admitted set on its own audit, so a producer that admitted a
    // transported, earlier, or write-position mention cannot pass validation.
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
                uses.push(ReboundUse {
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
    if uses.is_empty() {
        return Err(CopyRemovalError::UnsupportedUse);
    }
    // Nothing strictly between the copy and the last admitted use may redefine
    // the source; the interval stops before the last use's own instruction,
    // whose read still precedes any of its own definitions.
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
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        block: block.id,
        copy_index,
        input,
        output,
        register_index,
        uses,
    })
}

/// The validation work this audit performs, in the measured-step contract the
/// family publishes: one step per block plus one per instruction across the
/// plan, a second scan of the reconstructed function's blocks counting each
/// successor transport, and one step per roster, storage, access, and call
/// row.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
) -> Result<u64, CopyRemovalError> {
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
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
    u64::try_from(steps).map_err(|_| CopyRemovalError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own record:
/// every reconstructed use site rebinds to the surviving source register, the
/// copy instruction and the destination roster row leave, and boundary
/// settlements shift over the removed ordinal. The producer's `apply` is not
/// consulted; both sides derive the same function from the source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> Result<SelectedFunction, CopyRemovalError> {
    let mut expected = reconstructed.function.clone();
    let block = expected
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(CopyRemovalError::SourceMismatch)?;
    if block.id != reconstructed.block {
        return Err(CopyRemovalError::SourceMismatch);
    }
    for site in &reconstructed.uses {
        instruction_at_mut(block, site.position)
            .and_then(|instruction| instruction.operands.get_mut(site.operand))
            .ok_or(CopyRemovalError::SourceMismatch)?
            .virtual_register = reconstructed.input;
    }
    let body = block.instructions.len();
    expected.boundary_settlements = expected
        .boundary_settlements
        .into_iter()
        .map(|mut settlement| {
            if settlement.block == reconstructed.block {
                let position = settlement.instruction_index as usize;
                if position > body {
                    return Err(CopyRemovalError::SourceMismatch);
                }
                if position > reconstructed.copy_index {
                    settlement.instruction_index = u32::try_from(position - 1)
                        .map_err(|_| CopyRemovalError::IdentityOverflow)?;
                }
            }
            Ok(settlement)
        })
        .collect::<Result<Vec<_>, _>>()?;
    block.instructions.remove(reconstructed.copy_index);
    expected
        .virtual_registers
        .remove(reconstructed.register_index);
    Ok(expected)
}

/// Reinsert the copy and the roster row and un-rebind the reconstructed use
/// sites: undoing the validator's expected edit must restore the complete
/// source by content — every other instruction, register, call, settlement,
/// and function included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, CopyRemovalError> {
    let mut restored = proposed.clone();
    let function = &mut restored.functions[reconstructed.function_index];
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(CopyRemovalError::ReplayMismatch)?;
    if block.id != reconstructed.block {
        return Err(CopyRemovalError::ReplayMismatch);
    }
    block.instructions.insert(
        reconstructed.copy_index,
        reconstructed.function.blocks[reconstructed.block_index].instructions
            [reconstructed.copy_index]
            .clone(),
    );
    for site in &reconstructed.uses {
        instruction_at_mut(block, site.position)
            .and_then(|instruction| instruction.operands.get_mut(site.operand))
            .ok_or(CopyRemovalError::ReplayMismatch)?
            .virtual_register = reconstructed.output;
    }
    if reconstructed.register_index > function.virtual_registers.len() {
        return Err(CopyRemovalError::ReplayMismatch);
    }
    function.virtual_registers.insert(
        reconstructed.register_index,
        reconstructed.function.virtual_registers[reconstructed.register_index].clone(),
    );
    function.boundary_settlements = reconstructed.function.boundary_settlements.clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs the
/// removal's preconditions from the source, requires the proposal to equal
/// the function its own record produces, and restores the complete source by
/// content — every other instruction, register, call, and settlement
/// included. The producer's admission routine is never consulted, so a wrong
/// legality decision fails here even when the proposal matches the edit the
/// producer emitted.
pub fn validate_copy_removal(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    copy: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCopyRemoval, CopyRemovalError> {
    let reconstructed = reconstruct(source, function_index, copy, environment)?;
    if measured_steps(source.selected_plan(), reconstructed.function)? > budget.validation_steps() {
        return Err(CopyRemovalError::WorkBudgetExceeded);
    }
    let expected = expect(&reconstructed)?;
    if proposed.functions.get(function_index) != Some(&expected) {
        return Err(CopyRemovalError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(CopyRemovalError::ReplayMismatch);
    }
    Ok(ValidatedCopyRemoval {
        receipt: CopyRemovalReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

#[cfg(test)]
mod independence_tests {
    use std::sync::Arc;

    use abstract_operations::ValueBinding;
    use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
    use optimization_unit::ValueDefinitionSite;
    use register_environment::baseline_target_register_environment;
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding,
        SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType,
        ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        CopyRemovalError, CopyRemovalReceipt, ValidatedCopyRemoval, validate_copy_removal,
    };

    const LOAD: SelectedInstructionId = SelectedInstructionId(2);
    const COPY: SelectedInstructionId = SelectedInstructionId(3);
    const BRANCH: SelectedInstructionId = SelectedInstructionId(7);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(20);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const SOURCE: VirtualRegisterId = VirtualRegisterId(1);
    const COPIED: VirtualRegisterId = VirtualRegisterId(2);
    const SPARE: VirtualRegisterId = VirtualRegisterId(4);

    fn instruction(
        id: SelectedInstructionId,
        kind: SelectedInstructionKind,
        row: &RegisterInstructionConstraint,
        registers: &[VirtualRegisterId],
    ) -> SelectedInstruction {
        SelectedInstruction {
            id,
            kind,
            constraint: row.key,
            operands: row
                .operands
                .iter()
                .zip(registers)
                .map(|(operand, register)| SelectedOperand {
                    operand: operand.operand,
                    virtual_register: *register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
                .collect(),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: Default::default(),
        }
    }

    fn u64_scalar() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    }

    /// The destination leaves the copy's block on an edge transport — a
    /// source this family's contract refuses. A producer that admitted it
    /// anyway would publish a plan whose copy and roster row are gone while
    /// the edge still transports the vanished register; the validator's own
    /// audit must refuse the legality with `UnsupportedUse`, not merely diff
    /// the proposal. Feeding that forged proposal is the observable proof
    /// that validation no longer relies on the producer's admission routine.
    #[test]
    fn validator_refuses_a_proposal_its_own_audit_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let copy_row = environment.constraint(keys.copy_i64).unwrap();
        let load_row = environment.constraint(keys.load8.unwrap()).unwrap();
        let jump_row = environment.constraint(keys.jump).unwrap();
        let return_row = environment.constraint(keys.return_unit).unwrap();
        let class = copy_row.operands[0].class;
        let machine = MachineId::new(1).unwrap();
        let edge = SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            psi_edge: EdgeId::new(2).unwrap(),
            block: SelectedBlockId(1),
            source_target: BlockId::new(2).unwrap(),
            bindings: vec![SelectedValueBinding {
                semantic: ValueBinding {
                    parameter: ValueId::new(6).unwrap(),
                    argument: ValueId::new(1).unwrap(),
                    scalar_type: u64_scalar(),
                },
                transport: SelectedValueTransport::Registers {
                    argument: COPIED,
                    parameter: SPARE,
                },
            }],
            structural_bindings: Vec::new(),
            structural_case: None,
            fuel: Vec::new(),
        };
        let function = SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: vec![
                VirtualRegister {
                    id: POINTER,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value: ValueId::new(1).unwrap(),
                        parameter_index: 0,
                    },
                    definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: SOURCE,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: LOAD,
                        source_value: ValueId::new(2).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: COPIED,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: COPY,
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: SPARE,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::BlockParameter {
                        source_value: ValueId::new(6).unwrap(),
                        block: SelectedBlockId(1),
                        parameter_index: 0,
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
            ],
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: vec![
                        instruction(
                            LOAD,
                            SelectedInstructionKind::Load8 { byte_offset: 0 },
                            load_row,
                            &[POINTER, SOURCE],
                        ),
                        instruction(
                            COPY,
                            SelectedInstructionKind::CopyI64,
                            copy_row,
                            &[SOURCE, COPIED],
                        ),
                    ],
                    terminator: SelectedTerminator::Jump {
                        instruction: instruction(
                            BRANCH,
                            SelectedInstructionKind::Jump,
                            jump_row,
                            &[],
                        ),
                        successor: edge,
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(1),
                    origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(
                            TERMINAL,
                            SelectedInstructionKind::ReturnUnit,
                            return_row,
                            &[],
                        ),
                        psi_return_edge: EdgeId::new(3).unwrap(),
                    },
                },
            ],
        };
        let plan = SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target,
            entry: machine,
            functions: vec![function].into(),
        };
        let identity = selected_instruction_plan_identity(&plan);
        let source = ValidatedCopyRemoval {
            receipt: CopyRemovalReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        };
        // The proposal a defective producer would emit: the copy and the
        // destination's roster row removed while the edge still transports
        // the vanished register.
        let mut proposed = source.transformed().clone();
        let proposed_function = &mut proposed.functions[0];
        proposed_function.blocks[0].instructions.remove(1);
        proposed_function.virtual_registers.remove(2);
        let budget = OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap();
        assert_eq!(
            validate_copy_removal(&source, 0, COPY, &environment, budget, proposed).unwrap_err(),
            CopyRemovalError::UnsupportedUse
        );
    }
}
