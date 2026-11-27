/*
Shared cleanup for a selected function whose instructions a rewrite removed or
cloned. Instruction selection (parameter slot sharing) and the Stage 06
structural rewrites (call inlining, vector promotion) both leave unused
registers, slots and sparse ids behind, and every later stage requires dense
ids (`SelectedFunction::ids_are_positions`). `prune_dead_metadata` drops the
registers and inlined or structural slots nothing names, then numbers the
registers densely; `renumber_function` numbers blocks and instructions densely
and rewrites every reference to them.
*/

use crate::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId,
    SelectedCaseDispatchSource, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccessRole,
    VirtualRegisterId, VirtualRegisterOrigin, terminator_instruction, terminator_instruction_mut,
    terminator_successors, terminator_successors_mut,
};
use std::collections::{HashMap, HashSet};

pub fn prune_dead_metadata(function: &mut SelectedFunction) {
    let mut live_insts = HashSet::new();
    let mut live_vregs = HashSet::new();
    let mut live_slots = HashSet::new();

    for block in &function.blocks {
        for inst in block_instructions(block) {
            live_insts.insert(inst.id);
            for op in &inst.operands {
                live_vregs.insert(op.virtual_register);
            }
            match &inst.kind {
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s),
                    ..
                }
                | SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(s),
                    ..
                }
                | SelectedInstructionKind::HostedReadByte { slot: s }
                | SelectedInstructionKind::HostedWriteByteI32 { slot: s }
                | SelectedInstructionKind::SaveFloatingControl { slot: s }
                | SelectedInstructionKind::RestoreFloatingControl { slot: s } => {
                    live_slots.insert(*s);
                }
                _ => {}
            }
        }
        for succ in terminator_successors(&block.terminator) {
            successor_registers(succ, |_, register| {
                live_vregs.insert(register);
            });
            for sb in &succ.structural_bindings {
                match sb.transport {
                    crate::SelectedStructuralTransport::Address {
                        base, destination, ..
                    }
                    | crate::SelectedStructuralTransport::ScalarReference {
                        base,
                        destination,
                        ..
                    } => {
                        live_slots.insert(destination);
                        if let crate::SelectedAddressBase::Local(s) = base {
                            live_slots.insert(s);
                        }
                    }
                    crate::SelectedStructuralTransport::WholeValue { destination, .. } => {
                        live_slots.insert(destination);
                    }
                    crate::SelectedStructuralTransport::Unused => {}
                }
            }
            if let Some(case) = &succ.structural_case
                && let SelectedCaseDispatchSource::Local { slot } = case.source
            {
                live_slots.insert(slot);
            }
        }
    }

    for access in &function.memory_accesses {
        if let SelectedMemoryAccessRole::ReadLocal { slot }
        | SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } = access.role
        {
            live_slots.insert(slot);
        }
    }

    function.local_storage_slots.retain(|slot| {
        !matches!(
            slot.id,
            LocalStorageSlotId::Structural { .. } | LocalStorageSlotId::Inlined { .. }
        ) || live_slots.contains(&slot.id)
    });

    // Also retain any register referenced by a surviving SpillAddress origin.
    let mut extra_vregs = Vec::new();
    for reg in &function.virtual_registers {
        if let VirtualRegisterOrigin::SpillAddress {
            instruction,
            register,
        } = reg.origin
            && (live_vregs.contains(&reg.id) || live_insts.contains(&instruction))
        {
            extra_vregs.push(register);
        }
    }
    live_vregs.extend(extra_vregs);

    function.virtual_registers.retain(|reg| match reg.origin {
        VirtualRegisterOrigin::EntryParameter { .. }
        | VirtualRegisterOrigin::StructuralParameter { .. }
        | VirtualRegisterOrigin::BlockParameter { .. } => true,
        VirtualRegisterOrigin::InstructionScratch { instruction, .. }
        | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
        | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
        | VirtualRegisterOrigin::SpillAddress { instruction, .. }
        | VirtualRegisterOrigin::AbiTransport { instruction, .. }
        | VirtualRegisterOrigin::InstructionResult { instruction, .. } => {
            live_vregs.contains(&reg.id) && live_insts.contains(&instruction)
        }
    });

    // Renumber virtual registers densely to 0..len so `ids_are_positions()` holds.
    let mut vreg_map: HashMap<VirtualRegisterId, VirtualRegisterId> =
        HashMap::with_capacity(function.virtual_registers.len());
    for (idx, reg) in function.virtual_registers.iter_mut().enumerate() {
        let new_id = VirtualRegisterId(idx as u32);
        vreg_map.insert(reg.id, new_id);
        reg.id = new_id;
    }
    for reg in &mut function.virtual_registers {
        if let VirtualRegisterOrigin::SpillAddress { register, .. } = &mut reg.origin {
            *register = vreg_map[register];
        }
    }
    for block in &mut function.blocks {
        for inst in &mut block.instructions {
            for op in &mut inst.operands {
                op.virtual_register = vreg_map[&op.virtual_register];
            }
        }
        let term = terminator_instruction_mut(&mut block.terminator);
        for op in &mut term.operands {
            op.virtual_register = vreg_map[&op.virtual_register];
        }
        for succ in terminator_successors_mut(&mut block.terminator) {
            for b in &mut succ.bindings {
                if let crate::SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = &mut b.transport
                {
                    *argument = vreg_map[argument];
                    *parameter = vreg_map[parameter];
                }
            }
            for sb in &mut succ.structural_bindings {
                match &mut sb.transport {
                    crate::SelectedStructuralTransport::Address {
                        base: crate::SelectedAddressBase::Register(r),
                        ..
                    }
                    | crate::SelectedStructuralTransport::ScalarReference {
                        base: crate::SelectedAddressBase::Register(r),
                        ..
                    }
                    | crate::SelectedStructuralTransport::WholeValue { argument: r, .. } => {
                        *r = vreg_map[r];
                    }
                    _ => {}
                }
            }
            if let Some(case) = &mut succ.structural_case {
                if let SelectedCaseDispatchSource::Borrowed { pointer, .. } = &mut case.source {
                    *pointer = vreg_map[pointer];
                }
                for payload in &mut case.payloads {
                    match &mut payload.transport {
                        SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                            *parameter = vreg_map[parameter];
                        }
                        SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            *argument = vreg_map[argument];
                            *parameter = vreg_map[parameter];
                        }
                        SelectedCasePayloadTransport::Unused => {}
                    }
                }
            }
        }
    }
}

