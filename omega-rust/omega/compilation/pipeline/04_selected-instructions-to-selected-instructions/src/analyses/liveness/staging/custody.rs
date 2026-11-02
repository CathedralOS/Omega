use crate::LivenessValidationReceipt;

use super::StagedOptimizedLivenessCustodyReceipt;

pub(super) fn liveness_custody_receipt(
    core: selected_instructions::StagedOptimizedCustodyCore,
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    validation: LivenessValidationReceipt,
) -> StagedOptimizedLivenessCustodyReceipt {
    StagedOptimizedLivenessCustodyReceipt {
        core,
        selected,
        liveness: validation.identity(),
    }
}
