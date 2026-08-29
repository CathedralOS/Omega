use crate::tests::*;

#[test]
fn selected_liveness_is_exact_on_both_architectures() {
    for (target, before_compare, after_compare, after_branch) in [
        (
            NativeTarget::linux_x64(),
            vec!["rip", "rsp"],
            vec!["rflags", "rip", "rsp"],
            vec!["rsp"],
        ),
        (
            NativeTarget::linux_arm64(),
            vec!["pc", "sp", "x30"],
            vec!["nzcv", "pc", "sp", "x30"],
            vec!["sp", "x30"],
        ),
    ] {
        let staged = stage_optimized_liveness(staged_conditional(target)).unwrap();
        let plan = staged.liveness().plan();
        let function = &plan.functions[0];
        assert_eq!(function.entry_definitions.len(), 1);
        assert_eq!(
            function.entry_definitions[0].virtual_register,
            VirtualRegisterId(0)
        );
        assert!(function.entry_definitions[0].fixed_view.is_some());
        assert_eq!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .map(|instruction| instruction.position.0)
                .collect::<Vec<_>>(),
            (0..6).collect::<Vec<_>>()
        );

        let entry = &function.blocks[0];
        assert_eq!(entry.virtual_live_in, vec![VirtualRegisterId(0)]);
        assert!(entry.virtual_live_out.is_empty());
        assert_eq!(
            entry.instructions[0].virtual_uses,
            vec![VirtualRegisterId(0)]
        );
        assert!(entry.instructions[0].virtual_defs.is_empty());
        assert_eq!(
            entry.instructions[0].unit_live_in,
            named_units(&staged, &before_compare)
        );
        assert_eq!(
            entry.instructions[0].unit_live_out,
            named_units(&staged, &after_compare)
        );
        assert_eq!(
            entry.instructions[1].unit_live_in,
            entry.instructions[0].unit_live_out
        );
        assert_eq!(
            entry.instructions[1].unit_live_out,
            named_units(&staged, &after_branch)
        );
        assert_eq!(entry.successors.len(), 2);
        assert_eq!(entry.successors[0].polarity_ordinal, 0);
        assert_eq!(entry.successors[1].polarity_ordinal, 1);
        for successor in &entry.successors {
            let target_block = &function.blocks[successor.target.0 as usize];
            assert_eq!(successor.virtual_live, target_block.virtual_live_in);
            assert_eq!(successor.unit_live, target_block.unit_live_in);
        }

        for (block, register) in function.blocks[1..]
            .iter()
            .zip([VirtualRegisterId(1), VirtualRegisterId(2)])
        {
            assert!(block.virtual_live_in.is_empty());
            assert!(block.virtual_live_out.is_empty());
            assert_eq!(block.instructions[0].virtual_defs, vec![register]);
            assert_eq!(block.instructions[0].virtual_live_out, vec![register]);
            assert_eq!(block.instructions[1].virtual_uses, vec![register]);
            assert_eq!(block.instructions[1].virtual_live_in, vec![register]);
            assert!(block.instructions[1].virtual_live_out.is_empty());
        }
        assert_eq!(staged.custody().function_count(), 1);
        assert_eq!(staged.custody().block_count(), 3);
        assert_eq!(staged.custody().virtual_register_count(), 3);
        assert_eq!(staged.custody().instruction_count(), 6);
        assert_eq!(staged.custody().successor_count(), 2);
        assert_eq!(
            staged.custody().register_environment(),
            staged.selected_stage().register_environment().identity()
        );
        assert_eq!(
            staged.custody().liveness(),
            staged.liveness().receipt().identity()
        );
        assert_eq!(
            staged.custody().selected(),
            staged.selected_stage().selected().receipt().identity()
        );
    }
}

