use omega_regalloc::LiveRangeValidationReceipt;

use omega_selected_instructions_to_liveness::StagedOptimizedLivenessCustodyReceipt;

use super::model::StagedOptimizedLiveRangeCustodyReceipt;

pub(super) fn live_range_custody_receipt(
    upstream: StagedOptimizedLivenessCustodyReceipt,
    ranges: LiveRangeValidationReceipt,
) -> StagedOptimizedLiveRangeCustodyReceipt {
    StagedOptimizedLiveRangeCustodyReceipt {
        psi: upstream.psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        manifest: upstream.manifest(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: ranges.identity(),
        function_count: ranges.function_count(),
        structural_unit_function_count: ranges.structural_unit_function_count(),
        block_count: ranges.block_count(),
        virtual_register_count: ranges.virtual_register_count(),
        virtual_occurrence_count: ranges.virtual_occurrence_count(),
        fixed_constraint_count: ranges.fixed_constraint_count(),
        virtual_fragment_count: ranges.virtual_fragment_count(),
        architectural_unit_count: ranges.architectural_unit_count(),
        architectural_action_count: ranges.architectural_action_count(),
        architectural_fragment_count: ranges.architectural_fragment_count(),
        virtual_edge_connector_count: ranges.virtual_edge_connector_count(),
        architectural_edge_connector_count: ranges.architectural_edge_connector_count(),
        interference_count: ranges.interference_count(),
    }
}