/// A block's instructions followed by its terminator instruction.
pub fn block_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

/// Where on a successor edge a register appears. A value binding reads its
/// `Argument` in this block and defines its `Parameter` in the successor.
/// `Structural` covers transport bases and whole-value arguments. `Case` covers
/// the borrowed dispatch pointer, payload arguments and unmaterialized payload
/// parameters; `CaseParameter` is a materialized payload's parameter.
#[derive(Clone, Copy)]
pub enum EdgeRegister {
    Argument,
    Parameter,
    Structural,
    Case,
    CaseParameter,
}

/// Visit every virtual register a successor edge names. Each pass filters
/// the roles it needs instead of walking the edge itself.
pub fn successor_registers(
    succ: &crate::SelectedSuccessor,
    mut visit: impl FnMut(EdgeRegister, VirtualRegisterId),
) {
    for b in &succ.bindings {
        if let crate::SelectedValueTransport::Registers {
            argument,
            parameter,
        } = b.transport
        {
            visit(EdgeRegister::Argument, argument);
            visit(EdgeRegister::Parameter, parameter);
        }
    }
    for sb in &succ.structural_bindings {
        match sb.transport {
            crate::SelectedStructuralTransport::Address {
                base: crate::SelectedAddressBase::Register(r),
                ..
            }
            | crate::SelectedStructuralTransport::ScalarReference {
                base: crate::SelectedAddressBase::Register(r),
                ..
            }
            | crate::SelectedStructuralTransport::WholeValue { argument: r, .. } => {
                visit(EdgeRegister::Structural, r)
            }
            _ => {}
        }
    }
    if let Some(case) = &succ.structural_case {
        if let SelectedCaseDispatchSource::Borrowed { pointer, .. } = case.source {
            visit(EdgeRegister::Case, pointer);
        }
        for payload in &case.payloads {
            match payload.transport {
                SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } => {
                    visit(EdgeRegister::Case, argument);
                    visit(EdgeRegister::CaseParameter, parameter);
                }
                SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                    visit(EdgeRegister::Case, parameter)
                }
                SelectedCasePayloadTransport::Unused => {}
            }
        }
    }
}