#[test]
fn forwarded_parameter_conditional_retains_cross_edge_liveness() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_forwarded_conditional(target);
        assert_eq!(
            selected.legalized().plan().functions[0].recipe,
            LegalizationRecipe::ReturnU64EntryParameterConditionalV1
        );
        let selected_plan = selected.selected().plan();
        assert_eq!(selected_plan.functions[0].virtual_registers.len(), 2);
        assert_eq!(
            selected_plan.functions[0]
                .blocks
                .iter()
                .map(|block| block.instructions.len() + 1)
                .sum::<usize>(),
            4
        );
        assert!(
            selected_plan.functions[0].virtual_registers[0]
                .entry_fixed_view
                .is_some()
        );
        assert!(
            selected_plan.functions[0].virtual_registers[1]
                .entry_fixed_view
                .is_some()
        );

        let staged = stage_optimized_liveness(selected).unwrap();
        let function = &staged.liveness().plan().functions[0];
        assert_eq!(
            function.blocks[0].virtual_live_in,
            vec![VirtualRegisterId(0), VirtualRegisterId(1)]
        );
        assert_eq!(
            function.blocks[0].virtual_live_out,
            vec![VirtualRegisterId(1)]
        );
        for successor in &function.blocks[0].successors {
            assert_eq!(successor.virtual_live, vec![VirtualRegisterId(1)]);
        }
        for block in &function.blocks[1..] {
            assert_eq!(block.virtual_live_in, vec![VirtualRegisterId(1)]);
            assert!(block.virtual_live_out.is_empty());
            assert_eq!(
                block.instructions[0].virtual_uses,
                vec![VirtualRegisterId(1)]
            );
            assert!(block.instructions[0].virtual_live_out.is_empty());
            assert!(block.instructions[0].unit_live_out.is_empty());
        }
    }
}

#[test]
fn forwarded_parameter_selection_rejects_fixed_input_and_path_corruption() {
    let staged = staged_forwarded_conditional(NativeTarget::linux_x64());
    let mut corrupted = staged.selected().plan().clone();
    corrupted.functions[0].virtual_registers[1].entry_fixed_view = None;
    assert!(matches!(
        validate_raw_selection(&staged, corrupted),
        Err(SelectedInstructionError::VirtualRegisterProjectionMismatch { .. })
    ));

    let mut corrupted = staged.selected().plan().clone();
    let SelectedTerminator::Return { instruction, .. } =
        &mut corrupted.functions[0].blocks[1].terminator
    else {
        unreachable!()
    };
    instruction.operands[0].virtual_register = VirtualRegisterId(0);
    assert!(matches!(
        validate_raw_selection(&staged, corrupted),
        Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
    ));
}

#[test]
fn live_ranges_are_block_local_and_interference_is_cfg_exact() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
        )
        .unwrap();
        let function = &staged.ranges().plan().functions[0];
        assert_eq!(
            function
                .block_domains
                .iter()
                .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4), (1, 4, 6), (2, 6, 8)]
        );
        assert_eq!(function.virtual_registers.len(), 2);
        assert_eq!(
            function.virtual_registers[0].fragments,
            vec![LiveRangeFragment {
                block: omega_selected_instructions::SelectedBlockId(0),
                start: LiveRangePoint(0),
                end: LiveRangePoint(1),
            }]
        );
        assert_eq!(
            function.virtual_registers[1]
                .fragments
                .iter()
                .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4), (1, 4, 5), (2, 6, 7)]
        );
        assert_eq!(
            function.virtual_registers[1]
                .edge_connectors
                .iter()
                .map(|edge| (edge.polarity_ordinal, edge.psi_edge, edge.target.0))
                .collect::<Vec<_>>(),
            vec![
                (0, EdgeId::new(4_011).unwrap(), 1),
                (1, EdgeId::new(4_012).unwrap(), 2),
            ]
        );
        assert_eq!(
            function.interference,
            vec![VirtualInterference {
                lower: VirtualRegisterId(0),
                higher: VirtualRegisterId(1),
            }]
        );
        assert_eq!(function.virtual_registers[0].fixed_constraints.len(), 1);
        assert!(matches!(
            function.virtual_registers[0].fixed_constraints[0].site,
            VirtualFixedConstraintSite::Entry
        ));
        assert_eq!(function.virtual_registers[1].fixed_constraints.len(), 3);
        assert!(matches!(
            function.virtual_registers[1].fixed_constraints[0].site,
            VirtualFixedConstraintSite::Entry
        ));
        assert!(
            function.virtual_registers[1].fixed_constraints[1..]
                .iter()
                .all(|constraint| matches!(
                    constraint.site,
                    VirtualFixedConstraintSite::Operand { .. }
                ))
        );
        assert_eq!(staged.custody().interference_count(), 1);
        assert_eq!(
            staged.custody().register_environment(),
            staged
                .liveness_stage()
                .selected_stage()
                .register_environment()
                .identity()
        );
        assert_eq!(
            staged.custody().ranges(),
            staged.ranges().receipt().identity()
        );
        assert_eq!(
            staged.custody().liveness(),
            staged.liveness_stage().liveness().receipt().identity()
        );

        let repeated = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
        )
        .unwrap();
        assert_eq!(staged.ranges(), repeated.ranges());
        assert_eq!(staged.custody(), repeated.custody());
    }

    let constant = stage_optimized_live_ranges(
        stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap(),
    )
    .unwrap();
    let function = &constant.ranges().plan().functions[0];
    assert_eq!(
        function
            .block_domains
            .iter()
            .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
            .collect::<Vec<_>>(),
        vec![(0, 0, 4), (1, 4, 8), (2, 8, 12)]
    );
    assert_eq!(
        function
            .virtual_registers
            .iter()
            .flat_map(|range| &range.fragments)
            .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
            .collect::<Vec<_>>(),
        vec![(0, 0, 1), (1, 5, 7), (2, 9, 11)]
    );
    assert!(function.interference.is_empty());
    assert!(
        function
            .virtual_registers
            .iter()
            .all(|range| range.edge_connectors.is_empty())
    );
}

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
                corrupted,
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

