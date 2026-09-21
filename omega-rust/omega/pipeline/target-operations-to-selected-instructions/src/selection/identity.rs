use super::shared::*;
use selected_instructions::selected_instruction_plan_identity;

pub(super) fn receipt(
    plan: &SelectedInstructionPlan,
    legalized: &ValidatedLegalizedOperations,
) -> SelectedInstructionValidationReceipt {
    let function_count = plan.functions.len();
    let block_count = plan
        .functions
        .iter()
        .map(|function| function.blocks.len())
        .sum::<usize>();
    let virtual_register_count = plan
        .functions
        .iter()
        .map(|function| function.virtual_registers.len())
        .sum();
    let instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.instructions.len() + 1)
        .sum::<usize>();
    SelectedInstructionValidationReceipt {
        identity: selected_instruction_plan_identity(plan),
        legalized: legalized.receipt().identity(),
        legalization_validator: legalized.receipt().validator(),
        optimization_unit: legalized.receipt().optimization_unit(),
        fuel_schedule: legalized.receipt().fuel_schedule(),
        function_count,
        block_count,
        virtual_register_count,
        instruction_count,
    }
}
