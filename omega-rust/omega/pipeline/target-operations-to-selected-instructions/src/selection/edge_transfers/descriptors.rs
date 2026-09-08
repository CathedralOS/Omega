//! Whole-descriptor snapshots on the selected edge, before destination replacement.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedStructuralBinding, SelectedStructuralTransport,
};

pub(super) fn snapshot(
    function_index: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: semantic_vocabulary::EdgeId,
    bindings: &[SelectedStructuralBinding],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<Vec<[VirtualRegisterId; 2]>, SelectedInstructionError> {
    let mut snapshots = Vec::new();
    for binding in bindings {
        let SelectedStructuralTransport::Descriptor { argument, .. } = binding.transport else {
            continue;
        };
        let input = registers
            .get(argument.0 as usize)
            .cloned()
            .ok_or_else(|| invalid(function_index))?;
        let mut words = Vec::new();
        for byte_offset in [0, 8] {
            let id = SelectedInstructionId(
                (*next_instruction)
                    .try_into()
                    .map_err(|_| invalid(function_index))?,
            );
            *next_instruction += 1;
            let output = VirtualRegisterId(
                registers
                    .len()
                    .try_into()
                    .map_err(|_| invalid(function_index))?,
            );
            registers.push(VirtualRegister {
                id: output,
                scalar_type: input.scalar_type,
                class: input.class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: id,
                    place: binding.semantic.argument.place,
                    byte_offset,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            instructions.push(super::super::constraints::instruction(
                id,
                SelectedInstructionKind::Load64 { byte_offset },
                constraints
                    .keys
                    .load64
                    .ok_or_else(|| invalid(function_index))?,
                &[argument, output],
                provenance(edge),
                catalog,
            )?);
            memory.push(SelectedMemoryAccess {
                instruction: id,
                origin: SelectedMemoryAccessOrigin::Edge(edge),
                place: binding.semantic.argument.place,
                byte_offset,
                byte_count: 8,
                role: SelectedMemoryAccessRole::ReadPlace,
            });
            words.push(output);
        }
        snapshots.push([words[0], words[1]]);
    }
    Ok(snapshots)
}

pub(super) fn store(
    function_index: usize,
    memory: &mut Vec<SelectedMemoryAccess>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: semantic_vocabulary::EdgeId,
    bindings: &[SelectedStructuralBinding],
    snapshots: &[[VirtualRegisterId; 2]],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for (binding, words) in bindings
        .iter()
        .filter(|binding| {
            matches!(
                binding.transport,
                SelectedStructuralTransport::Descriptor { .. }
            )
        })
        .zip(snapshots)
    {
        let SelectedStructuralTransport::Descriptor { destination, .. } = binding.transport else {
            return Err(invalid(function_index));
        };
        for (byte_offset, input) in [0, 8].into_iter().zip(words) {
            let id = SelectedInstructionId(
                (*next_instruction)
                    .try_into()
                    .map_err(|_| invalid(function_index))?,
            );
            *next_instruction += 1;
            instructions.push(super::super::constraints::instruction(
                id,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(destination),
                    byte_offset,
                },
                constraints
                    .keys
                    .store64
                    .ok_or_else(|| invalid(function_index))?,
                &[*input],
                provenance(edge),
                catalog,
            )?);
            memory.push(SelectedMemoryAccess {
                instruction: id,
                origin: SelectedMemoryAccessOrigin::Edge(edge),
                place: binding.semantic.parameter,
                byte_offset,
                byte_count: 8,
                role: SelectedMemoryAccessRole::WriteLocal { slot: destination },
            });
        }
    }
    Ok(())
}

pub(super) fn provenance(edge: semantic_vocabulary::EdgeId) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        edges: vec![edge],
        ..Default::default()
    }
}
