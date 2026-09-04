use crate::tests::*;

#[test]
fn named_exact_subtract_immediate_suite_closes_pressure_and_rejects_policy_substitution() {
    for (target, sole_view_name) in [
        (NativeTarget::linux_x64(), "rax"),
        (NativeTarget::linux_arm64(), "x0"),
    ] {
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ])
        .unwrap();
        let ranges = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_subtract_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
            ))
            .unwrap(),
        )
        .unwrap();
        let environment = ranges
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let sole_view = environment
            .physical()
            .model()
            .view_named(sole_view_name)
            .unwrap()
            .id;
        let availability = materialize_allocator_availability(
            environment.identity(),
            environment.target(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                views: vec![sole_view],
            },
        )
        .unwrap();
        let legality =
            stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap();
        let run = run_selected_lowering_optimizations(legality).unwrap();

        assert_eq!(
            run.selected_lowering_selections().as_slice(),
            &[Optimization::SelectedIncomingU12ExactSubtractImmediate]
        );
        assert_eq!(run.steps().len(), 2);
        assert_eq!(run.custody().action_count(), 2);
        assert_eq!(run.attempt().fold().receipt().applied_count(), 0);
        assert_eq!(
            validate_selected_lowering_optimization_custody(&run).unwrap(),
            *run.custody()
        );

        let final_plan = run.attempt().fold().transformed();
        let folded = final_plan.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.kind {
                SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
                    Some((immediate, &instruction.provenance))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            folded
                .iter()
                .map(|(immediate, _)| *immediate)
                .collect::<Vec<_>>(),
            vec![IntegerValue::Unsigned(5), IntegerValue::Unsigned(8)]
        );
        for (_, provenance) in folded {
            assert_eq!(provenance.operations.len(), 2);
            assert_eq!(provenance.fuel.len(), 2);
            assert_eq!(provenance.obligations.len(), 1);
        }

        let source = run.source_legality_stage();
        let selected = source.live_range_stage().liveness_stage().selected_stage();
        let environment = selected.register_environment();
        let first = &run.steps()[0];
        let mut substituted_policy = first.fold().plan().clone();
        substituted_policy.policy = LiteralFoldPolicy::EXACT_ADD_V1;
        assert!(
            validate_literal_fold(
                selected.selected(),
                source.live_range_stage().ranges(),
                source.legality(),
                first.choices(),
                first.recovery(),
                source.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                substituted_policy,
            )
            .is_err()
        );

        let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
        assert_eq!(
            validate_optimized_register_home_after_selected_lowering_custody(&homes).unwrap(),
            *homes.custody()
        );
    }
}

#[test]
fn combined_exact_immediate_selection_executes_each_named_shape() {
    for subtract in [false, true] {
        let target = NativeTarget::linux_x64();
        let selections = OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ])
        .unwrap();
        let selected = if subtract {
            staged_exact_subtract_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
            )
        } else {
            staged_exact_add_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
            )
        };
        let ranges =
            stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
        let environment = ranges
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let sole_view = environment.physical().model().view_named("rax").unwrap().id;
        let availability = materialize_allocator_availability(
            environment.identity(),
            environment.target(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                views: vec![sole_view],
            },
        )
        .unwrap();
        let run = run_selected_lowering_optimizations(
            stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap(),
        )
        .unwrap();

        let resolved_policy = run.attempt().fold().receipt().policy();
        assert!(resolved_policy.enables_exact_add());
        assert!(resolved_policy.enables_exact_subtract());
        assert_eq!(run.custody().action_count(), 2);
        let final_plan = run.attempt().fold().transformed();
        let matching_immediate_count = final_plan.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                if subtract {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::ExactSubtractI64Immediate { .. }
                    )
                } else {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::ExactAddI64Immediate { .. }
                    )
                }
            })
            .count();
        assert_eq!(matching_immediate_count, 2);
        assert_eq!(
            validate_selected_lowering_optimization_custody(&run).unwrap(),
            *run.custody()
        );
    }
}
