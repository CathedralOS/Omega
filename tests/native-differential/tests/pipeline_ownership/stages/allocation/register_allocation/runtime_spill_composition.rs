//! Fixed-view copies and active-resident rematerialization composed with
//! runtime spill. When post-prefix assignment still reports
//! `NoCompatibleHome`, the proven prefix — the complete reanalysis for the
//! fixed-view arms, the validated rematerialization sweep for the
//! active-resident arm — not the original legality becomes spill recovery's
//! source, the manifest keeps the prefix transformation ahead of every spill
//! step, and the recorded prefix binds the declared recovery selection
//! through retained replay.

use crate::tests::{
    AllocationEvidence, AllocationReplayError, NativeTarget, Optimization, OptimizationSelections,
    OptimizedActiveResidentRematerializationError, OptimizedPostAllocationMachinePipelineError,
    PostAllocationSelectedTransformation, RuntimeSpillAllocationError, SpillChoicePolicy,
    StagedOptimizedSelectedInstructions, choose_spill_victims, selected_lowering_budget,
    stage_active_resident_register_allocation,
    stage_leaf_local_fixed_view_register_allocation_composing, stage_optimized_live_ranges,
    stage_optimized_liveness, stage_optimized_post_allocation_machine_plan,
    stage_shared_entry_fixed_view_register_allocation,
    staged_active_resident_exact_add_bridge_chain,
    staged_active_resident_exact_add_bridge_chain_with_selections,
    staged_active_resident_exact_add_chain, staged_active_resident_exact_add_original_victim_chain,
    staged_active_resident_two_view_legality, staged_composition_pressure_computed_killer_legality,
    staged_composition_pressure_module_legality,
};
use selected_instructions::LocalStorageSlotId;
use selected_instructions_to_register_homes::{
    AllocationSource, RegisterAllocationError, ValidatedSelectedAnalysis,
};

fn transformations(
    retained: &selected_instructions_to_register_homes::RetainedAllocation,
) -> &[PostAllocationSelectedTransformation] {
    &retained
        .current()
        .post_allocation_manifest()
        .record()
        .selected_transformations
}

/// The fixture's pressure survives the fixed/precolored segment homes and the
/// copy sequence, so post-copy assignment still reports `NoCompatibleHome`
/// and custody passes to runtime spill: the recorded ledger opens with the
/// copy transformation and continues with the runtime steps that resolved
/// the pressure.
#[test]
fn fixed_view_copies_compose_into_runtime_spill_when_post_copy_pressure_remains() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let retained = stage_leaf_local_fixed_view_register_allocation_composing(
            staged_composition_pressure_module_legality(
                target,
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!("{target:?}: leaf-local composition must complete: {error}")
        });
        let current = retained.current();
        assert!(
            matches!(current.evidence(), AllocationEvidence::RuntimeSpill(_)),
            "{target:?}: residual pressure must publish runtime-spill evidence"
        );
        let ledger = transformations(&retained);
        assert!(
            matches!(
                ledger.first(),
                Some(PostAllocationSelectedTransformation::FixedViewCopy(_))
            ),
            "{target:?}: the fixed-view transformation stays first in the ledger"
        );
        assert!(
            ledger.len() > 1
                && ledger[1..].iter().all(|transformation| matches!(
                    transformation,
                    PostAllocationSelectedTransformation::RuntimeSpill(_)
                        | PostAllocationSelectedTransformation::RuntimeRematerialization(_)
                )),
            "{target:?}: runtime steps follow the fixed-view prefix"
        );
        // Fresh and retained replays rejoin the same facts: the prefix is
        // validated custody, not a downstream representation selector.
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(current.selected_plan(), replayed.selected_plan());
        assert_eq!(current.homes(), replayed.homes());
        assert_eq!(current.evidence(), replayed.evidence());
        assert_eq!(
            current.post_allocation_manifest(),
            replayed.post_allocation_manifest()
        );
    }
}

