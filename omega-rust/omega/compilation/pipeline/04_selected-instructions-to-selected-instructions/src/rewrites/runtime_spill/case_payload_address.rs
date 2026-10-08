//! Case-payload load correspondence: original bounds and edge footprint,
//! followed through actual pointer definitions to the declared dispatch root.

use selected_instructions::{SelectedFunction, SelectedInstructionKind};
use semantic_vocabulary::EdgeId;

/// Restate the bridge load's root-relative footprint from its actual pointer
/// dataflow. An encoded zero displacement alone does not identify the field:
/// every intervening address projection must reach the declared case root.
#[allow(clippy::too_many_arguments)]
pub(super) fn case_payload_address_corresponds(
    function: &SelectedFunction,
    block_index: usize,
    load: &selected_instructions::SelectedInstruction,
    source: selected_instructions::SelectedCaseDispatchSource,
    edge: EdgeId,
    field_byte_offset: u32,
    byte_count: u32,
    displacement: u32,
) -> bool {
    use crate::rewrites::operand_probes::last_definition_before;
    use register_model::RegisterOperandAccess;
    use selected_instructions::{
        FrameStorageSlotId, SelectedCaseDispatchSource, SelectedMemoryAccessOrigin,
        SelectedMemoryAccessRole,
    };
    let Some(place) = source.place() else {
        return false;
    };
    let byte_bound = match source {
        SelectedCaseDispatchSource::Local { slot } => {
            let Some(slot) = function
                .local_storage_slots
                .iter()
                .find(|row| row.id == slot)
            else {
                return false;
            };
            slot.byte_size
        }
        SelectedCaseDispatchSource::Borrowed { byte_size, .. } => byte_size,
    };
    if field_byte_offset
        .checked_add(byte_count)
        .is_none_or(|end| end > byte_bound)
        || !field_byte_offset.is_multiple_of(byte_count)
    {
        return false;
    }
    let mut accesses = function
        .memory_accesses
        .iter()
        .filter(|row| row.instruction == load.id);
    let Some(access) = accesses.next() else {
        return false;
    };
    if accesses.next().is_some()
        || access.origin != SelectedMemoryAccessOrigin::Edge(edge)
        || access.role != SelectedMemoryAccessRole::ReadPlace
        || access.place != place
        || access.byte_offset != field_byte_offset
        || access.byte_count != byte_count
    {
        return false;
    }
    let clean_edge_instruction = |instruction: &selected_instructions::SelectedInstruction| {
        instruction.provenance.edges == [edge]
            && instruction.implicit_uses.is_empty()
            && instruction.implicit_defs.is_empty()
            && instruction.clobbers.is_empty()
    };
    let [pointer_operand, result_operand] = load.operands.as_slice() else {
        return false;
    };
    if !clean_edge_instruction(load)
        || pointer_operand.operand != 0
        || pointer_operand.access != RegisterOperandAccess::Use
        || result_operand.operand != 1
        || result_operand.access != RegisterOperandAccess::Def
    {
        return false;
    }
    let instructions = &function.blocks[block_index].instructions;
    let Some(mut position) = instructions.iter().position(|row| row.id == load.id) else {
        return false;
    };
    let mut pointer = pointer_operand.virtual_register;
    let mut offset = displacement;
    loop {
        let definition = last_definition_before(instructions, position, pointer);
        if let SelectedCaseDispatchSource::Borrowed { pointer: root, .. } = source
            && pointer == root
        {
            // The root is retained at edge entry, not redefined by this bridge.
            return definition.is_none() && offset == field_byte_offset;
        }
        let Some(definition) = definition else {
            return false;
        };
        let producer = &instructions[definition];
        if !clean_edge_instruction(producer) {
            return false;
        }
        match producer.kind {
            SelectedInstructionKind::AddressOffset { byte_offset } => {
                let [base, result] = producer.operands.as_slice() else {
                    return false;
                };
                if base.operand != 0
                    || base.access != RegisterOperandAccess::Use
                    || result.operand != 1
                    || result.access != RegisterOperandAccess::Def
                    || result.virtual_register != pointer
                    || base.virtual_register == pointer
                {
                    return false;
                }
                let Some(combined) = offset.checked_add(byte_offset) else {
                    return false;
                };
                offset = combined;
                pointer = base.virtual_register;
                position = definition;
            }
            SelectedInstructionKind::FrameAddress {
                slot,
                byte_offset: 0,
            } => {
                let SelectedCaseDispatchSource::Local { slot: root } = source else {
                    return false;
                };
                let [result] = producer.operands.as_slice() else {
                    return false;
                };
                return slot == FrameStorageSlotId::Local(root)
                    && result.operand == 0
                    && result.access == RegisterOperandAccess::Def
                    && result.virtual_register == pointer
                    && offset == field_byte_offset;
            }
            _ => return false,
        }
    }
}
