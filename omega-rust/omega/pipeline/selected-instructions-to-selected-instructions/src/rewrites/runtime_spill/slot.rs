//! Physical reuse of an existing spill slot by a later victim. Reuse keeps the
//! function's declared storage set — and therefore every derived frame-byte
//! demand — unchanged, so the same slot must never be charged to two victims.
//! It is admitted only when the slot's exact access stream and the rewrite's
//! proposed accesses cannot interleave destructively.
//!
//! The check replays the candidate slot as a two-class last-writer state
//! machine over the whole function. `EXISTING` means the last observed writer
//! is one of the slot's already-admitted stores; `NEW` means it is a store the
//! pending rewrite would insert. An existing load stays correct only while no
//! new store can be its last writer; a proposed reload reads the victim's own
//! value only while every reaching writer is a new store. Blocks join by
//! union, so loops and shared successors are covered by the same fixpoint —
//! unreachable code contributes nothing because it carries no real writer.
//!
//! A slot is a candidate only while every access to it follows the idiom this
//! rewrite itself emits: a zero-offset `Store64`, or a zero-offset
//! `FrameAddress` whose result feeds nothing but zero-offset `Load64`s. Any
//! other naming of the slot — a byte-oriented hosted access, a structural
//! transport destination, a case edge's storage, a nonzero offset, or an
//! address used anywhere but a load's address operand — is a use the analysis
//! cannot classify, so the slot stays private to its first victim.

use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstructionKind,
    VirtualRegisterId,
};

use super::admission::{StorageDefinition, StoragePosition};

/// Admitted use positions for the victim, one entry per block in order. The
/// sharing check needs positions, not just block membership: a proposed reload
/// reads the slot immediately before its consumer's instruction, and a
/// block-shared reload reads once at the first unpinned use of each span —
/// an admitted span-closing instruction (a victim redefinition, or a unit
/// writer the crossing policy did not keep open) ends the span, so the next
/// unpinned use reads the slot again.
#[derive(Debug, Default)]
pub(super) struct BlockUsePositions {
    /// Instruction indices holding at least one admitted unpinned use.
    pub unpinned: Vec<usize>,
    /// Instruction indices holding at least one admitted fixed-view use.
    pub pinned: Vec<usize>,
    /// At least one admitted use at the end-of-block position: a terminator
    /// operand or an outgoing-edge transport argument.
    pub end_of_block: bool,
}

const EXISTING: u8 = 1;
const NEW: u8 = 2;

#[derive(Debug, Clone, Copy)]
enum Event {
    /// An already-admitted store writes the slot.
    ExistingStore,
    /// An already-admitted load reads the slot.
    ExistingLoad,
    /// The pending rewrite stores the victim after this instruction.
    NewStore,
    /// The pending rewrite reloads the victim before this instruction — or at
    /// the end-of-block position, which is ordered after every body access.
    NewLoad,
}

/// The first declared spill slot the new victim can share, in declaration
/// order so proposal and independent replay resolve identically. `None` keeps
/// today's behavior: the victim declares its own private slot.
pub(super) fn shared_slot(
    function: &SelectedFunction,
    victim: VirtualRegisterId,
    definitions: &[StorageDefinition],
    uses: &[BlockUsePositions],
    shared_reload: &[bool],
    span_closes: &[std::collections::BTreeSet<usize>],
) -> Option<LocalStorageSlotId> {
    function
        .local_storage_slots
        .iter()
        .filter(|slot| {
            slot.byte_size == 8
                && slot.alignment == 8
                && matches!(slot.id, LocalStorageSlotId::Spill { register } if register != victim)
        })
        .map(|slot| slot.id)
        .find(|slot| {
            shareable(
                function,
                *slot,
                definitions,
                uses,
                shared_reload,
                span_closes,
            )
        })
}