/// The shared-entry policy's prefix binds the declared recovery selection:
/// without it the composed allocation must fail retained replay, and with it
/// the same composition retains and publishes runtime-spill evidence.
#[test]
fn shared_entry_fixed_view_composition_binds_the_declared_recovery_selection() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let undeclared = stage_shared_entry_fixed_view_register_allocation(
            staged_composition_pressure_module_legality(
                target,
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ),
        );
        assert!(
            matches!(
                undeclared,
                Err(RegisterAllocationError::Replay(
                    AllocationReplayError::SelectionMismatch
                ))
            ),
            "{target:?}: a shared-entry prefix under no declared selection must reject"
        );

        let retained = stage_shared_entry_fixed_view_register_allocation(
            staged_composition_pressure_module_legality(
                target,
                OptimizationSelections::new([
                    Optimization::CopyPropagation,
                    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                ])
                .unwrap(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!("{target:?}: declared shared-entry composition must complete: {error}")
        });
        let current = retained.current();
        assert!(
            matches!(current.evidence(), AllocationEvidence::RuntimeSpill(_)),
            "{target:?}: residual pressure must publish runtime-spill evidence"
        );
        let ledger = transformations(&retained);
        assert!(
            matches!(
                ledger.first(),
                Some(PostAllocationSelectedTransformation::FixedViewCopy(_))
            ),
            "{target:?}: the shared-entry copy transformation stays first in the ledger"
        );
        assert!(ledger.len() > 1);
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(current.selected_plan(), replayed.selected_plan());
        assert_eq!(current.homes(), replayed.homes());
    }
}

/// The leaf-local prefix is default-path recovery, so the same composed
/// allocation under a declared shared-entry selection must reject — the
/// recorded copy policy is custody evidence, not interchangeable labeling.
#[test]
fn leaf_local_composition_rejects_a_declared_shared_entry_selection() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let allocation = stage_leaf_local_fixed_view_register_allocation_composing(
            staged_composition_pressure_module_legality(
                target,
                OptimizationSelections::new([
                    Optimization::CopyPropagation,
                    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                ])
                .unwrap(),
            ),
        );
        assert!(
            matches!(
                allocation,
                Err(RegisterAllocationError::Replay(
                    AllocationReplayError::SelectionMismatch
                ))
            ),
            "{target:?}: a leaf-local prefix must not absorb a declared selection"
        );
    }
}

/// With each killer defined by an add instead of a literal, the stranded
/// register's definition is never a materialization: recovery's
/// rematerialization-first cost decision declines it, and the same victim
/// commits a genuine `RuntimeSpill` step. The retained ledger keeps the
/// fixed-view prefix ahead of that step, the realized program declares the
/// spill's private storage, and the whole retained allocation replays and
/// composes into the machine plan.
#[test]
fn non_immediate_pressure_retains_a_runtime_spill_step() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let retained = stage_leaf_local_fixed_view_register_allocation_composing(
            staged_composition_pressure_computed_killer_legality(
                target,
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!("{target:?}: leaf-local composition must complete: {error}")
        });
        let current = retained.current();
        assert!(
            matches!(current.evidence(), AllocationEvidence::RuntimeSpill(_)),
            "{target:?}: residual pressure must publish runtime-spill evidence"
        );
        let ledger = transformations(&retained);
        assert!(
            matches!(
                ledger.first(),
                Some(PostAllocationSelectedTransformation::FixedViewCopy(_))
            ),
            "{target:?}: the fixed-view transformation stays first in the ledger"
        );
        assert!(
            ledger[1..].iter().any(|transformation| matches!(
                transformation,
                PostAllocationSelectedTransformation::RuntimeSpill(_)
            )),
            "{target:?}: a non-materialized victim must commit a RuntimeSpill step, got {ledger:?}"
        );
        // The spill's private storage is a declared slot in the realized
        // program — the frame realization downstream demand composes from,
        // not a ledger annotation.
        let caller = retained
            .program()
            .selected
            .functions
            .iter()
            .find(|function| !function.local_storage_slots.is_empty())
            .unwrap_or_else(|| panic!("{target:?}: a committed spill must declare its slot"));
        assert!(
            caller
                .local_storage_slots
                .iter()
                .any(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. })),
            "{target:?}: the realized program must declare a Spill slot"
        );
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(current.selected_plan(), replayed.selected_plan());
        assert_eq!(current.homes(), replayed.homes());
        assert_eq!(current.evidence(), replayed.evidence());
        assert_eq!(
            current.post_allocation_manifest(),
            replayed.post_allocation_manifest()
        );
        let machine = stage_optimized_post_allocation_machine_plan(&retained).unwrap();
        assert_eq!(
            machine.machine().plan().selected,
            retained.current().selected().selected_identity()
        );
    }
}

