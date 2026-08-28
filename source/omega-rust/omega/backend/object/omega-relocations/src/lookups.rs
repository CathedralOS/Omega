use crate::RelocationPlanningInput;
use omega_calling_conventions::{HostBinding, HostOperationKey};
use omega_target_operations::FunctionInstructionPlan;
use psi_diagnostics::Diagnostic;

#[derive(Debug, Default)]
pub(super) struct SelectedInstructionTextLayouts {
    entries: Vec<Option<SelectedInstructionTextLayout>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectedInstructionTextLayout {
    offset: usize,
    width: usize,
}

impl SelectedInstructionTextLayouts {
    pub(super) fn collect(input: RelocationPlanningInput<'_>) -> Self {
        let mut entries = Vec::with_capacity(input.encoded_machine.code.instructions.len());
        let mut offset = 0usize;

        for (_, instruction) in input.encoded_machine.code.instructions.iter() {
            let width = input
                .encoded_machine
                .code
                .bytes
                .span(instruction.bytes)
                .map(|bytes| bytes.len())
                .unwrap_or(0);
            let index = instruction.selected_instruction_index as usize;
            if entries.len() <= index {
                entries.resize(index.saturating_add(1), None);
            }
            entries[index] = Some(SelectedInstructionTextLayout { offset, width });
            offset = offset.saturating_add(width);
        }

        Self { entries }
    }

    pub(super) fn offset(
        &self,
        function: &FunctionInstructionPlan,
        selected_instruction_index: u32,
    ) -> Result<usize, Diagnostic> {
        self.layout(selected_instruction_index)
            .map(|layout| layout.offset)
            .ok_or_else(|| missing_selected_instruction_bytes(function, selected_instruction_index))
    }

    pub(super) fn width(&self, selected_instruction_index: u32) -> usize {
        self.layout(selected_instruction_index)
            .map(|layout| layout.width)
            .unwrap_or(0)
    }

    fn layout(&self, selected_instruction_index: u32) -> Option<SelectedInstructionTextLayout> {
        self.entries
            .get(selected_instruction_index as usize)
            .copied()
            .flatten()
    }
}

fn missing_selected_instruction_bytes(
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot plan relocation for `{}` selected instruction #{}: missing encoded instruction bytes",
        function.symbol, selected_instruction_index
    ))
}

pub(super) fn find_host_binding<'plan>(
    input: RelocationPlanningInput<'plan>,
    operation_key: HostOperationKey,
) -> Option<&'plan HostBinding> {
    input
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.operation_key == operation_key)
        .map(|(_, binding)| binding)
}
