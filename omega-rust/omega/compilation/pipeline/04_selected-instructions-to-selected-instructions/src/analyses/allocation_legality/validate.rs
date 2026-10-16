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
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    // Compact spans can describe more logical points than could be allocated
    // as dense rows. Receipt counts must not wrap on that larger domain.
    let mut point_count = 0usize;
    let mut candidate_count = 0usize;
    for (function_index, function) in plan.functions.iter().enumerate() {
        let overflow = || AllocationLegalityError::PointOverflow {
            function: function_index,
        };
        for span in function
            .virtual_registers
            .iter()
            .flat_map(|register| &register.spans)
        {
            let width = usize::try_from(span.end.0 - span.start.0).map_err(|_| overflow())?;
            point_count = point_count.checked_add(width).ok_or_else(overflow)?;
            let candidates = width
                .checked_mul(span.candidates.len())
                .ok_or_else(overflow)?;
            candidate_count = candidate_count
                .checked_add(candidates)
                .ok_or_else(overflow)?;
        }
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
        virtual_register_count: plan
            .functions
            .iter()
            .map(|function| function.virtual_registers.len())
            .sum(),
        point_count,
        candidate_count,
        early_clobber_point_count: plan
            .virtual_registers()
            .map(|register| register.early_clobber_points.len())
            .sum(),
        early_clobber_candidate_count: plan
            .virtual_registers()
            .flat_map(|register| &register.early_clobber_points)
            .map(|point| point.candidates.len())
            .sum(),
        entry_transition_count: plan
            .virtual_registers()
            .map(|register| register.entry_transitions.len())
            .sum(),
    };
    Ok(ValidatedAllocationLegality {
        plan: plan.into(),
        receipt,
        function_digests,
    })
}
