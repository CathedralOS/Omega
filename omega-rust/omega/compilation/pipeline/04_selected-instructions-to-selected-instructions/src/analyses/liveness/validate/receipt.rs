//! Projects report-only counts and admits the exact validated liveness plan.

use super::FunctionCounts;

use crate::analyses::liveness::{LivenessValidationReceipt, ValidatedLiveness};
use selected_instructions::{
    LivenessPlan, liveness_function_digest, liveness_identity_from_digests,
};

pub(crate) fn admit_validated_liveness(
    selected: &impl crate::ValidatedSelectedAnalysis,
    plan: LivenessPlan,
    carried: &[bool],
    previous_digests: &[[u8; 32]],
    previous_counts: &[FunctionCounts],
) -> ValidatedLiveness {
    let function_digests =
        crate::analyses::carried_digests(&plan.functions, carried, previous_digests, |function| {
            liveness_function_digest(function)
        });
    // The carry mask already binds the exact admitted function. Keep its
    // report counts beside its digest instead of scanning its operands again.
    let function_counts: std::sync::Arc<[FunctionCounts]> = plan
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| match previous_counts.get(index) {
            Some(counts) if carried.get(index) == Some(&true) => *counts,
            _ => FunctionCounts::new(function),
        })
        .collect();
    let receipt = LivenessValidationReceipt {
        identity: liveness_identity_from_digests(&plan, &function_digests),
        selected: plan.selected,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        function_count: plan.functions.len(),
        block_count: function_counts
            .iter()
            .map(|counts| counts.block_count)
            .sum(),
        virtual_register_count: selected
            .selected_plan()
            .functions
            .iter()
            .map(|function| function.virtual_registers.len())
            .sum(),
        instruction_count: function_counts
            .iter()
            .map(|counts| counts.instruction_count)
            .sum(),
        successor_count: function_counts
            .iter()
            .map(|counts| counts.successor_count)
            .sum(),
        tied_pair_count: function_counts
            .iter()
            .map(|counts| counts.tied_pair_count)
            .sum(),
        early_clobber_count: function_counts
            .iter()
            .map(|counts| counts.early_clobber_count)
            .sum(),
    };
    ValidatedLiveness {
        plan: plan.into(),
        receipt,
        function_digests,
        function_counts,
    }
}
