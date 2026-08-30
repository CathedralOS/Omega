use crate::tests::*;

#[test]
fn exact_add_pressure_reaches_deterministic_homes_on_both_architectures() {
    for (target, expected_homes) in [
        (
            NativeTarget::linux_x64(),
            ["rdi", "rax", "rbx", "rax", "rax", "rbx", "rax"],
        ),
        (
            NativeTarget::linux_arm64(),
            ["x0", "x0", "x1", "x0", "x0", "x1", "x0"],
        ),
    ] {
        let legality = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        let choices = choose_spill_victims(
            legality.legality(),
            ranges.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
        )
        .unwrap();
        assert!(
            choices
                .plan()
                .functions
                .iter()
                .all(|function| function.choice.is_none())
        );
        let recovery = classify_pressure_recovery(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
        )
        .unwrap();
        assert!(
            recovery
                .plan()
                .functions
                .iter()
                .all(|function| function.classification.is_none())
        );
        assert_eq!(recovery.receipt().selected(), selected.custody().selected());
        assert_eq!(recovery.receipt().ranges(), ranges.custody().ranges());
        assert_eq!(recovery.receipt().legality(), legality.custody().legality());
        assert_eq!(
            recovery.receipt().spill_choices(),
            choices.receipt().identity()
        );
        let staged = stage_optimized_register_homes(legality).unwrap();
        let post = stage_optimized_post_allocation_machine_plan(&staged).unwrap();
        assert_eq!(
            post.machine().receipt().selected(),
            staged.custody().selected()
        );
        assert!(post.machine().plan().functions.iter().all(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .all(|instruction| instruction.alternative.key.variant == 0)
        }));
        let legality_stage = staged.legality_stage();
        let ranges_stage = legality_stage.live_range_stage();
        let liveness_stage = ranges_stage.liveness_stage();
        let liveness = &liveness_stage.liveness().plan().functions[0];
        for (block, registers) in liveness.blocks[1..].iter().zip([[1_u32, 2, 3], [4, 5, 6]]) {
            assert_eq!(block.instructions.len(), 4);
            assert_eq!(
                block.instructions[2].virtual_uses,
                registers[..2]
                    .iter()
                    .copied()
                    .map(VirtualRegisterId)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                block.instructions[2].virtual_defs,
                vec![VirtualRegisterId(registers[2])]
            );
            assert_eq!(
                block.instructions[2].virtual_live_out,
                vec![VirtualRegisterId(registers[2])]
            );
        }

        let ranges = &ranges_stage.ranges().plan().functions[0];
        assert_eq!(
            ranges
                .block_domains
                .iter()
                .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4), (1, 4, 12), (2, 12, 20)]
        );
        assert_eq!(
            ranges.interference,
            vec![
                VirtualInterference {
                    lower: VirtualRegisterId(1),
                    higher: VirtualRegisterId(2),
                },
                VirtualInterference {
                    lower: VirtualRegisterId(4),
                    higher: VirtualRegisterId(5),
                },
            ]
        );
        assert!(
            ranges
                .virtual_registers
                .iter()
                .all(|register| register.edge_connectors.is_empty())
        );
        assert_eq!(legality_stage.custody().entry_transition_count(), 0);

        let environment = liveness_stage.selected_stage().register_environment();
        let model = environment.physical().model();
        let homes = &staged.homes().plan().functions[0];
        assert_eq!(homes.assignments.len(), 7);
        assert_eq!(
            homes
                .assignments
                .iter()
                .map(|assignment| {
                    model
                        .views
                        .iter()
                        .find(|view| view.id == assignment.view)
                        .unwrap()
                        .name
                        .as_str()
                })
                .collect::<Vec<_>>(),
            expected_homes
        );
        assert_eq!(homes.assignments[1].view, homes.assignments[4].view);
        assert_eq!(homes.assignments[2].view, homes.assignments[5].view);
        assert_ne!(homes.assignments[1].view, homes.assignments[2].view);
    }
}

