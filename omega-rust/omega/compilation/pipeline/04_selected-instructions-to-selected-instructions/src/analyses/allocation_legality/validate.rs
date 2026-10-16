mod counts;
pub(crate) use counts::FunctionCounts;

use crate::{
    AllocationLegalityError, AllocationLegalityValidationReceipt, ValidatedAllocationLegality,
};
use register_homes::{
    AllocationLegalityPlan, allocation_legality_function_digest,
    allocation_legality_identity_from_digests,
};
/// Seal an allocation-legality plan with its receipt. A function marked in
/// `carried` keeps its predecessor's identity digest from `previous_digests`.
pub(crate) fn admit(
    plan: AllocationLegalityPlan,
    carried: &[bool],
    previous_digests: &[[u8; 32]],
    previous_counts: &[FunctionCounts],
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    // Fold functions in source order, including carried report metadata. A
    // later function's local overflow must never overtake an earlier total's
    // overflow. Counts and digests reuse the same admitted-function carry mask.
    let mut point_count = 0usize;
    let mut candidate_count = 0usize;
    let mut function_counts = Vec::with_capacity(plan.functions.len());
    for (function_index, function) in plan.functions.iter().enumerate() {
        let overflow = || AllocationLegalityError::PointOverflow {
            function: function_index,
        };
        let counts = match previous_counts.get(function_index) {
            Some(counts) if carried.get(function_index) == Some(&true) => *counts,
            _ => FunctionCounts::new(function, function_index)?,
        };
        point_count = point_count
            .checked_add(counts.point_count)
            .ok_or_else(overflow)?;
        candidate_count = candidate_count
            .checked_add(counts.candidate_count)
            .ok_or_else(overflow)?;
        function_counts.push(counts);
    }
    let function_digests =
        crate::analyses::carried_digests(&plan.functions, carried, previous_digests, |function| {
            allocation_legality_function_digest(function)
        });
    let identity = allocation_legality_identity_from_digests(&plan, &function_digests);
    let receipt = AllocationLegalityValidationReceipt {
        identity,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        function_count: plan.functions.len(),
        point_count,
        candidate_count,
        virtual_register_count: function_counts
            .iter()
            .map(|counts| counts.virtual_register_count)
            .sum(),
        early_clobber_point_count: function_counts
            .iter()
            .map(|counts| counts.early_clobber_point_count)
            .sum(),
        early_clobber_candidate_count: function_counts
            .iter()
            .map(|counts| counts.early_clobber_candidate_count)
            .sum(),
        entry_transition_count: function_counts
            .iter()
            .map(|counts| counts.entry_transition_count)
            .sum(),
    };
    Ok(ValidatedAllocationLegality {
        plan: plan.into(),
        receipt,
        function_digests,
        function_counts: function_counts.into(),
    })
}
