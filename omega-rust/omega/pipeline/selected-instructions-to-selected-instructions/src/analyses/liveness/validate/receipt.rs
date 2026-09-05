//! Projects report-only counts and admits the exact validated liveness plan.

use super::shared::*;

pub(super) fn admit_validated_liveness(
    selected: &impl crate::ValidatedSelectedAnalysis,
    plan: LivenessPlan,
) -> ValidatedLiveness {
    let block_count = plan
        .functions
        .iter()
        .chain(&plan.structural_unit_functions)
        .map(|function| function.blocks.len())
        .sum();
    let instruction_count = plan
        .functions
        .iter()
        .chain(&plan.structural_unit_functions)
        .flat_map(|function| &function.blocks)
        .map(|block| block.instructions.len())
        .sum();
    let successor_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.successors.len())
        .sum();
    let tied_pair_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.operand_positions)
        .filter(|operand| operand.tied_to.is_some())
        .count();
    let early_clobber_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.operand_positions)
        .filter(|operand| operand.early_clobber)
        .count();
    let receipt = LivenessValidationReceipt {
        identity: liveness_identity(&plan),
        selected: plan.selected,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        function_count: plan.functions.len(),
        structural_unit_function_count: plan.structural_unit_functions.len(),
        block_count,
        virtual_register_count: selected
            .selected_plan()
            .functions
            .iter()
            .map(|function| function.virtual_registers.len())
            .sum(),
        instruction_count,
        successor_count,
        tied_pair_count,
        early_clobber_count,
    };
    ValidatedLiveness {
        plan: plan.into(),
        receipt,
    }
}