#[test]
fn active_resident_multi_use_rematerialization_reaches_fresh_homes_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = staged_active_resident_two_view_legality(target);
        assert_eq!(
            source
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .legalized()
                .plan()
                .functions[0]
                .recipe,
            LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1
        );
        let source_selected = source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .selected()
            .plan()
            .clone();
        let source_resident = source_selected.functions[0].blocks[1].instructions[0].clone();
        assert_eq!(source_resident.id.0, 2);
        assert!(matches!(
            source_resident.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(3)
            }
        ));

        let staged = stage_optimized_active_resident_rematerialization(
            source,
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        assert_eq!(
            validate_optimized_active_resident_rematerialization(&staged).unwrap(),
            staged.custody()
        );
        let choice = staged.choices().plan().functions[0]
            .choice
            .as_ref()
            .unwrap();
        assert_eq!(choice.incoming, VirtualRegisterId(3));
        assert_eq!(choice.selected_victim, VirtualRegisterId(1));
        let classification = staged.classifications().plan().functions[0]
            .classification
            .as_ref()
            .unwrap();
        assert_eq!(classification.victim, VirtualRegisterId(1));
        assert!(matches!(
            classification.role,
            RecoveryVictimRole::ActiveResident { .. }
        ));
        let RecoveryClassification::ImmediateU64RematerializationCandidate {
            defining_instruction,
            source_value,
            value,
            provenance,
            future_uses,
        } = &classification.classification
        else {
            panic!("active resident must retain literal eligibility")
        };
        assert_eq!(*defining_instruction, source_resident.id);
        assert_eq!(*source_value, ValueId::new(5_206).unwrap());
        assert_eq!(*value, IntegerValue::Unsigned(3));
        assert_eq!(provenance, &source_resident.provenance);
        assert_eq!(future_uses.len(), 2);

        let action = staged.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .unwrap();
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.original_materialize, source_resident.id);
        assert_eq!(action.rewrites.len(), 2);
        assert_eq!(
            staged.rematerialization().receipt().rewritten_use_count(),
            2
        );
        assert_eq!(staged.rematerialization().receipt().applied_count(), 1);
        let transformed = staged.rematerialization().transformed();
        assert_eq!(
            transformed.functions[0].blocks[1].instructions[0],
            source_resident
        );
        let fresh = transformed.functions[0].blocks[1]
            .instructions
            .iter()
            .find(|instruction| instruction.id == action.fresh_materialize)
            .unwrap();
        assert!(fresh.provenance.operations.is_empty());
        assert!(fresh.provenance.edges.is_empty());
        assert!(fresh.provenance.obligations.is_empty());
        assert!(fresh.provenance.fuel.is_empty());
        assert_eq!(fresh.provenance.values, vec![ValueId::new(5_206).unwrap()]);
        let rewritten_uses = transformed.functions[0].blocks[1]
            .instructions
            .iter()
            .flat_map(|instruction| &instruction.operands)
            .filter(|operand| operand.virtual_register == action.result_virtual_register)
            .count();
        assert_eq!(rewritten_uses, 3);
        assert_ne!(
            staged.liveness().receipt().identity(),
            staged.source().custody().liveness()
        );
        assert_ne!(
            staged.ranges().receipt().identity(),
            staged.source().custody().ranges()
        );
        assert_ne!(
            staged.legality().receipt().identity(),
            staged.source().custody().legality()
        );
        assert_eq!(staged.legality().receipt().entry_transition_count(), 0);
        assert_eq!(
            staged.homes().receipt().ranges(),
            staged.ranges().receipt().identity()
        );
        assert_eq!(
            staged.homes().receipt().legality(),
            staged.legality().receipt().identity()
        );
        assert_eq!(staged.homes().receipt().assignment_count(), 9);
        assert_eq!(
            staged
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            vec![
                PostAllocationSelectedTransformation::PressureRematerialization(
                    staged.rematerialization().receipt().identity()
                )
            ]
        );
        assert_eq!(
            staged.post_allocation_manifest().record().selected,
            staged.rematerialization().receipt().transformed_selected()
        );
    }
}

