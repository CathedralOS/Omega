//! Construct the fixed layout after checking its shared input contract.
use crate::selection::shared::*;

pub(super) fn reconstruct(
    function: usize,
    source: &SourceStructuralUnitFunction,
) -> Result<SelectedMicrosoftX64OwnedIndirectPairLayout, SelectedInstructionError> {
    let parameters = source
        .parameters
        .iter()
        .map(|parameter| crate::structural_unit_input::Parameter {
            semantic: &parameter.semantic,
            target: &parameter.target,
        })
        .collect::<Vec<_>>();
    if !crate::structural_unit_input::accepts(
        &source.call_plan,
        &parameters,
        &source.structural_types,
    ) {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    Ok(SelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count: 32,
        outgoing_frame_byte_count: 72,
        pre_call_stack_alignment: 16,
        bindings: std::array::from_fn(|index| SelectedStructuralUnitIndirectBinding {
            parameter_index: index,
            pointer: [MachineRegister::X86Rcx, MachineRegister::X86Rdx][index],
            copy_stack_byte_offset: [32, 48][index],
            byte_count: 16,
            alignment: 8,
        }),
    })
}
