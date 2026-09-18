//! Physical spill-slot reuse through executable runtime-spill recovery. The
//! fixture's acyclic caller holds two sequential waves of simultaneous live
//! values past a five-view allowlist, so recovery commits one `RuntimeSpill`
//! step per wave. The second wave's victim writes and reloads strictly after
//! the first victim's spill window closed, so the rewrite's last-writer slot
//! check admits the first victim's declared `Spill` slot instead of appending
//! another: the retained program declares fewer slots than committed steps —
//! the reuse is never double-counted — and both retained replay and the
//! independently derived machine plan bind the same shared geometry.

use crate::tests::{
    AllocationEvidence, AllocationReplayError, NativeTarget,
    OptimizedPostAllocationMachinePipelineError, PostAllocationSelectedTransformation,
    shared_spill_slot_caller, stage_optimized_post_allocation_machine_plan,
    stage_shared_entry_fixed_view_register_allocation, staged_shared_spill_slot_legality,
};
use selected_instructions::LocalStorageSlotId;
use selected_instructions_to_register_homes::{AllocationSource, ValidatedSelectedAnalysis};

fn runtime_spill_steps(
    retained: &selected_instructions_to_register_homes::RetainedAllocation,
) -> usize {
    retained
        .current()
        .post_allocation_manifest()
        .record()
        .selected_transformations
        .iter()
        .filter(|transformation| {
            matches!(
                transformation,
                PostAllocationSelectedTransformation::RuntimeSpill(_)
            )
        })
        .count()
}

fn declared_spill_slots(
    retained: &selected_instructions_to_register_homes::RetainedAllocation,
) -> usize {
    retained
        .program()
        .selected
        .functions
        .iter()
        .find(|function| function.machine == shared_spill_slot_caller())
        .expect("the caller must survive into the retained program")
        .local_storage_slots
        .iter()
        .filter(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. }))
        .count()
}

/// The wave-two spill's store and reloads all follow the wave-one victim's
/// closed window in block order, so admission reuses the declared slot: the
/// retained program holds fewer `Spill` slots than the ledger's committed
/// `RuntimeSpill` steps, and every downstream leg re-derives exactly that
/// shared geometry.
#[test]
fn disjoint_spill_windows_share_one_declared_slot_through_recovery() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let legality = staged_shared_spill_slot_legality(target);
        let retained = stage_shared_entry_fixed_view_register_allocation(legality)
            .unwrap_or_else(|error| panic!("{target:?}: allocation must complete: {error}"));
        let current = retained.current();
        assert!(
            matches!(current.evidence(), AllocationEvidence::RuntimeSpill(_)),
            "{target:?}: residual pressure must publish runtime-spill evidence, got {:?}",
            current.evidence()
        );
        let steps = runtime_spill_steps(&retained);
        assert!(
            steps >= 2,
            "{target:?}: the two waves must strand at least one victim each, got {steps} spill steps"
        );
        let slots = declared_spill_slots(&retained);
        assert!(
            slots < steps,
            "{target:?}: {slots} declared Spill slots for {steps} committed steps — a reused slot must not be double-counted"
        );
        // Independent replay re-derives every step — including the shared
        // slot choice — and rejoins the same realized program and homes.
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(current.selected_plan(), replayed.selected_plan());
        assert_eq!(current.homes(), replayed.homes());
        assert_eq!(current.evidence(), replayed.evidence());
        assert_eq!(
            current.post_allocation_manifest(),
            replayed.post_allocation_manifest()
        );
        // Frame demand composes from the realized program's declared storage:
        // the shared slot enters once, and the machine plan binds the same
        // selected identity the retained replay proved.
        let machine = stage_optimized_post_allocation_machine_plan(&retained).unwrap();
        assert_eq!(
            machine.machine().plan().selected,
            current.selected().selected_identity()
        );
    }
}

/// The shared slot is the frame realization downstream demand composes from:
/// growing its declared extent must invalidate retained replay and reject the
/// machine plan before any stale demand can derive, exactly as a private
/// slot's mutation does.
#[test]
fn changed_shared_spill_slot_geometry_invalidates_retained_demand() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut retained = stage_shared_entry_fixed_view_register_allocation(
            staged_shared_spill_slot_legality(target),
        )
        .unwrap_or_else(|error| panic!("{target:?}: allocation must complete: {error}"));
        retained.replay_allocation().unwrap();
        let original = retained.program().clone();
        let mut grown = original.clone();
        let slot = std::sync::Arc::make_mut(&mut grown.selected)
            .functions
            .iter_mut()
            .flat_map(|function| function.local_storage_slots.iter_mut())
            .find(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. }))
            .expect("the shared spill must declare its slot");
        slot.byte_size = 16;
        retained.substitute_current_program_for_test(grown);
        assert!(
            matches!(
                retained.replay_allocation(),
                Err(AllocationReplayError::CurrentProgramMismatch)
            ),
            "{target:?}: a changed shared spill-slot extent must fail retained replay"
        );
        assert!(
            matches!(
                stage_optimized_post_allocation_machine_plan(&retained),
                Err(OptimizedPostAllocationMachinePipelineError::Allocation(
                    AllocationReplayError::CurrentProgramMismatch
                ))
            ),
            "{target:?}: stale demand must reject before post-allocation derivation"
        );
        retained.substitute_current_program_for_test(original);
        retained.replay_allocation().unwrap();
    }
}