#[test]
fn active_resident_rematerialization_reaches_machine_custody_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(target),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let source_selected = source
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let transformed_selected = source.rematerialization().receipt().transformed_selected();

        let effects =
            stage_optimized_machine_effects_after_active_resident_rematerialization(&source)
                .unwrap();
        assert_eq!(effects.effects().receipt().selected(), transformed_selected);
        assert_eq!(
            effects.effects().plan().optimization_unit,
            source.custody().source().optimization_unit()
        );
        assert_eq!(
            effects.effects().plan().fuel_schedule,
            source.custody().source().fuel_schedule()
        );
        assert_eq!(effects.effects().plan().target, target);
        assert_eq!(
            effects.effects().receipt().register_environment(),
            source_selected.register_environment().identity()
        );
        assert_eq!(
            effects.custody().source(),
            &StagedOptimizedMachineEffectSourceCustodyReceipt::ActiveResidentRematerialization(
                source.custody()
            )
        );
        assert_eq!(
            &validate_optimized_machine_effect_custody_after_active_resident_rematerialization(
                &source,
                effects.effects(),
            )
            .unwrap(),
            effects.custody()
        );

        let post =
            stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
                &source,
            )
            .unwrap();
        assert_eq!(post.machine().receipt().selected(), transformed_selected);
        assert_eq!(
            post.machine().receipt().effects(),
            post.effects().effects().receipt().identity()
        );
        assert_eq!(
            post.machine().receipt().homes(),
            source.homes().receipt().identity()
        );
        assert_eq!(
            post.machine().receipt().post_allocation_manifest(),
            source.post_allocation_manifest().record().identity
        );
        assert_eq!(
            post.machine().receipt().register_environment(),
            source_selected.register_environment().identity()
        );
        assert_eq!(
            post.custody().source(),
            &StagedOptimizedPostAllocationMachineSourceCustodyReceipt::ActiveResidentRematerialization(
                source.custody()
            )
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
                &source,
                &post,
            )
            .unwrap(),
            post.custody()
        );

        assert_eq!(
            omega_machine_optimizer::validate_post_allocation_machine_plan(
                source_selected.selected(),
                post.effects().effects(),
                source.ranges(),
                source.legality(),
                source.homes(),
                source.post_allocation_manifest(),
                source_selected.register_environment().identity(),
                source_selected.register_environment().physical(),
                source_selected.register_environment().constraints(),
                post.machine().plan().clone(),
            ),
            Err(omega_machine_optimizer::PostAllocationMachineError::SelectedRootMismatch)
        );
    }

    let mut corrupted = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    crate::stages::machine::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
        &mut corrupted,
    );
    assert!(matches!(
        stage_optimized_machine_effects_after_active_resident_rematerialization(&corrupted),
        Err(
            OptimizedMachineEffectPipelineError::ActiveResidentRematerialization(
                OptimizedActiveResidentRematerializationError::ReceiptMismatch
            )
        )
    ));
    assert!(matches!(
        stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
            &corrupted,
        ),
        Err(
            OptimizedPostAllocationMachinePipelineError::ActiveResidentRematerialization(
                OptimizedActiveResidentRematerializationError::ReceiptMismatch
            )
        )
    ));

    let x86 = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let arm = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(NativeTarget::linux_arm64()),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let x86_post =
        stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(&x86)
            .unwrap();
    assert!(
        validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
            &arm,
            &x86_post,
        )
        .is_err()
    );
}

#[test]
fn active_resident_rematerialization_reaches_layout_independent_encoding_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
        let transformed_selected = source.rematerialization().receipt().transformed_selected();
        let machine_root = machine.machine().receipt().identity();
        let machine_row_count = machine.custody().instruction_count();
        let rematerialization = source.custody();
        let fresh_materialize = source.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .unwrap()
            .fresh_materialize;

        let staged = stage_optimized_active_resident_rematerialization_selected_form_encoding(
            source, machine,
        )
        .unwrap();
        assert_eq!(staged.encoding().selected(), transformed_selected);
        assert_eq!(staged.encoding().machine(), machine_root);
        assert_eq!(staged.custody().rematerialization(), rematerialization);
        assert_eq!(staged.custody().machine(), staged.machine().custody());
        assert_eq!(
            staged.custody().transformed_selected(),
            transformed_selected
        );
        assert_eq!(staged.custody().encoding(), staged.encoding().identity());
        assert_eq!(staged.custody().row_count(), machine_row_count);
        assert_eq!(
            staged.custody().encoded_count() + staged.custody().deferred_count(),
            machine_row_count
        );
        assert_eq!(staged.custody().deferred_count(), 1);
        assert!(staged.encoding().rows().iter().all(|row| match &row.state {
            SelectedFormEncodingState::Encoded { bytes, .. } => !bytes.is_empty(),
            SelectedFormEncodingState::DeferredControl { .. } => true,
        }));
        let fresh_row = staged
            .encoding()
            .rows()
            .iter()
            .find(|row| row.instruction == fresh_materialize)
            .expect("fresh rematerialization must reach the encoder roster");
        assert_eq!(
            fresh_row.alternative.family,
            omega_selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(matches!(
            &fresh_row.state,
            SelectedFormEncodingState::Encoded { bytes, .. } if !bytes.is_empty()
        ));
        assert_eq!(
            validate_optimized_active_resident_rematerialization_selected_form_encoding(&staged,)
                .unwrap(),
            staged.custody().clone()
        );
    }
}