/// The same divergent pressure under the declared shared-entry selection
/// keeps the shared-entry copy transformation first and still commits a real
/// `RuntimeSpill` step for the non-materialized victim — the recorded copy
/// policy binds the selection exactly as it does on the rematerialized
/// shape.
#[test]
fn shared_entry_composition_retains_a_runtime_spill_step() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let undeclared = stage_shared_entry_fixed_view_register_allocation(
            staged_composition_pressure_computed_killer_legality(
                target,
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ),
        );
        assert!(
            matches!(
                undeclared,
                Err(RegisterAllocationError::Replay(
                    AllocationReplayError::SelectionMismatch
                ))
            ),
            "{target:?}: a shared-entry prefix under no declared selection must reject"
        );

        let retained = stage_shared_entry_fixed_view_register_allocation(
            staged_composition_pressure_computed_killer_legality(
                target,
                OptimizationSelections::new([
                    Optimization::CopyPropagation,
                    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                ])
                .unwrap(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!("{target:?}: declared shared-entry composition must complete: {error}")
        });
        let current = retained.current();
        assert!(
            matches!(current.evidence(), AllocationEvidence::RuntimeSpill(_)),
            "{target:?}: residual pressure must publish runtime-spill evidence"
        );
        let ledger = transformations(&retained);
        assert!(
            matches!(
                ledger.first(),
                Some(PostAllocationSelectedTransformation::FixedViewCopy(_))
            ),
            "{target:?}: the shared-entry copy transformation stays first in the ledger"
        );
        assert!(
            ledger[1..].iter().any(|transformation| matches!(
                transformation,
                PostAllocationSelectedTransformation::RuntimeSpill(_)
            )),
            "{target:?}: a non-materialized victim must commit a RuntimeSpill step, got {ledger:?}"
        );
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(current.selected_plan(), replayed.selected_plan());
        assert_eq!(current.homes(), replayed.homes());
    }
}

