use crate::tests::*;

pub(super) fn caller_machine() -> MachineId {
    MachineId::new(SCALAR_CALL_UNIT_CALLER).unwrap()
}

pub(super) fn staged_selected(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
    staged_scalar_call_unit(target)
}

pub(super) fn staged_liveness(target: NativeTarget) -> StagedOptimizedLiveness {
    stage_optimized_liveness(staged_selected(target)).unwrap()
}

pub(super) fn staged_legality(target: NativeTarget) -> StagedOptimizedAllocationLegality {
    stage_optimized_allocation_legality(
        stage_optimized_live_ranges(staged_liveness(target)).unwrap(),
    )
    .unwrap()
}

pub(super) fn staged_homes(target: NativeTarget) -> StagedOptimizedRegisterHomes {
    stage_optimized_register_homes(staged_legality(target)).unwrap()
}
