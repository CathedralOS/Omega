use crate::LivenessValidationReceipt;

use selected_instructions::SelectionCustodyReceipt;

use super::model::LivenessCustodyReceipt;

pub(super) fn liveness_custody_receipt(
    upstream: SelectionCustodyReceipt,
    validation: LivenessValidationReceipt,
) -> LivenessCustodyReceipt {
    LivenessCustodyReceipt {
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
        liveness: validation.identity(),
        function_count: validation.function_count(),
        structural_unit_function_count: validation.structural_unit_function_count(),
        block_count: validation.block_count(),
        virtual_register_count: validation.virtual_register_count(),
        instruction_count: validation.instruction_count(),
        successor_count: validation.successor_count(),
    }
}