/// The computed-killer program's declared `Spill` slot is the frame
/// realization itself: mutating its geometry — not just appending a foreign
/// slot — must invalidate retained replay before the machine plan, and
/// therefore any stack demand, derives from stale facts.
#[test]
fn changed_real_spill_slot_geometry_invalidates_retained_demand() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut retained = stage_leaf_local_fixed_view_register_allocation_composing(
            staged_composition_pressure_computed_killer_legality(
                target,
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!("{target:?}: leaf-local composition must complete: {error}")
        });
        retained.replay_allocation().unwrap();
        let original = retained.program().clone();
        let mut grown = original.clone();
        let slot = std::sync::Arc::make_mut(&mut grown.selected)
            .functions
            .iter_mut()
            .flat_map(|function| function.local_storage_slots.iter_mut())
            .find(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. }))
            .expect("the computed-killer spill must declare its slot");
        slot.byte_size = 16;
        retained.substitute_current_program_for_test(grown);
        assert!(
            matches!(
                retained.replay_allocation(),
                Err(AllocationReplayError::CurrentProgramMismatch)
            ),
            "{target:?}: a changed real spill-slot extent must fail retained replay"
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

fn active_resident_ranges(
    staged: StagedOptimizedSelectedInstructions,
) -> selected_instructions_to_register_homes::StagedOptimizedLiveRanges {
    stage_optimized_live_ranges(stage_optimized_liveness(staged).unwrap()).unwrap()
}

/// The declared active-resident route proves its rematerialization sweep as a
/// validated prefix — the transformed program plus rebuilt liveness, ranges,
/// and legality — then attempts assignment over those rebuilt facts. The
/// plain chain resolves inside the sweep and keeps rematerialization
/// evidence; the bridge and original-victim graphs still report
/// `NoCompatibleHome`, so the same proven prefix — not the original
/// legality — becomes runtime spill's source, the recorded sweep stays
/// first in the ledger ahead of every runtime step, and the whole retained
/// allocation replays into the machine plan.
#[test]
fn active_resident_rematerialization_composes_into_runtime_spill_when_residual_pressure_remains() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let retained = stage_active_resident_register_allocation(active_resident_ranges(
            staged_active_resident_exact_add_chain(target),
        ))
        .unwrap_or_else(|error| {
            panic!("{target:?}: the resolved chain must keep its terminal sweep: {error}")
        });
        let current = retained.current();
        assert!(
            matches!(
                current.evidence(),
                AllocationEvidence::ActiveResidentRematerialization(_)
            ),
            "{target:?}: resolved pressure must publish rematerialization evidence"
        );
        assert!(
            matches!(
                transformations(&retained),
                [PostAllocationSelectedTransformation::PressureRematerialization(_)]
            ),
            "{target:?}: the resolved route records exactly its sweep transformation"
        );
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(current.homes(), replayed.homes());
        assert_eq!(current.evidence(), replayed.evidence());

        for (name, staged) in [
            (
                "bridge",
                staged_active_resident_exact_add_bridge_chain(target),
            ),
            (
                "original-victim",
                staged_active_resident_exact_add_original_victim_chain(target),
            ),
        ] {
            let retained = stage_active_resident_register_allocation(active_resident_ranges(
                staged,
            ))
            .unwrap_or_else(|error| {
                panic!(
                    "{target:?} {name}: residual pressure must compose into runtime spill: {error}"
                )
            });
            let current = retained.current();
            assert!(
                matches!(current.evidence(), AllocationEvidence::RuntimeSpill(_)),
                "{target:?} {name}: residual pressure must publish runtime-spill evidence"
            );
            let ledger = transformations(&retained);
            assert!(
                matches!(
                    ledger.first(),
                    Some(PostAllocationSelectedTransformation::PressureRematerialization(_))
                ),
                "{target:?} {name}: the rematerialization sweep stays first in the ledger"
            );
            assert!(
                ledger.len() > 1
                    && ledger[1..].iter().all(|transformation| matches!(
                        transformation,
                        PostAllocationSelectedTransformation::RuntimeSpill(_)
                            | PostAllocationSelectedTransformation::RuntimeRematerialization(_)
                    )),
                "{target:?} {name}: runtime steps follow the rematerialization prefix, got {ledger:?}"
            );
            // Fresh and retained replays rejoin the same facts: the prefix
            // is validated custody, not a downstream representation selector.
            let replayed = retained.replay_allocation().unwrap();
            assert_eq!(current.selected_plan(), replayed.selected_plan());
            assert_eq!(current.homes(), replayed.homes());
            assert_eq!(current.evidence(), replayed.evidence());
            assert_eq!(
                current.post_allocation_manifest(),
                replayed.post_allocation_manifest()
            );
            let machine = stage_optimized_post_allocation_machine_plan(&retained).unwrap();
            assert_eq!(
                machine.machine().plan().selected,
                retained.current().selected().selected_identity()
            );
        }
    }
}