#[test]
fn active_resident_rematerialization_encoding_rejects_detached_or_corrupt_custody() {
    let (mut corrupt_source, machine) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
    crate::stages::machine::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
        &mut corrupt_source,
    );
    assert!(matches!(
        stage_optimized_active_resident_rematerialization_selected_form_encoding(
            corrupt_source,
            machine,
        ),
        Err(
            OptimizedActiveResidentRematerializationSelectedFormEncodingError::Rematerialization(
                OptimizedActiveResidentRematerializationError::ReceiptMismatch
            )
        )
    ));

    let (x86_source, _) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
    let (_, arm_machine) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_arm64());
    assert!(matches!(
        stage_optimized_active_resident_rematerialization_selected_form_encoding(
            x86_source,
            arm_machine,
        ),
        Err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Machine(_))
    ));

    let (source, machine) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
    let mut staged =
        stage_optimized_active_resident_rematerialization_selected_form_encoding(source, machine)
            .unwrap();
    crate::stages::encoding::active_resident_selected_form_encoding::corrupt_active_resident_selected_form_encoding_byte_for_test(
        &mut staged,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_selected_form_encoding(&staged),
        Err(
            OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding(
                OptimizedSelectedFormEncodingError::ArtifactMismatch
            )
        )
    );
}

#[test]
fn active_resident_rematerialization_reaches_resolved_layout_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
        let fresh_materialize = source.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .unwrap()
            .fresh_materialize;
        let physical = source
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
            .physical()
            .identity();
        let pre_layout = stage_optimized_active_resident_rematerialization_selected_form_encoding(
            source, machine,
        )
        .unwrap();
        let pre_layout_custody = pre_layout.custody().clone();
        let selected = pre_layout.encoding().selected();
        let machine = pre_layout.encoding().machine();
        let pre_layout_encoding = pre_layout.encoding().identity();

        let staged =
            stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                pre_layout,
            )
            .unwrap();
        let layout = staged.layout();
        let custody = staged.custody();
        assert_eq!(custody.pre_layout_custody(), &pre_layout_custody);
        assert_eq!(custody.selected(), selected);
        assert_eq!(custody.machine(), machine);
        assert_eq!(custody.pre_layout(), pre_layout_encoding);
        assert_eq!(custody.physical(), physical);
        assert_eq!(custody.layout(), layout.identity());
        assert_eq!(custody.target(), target);
        assert_eq!(
            custody.policy(),
            SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
        );
        assert_eq!(custody.function_count(), 1);
        assert_eq!(custody.block_count(), 3);
        assert_eq!(
            custody.instruction_count(),
            custody.pre_layout_custody().row_count()
        );
        assert_eq!(
            custody.instruction_count(),
            layout
                .functions()
                .iter()
                .flat_map(|function| &function.blocks)
                .map(|block| block.instructions.len())
                .sum::<usize>()
        );
        assert_eq!(
            custody.byte_count(),
            layout
                .functions()
                .iter()
                .map(|function| function.byte_count)
                .sum::<u64>()
        );
        assert_eq!(custody.resolved_branch_count(), 1);
        let rows = layout
            .functions()
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let fresh_row = rows
            .iter()
            .find(|row| row.instruction == fresh_materialize)
            .expect("fresh rematerialization must survive resolved layout");
        assert_eq!(
            fresh_row.alternative.family,
            omega_selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(!fresh_row.bytes.is_empty());
        assert_eq!(
            rows.iter().filter(|row| row.branch.is_some()).count(),
            custody.resolved_branch_count()
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                &staged,
            )
            .unwrap(),
            custody.clone()
        );
    }
}

