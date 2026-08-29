//! Independent reconstruction of the supported structural-Unit ABI layout.

use crate::selection::shared::*;

use super::shape::is_extent_structural_type;

pub(in crate::selection::validation) fn reconstruct_structural_unit_layout(
    function: usize,
    source: &SourceStructuralUnitFunction,
) -> Result<SelectedMicrosoftX64OwnedIndirectPairLayout, SelectedInstructionError> {
    if source.call_plan.policy != CallingPolicy::MicrosoftX64
        || source.call_plan.result.is_some()
        || !source.call_plan.callback_materializations.is_empty()
        || source.call_plan.stack_alignment != 16
        || source.call_plan.shadow_bytes != 32
        || source.call_plan.entry_control != EntryControl::CallReturn
        || source.parameters.len() != 2
        || source.call_plan.parameters.len() != 2
    {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }

    let pointers = [MachineRegister::X86Rcx, MachineRegister::X86Rdx];
    let offsets = [32, 48];
    let mut bindings = [SelectedStructuralUnitIndirectBinding {
        parameter_index: 0,
        pointer: pointers[0],
        copy_stack_byte_offset: offsets[0],
        byte_count: 16,
        alignment: 8,
    }; 2];
    for (index, parameter) in source.parameters.iter().enumerate() {
        if parameter.semantic.position != index as u32
            || parameter.semantic.is_self
            || parameter.semantic.access != StructuralAccess::Owned
            || parameter.target.place != parameter.semantic.place
            || parameter.target.structural_type != parameter.semantic.structural_type
            || parameter.target.multiplicity != parameter.semantic.multiplicity
            || parameter.target.access != StructuralAccess::Owned
            || parameter.target.shape.class != ValueClass::Integer
            || parameter.target.shape.byte_size != 16
            || parameter.target.shape.alignment != 8
            || parameter.target.placement != source.call_plan.parameters[index]
            || parameter.target.placement.locations.len() != 1
        {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        }
        let ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(pointer),
            copy_stack_byte_offset: Some(copy_stack_byte_offset),
            byte_size,
            alignment,
        } = parameter.target.placement.locations[0]
        else {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        };
        if pointer != pointers[index]
            || copy_stack_byte_offset != offsets[index]
            || byte_size != 16
            || alignment != 8
        {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        }
        bindings[index] = SelectedStructuralUnitIndirectBinding {
            parameter_index: index,
            pointer,
            copy_stack_byte_offset,
            byte_count: byte_size,
            alignment,
        };
    }

    if source.parameters[0].semantic.structural_type
        != source.parameters[1].semantic.structural_type
        || source.parameters[0].semantic.multiplicity != source.parameters[1].semantic.multiplicity
        || source.parameters[0].semantic.qualifications
            != source.parameters[1].semantic.qualifications
        || source.parameters[0].semantic.place == source.parameters[1].semantic.place
        || !is_extent_structural_type(source)
    {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }

    Ok(SelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count: 32,
        outgoing_frame_byte_count: 72,
        pre_call_stack_alignment: 16,
        bindings,
    })
}
