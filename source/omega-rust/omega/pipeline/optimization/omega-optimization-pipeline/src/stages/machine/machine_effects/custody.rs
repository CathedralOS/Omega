use omega_machine_optimizer::ValidatedPreAllocationMachineEffects;

use super::model::{
    StagedOptimizedMachineEffectCustodyReceipt, StagedOptimizedMachineEffectSourceCustodyReceipt,
};

pub(super) fn custody_receipt(
    source: StagedOptimizedMachineEffectSourceCustodyReceipt,
    effects: &ValidatedPreAllocationMachineEffects,
) -> StagedOptimizedMachineEffectCustodyReceipt {
    StagedOptimizedMachineEffectCustodyReceipt {
        source,
        effects: effects.receipt().identity(),
        catalog: effects.receipt().machine_effect_catalog(),
        instruction_count: effects.receipt().instruction_count(),
    }
}
