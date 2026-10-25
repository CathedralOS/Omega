//! Validation receipt projection from an already replayed live-range plan.

use super::FunctionCounts;

use crate::{LiveRangeValidationReceipt, ValidatedLiveRanges};
use selected_instructions::{LiveRangePlan, live_range_identity_from_digests};

/// Seal a live-range plan with its receipt. A function marked in `carried`
/// keeps its predecessor's identity digest from `previous_digests`.
pub(crate) fn admit(
    plan: LiveRangePlan,
    carried: &[bool],
    previous_digests: &[[u8; 32]],
    previous_counts: &[FunctionCounts],
) -> ValidatedLiveRanges {
    let function_digests =
        crate::analyses::carried_digests(&plan.functions, carried, previous_digests, |function| {
            selected_instructions::live_range_function_digest(function)
        });
    // Reuse report metadata under precisely the same function carry decision
    // as its canonical digest. Missing metadata is computed from current rows.
    let function_counts: std::sync::Arc<[FunctionCounts]> = plan
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| match previous_counts.get(index) {
            Some(counts) if carried.get(index) == Some(&true) => *counts,
            _ => FunctionCounts::new(function),
        })
        .collect();
    let receipt = build_receipt(&plan, &function_digests, &function_counts);
    ValidatedLiveRanges {
        plan: plan.into(),
        receipt,
        function_digests,
        function_counts,
    }
}

fn build_receipt(
    plan: &LiveRangePlan,
    function_digests: &[[u8; 32]],
    function_counts: &[FunctionCounts],
) -> LiveRangeValidationReceipt {
    LiveRangeValidationReceipt {
        identity: live_range_identity_from_digests(plan, function_digests),
        selected: plan.selected,
        liveness: plan.liveness,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        virtual_register_count: function_counts
            .iter()
            .map(|counts| counts.virtual_register_count)
            .sum(),
        virtual_fragment_count: function_counts
            .iter()
            .map(|counts| counts.virtual_fragment_count)
            .sum(),
    }
}
