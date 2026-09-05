use crate::tests::*;
#[test]
fn transition_free_register_homes_are_deterministic_and_cfg_exact() {
    for (target, condition_view, result_view) in [
        (NativeTarget::linux_x64(), "rdi", "rax"),
        (NativeTarget::linux_arm64(), "x0", "x0"),
    ] {
        let staged = stage_optimized_register_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let function = &staged.homes().plan().functions[0];
        assert_eq!(function.assignments.len(), 3);
        let environment = staged
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let encoded = staged.homes().plan().encode();
        let decoded = RegisterHomePlan::decode(&encoded).unwrap();
        assert_eq!(&decoded, staged.homes().plan());
        let legality = staged.legality_stage();
        let ranges = legality.live_range_stage();
        let replay = validate_register_homes(
            legality.legality(),
            ranges.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            decoded,
        )
        .unwrap();
        assert_eq!(replay, *staged.homes());
        let manifest = staged.post_allocation_manifest().record();
        assert_eq!(manifest.identity, manifest.recomputed_identity());
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&manifest.encode()),
            Ok(manifest.clone())
        );
        assert_eq!(manifest.pre_physical, staged.custody().manifest());
        assert_eq!(manifest.target, target);
        assert!(manifest.selected_transformations.is_empty());
        assert_eq!(manifest.homes, staged.homes().receipt().identity());
        assert_eq!(manifest.statistics.functions, 1);
        assert_eq!(manifest.statistics.assignments, 3);
        assert_eq!(manifest.statistics.fixed_view_transitions, 0);
        assert_eq!(
            staged.custody().post_allocation_manifest(),
            manifest.identity
        );
        assert_eq!(
            validate_optimized_register_home_custody(
                legality,
                staged.homes(),
                staged.post_allocation_manifest(),
            )
            .unwrap(),
            staged.custody()
        );
        assert!(manifest.render_text().contains("frame: unavailable"));
        assert_eq!(
            validate_post_allocation_optimization_manifest(
                manifest,
                staged.custody().manifest(),
                &[],
                ranges.ranges(),
                legality.legality(),
                staged.homes(),
            )
            .unwrap(),
            *staged.post_allocation_manifest()
        );
        let mut corrupted = manifest.clone();
        corrupted.statistics.assignments += 1;
        assert_eq!(
            validate_post_allocation_optimization_manifest(
                &corrupted,
                staged.custody().manifest(),
                &[],
                ranges.ranges(),
                legality.legality(),
                staged.homes(),
            ),
            Err(PostAllocationOptimizationManifestError::IdentityMismatch)
        );
        corrupted.identity = corrupted.recomputed_identity();
        assert_eq!(
            validate_post_allocation_optimization_manifest(
                &corrupted,
                staged.custody().manifest(),
                &[],
                ranges.ranges(),
                legality.legality(),
                staged.homes(),
            ),
            Err(PostAllocationOptimizationManifestError::ContentMismatch)
        );
        let model = environment.physical().model();
        assert_eq!(
            function.assignments[0].view,
            model.view_named(condition_view).unwrap().id
        );
        assert_eq!(
            function.assignments[1].view,
            model.view_named(result_view).unwrap().id
        );
        assert_eq!(function.assignments[1].view, function.assignments[2].view);
        assert!(
            staged
                .legality_stage()
                .live_range_stage()
                .ranges()
                .plan()
                .functions[0]
                .interference
                .is_empty()
        );
        for assignment in &function.assignments {
            let view = &model.views[usize::from(assignment.view.0)];
            assert_eq!(view.class, assignment.class);
            assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                environment
                    .reservations()
                    .reserved_units()
                    .binary_search(unit)
                    .is_err()
            }));
        }
        assert_eq!(staged.custody().assignment_count(), 3);
        assert_eq!(
            staged.custody().homes(),
            staged.homes().receipt().identity()
        );
        assert_eq!(
            staged.custody().register_environment(),
            environment.identity()
        );

        let repeated = stage_optimized_register_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(staged.homes(), repeated.homes());
        assert_eq!(staged.custody(), repeated.custody());

        let mut corrupted = staged.homes().plan().clone();
        let original_view = corrupted.functions[0].assignments[0].view;
        corrupted.functions[0].assignments[0].view = model
            .views
            .iter()
            .find(|view| {
                view.class == corrupted.functions[0].assignments[0].class
                    && view.id != original_view
            })
            .expect("fixture register class has a distinct corruption view")
            .id;
        assert_ne!(
            register_home_identity(&corrupted),
            staged.homes().receipt().identity()
        );
        // A fresh canonical frame can carry invalid assignments. Data integrity
        // does not grant the independent allocator admission retained above.
        let decoded = RegisterHomePlan::decode(&corrupted.encode()).unwrap();
        assert_eq!(decoded, corrupted);
        let legality = staged.legality_stage();
        let ranges = legality.live_range_stage();
        assert!(matches!(
            validate_register_homes(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                decoded,
            ),
            Err(RegisterHomeError::VirtualRegisterMismatch { .. })
                | Err(RegisterHomeError::UnknownOrIncompatibleView { .. })
        ));
    }

    let forwarded = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        stage_optimized_register_homes(forwarded),
        Err(OptimizedRegisterHomeCustodyError::Assignment(
            RegisterHomeError::UnresolvedEntryTransitions { count: 2, .. }
        ))
    ));
}
