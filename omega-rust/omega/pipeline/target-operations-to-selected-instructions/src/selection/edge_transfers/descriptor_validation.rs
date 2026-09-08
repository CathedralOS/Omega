//! Independently check edge-local descriptor reads and destination writes.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedStructuralTransport,
};

pub(super) fn check(
    function: usize,
    prepared: &SelectedFunction,
    bridge: &SelectedBlock,
    continuation: &SelectedSuccessor,
    scalar_count: usize,
    instruction_start: usize,
    register_start: usize,
    original_register_count: usize,
    constraints: &SelectedSelectionConstraints,
) -> Result<Vec<SelectedMemoryAccess>, SelectedInstructionError> {
    let error = || invalid(function);
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let descriptor_words = continuation
        .structural_bindings
        .len()
        .checked_mul(2)
        .ok_or_else(error)?;
    for (position, binding) in continuation.structural_bindings.iter().enumerate() {
        let SelectedStructuralTransport::Descriptor {
            argument,
            destination,
        } = binding.transport
        else {
            return Err(error());
        };
        if argument.0 as usize >= original_register_count
            || destination
                != (selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                    block: continuation.source_target,
                    place: binding.semantic.parameter,
                })
        {
            return Err(error());
        }
        let original = prepared
            .virtual_registers
            .get(argument.0 as usize)
            .ok_or_else(error)?;
        for (word, byte_offset) in [0, 8].into_iter().enumerate() {
            let load_position = scalar_count + position * 2 + word;
            let store_position = scalar_count * 2 + descriptor_words + position * 2 + word;
            let load = bridge.instructions.get(load_position).ok_or_else(error)?;
            let store = bridge.instructions.get(store_position).ok_or_else(error)?;
            let register_position = register_start + load_position;
            let register = prepared
                .virtual_registers
                .get(register_position)
                .ok_or_else(error)?;
            let provenance = SelectedInstructionProvenance {
                edges: vec![continuation.psi_edge],
                ..Default::default()
            };
            if load.id.0 as usize != instruction_start + load_position
                || load.kind != (SelectedInstructionKind::Load64 { byte_offset })
                || Some(load.constraint) != constraints.keys.load64
                || load.provenance != provenance
                || load.operands.len() != 2
                || load.operands[0].virtual_register != argument
                || load.operands[1].virtual_register != register.id
                || register.id.0 as usize != register_position
                || register.scalar_type != original.scalar_type
                || register.class != original.class
                || register.definition_site.is_some()
                || register.entry_fixed_view.is_some()
                || register.origin
                    != (VirtualRegisterOrigin::AbiTransport {
                        instruction: load.id,
                        place: binding.semantic.argument.place,
                        byte_offset,
                    })
                || store.id.0 as usize != instruction_start + store_position
                || store.kind
                    != (SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(destination),
                        byte_offset,
                    })
                || Some(store.constraint) != constraints.keys.store64
                || store.provenance != provenance
                || store.operands.len() != 1
                || store.operands[0].virtual_register != register.id
            {
                return Err(error());
            }
            reads.push(SelectedMemoryAccess {
                instruction: load.id,
                origin: SelectedMemoryAccessOrigin::Edge(continuation.psi_edge),
                place: binding.semantic.argument.place,
                byte_offset,
                byte_count: 8,
                role: SelectedMemoryAccessRole::ReadPlace,
            });
            writes.push(SelectedMemoryAccess {
                instruction: store.id,
                origin: SelectedMemoryAccessOrigin::Edge(continuation.psi_edge),
                place: binding.semantic.parameter,
                byte_offset,
                byte_count: 8,
                role: SelectedMemoryAccessRole::WriteLocal { slot: destination },
            });
        }
    }
    reads.extend(writes);
    Ok(reads)
}