/// The active-resident prefix binds the declared recovery selection: under no
/// declared selection — or under a different declared recovery rule — the
/// composed allocation must fail retained replay with `SelectionMismatch`,
/// while the same composition under the declared selection retains and
/// publishes runtime-spill evidence.
#[test]
fn active_resident_composition_binds_the_declared_recovery_selection() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for (name, selections) in [
            (
                "undeclared",
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ),
            (
                "foreign-recovery",
                OptimizationSelections::new([
                    Optimization::CopyPropagation,
                    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                ])
                .unwrap(),
            ),
        ] {
            let allocation = stage_active_resident_register_allocation(active_resident_ranges(
                staged_active_resident_exact_add_bridge_chain_with_selections(target, selections),
            ));
            assert!(
                matches!(
                    allocation,
                    Err(RegisterAllocationError::Replay(
                        AllocationReplayError::SelectionMismatch
                    ))
                ),
                "{target:?} {name}: an active-resident prefix must not absorb a missing or foreign selection"
            );
        }

        let retained = stage_active_resident_register_allocation(active_resident_ranges(
            staged_active_resident_exact_add_bridge_chain(target),
        ))
        .unwrap_or_else(|error| {
            panic!("{target:?}: declared active-resident composition must complete: {error}")
        });
        assert!(
            matches!(
                retained.current().evidence(),
                AllocationEvidence::RuntimeSpill(_)
            ),
            "{target:?}: residual pressure must publish runtime-spill evidence"
        );
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(retained.current().homes(), replayed.homes());
    }
}

/// A corrupted active-resident prefix inside a composed runtime-spill source
/// must fail the same independent replay the constructor ran — the prefix is
/// replayed custody, not a producer assertion — before any spill step is
/// trusted.
#[test]
fn corrupted_active_resident_prefix_rejects_composed_replay() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut retained = stage_active_resident_register_allocation(active_resident_ranges(
            staged_active_resident_exact_add_bridge_chain(target),
        ))
        .unwrap_or_else(|error| {
            panic!("{target:?}: declared active-resident composition must complete: {error}")
        });
        retained.replay_allocation().unwrap();
        assert!(
            retained.corrupt_runtime_spill_active_resident_prefix_custody_for_test(),
            "{target:?}: the composed source must carry an active-resident prefix"
        );
        assert!(
            matches!(
                retained.fresh_source_replay_for_test(),
                Err(AllocationReplayError::RuntimeSpill(
                    RuntimeSpillAllocationError::UpstreamRematerialization(
                        OptimizedActiveResidentRematerializationError::ReceiptMismatch
                    )
                ))
            ),
            "{target:?}: a corrupted prefix must reject before spill steps are trusted"
        );
    }
}

/// A changed retained allocation — the homes alone — must invalidate replay
/// of the composed route before any downstream demand derives from stale
/// facts, exactly as it does for every other retained allocation.
#[test]
fn changed_allocation_invalidates_active_resident_composed_replay() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut retained = stage_active_resident_register_allocation(active_resident_ranges(
            staged_active_resident_exact_add_bridge_chain(target),
        ))
        .unwrap_or_else(|error| {
            panic!("{target:?}: declared active-resident composition must complete: {error}")
        });
        retained.replay_allocation().unwrap();
        let mut changed_homes = retained.program().clone();
        std::sync::Arc::make_mut(&mut changed_homes.homes)
            .functions
            .clear();
        retained.substitute_current_program_for_test(changed_homes);
        assert!(
            matches!(
                retained.replay_allocation(),
                Err(AllocationReplayError::CurrentProgramMismatch)
            ),
            "{target:?}: a changed allocation must fail retained replay"
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
    }
}

