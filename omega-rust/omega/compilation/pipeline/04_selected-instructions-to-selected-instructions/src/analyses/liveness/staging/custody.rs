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
    }
}
