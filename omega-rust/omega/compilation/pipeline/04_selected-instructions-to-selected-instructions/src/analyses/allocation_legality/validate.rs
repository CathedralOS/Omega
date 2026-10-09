use crate::{AllocationLegalityValidationReceipt, ValidatedAllocationLegality};
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
) -> ValidatedAllocationLegality {
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
        point_count: plan
            .virtual_registers()
            .map(|register| register.points.len())
            .sum(),
        candidate_count: plan
            .virtual_registers()
            .flat_map(|register| &register.points)
            .map(|point| point.candidates.len())
            .sum(),
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
    ValidatedAllocationLegality {
        plan: plan.into(),
        receipt,
        function_digests,
    }
}