pub fn renumber_function(function: &mut SelectedFunction) {
    let mut block_numbers = DenseRenumbering::sized(function.blocks.iter().map(|block| block.id.0));
    for (idx, block) in function.blocks.iter().enumerate() {
        block_numbers.assign(block.id.0, idx as u32);
    }
    let block_map = |old: &SelectedBlockId| SelectedBlockId(block_numbers.get(old.0));

    function.entry_block = block_map(&function.entry_block);

    let mut instruction_numbers = DenseRenumbering::sized(
        function
            .instructions_and_terminators()
            .map(|instruction| instruction.id.0),
    );
    let mut next_inst_id = 0u32;

    for (idx, block) in function.blocks.iter_mut().enumerate() {
        block.id = SelectedBlockId(idx as u32);
        for inst in &mut block.instructions {
            instruction_numbers.assign(inst.id.0, next_inst_id);
            inst.id = SelectedInstructionId(next_inst_id);
            next_inst_id += 1;
        }
        let term_inst = terminator_instruction_mut(&mut block.terminator);
        instruction_numbers.assign(term_inst.id.0, next_inst_id);
        term_inst.id = SelectedInstructionId(next_inst_id);
        next_inst_id += 1;

        for succ in terminator_successors_mut(&mut block.terminator) {
            succ.block = block_map(&succ.block);
        }
    }
    let inst_map =
        |old: &SelectedInstructionId| SelectedInstructionId(instruction_numbers.get(old.0));

    for reg in &mut function.virtual_registers {
        match &mut reg.origin {
            VirtualRegisterOrigin::InstructionScratch { instruction, .. }
            | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
            | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
            | VirtualRegisterOrigin::SpillAddress { instruction, .. }
            | VirtualRegisterOrigin::AbiTransport { instruction, .. }
            | VirtualRegisterOrigin::InstructionResult { instruction, .. } => {
                *instruction = inst_map(instruction);
            }
            VirtualRegisterOrigin::BlockParameter { block, .. } => {
                *block = block_map(block);
            }
            VirtualRegisterOrigin::EntryParameter { .. }
            | VirtualRegisterOrigin::StructuralParameter { .. } => {}
        }
    }

    for call in &mut function.calls {
        call.instruction = inst_map(&call.instruction);
    }
    for foreign in &mut function.normalized_foreign_calls {
        foreign.instruction = inst_map(&foreign.instruction);
    }
    for access in &mut function.memory_accesses {
        access.instruction = inst_map(&access.instruction);
    }
    for settlement in &mut function.boundary_settlements {
        settlement.block = block_map(&settlement.block);
    }

    debug_assert!(function.ids_are_positions());
}

/// New numbers for old block or instruction ids, indexed by the old id. A
/// reference to an id the function no longer defines is a broken function, so
/// `get` panics on it as the map lookup it replaces did.
struct DenseRenumbering(Vec<Option<u32>>);

impl DenseRenumbering {
    fn sized(old_ids: impl Iterator<Item = u32>) -> Self {
        let length = old_ids.max().map_or(0, |largest| largest as usize + 1);
        Self(vec![None; length])
    }

    fn assign(&mut self, old: u32, new: u32) {
        self.0[old as usize] = Some(new);
    }

    fn get(&self, old: u32) -> u32 {
        self.0
            .get(old as usize)
            .copied()
            .flatten()
            .expect("renumbering reaches an id the function defines")
    }
}