#[test]
fn fixed_view_copies_are_explicit_reanalyzed_and_deterministic() {
    for (target, entry_name, result_name) in [
        (NativeTarget::linux_x64(), "rsi", "rax"),
        (NativeTarget::linux_arm64(), "x1", "x0"),
    ] {
        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let source_selected = source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .selected()
            .plan()
            .clone();
        let source_manifest = source.custody().manifest();
        let materialized = stage_optimized_fixed_view_copies(
            source,
            FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            budget(),
        )
        .unwrap();
        let machine_effects =
            stage_optimized_machine_effects_after_fixed_view_copies(&materialized).unwrap();
        assert_eq!(
            machine_effects.effects().receipt().selected(),
            materialized.custody().transformed_selected()
        );
        assert_eq!(
            machine_effects.custody().source(),
            &StagedOptimizedMachineEffectSourceCustodyReceipt::FixedViewCopies(
                materialized.custody()
            )
        );
        assert_eq!(
            &validate_optimized_machine_effect_custody_after_fixed_view_copies(
                &materialized,
                machine_effects.effects(),
            )
            .unwrap(),
            machine_effects.custody()
        );
        let copy_plan = materialized.copies().plan();
        assert_eq!(copy_plan.copies.len(), 2);
        assert_eq!(materialized.custody().copy_count(), 2);
        assert_eq!(materialized.custody().manifest(), source_manifest);
        assert_eq!(
            copy_plan.usage,
            omega_optimization_core::OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 2,
                validation_steps: 2,
                commits: 2,
                iterations: 1,
            }
        );
        assert_ne!(
            materialized.custody().source_selected(),
            materialized.custody().transformed_selected()
        );
        assert_eq!(
            fixed_view_copy_identity(copy_plan),
            materialized.custody().transformation()
        );
        let transformed = &copy_plan.transformed;
        assert_eq!(transformed.functions[0].virtual_registers.len(), 4);
        let environment = materialized
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let entry_view = environment
            .physical()
            .model()
            .view_named(entry_name)
            .unwrap()
            .id;
        let result_view = environment
            .physical()
            .model()
            .view_named(result_name)
            .unwrap()
            .id;
        for (index, copy) in copy_plan.copies.iter().enumerate() {
            assert_eq!(copy.source_virtual_register, VirtualRegisterId(1));
            assert_eq!(copy.result_virtual_register.0, 2 + index as u32);
            assert_eq!(copy.copy_instruction.0, 4 + index as u32);
            assert_eq!(copy.from_view, entry_view);
            assert_eq!(copy.to_view, result_view);
            assert_eq!(copy.copy_constraint, environment.selected_keys().copy_i64);
            let block = &transformed.functions[0].blocks[index + 1];
            let instruction = block.instructions.last().unwrap();
            assert_eq!(instruction.id, copy.copy_instruction);
            assert_eq!(instruction.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(
                instruction.operands[0].virtual_register,
                copy.source_virtual_register
            );
            assert_eq!(
                instruction.operands[1].virtual_register,
                copy.result_virtual_register
            );
            assert!(instruction.provenance.operations.is_empty());
            assert_eq!(instruction.provenance.values, vec![copy.source_value]);
            assert!(instruction.provenance.edges.is_empty());
            assert!(instruction.provenance.obligations.is_empty());
            assert!(instruction.provenance.fuel.is_empty());
            let SelectedTerminator::Return {
                instruction: source_return,
                ..
            } = &source_selected.functions[0].blocks[index + 1].terminator
            else {
                unreachable!()
            };
            let SelectedTerminator::Return {
                instruction: transformed_return,
                ..
            } = &block.terminator
            else {
                unreachable!()
            };
            assert_eq!(source_return.id, transformed_return.id);
            assert_eq!(source_return.provenance, transformed_return.provenance);
            assert_eq!(
                transformed_return.operands[0].virtual_register,
                copy.result_virtual_register
            );
        }

        let mut corrupted = materialized.copies().plan().clone();
        corrupted.copies[0].from_view = result_view;
        assert!(matches!(
            validate_fixed_view_copies(
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .selected(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .ranges(),
                materialized.source_legality_stage().legality(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted,
            ),
            Err(FixedViewCopyError::CopyMismatch { index: 0 })
        ));
        let mut corrupted = materialized.copies().plan().clone();
        corrupted.transformed.functions[0].blocks[1].instructions[0]
            .provenance
            .values
            .clear();
        assert!(matches!(
            validate_fixed_view_copies(
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .selected(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .ranges(),
                materialized.source_legality_stage().legality(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted,
            ),
            Err(FixedViewCopyError::TransformedPlanMismatch)
        ));
        let mut corrupted = materialized.copies().plan().clone();
        corrupted.usage.commits += 1;
        assert!(matches!(
            validate_fixed_view_copies(
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .selected(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .ranges(),
                materialized.source_legality_stage().legality(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted,
            ),
            Err(FixedViewCopyError::ReceiptMismatch)
        ));
        assert!(matches!(
            validate_live_ranges(
                materialized.copies(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .liveness(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .ranges()
                    .plan()
                    .clone(),
            ),
            Err(LiveRangeError::LivenessRevalidation(
                LivenessError::RootMismatch
            ))
        ));

        let reanalyzed = stage_optimized_selected_reanalysis(materialized).unwrap();
        assert_eq!(reanalyzed.custody().entry_transition_count(), 0);
        assert_eq!(reanalyzed.legality().receipt().entry_transition_count(), 0);
        let homes = stage_optimized_register_homes_after_fixed_view_copies(reanalyzed).unwrap();
        let post =
            stage_optimized_post_allocation_machine_plan_after_fixed_view_copies(&homes).unwrap();
        assert_eq!(
            post.machine().receipt().selected(),
            homes.reanalysis_stage().ranges().receipt().selected()
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
                &homes, &post,
            )
            .unwrap(),
            post.custody()
        );
        let assignments = &homes.homes().plan().functions[0].assignments;
        assert_eq!(assignments.len(), 4);
        assert_eq!(assignments[1].view, entry_view);
        assert_ne!(assignments[0].view, assignments[1].view);
        assert_eq!(assignments[2].view, result_view);
        assert_eq!(assignments[3].view, result_view);
        assert_eq!(
            homes.reanalysis_stage().ranges().plan().functions[0].interference,
            vec![VirtualInterference {
                lower: VirtualRegisterId(0),
                higher: VirtualRegisterId(1),
            }]
        );
        assert_eq!(homes.custody().assignment_count(), 4);
        let manifest = homes.post_allocation_manifest().record();
        assert_eq!(manifest.identity, manifest.recomputed_identity());
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&manifest.encode()),
            Ok(manifest.clone())
        );
        assert_eq!(
            manifest.selected_transformations,
            vec![PostAllocationSelectedTransformation::FixedViewCopy(
                homes.custody().source().source().transformation()
            )]
        );
        assert_eq!(
            manifest.selected,
            homes.reanalysis_stage().ranges().plan().selected
        );
        assert_eq!(manifest.statistics.assignments, 4);
        assert_eq!(manifest.statistics.virtual_interferences, 1);
        let transformation = PostAllocationSelectedTransformation::FixedViewCopy(
            homes.custody().source().source().transformation(),
        );
        assert_eq!(
            validate_post_allocation_optimization_manifest(
                manifest,
                homes.custody().source().source().manifest(),
                &[transformation, transformation],
                homes.reanalysis_stage().ranges(),
                homes.reanalysis_stage().legality(),
                homes.homes(),
            ),
            Err(PostAllocationOptimizationManifestError::NonCanonicalTransformationLedger)
        );
        assert_eq!(
            homes.custody().post_allocation_manifest(),
            manifest.identity
        );
        assert_eq!(
            validate_optimized_register_home_after_fixed_view_copy_custody(
                homes.reanalysis_stage(),
                homes.homes(),
                homes.post_allocation_manifest(),
            )
            .unwrap(),
            homes.custody()
        );

        let repeated = stage_optimized_register_homes_after_fixed_view_copies(
            stage_optimized_selected_reanalysis(
                stage_optimized_fixed_view_copies(
                    stage_optimized_allocation_legality(
                        stage_optimized_live_ranges(
                            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                    FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                    budget(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(homes.homes(), repeated.homes());
        assert_eq!(homes.custody(), repeated.custody());
    }

    let constrained = OptimizationWorkBudget::new(128, 128, 128, 1, 16).unwrap();
    let source = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        stage_optimized_fixed_view_copies(
            source,
            FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            constrained,
        ),
        Err(OptimizedFixedViewCopyCustodyError::Materialization(
            FixedViewCopyError::BudgetExceeded { .. }
        ))
    ));

    let constant = stage_optimized_fixed_view_copies(
        stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
        budget(),
    )
    .unwrap();
    assert!(constant.copies().plan().copies.is_empty());
    assert_eq!(
        constant.copies().plan().source_selected,
        constant.copies().receipt().transformed_selected()
    );
}

#[test]
fn architectural_actions_do_not_inflate_semantic_unit_fragments() {
    for (target, instruction_pointer) in [
        (NativeTarget::linux_x64(), "rip"),
        (NativeTarget::linux_arm64(), "pc"),
    ] {
        let staged = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
        )
        .unwrap();
        let unit = named_units(staged.liveness_stage(), &[instruction_pointer])[0];
        let range = staged.ranges().plan().functions[0]
            .architectural_units
            .iter()
            .find(|range| range.unit == unit)
            .unwrap();
        assert_eq!(
            range
                .fragments
                .iter()
                .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 3)]
        );
        assert!(range.actions.iter().any(|action| {
            action.point == LiveRangePoint(3) && action.kind == ArchitecturalUnitActionKind::Def
        }));
    }
}

#[test]
fn independent_live_range_validation_rejects_corruption_and_detachment() {
    let staged =
        stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64())).unwrap();
    let valid = analyze_live_ranges(staged.selected_stage().selected(), staged.liveness()).unwrap();
    let identity = live_range_identity(valid.plan());

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].virtual_registers[1].fragments[0]
        .end
        .0 -= 1;
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted.clone(),
        ),
        Err(LiveRangeError::VirtualRegisterMismatch { .. })
    ));
    assert_ne!(live_range_identity(&corrupted), identity);

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].virtual_registers[1].edge_connectors[0].polarity_ordinal = 1;
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted,
        ),
        Err(LiveRangeError::NonCanonicalRows { .. })
            | Err(LiveRangeError::VirtualRegisterMismatch { .. })
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].interference.clear();
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted,
        ),
        Err(LiveRangeError::InterferenceMismatch { .. })
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].virtual_registers[1].fixed_constraints[0]
        .view
        .0 += 1;
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted,
        ),
        Err(LiveRangeError::VirtualRegisterMismatch { .. })
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].architectural_units[0].actions[0]
        .point
        .0 += 1;
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted,
        ),
        Err(LiveRangeError::ArchitecturalUnitMismatch { .. })
    ));

    let arm = stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_arm64()))
        .unwrap();
    let arm_ranges = analyze_live_ranges(arm.selected_stage().selected(), arm.liveness()).unwrap();
    assert!(matches!(
        validate_optimized_live_range_custody(&staged, &arm_ranges),
        Err(OptimizedLiveRangeCustodyError::Revalidation(
            LiveRangeError::RootMismatch
        ))
    ));
}

