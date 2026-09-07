use crate::tests::*;

pub(super) fn analyze_and_allocate_structural_call(
    selected: StagedOptimizedSelectedInstructions,
) -> StagedOptimizedRegisterHomes {
    let selected_call = selected.selected().plan().functions[0].calls[0].instruction;
    let effects =
        analyze_machine_effects(selected.selected(), selected.register_environment()).unwrap();
    assert_eq!(effects.plan().functions.len(), 2);
    let effect_call = effects.plan().functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|row| row.instruction == selected_call)
        .unwrap();
    assert!(matches!(
        effect_call.kind,
        SelectedInstructionKind::CallUnit { .. }
    ));
    assert!(!effect_call.unit_clobbers.is_empty());
    validate_machine_effects(
        selected.selected(),
        selected.register_environment(),
        &effects,
    )
    .unwrap();
    let mut corrupted = effects.plan().clone();
    corrupted.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.instruction == selected_call)
        .unwrap()
        .unit_clobbers
        .clear();
    let environment = selected.register_environment();
    let catalog = isa_x86_64::validate_x86_64_machine_effect_catalog(
        NativeTarget::uefi_x64(),
        environment.constraints(),
        isa_x86_64::x86_64_machine_effect_catalog(
            NativeTarget::uefi_x64(),
            environment.constraints(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        selected_instructions_to_register_homes::validate_pre_allocation_machine_effects(
            selected.selected(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            &catalog,
            corrupted,
        )
        .is_err()
    );

    let liveness = stage_optimized_liveness(selected).unwrap();
    assert_eq!(liveness.custody().function_count(), 2);
    assert_eq!(liveness.liveness().plan().functions.len(), 2);
    assert!(
        !liveness.liveness().plan().functions[0]
            .entry_definitions
            .is_empty()
    );
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    assert_eq!(ranges.custody().function_count(), 2);
    let range_caller = &ranges.ranges().plan().functions[0];
    assert!(!range_caller.virtual_registers.is_empty());
    assert!(
        range_caller
            .architectural_units
            .iter()
            .any(|unit| !unit.actions.is_empty())
    );
    let mut corrupted = ranges.ranges().plan().clone();
    corrupted.functions[0]
        .architectural_units
        .iter_mut()
        .find(|unit| !unit.actions.is_empty())
        .unwrap()
        .actions
        .clear();
    assert!(
        validate_live_ranges(
            ranges.liveness_stage().selected_stage().selected(),
            ranges.liveness_stage().liveness(),
            corrupted,
        )
        .is_err()
    );

    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    assert_eq!(legality.custody().function_count(), 2);
    assert!(
        !legality.legality().plan().functions[0]
            .virtual_registers
            .is_empty()
    );
    let mut corrupted = legality.legality().plan().clone();
    corrupted.functions.swap(0, 1);
    let range_stage = legality.live_range_stage();
    let environment = range_stage
        .liveness_stage()
        .selected_stage()
        .register_environment();
    assert!(
        validate_allocation_legality(
            range_stage.ranges(),
            legality.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            corrupted,
        )
        .is_err()
    );

    let homes = stage_optimized_register_homes(legality).unwrap();
    assert_eq!(homes.custody().function_count(), 2);
    assert!(!homes.homes().plan().functions[0].assignments.is_empty());
    assert_eq!(
        homes
            .post_allocation_manifest()
            .record()
            .statistics
            .functions,
        2
    );
    let mut corrupted = homes.homes().plan().clone();
    corrupted.functions.swap(0, 1);
    let legality_stage = homes.legality_stage();
    let range_stage = legality_stage.live_range_stage();
    let environment = range_stage
        .liveness_stage()
        .selected_stage()
        .register_environment();
    assert!(
        validate_register_homes(
            legality_stage.legality(),
            range_stage.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            corrupted,
        )
        .is_err()
    );

    homes
}