fn shareable(
    function: &SelectedFunction,
    slot: LocalStorageSlotId,
    definitions: &[StorageDefinition],
    uses: &[BlockUsePositions],
    shared_reload: &[bool],
    span_closes: &[std::collections::BTreeSet<usize>],
) -> bool {
    let frame_slot = FrameStorageSlotId::Local(slot);
    // Instruction indices that are existing stores or loads of the candidate
    // slot, plus every register a zero-offset `FrameAddress` for it defines.
    let mut accesses: Vec<std::collections::BTreeMap<usize, Event>> = function
        .blocks
        .iter()
        .map(|_| std::collections::BTreeMap::new())
        .collect();
    let mut slot_addresses = std::collections::BTreeSet::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (index, instruction) in block.instructions.iter().enumerate() {
            match instruction.kind {
                SelectedInstructionKind::Store64 {
                    slot: target,
                    byte_offset,
                } if target == frame_slot => {
                    if byte_offset != 0 {
                        return false;
                    }
                    accesses[block_index].insert(index, Event::ExistingStore);
                }
                SelectedInstructionKind::FrameAddress {
                    slot: target,
                    byte_offset,
                } if target == frame_slot => {
                    if byte_offset != 0 {
                        return false;
                    }
                    slot_addresses.extend(
                        instruction
                            .operands
                            .iter()
                            .filter(|operand| operand.access != RegisterOperandAccess::Use)
                            .map(|operand| operand.virtual_register),
                    );
                }
                SelectedInstructionKind::HostedReadByte { slot: target }
                | SelectedInstructionKind::HostedWriteByteI32 { slot: target }
                    if target == slot =>
                {
                    return false;
                }
                _ => {}
            }
        }
        let terminal = super::control(&block.terminator).0;
        if names_slot(terminal, slot, frame_slot) {
            return false;
        }
        for successor in super::control(&block.terminator).1.into_iter().flatten() {
            if successor
                .structural_bindings
                .iter()
                .any(|binding| match binding.transport {
                    selected_instructions::SelectedStructuralTransport::WholeValue {
                        destination,
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::Descriptor {
                        destination,
                        ..
                    } => destination == slot,
                    selected_instructions::SelectedStructuralTransport::Unused => false,
                })
                || successor
                    .structural_case
                    .as_ref()
                    .is_some_and(|case| case.slot == slot)
            {
                return false;
            }
        }
    }
    // Every use of a candidate-slot address must be the address operand of a
    // zero-offset `Load64`; anything else reaches the slot through a shape the
    // analysis cannot classify. A terminator load executes after every emitted
    // reload pair, so it joins the event stream at the end-of-block position.
    let mut terminal_loads = vec![false; function.blocks.len()];
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (index, instruction) in block
            .instructions
            .iter()
            .enumerate()
            .chain(std::iter::once((
                usize::MAX,
                super::control(&block.terminator).0,
            )))
        {
            for operand in &instruction.operands {
                if !slot_addresses.contains(&operand.virtual_register) {
                    continue;
                }
                let legal_load = matches!(
                    instruction.kind,
                    SelectedInstructionKind::Load64 { byte_offset: 0 }
                ) && operand.access == RegisterOperandAccess::Use;
                let own_definition = matches!(instruction.kind, SelectedInstructionKind::FrameAddress { slot: target, .. }
                        if target == frame_slot)
                    && operand.access != RegisterOperandAccess::Use;
                if !legal_load && !own_definition {
                    return false;
                }
                if legal_load {
                    if index == usize::MAX {
                        terminal_loads[block_index] = true;
                    } else {
                        accesses[block_index].insert(index, Event::ExistingLoad);
                    }
                }
            }
        }
        // An address that crosses an edge could feed loads this scan never
        // sees; sharing is then unverifiable.
        for successor in super::control(&block.terminator).1.into_iter().flatten() {
            let escapes = successor
                .bindings
                .iter()
                .any(|binding| match binding.transport {
                    selected_instructions::SelectedValueTransport::Registers {
                        argument, ..
                    } => slot_addresses.contains(&argument),
                    selected_instructions::SelectedValueTransport::Unused => false,
                })
                || successor
                    .structural_bindings
                    .iter()
                    .any(|binding| match binding.transport {
                        selected_instructions::SelectedStructuralTransport::WholeValue {
                            argument,
                            ..
                        }
                        | selected_instructions::SelectedStructuralTransport::Descriptor {
                            argument,
                            ..
                        } => slot_addresses.contains(&argument),
                        selected_instructions::SelectedStructuralTransport::Unused => false,
                    })
                || successor.structural_case.as_ref().is_some_and(|case| {
                    case.payloads.iter().any(|payload| {
                        matches!(
                            payload.transport,
                            selected_instructions::SelectedCasePayloadTransport::Registers {
                                argument,
                                ..
                            } if slot_addresses.contains(&argument))
                    })
                });
            if escapes {
                return false;
            }
        }
    }
    // Proposed positions: one store after each definition and one reload
    // before each emitted load — the first unpinned use of each shared
    // span when the block shares its reload register, every use otherwise.
    let mut events: Vec<Vec<Event>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            let mut events = Vec::new();
            // A boundary definition's store opens the block: it is the first
            // writer event, ahead of every instruction-positioned access.
            events.extend(
                definitions
                    .iter()
                    .filter(|definition| {
                        definition.block_index == block_index
                            && matches!(definition.position, StoragePosition::BlockStart)
                    })
                    .map(|_| Event::NewStore),
            );
            for (index, instruction) in block.instructions.iter().enumerate() {
                if loads_before(uses, shared_reload, span_closes, block_index, index) {
                    events.push(Event::NewLoad);
                }
                if let Some(access) = accesses[block_index].get(&index) {
                    events.push(*access);
                }
                events.extend(
                    definitions
                        .iter()
                        .filter(|definition| {
                            definition.block_index == block_index
                                && match definition.position {
                                    StoragePosition::AfterInstruction(anchor)
                                    | StoragePosition::AfterUseDef {
                                        instruction: anchor,
                                        ..
                                    } => anchor == instruction.id,
                                    StoragePosition::BlockStart => false,
                                }
                        })
                        .map(|_| Event::NewStore),
                );
            }
            if uses[block_index].end_of_block {
                events.push(Event::NewLoad);
            }
            // Emitted reload pairs sit in the block body; the terminator
            // instruction — and so an existing terminator load — runs last.
            if terminal_loads[block_index] {
                events.push(Event::ExistingLoad);
            }
            events
        })
        .collect();
    // Fixpoint on last-writer sets, then verify every load at its position.
    let mut entries = vec![0u8; function.blocks.len()];
    loop {
        let mut changed = false;
        for (block_index, block) in function.blocks.iter().enumerate() {
            let exit = events[block_index]
                .iter()
                .fold(entries[block_index], |state, event| match event {
                    Event::ExistingStore => EXISTING,
                    Event::NewStore => NEW,
                    Event::ExistingLoad | Event::NewLoad => state,
                });
            for successor in super::control(&block.terminator).1.into_iter().flatten() {
                if let Some(destination) = function
                    .blocks
                    .iter()
                    .position(|candidate| candidate.id == successor.block)
                {
                    let merged = entries[destination] | exit;
                    if merged != entries[destination] {
                        entries[destination] = merged;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    events.iter_mut().enumerate().all(|(block_index, events)| {
        let mut state = entries[block_index];
        events.iter().all(|event| match event {
            Event::ExistingStore => {
                state = EXISTING;
                true
            }
            Event::NewStore => {
                state = NEW;
                true
            }
            Event::ExistingLoad => state & NEW == 0,
            Event::NewLoad => state == NEW,
        })
    })
}

fn names_slot(
    instruction: &selected_instructions::SelectedInstruction,
    slot: LocalStorageSlotId,
    frame_slot: FrameStorageSlotId,
) -> bool {
    match instruction.kind {
        SelectedInstructionKind::Store64 { slot: target, .. }
        | SelectedInstructionKind::FrameAddress { slot: target, .. } => target == frame_slot,
        SelectedInstructionKind::HostedReadByte { slot: target }
        | SelectedInstructionKind::HostedWriteByteI32 { slot: target } => target == slot,
        _ => false,
    }
}

fn loads_before(
    uses: &[BlockUsePositions],
    shared_reload: &[bool],
    span_closes: &[std::collections::BTreeSet<usize>],
    block_index: usize,
    index: usize,
) -> bool {
    let positions = &uses[block_index];
    if positions.pinned.contains(&index) {
        return true;
    }
    if !positions.unpinned.contains(&index) {
        return false;
    }
    if !shared_reload[block_index] {
        return true;
    }
    // A shared reload loads at the first unpinned use of its span. An
    // admitted span-closing instruction — a victim redefinition, or a unit
    // writer the crossing policy did not keep open — ends the span, so the
    // first unpinned use after one opens a fresh pair and reads the slot
    // again.
    let Some(&previous) = positions
        .unpinned
        .iter()
        .rev()
        .find(|&point| *point < index)
    else {
        return true;
    };
    (previous..index).any(|point| span_closes[block_index].contains(&point))
}
