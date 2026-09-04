use crate::tests::*;
#[test]
fn allocation_legality_is_phase_exact_and_exposes_fixed_view_transitions() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let function = &staged.legality().plan().functions[0];
        assert_eq!(function.virtual_registers.len(), 2);
        assert_eq!(function.virtual_registers[0].entry_transitions.len(), 0);
        assert_eq!(function.virtual_registers[1].entry_transitions.len(), 2);
        assert!(
            function.virtual_registers[1]
                .entry_transitions
                .iter()
                .all(|transition| transition.from_view != transition.to_view)
        );

        let environment = staged
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let model = environment.physical().model();
        let range_function = &staged.live_range_stage().ranges().plan().functions[0];
        for register in &function.virtual_registers {
            for point in &register.points {
                assert!(!point.candidates.is_empty());
                for candidate in &point.candidates {
                    let view = &model.views[usize::from(candidate.0)];
                    assert_eq!(view.class, register.class);
                    assert!(view.allocatable);
                    assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                        environment
                            .reservations()
                            .reserved_units()
                            .binary_search(unit)
                            .is_err()
                    }));
                    assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                        range_function
                            .architectural_units
                            .iter()
                            .find(|row| row.unit == *unit)
                            .is_none_or(|row| {
                                !row.fragments.iter().any(|fragment| {
                                    fragment.block == point.block
                                        && fragment.start <= point.point
                                        && point.point < fragment.end
                                }) && !row.actions.iter().any(|action| {
                                    action.block == point.block && action.point == point.point
                                })
                            })
                    }));
                }
            }
        }
        assert_eq!(
            staged.custody().register_environment(),
            environment.identity()
        );
        assert_eq!(
            staged.custody().legality(),
            staged.legality().receipt().identity()
        );
        assert_eq!(staged.custody().entry_transition_count(), 2);

        if target == NativeTarget::linux_x64() {
            let mut overlays = environment.reservations().profile().active_overlays.clone();
            overlays.retain(|name| name != "omega.x86.metering");
            let reduced = validate_register_reservation_profile(
                RegisterReservationProfile {
                    name: "test.no-metering".into(),
                    active_overlays: overlays,
                },
                target,
                environment.physical(),
            )
            .unwrap();
            let reduced_identity = target_register_environment_identity(
                target,
                environment.physical(),
                environment.constraints(),
                &reduced,
                environment.allocation_constraint_keys(),
            );
            let reduced_availability = materialize_allocator_availability(
                reduced_identity,
                target,
                environment.physical(),
                environment.constraints(),
                &reduced,
                environment.allocation_constraint_keys(),
                AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
            )
            .unwrap();
            let reduced_legality = omega_regalloc::analyze_allocation_legality(
                staged.live_range_stage().ranges(),
                &reduced_availability,
                reduced_identity,
                environment.physical(),
                environment.constraints(),
                &reduced,
                environment.allocation_constraint_keys(),
            )
            .unwrap();
            let r15 = model.view_named("r15").unwrap().id;
            assert!(
                function
                    .virtual_registers
                    .iter()
                    .flat_map(|register| &register.points)
                    .all(|point| !point.candidates.contains(&r15))
            );
            assert!(
                reduced_legality.plan().functions[0]
                    .virtual_registers
                    .iter()
                    .flat_map(|register| &register.points)
                    .any(|point| point.candidates.contains(&r15))
            );
            assert_ne!(
                reduced_legality.receipt().identity(),
                staged.legality().receipt().identity()
            );
        }

        let repeated = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(staged.legality(), repeated.legality());
        assert_eq!(staged.custody(), repeated.custody());

        let mut corrupted = staged.legality().plan().clone();
        corrupted.functions[0].virtual_registers[0].points[0]
            .candidates
            .clear();
        assert_ne!(
            allocation_legality_identity(&corrupted),
            staged.legality().receipt().identity()
        );
        let ranges = staged.live_range_stage();
        assert!(matches!(
            validate_allocation_legality(
                ranges.ranges(),
                staged.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted,
            ),
            Err(AllocationLegalityError::VirtualRegisterMismatch { .. })
        ));
    }

    let constant = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(constant.custody().entry_transition_count(), 0);
}
