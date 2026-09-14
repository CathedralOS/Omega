use crate::tests::*;

pub(super) fn caller_machine() -> MachineId {
    MachineId::new(STRUCTURAL_CALL_PRESERVING_CALLER).unwrap()
}

pub(super) fn staged_legality(target: NativeTarget) -> StagedOptimizedAllocationLegality {
    stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_structural_call_preserving(target)).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

pub(super) fn staged_homes(target: NativeTarget) -> StagedOptimizedRegisterHomes {
    stage_optimized_register_homes(staged_legality(target)).unwrap()
}