#[test]
fn active_resident_resolved_layout_rejects_pre_layout_layout_and_receipt_mutation() {
    let mut corrupt_pre_layout = staged_active_resident_resolved_layout(NativeTarget::linux_x64());
    crate::stages::layout::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_pre_layout_byte_for_test(
        &mut corrupt_pre_layout,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
            &corrupt_pre_layout,
        ),
        Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::PreLayout(
                OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding(
                    OptimizedSelectedFormEncodingError::ArtifactMismatch,
                ),
            ),
        )
    );

    let mut corrupt_layout = staged_active_resident_resolved_layout(NativeTarget::linux_x64());
    crate::stages::layout::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_byte_for_test(
        &mut corrupt_layout,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
            &corrupt_layout,
        ),
        Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout(
                OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch,
            ),
        )
    );

    let mut corrupt_receipt = staged_active_resident_resolved_layout(NativeTarget::linux_x64());
    crate::stages::layout::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_receipt_for_test(
        &mut corrupt_receipt,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
            &corrupt_receipt,
        ),
        Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
        )
    );
}

#[test]
fn active_resident_rematerialization_reaches_function_relative_exit_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_active_resident_function_relative_realization(target);
        let source = staged.source();
        let rematerialization = source.pre_layout().source();
        let physical = rematerialization
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
            .physical();
        let admitted_names = match target.architecture {
            omega_target::Architecture::X86_64 => ["rax", "rcx"],
            omega_target::Architecture::Aarch64 => ["x0", "x1"],
        };
        let admitted_views = admitted_names
            .into_iter()
            .map(|name| physical.model().view_named(name).unwrap().id)
            .collect::<BTreeSet<_>>();
        let AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } =
            &rematerialization
                .source()
                .allocator_availability()
                .plan()
                .policy
        else {
            panic!("pressure fixture must retain an explicit caller-saved allowlist")
        };
        assert_eq!(
            views.iter().copied().collect::<BTreeSet<_>>(),
            admitted_views
        );
        let action = rematerialization.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .expect("the explicit active-resident staging route must rematerialize");
        let fresh = action.fresh_materialize;
        let transformed_selected = rematerialization
            .rematerialization()
            .receipt()
            .transformed_selected();
        let fresh_layout_row = source
            .layout()
            .functions()
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|instruction| instruction.instruction == fresh)
            .expect("fresh rematerialization must survive function-relative layout");
        assert_eq!(
            fresh_layout_row.alternative.family,
            omega_selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(!fresh_layout_row.bytes.is_empty());

        let manifest = staged.manifest().record();
        let empty = OptimizationSelections::default().identity();
        assert_eq!(
            manifest.selections,
            OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap()
            .identity()
        );
        assert_eq!(manifest.selected_lowering_selections, empty);
        assert_eq!(manifest.selected_lowering_completion, None);
        assert_eq!(manifest.allocation_recovery_selections, manifest.selections);
        assert_eq!(manifest.post_allocation_machine_selections, empty);
        assert_eq!(manifest.function_relative_layout_selections, empty);
        assert_eq!(
            manifest.pre_physical_manifest,
            rematerialization.custody().source().manifest()
        );
        assert_eq!(
            manifest.post_allocation_manifest,
            rematerialization
                .post_allocation_manifest()
                .record()
                .identity
        );
        assert_eq!(manifest.selected, transformed_selected);
        assert_eq!(manifest.baseline_pre_layout, manifest.pre_layout);
        assert_eq!(manifest.baseline_resolved_layout, manifest.resolved_layout);
        assert_eq!(manifest.x86_branch_relaxation, None);
        assert_eq!(manifest.post_allocation_machine_optimization, None);
        assert_eq!(manifest.target, target);
        assert_eq!(
            rematerialization
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    rematerialization.rematerialization().receipt().identity(),
                )
            ]
        );
        assert_eq!(
            staged.exit_contract().contract().selected,
            transformed_selected
        );
        assert_eq!(
            staged.exit_contract().contract().resolved_layout,
            source.layout().identity()
        );
        assert!(matches!(
            staged.exit_contract().contract().layout_custody,
            WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        ));
        assert!(
            staged
                .exit_contract()
                .contract()
                .functions
                .iter()
                .all(|function| function.modified_callee_saved_units.is_empty())
        );
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&manifest.encode()),
            Ok(manifest.clone())
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_function_relative_realization(
                &staged,
            )
            .unwrap(),
            staged.custody().clone()
        );
        assert_eq!(staged.custody().source(), source.custody());
        assert_eq!(
            staged.custody().exit_contract(),
            staged.exit_contract().identity()
        );
        assert_eq!(staged.custody().realization(), manifest.identity);
    }
}

