use crate::{AllocationLegalityValidationReceipt, AllocatorAvailabilityIdentity};

use crate::LiveRangeCustodyReceipt;

use super::model::AllocationLegalityCustodyReceipt;

pub(super) fn custody_receipt(
    upstream: LiveRangeCustodyReceipt,
    allocator_availability: AllocatorAvailabilityIdentity,
    legality: AllocationLegalityValidationReceipt,
) -> AllocationLegalityCustodyReceipt {
    AllocationLegalityCustodyReceipt {
        psi: upstream.psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        manifest: upstream.manifest(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        allocator_availability,
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: upstream.ranges(),
        legality: legality.identity(),
        function_count: legality.function_count(),
        structural_unit_function_count: legality.structural_unit_function_count(),
        virtual_register_count: legality.virtual_register_count(),
        point_count: legality.point_count(),
        candidate_count: legality.candidate_count(),
        entry_transition_count: legality.entry_transition_count(),
    }
}
