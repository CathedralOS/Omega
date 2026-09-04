use crate::tests::*;

#[test]
fn disconnected_functions_reach_independent_allocator_and_machine_custody() {
    let expected_machines = [
        MachineId::new(16_001).unwrap(),
        MachineId::new(17_001).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = disconnected_conditional_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target).unwrap();
        let selected = stage_optimized_instruction_selection(target).unwrap();

        assert_eq!(selected.selected().plan().functions.len(), 2);
        assert_eq!(
            selected
                .selected()
                .plan()
                .functions
                .iter()
                .map(|function| function.machine)
                .collect::<Vec<_>>(),
            expected_machines
        );
        for function in &selected.selected().plan().functions {
            assert_eq!(
                function
                    .virtual_registers
                    .iter()
                    .map(|register| register.id.0)
                    .collect::<Vec<_>>(),
                vec![0, 1, 2]
            );
            assert_eq!(
                function
                    .blocks
                    .iter()
                    .map(|block| block.id.0)
                    .collect::<Vec<_>>(),
                vec![0, 1, 2]
            );
        }

        let liveness = stage_optimized_liveness(selected).unwrap();
        assert_eq!(liveness.custody().function_count(), 2);
        assert_eq!(liveness.custody().structural_unit_function_count(), 0);
        assert_eq!(liveness.custody().block_count(), 6);
        assert_eq!(liveness.custody().virtual_register_count(), 6);
        assert_eq!(liveness.custody().instruction_count(), 12);
        assert_eq!(liveness.custody().successor_count(), 4);
        for (function, machine) in liveness
            .liveness()
            .plan()
            .functions
            .iter()
            .zip(expected_machines)
        {
            assert_eq!(function.machine, machine);
            assert_eq!(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .map(|instruction| instruction.position.0)
                    .collect::<Vec<_>>(),
                (0..6).collect::<Vec<_>>()
            );
        }
        let mut corrupted_liveness = liveness.liveness().plan().clone();
        corrupted_liveness.functions[1].machine = expected_machines[0];
        assert_eq!(
            validate_liveness(liveness.selected_stage().selected(), corrupted_liveness,),
            Err(LivenessError::FunctionMismatch { function: 1 })
        );

        let ranges = stage_optimized_live_ranges(liveness).unwrap();
        assert_eq!(ranges.custody().function_count(), 2);
        assert_eq!(ranges.custody().structural_unit_function_count(), 0);
        assert_eq!(ranges.custody().block_count(), 6);
        assert_eq!(ranges.custody().virtual_register_count(), 6);
        assert_eq!(ranges.custody().interference_count(), 0);
        for (function, machine) in ranges
            .ranges()
            .plan()
            .functions
            .iter()
            .zip(expected_machines)
        {
            assert_eq!(function.machine, machine);
            assert_eq!(
                function
                    .block_domains
                    .iter()
                    .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 4), (1, 4, 8), (2, 8, 12)]
            );
            assert!(function.interference.is_empty());
        }
        let mut corrupted_ranges = ranges.ranges().plan().clone();
        corrupted_ranges.functions[1].machine = expected_machines[0];
        assert!(
            validate_live_ranges(
                ranges.liveness_stage().selected_stage().selected(),
                ranges.liveness_stage().liveness(),
                corrupted_ranges,
            )
            .is_err()
        );

        let legality = stage_optimized_allocation_legality(ranges).unwrap();
        assert_eq!(legality.custody().function_count(), 2);
        assert_eq!(legality.custody().structural_unit_function_count(), 0);
        assert_eq!(legality.custody().virtual_register_count(), 6);
        let range_stage = legality.live_range_stage();
        let environment = range_stage
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let mut corrupted_legality = legality.legality().plan().clone();
        corrupted_legality.functions[1].machine = expected_machines[0];
        assert!(
            validate_allocation_legality(
                range_stage.ranges(),
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted_legality,
            )
            .is_err()
        );

        let homes = stage_optimized_register_homes(legality).unwrap();
        assert_eq!(homes.custody().function_count(), 2);
        assert_eq!(homes.custody().structural_unit_function_count(), 0);
        assert_eq!(homes.custody().assignment_count(), 6);
        assert_eq!(
            homes
                .homes()
                .plan()
                .functions
                .iter()
                .map(|function| function.machine)
                .collect::<Vec<_>>(),
            expected_machines
        );
        assert_eq!(
            homes.homes().plan().functions[0].assignments,
            homes.homes().plan().functions[1].assignments
        );
        let legality_stage = homes.legality_stage();
        let range_stage = legality_stage.live_range_stage();
        let environment = range_stage
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let mut corrupted_homes = homes.homes().plan().clone();
        corrupted_homes.functions[1].machine = expected_machines[0];
        assert!(
            validate_register_homes(
                legality_stage.legality(),
                range_stage.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted_homes,
            )
            .is_err()
        );

        let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        assert_eq!(post.custody().instruction_count(), 12);
        assert_eq!(post.machine().plan().functions.len(), 2);
        assert_eq!(
            post.machine()
                .plan()
                .functions
                .iter()
                .map(|function| function.machine)
                .collect::<Vec<_>>(),
            expected_machines
        );
        let mut corrupted_post = post.machine().plan().clone();
        corrupted_post.functions[1].machine = expected_machines[0];
        let legality_stage = homes.legality_stage();
        let range_stage = legality_stage.live_range_stage();
        let selected_stage = range_stage.liveness_stage().selected_stage();
        let environment = selected_stage.register_environment();
        assert!(
            omega_machine_optimizer::validate_post_allocation_machine_plan(
                selected_stage.selected(),
                post.effects().effects(),
                range_stage.ranges(),
                legality_stage.legality(),
                homes.homes(),
                homes.post_allocation_manifest(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                corrupted_post,
            )
            .is_err()
        );
    }
}