#[test]
fn active_resident_function_relative_realization_rejects_corrupt_or_detached_custody() {
    let target = NativeTarget::linux_x64();

    let mut source_corruption = staged_active_resident_function_relative_realization(target);
    crate::stages::realization::active_resident_function_relative_realization::corrupt_active_resident_function_relative_source_for_test(
        &mut source_corruption,
    );
    assert!(matches!(
        validate_optimized_active_resident_rematerialization_function_relative_realization(
            &source_corruption,
        ),
        Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::Source(
                OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout(
                    OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch,
                ),
            ),
        )
    ));

    let mut exit_corruption = staged_active_resident_function_relative_realization(target);
    crate::stages::realization::active_resident_function_relative_realization::corrupt_active_resident_function_relative_exit_for_test(
        &mut exit_corruption,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_function_relative_realization(
            &exit_corruption,
        ),
        Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            ),
        )
    );

    let mut manifest_corruption = staged_active_resident_function_relative_realization(target);
    crate::stages::realization::active_resident_function_relative_realization::corrupt_active_resident_function_relative_manifest_for_test(
        &mut manifest_corruption,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_function_relative_realization(
            &manifest_corruption,
        ),
        Err(OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::RootMismatch,)
    );

    let mut receipt_corruption = staged_active_resident_function_relative_realization(target);
    crate::stages::realization::active_resident_function_relative_realization::corrupt_active_resident_function_relative_receipt_for_test(
        &mut receipt_corruption,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_function_relative_realization(
            &receipt_corruption,
        ),
        Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ReceiptMismatch,
        )
    );

    let mut detached = staged_active_resident_function_relative_realization(target);
    let foreign = staged_active_resident_function_relative_realization(NativeTarget::linux_arm64());
    crate::stages::realization::active_resident_function_relative_realization::replace_active_resident_function_relative_exit_for_test(
        &mut detached,
        &foreign,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_function_relative_realization(
            &detached,
        ),
        Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            ),
        )
    );
}

#[test]
fn active_resident_function_relative_realization_rejects_unexecuted_later_phase_selections() {
    for later in [
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
    ] {
        let selections = OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            later,
        ])
        .unwrap();
        let source = staged_active_resident_resolved_layout_with_selections(
            NativeTarget::linux_x64(),
            selections,
        );
        assert!(matches!(
            stage_optimized_active_resident_rematerialization_function_relative_realization(
                source,
            ),
            Err(
                OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::LaterPhaseSelected,
            )
        ));
    }
}

#[test]
fn active_resident_stage_declines_default_single_use_and_exhausted_budget() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let default = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_active_resident_exact_add_chain(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_active_resident_rematerialization(
                default,
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                selected_lowering_budget(),
            ),
            Err(OptimizedActiveResidentRematerializationError::Rematerialization(
                PressureRematerializationError::NoAction
            ))
        ));
        assert!(matches!(
            stage_optimized_active_resident_rematerialization(
                staged_active_resident_two_view_legality(target),
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
                selected_lowering_budget(),
            ),
            Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy)
        ));
        assert!(matches!(
            stage_optimized_active_resident_rematerialization(
                staged_active_resident_two_view_legality(target),
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
            ),
            Err(OptimizedActiveResidentRematerializationError::SpillChoice(
                omega_regalloc::SpillChoiceError::BudgetExceeded { .. }
            ))
        ));
    }
}

#[test]
fn active_resident_stage_rejects_corrupted_vertical_custody() {
    let mut staged = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    crate::stages::machine::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
        &mut staged,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization(&staged),
        Err(OptimizedActiveResidentRematerializationError::ReceiptMismatch)
    );
}