/// The retained runtime-spill program's `local_storage_slots` are the
/// frame realization downstream stack demand composes from. A changed
/// realization — or a changed allocation — must invalidate retained replay
/// before a post-allocation machine plan, and therefore any demand, can
/// derive from the stale facts.
#[test]
fn changed_spill_frame_realization_invalidates_retained_demand() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut retained = stage_leaf_local_fixed_view_register_allocation_composing(
            staged_composition_pressure_module_legality(
                target,
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!("{target:?}: leaf-local composition must complete: {error}")
        });
        assert!(
            matches!(
                retained.current().evidence(),
                AllocationEvidence::RuntimeSpill(_)
            ),
            "{target:?}: residual pressure must publish runtime-spill evidence"
        );
        // Demand derivation accepts the retained program as staged: the frame
        // layout composes the program's declared local storage.
        retained.replay_allocation().unwrap();
        let machine = stage_optimized_post_allocation_machine_plan(&retained).unwrap();
        assert_eq!(
            machine.machine().plan().selected,
            retained.current().selected().selected_identity()
        );

        // Grown realization: the declaration a runtime spill appends —
        // `{Spill, byte_size: 8, alignment: 8}` — must fail retained replay
        // and must reject the machine plan before any demand derives.
        let original = retained.program().clone();
        let mut grown = original.clone();
        let caller = std::sync::Arc::make_mut(&mut grown.selected)
            .functions
            .iter_mut()
            .find(|function| !function.virtual_registers.is_empty())
            .expect("the caller keeps virtual registers");
        let register = caller.virtual_registers[0].id;
        caller
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Spill { register },
                byte_size: 8,
                alignment: 8,
            });
        retained.substitute_current_program_for_test(grown);
        assert!(
            matches!(
                retained.replay_allocation(),
                Err(AllocationReplayError::CurrentProgramMismatch)
            ),
            "{target:?}: a grown frame realization must fail retained replay"
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

        // The admission is exact: restoring the recorded program re-admits,
        // while a changed allocation — the homes alone — rejects identically.
        retained.substitute_current_program_for_test(original.clone());
        retained.replay_allocation().unwrap();
        let mut changed_homes = original;
        std::sync::Arc::make_mut(&mut changed_homes.homes)
            .functions
            .clear();
        retained.substitute_current_program_for_test(changed_homes);
        assert!(
            matches!(
                retained.replay_allocation(),
                Err(AllocationReplayError::CurrentProgramMismatch)
            ),
            "{target:?}: a changed allocation must fail retained replay"
        );
    }
}

/// The runtime-spill sequence retains the sequenced logical-spill boundary's
/// outcome with the recovered allocation, and replay re-derives it from the
/// same source facts. This fixture's pressure shape declines the bounded
/// boundary, so custody records `None` and replay rejoins; custody claiming
/// a plan recovered under foreign facts must fail replay.
#[test]
fn runtime_spill_retains_and_replays_the_sequenced_logical_spill_boundary() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut retained = stage_leaf_local_fixed_view_register_allocation_composing(
            staged_composition_pressure_module_legality(
                target,
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!("{target:?}: leaf-local composition must complete: {error}")
        });
        assert!(
            matches!(
                retained.current().evidence(),
                AllocationEvidence::RuntimeSpill(_)
            ),
            "{target:?}: residual pressure must publish runtime-spill evidence"
        );
        assert!(
            retained.logical_spill_operations().is_none(),
            "{target:?}: the bounded boundary declines this shape and records `None`"
        );
        retained.replay_allocation().unwrap();
        assert!(!retained.corrupt_runtime_spill_logical_operations_for_test());

        let foreign_legality = staged_active_resident_two_view_legality(target);
        let foreign_ranges = foreign_legality.live_range_stage();
        let foreign_selected = foreign_ranges.liveness_stage().selected_stage();
        let environment = foreign_selected.register_environment();
        let foreign_choices = choose_spill_victims(
            foreign_legality.legality(),
            foreign_ranges.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let foreign_operations = selected_instructions_to_register_homes::plan_logical_spill_operations(
            foreign_selected.selected(),
            foreign_ranges.ranges(),
            foreign_legality.legality(),
            &foreign_choices,
            selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
            selected_lowering_budget(),
        )
        .unwrap();
        assert!(retained.substitute_runtime_spill_logical_operations_for_test(foreign_operations));
        assert!(
            matches!(
                retained.fresh_source_replay_for_test(),
                Err(AllocationReplayError::RuntimeSpill(
                    RuntimeSpillAllocationError::LogicalOperationsMismatch
                ))
            ),
            "{target:?}: custody carrying a foreign logical-spill plan must fail replay"
        );
    }
}