#[test]
fn selected_liveness_is_deterministic_and_identity_binds_every_domain() {
    let first = stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap();
    let second = stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap();
    assert_eq!(first.liveness(), second.liveness());
    assert_eq!(first.custody(), second.custody());

    let original = first.liveness().plan();
    let identity = liveness_identity(original);
    let mut mutations = Vec::new();
    let mut changed = original.clone();
    changed.selected =
        omega_selected_instructions::SelectedInstructionPlanIdentity::from_canonical_bytes(
            b"changed-selected",
        );
    mutations.push(changed);
    let mut changed = original.clone();
    changed.target = NativeTarget::windows_x64();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.fuel_schedule = psi_core::FuelScheduleIdentity::new(
        original.fuel_schedule.marker().checked_add(1).unwrap(),
    )
    .unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.optimization_unit =
        omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"changed");
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].machine = MachineId::new(8_101).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].entry_definitions[0].fixed_view = None;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].entry_definitions[0].virtual_register = VirtualRegisterId(8);
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].entry_definitions[0].class.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].position.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].instruction.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].operand += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].virtual_register.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].access = RegisterOperandAccess::Def;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].class.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].fixed_view =
        changed.functions[0].entry_definitions[0].fixed_view;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].tied_to = Some(0);
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].early_clobber = true;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].block.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].source_block = BlockId::new(8_103).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .virtual_live_in
        .push(VirtualRegisterId(9));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .unit_live_in
        .push(RegisterUnitId(u16::MAX));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .virtual_live_out
        .push(VirtualRegisterId(9));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .unit_live_out
        .push(RegisterUnitId(u16::MAX));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0].position.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0].instruction.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .virtual_uses
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .virtual_defs
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .virtual_live_in
        .push(VirtualRegisterId(9));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .virtual_live_out
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[1]
        .unit_uses
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_defs
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_clobbers
        .push(RegisterUnitId(u16::MAX));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_live_in
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_live_out
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0].polarity_ordinal = 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0].psi_edge = EdgeId::new(8_102).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0].terminator.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0].target.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0]
        .virtual_live
        .push(VirtualRegisterId(9));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0]
        .unit_live
        .clear();
    mutations.push(changed);
    for mutation in mutations {
        assert_ne!(liveness_identity(&mutation), identity);
    }
}

#[test]
fn independent_liveness_validator_rejects_raw_transfer_and_path_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_conditional(target);
        let valid = analyze_liveness(selected.selected()).unwrap();

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[0].virtual_live_in.clear();
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::BlockMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[0].instructions[0]
            .unit_live_out
            .clear();
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::TransferMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[1].instructions[1]
            .virtual_live_in
            .clear();
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::TransferMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[0].successors.swap(0, 1);
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::SuccessorMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[2].instructions[0].position.0 = 99;
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::NonDensePositions { .. })
        ));
    }
}

#[test]
fn liveness_custody_rejects_a_detached_same_shape_target() {
    let x86 = staged_conditional(NativeTarget::linux_x64());
    let arm = staged_conditional(NativeTarget::linux_arm64());
    let arm_liveness = analyze_liveness(arm.selected()).unwrap();
    assert!(matches!(
        validate_optimized_liveness_custody(&x86, &arm_liveness),
        Err(OptimizedLivenessCustodyError::Revalidation(
            LivenessError::RootMismatch
        ))
    ));
}
