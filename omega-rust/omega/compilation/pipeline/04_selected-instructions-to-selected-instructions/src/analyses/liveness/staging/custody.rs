use crate::LivenessValidationReceipt;

use target_operations_to_selected_instructions::StagedOptimizedSelectionCustodyReceipt;

use super::StagedOptimizedLivenessCustodyReceipt;

pub(super) fn liveness_custody_receipt(
    upstream: StagedOptimizedSelectionCustodyReceipt,
    validation: LivenessValidationReceipt,
) -> StagedOptimizedLivenessCustodyReceipt {
    StagedOptimizedLivenessCustodyReceipt {
        core: upstream.core(),
        selected: upstream.selected(),
        liveness: validation.identity(),
        function_count: validation.function_count(),
        block_count: validation.block_count(),
        virtual_register_count: validation.virtual_register_count(),
        instruction_count: validation.instruction_count(),
        successor_count: validation.successor_count(),
    }
}
