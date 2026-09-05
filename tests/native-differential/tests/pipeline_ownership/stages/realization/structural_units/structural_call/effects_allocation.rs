use crate::tests::*;

pub(super) fn analyze_and_allocate_structural_call(
    selected: StagedOptimizedSelectedInstructions,
) -> StagedOptimizedRegisterHomes {
    let selected_call = selected.selected().plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("caller owns one atomic structural Unit call");
    let selected_call_uses = selected_call.implicit_uses.clone();
    let effects = analyze_machine_effects(selected.selected(), selected.register_environment())
        .expect("structural call must reach pre-allocation effect custody");
    assert_eq!(effects.plan().structural_unit_functions.len(), 2);
    let effect_call = effects.plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("caller effect roster owns the atomic call");
    assert_eq!(effect_call.callee, selected_call.callee);
    assert_eq!(effect_call.unit_uses, selected_call.implicit_uses);
    assert_eq!(effect_call.effect, selected_call.effect);
    assert_eq!(effect_call.ownership, selected_call.ownership);
    assert_eq!(
        effect_call.declaration.frame,
        selected_instructions::StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count: 72,
            shadow_byte_count: 32,
            pre_call_stack_alignment: 16,
        }
    );
    validate_machine_effects(
        selected.selected(),
        selected.register_environment(),
        &effects,
    )
    .unwrap();

    let mut corrupted = effects.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .unit_uses
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
            environment.allocation_constraint_keys(),
            &catalog,
            corrupted,
        )
        .is_err()
    );

    let liveness = stage_optimized_liveness(selected)
        .expect("zero-VReg structural functions must retain architectural liveness");
    assert_eq!(liveness.custody().function_count(), 0);
    assert_eq!(liveness.custody().structural_unit_function_count(), 2);
    assert!(liveness.liveness().plan().functions.is_empty());
    assert_eq!(
        liveness.liveness().plan().structural_unit_functions.len(),
        2
    );
    let live_caller = &liveness.liveness().plan().structural_unit_functions[0];
    assert!(live_caller.entry_definitions.is_empty());
    assert!(live_caller.operand_positions.is_empty());
    assert_eq!(live_caller.blocks[0].instructions.len(), 2);
    assert_eq!(
        live_caller.blocks[0].instructions[0].unit_uses,
        selected_call_uses
    );

    let ranges = stage_optimized_live_ranges(liveness)
        .expect("structural architectural flow must reach zero-VReg ranges");
    assert_eq!(ranges.custody().function_count(), 0);
    assert_eq!(ranges.custody().structural_unit_function_count(), 2);
    assert!(ranges.ranges().plan().functions.is_empty());
    assert_eq!(ranges.ranges().plan().structural_unit_functions.len(), 2);
    let range_caller = &ranges.ranges().plan().structural_unit_functions[0];
    assert!(range_caller.virtual_registers.is_empty());
    assert!(range_caller.tied_pairs.is_empty());
    assert!(range_caller.early_clobbers.is_empty());
    assert!(range_caller.interference.is_empty());
    assert!(!range_caller.architectural_units.is_empty());
    assert!(
        range_caller
            .architectural_units
            .iter()
            .any(|unit| !unit.actions.is_empty())
    );
    let mut corrupted = ranges.ranges().plan().clone();
    corrupted.structural_unit_functions[0].architectural_units[0]
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

    let legality = stage_optimized_allocation_legality(ranges)
        .expect("zero-VReg structural functions must require no candidate homes");
    assert_eq!(legality.custody().function_count(), 0);
    assert_eq!(legality.custody().structural_unit_function_count(), 2);
    assert!(legality.legality().plan().functions.is_empty());
    assert_eq!(
        legality.legality().plan().structural_unit_functions.len(),
        2
    );
    assert!(
        legality
            .legality()
            .plan()
            .structural_unit_functions
            .iter()
            .all(|function| function.virtual_registers.is_empty())
    );
    let mut corrupted = legality.legality().plan().clone();
    corrupted.structural_unit_functions.swap(0, 1);
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
            environment.allocation_constraint_keys(),
            corrupted,
        )
        .is_err()
    );

    let homes = stage_optimized_register_homes(legality)
        .expect("structural functions must receive exact empty home rosters");
    assert_eq!(homes.custody().function_count(), 0);
    assert_eq!(homes.custody().structural_unit_function_count(), 2);
    assert!(homes.homes().plan().functions.is_empty());
    assert_eq!(homes.homes().plan().structural_unit_functions.len(), 2);
    assert!(
        homes
            .homes()
            .plan()
            .structural_unit_functions
            .iter()
            .all(|function| function.assignments.is_empty())
    );
    assert_eq!(
        homes
            .post_allocation_manifest()
            .record()
            .statistics
            .functions,
        0
    );
    assert_eq!(
        homes
            .post_allocation_manifest()
            .record()
            .statistics
            .structural_unit_functions,
        2
    );
    let mut corrupted = homes.homes().plan().clone();
    corrupted.structural_unit_functions.swap(0, 1);
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
            environment.allocation_constraint_keys(),
            corrupted,
        )
        .is_err()
    );

    homes
}
