use super::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, LegalizationError, MachineId,
    NativeTarget, ObligationId, OperationId, Optimization, OptimizationSelections,
    RecoveryClassificationPolicy, RegisterOperandAccess, RegisterUnitId, ScalarType,
    SelectedInstructionError, SelectedInstructionKind, SelectedTerminator, SpillChoicePolicy,
    ValueBinding, ValueDefinitionSite, ValueId, budget, selected_instruction_plan_identity,
    selected_lowering_budget, stage_next_optimized_literal_fold,
    stage_optimized_allocation_legality, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_register_homes, staged_conditional, staged_exact_add_conditional,
    staged_single_block_exact_add_fold_legality,
    staged_widened_u8_exact_add_conditional_with_selections, validate_legalized_operations,
    validate_optimized_literal_fold_custody, validate_raw_selection,
    validate_selected_lowering_optimization_custody,
};
use crate::{
    LiteralFoldPolicy, OptimizedLiteralFoldCustodyError, OptimizedLiteralFoldCustodyFieldForTest,
    OptimizedSelectionCustodyError, SelectedLoweringOptimizationCustodyFieldForTest,
    run_selected_lowering_optimizations, stage_first_optimized_literal_fold,
    validate_optimized_selection_custody,
};
#[test]
fn selected_lowering_runner_rejects_a_psi_only_source_suite() {
    let legality = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        run_selected_lowering_optimizations(legality),
        Err(OptimizedLiteralFoldCustodyError::MissingSelectedLoweringOptimization)
    ));
}

#[test]
fn literal_fold_staging_rejects_an_explicit_no_action_request() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let legality = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_first_optimized_literal_fold(
                legality,
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                LiteralFoldPolicy::EXACT_ADD_V1,
                budget(),
            ),
            Err(OptimizedLiteralFoldCustodyError::NoAppliedFold)
        ));
    }
}

#[test]
fn selected_cfg_validator_rejects_target_state_path_and_value_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_conditional(target);

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| !instruction.implicit_defs.is_empty())
            .expect("branch comparison sets flags")
            .implicit_defs
            .clear();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::ConstraintEffectMismatch { .. })
                | Err(SelectedInstructionError::FunctionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &mut corrupted.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        std::mem::swap(when_nonzero, when_zero);
        assert_eq!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SourceCustodyMismatch)
        );

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].virtual_registers[0].entry_fixed_view = None;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::VirtualRegisterProjectionMismatch { .. })
                | Err(SelectedInstructionError::FunctionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::Return { instruction, .. } =
            &mut corrupted.functions[0].blocks[1].terminator
        else {
            unreachable!()
        };
        instruction.operands[0].fixed_view = None;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
                | Err(SelectedInstructionError::FunctionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[0].operands[0].tied_to = Some(0);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
                | Err(SelectedInstructionError::FunctionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[0].operands[0].early_clobber = true;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
                | Err(SelectedInstructionError::FunctionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::Return { instruction, .. } =
            &mut corrupted.functions[0].blocks[1].terminator
        else {
            unreachable!()
        };
        instruction.operands[0].virtual_register = selected_instructions::VirtualRegisterId(2);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::UseBeforeDefinition { .. })
                | Err(SelectedInstructionError::FunctionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[0].kind =
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(11),
            };
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::FunctionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[0]
            .provenance
            .values[0] = ValueId::new(8_001).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::FunctionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_nonzero.psi_edge = EdgeId::new(8_002).unwrap();
        assert_eq!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SourceCustodyMismatch)
        );

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { when_zero, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_zero.fuel[0].units += 1;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SourceCustodyMismatch)
                | Err(SelectedInstructionError::ProvenancePartitionMismatch { .. })
        ));
    }
}

#[test]
fn selected_content_identity_binds_every_retained_field_class() {
    let staged = staged_conditional(NativeTarget::linux_x64());
    let original = staged.selected().plan();
    let identity = selected_instruction_plan_identity(original);
    let mut mutations = Vec::new();

    let mut changed = original.clone();
    changed.target = NativeTarget::windows_x64();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.entry = MachineId::new(8_009).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].machine = MachineId::new(8_018).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].attachment =
        Some(semantic_vocabulary::StructuralTypeId::new(8_010).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0]
        .provenance
        .operations
        .push(OperationId::new(8_011).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0]
        .provenance
        .edges
        .push(EdgeId::new(8_019).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].entry_block.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].id.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].class.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].origin =
        selected_instructions::VirtualRegisterOrigin::InstructionResult {
            instruction: selected_instructions::SelectedInstructionId(4),
            source_value: ValueId::new(8_012).unwrap(),
        };
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].definition_site = Some(ValueDefinitionSite::Node {
        block: BlockId::new(8_013).unwrap(),
        node: 7,
    });
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[0].entry_fixed_view = None;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].id.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].origin =
        selected_instructions::SelectedBlockOrigin::Source(BlockId::new(8_020).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions.clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0].id.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .constraint
        .variant += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0].operands[0].access = RegisterOperandAccess::Use;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0].operands[0].tied_to = Some(0);
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0].operands[0].early_clobber = true;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .implicit_uses
        .push(RegisterUnitId(999));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .instructions
        .iter_mut()
        .find(|instruction| !instruction.implicit_defs.is_empty())
        .expect("branch comparison sets flags")
        .implicit_defs
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .clobbers
        .push(RegisterUnitId(998));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .provenance
        .operations
        .push(OperationId::new(8_021).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .provenance
        .values
        .push(ValueId::new(8_022).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .provenance
        .edges
        .push(EdgeId::new(8_023).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .provenance
        .obligations
        .push(ObligationId::new(8_014).unwrap());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .provenance
        .fuel[0]
        .units += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
        &mut changed.functions[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_nonzero
        .bindings
        .push(selected_instructions::SelectedValueBinding {
            semantic: ValueBinding {
                parameter: ValueId::new(8_015).unwrap(),
                argument: ValueId::new(8_016).unwrap(),
                scalar_type: ScalarType::Boolean,
            },
            transport: selected_instructions::SelectedValueTransport::Unused,
        });
    mutations.push(changed);
    let mut changed = original.clone();
    let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
        &mut changed.functions[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_nonzero.source_target = BlockId::new(8_024).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    let SelectedTerminator::ConditionalBranch { when_zero, .. } =
        &mut changed.functions[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_zero.fuel[0].units += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    let SelectedTerminator::Return {
        psi_return_edge, ..
    } = &mut changed.functions[0].blocks[1].terminator
    else {
        unreachable!()
    };
    *psi_return_edge = EdgeId::new(8_017).unwrap();
    mutations.push(changed);

    for (position, mutation) in mutations.into_iter().enumerate() {
        assert_ne!(
            &mutation, original,
            "mutation {position} must change a retained field"
        );
        assert_ne!(
            selected_instruction_plan_identity(&mutation),
            identity,
            "mutation {position}"
        );
    }
}

#[test]
fn staged_selection_custody_rejects_detached_environment_and_selected_plan() {
    let x86 = staged_conditional(NativeTarget::linux_x64());
    let arm = staged_conditional(NativeTarget::linux_arm64());
    assert_eq!(
        validate_optimized_selection_custody(
            x86.optimized_target(),
            arm.register_environment(),
            x86.legalized(),
            x86.selected(),
        ),
        Err(OptimizedSelectionCustodyError::RegisterEnvironmentTargetMismatch)
    );
    assert_eq!(
        validate_optimized_selection_custody(
            x86.optimized_target(),
            x86.register_environment(),
            x86.legalized(),
            arm.selected(),
        ),
        Err(OptimizedSelectionCustodyError::RootMismatch)
    );

    let mut target = x86.optimized_target().target_operations().clone();
    let forged_operation = OperationId::new(8_030).unwrap();
    target.functions[0]
        .provenance
        .operations
        .push(forged_operation);
    // Ordinary graph correspondence rejects the forged provenance as a custody
    // disagreement between the raw target and its abstract and optimized
    // owners. The structural replay fallback that once reported it as a
    // noncanonical proposal was removed with the whole-function forks.
    assert_eq!(
        validate_legalized_operations(
            &target,
            x86.optimized_target().optimized().plan(),
            x86.optimized_target().optimized().unit(),
            x86.legalized().plan().clone(),
        ),
        Err(LegalizationError::SourceCustodyMismatch)
    );

    let mut unit = x86.optimized_target().optimized().unit().clone();
    unit.functions[0].blocks[0].nodes[0].effect.output += 1_000;
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    assert_eq!(
        validate_legalized_operations(
            x86.optimized_target().target_operations(),
            x86.optimized_target().optimized().plan(),
            &unit,
            x86.legalized().plan().clone(),
        ),
        Err(LegalizationError::SourceCustodyMismatch)
    );
}

#[test]
fn physical_stage_receipts_retain_the_pre_physical_manifest_identity() {
    let selected = staged_conditional(NativeTarget::linux_x64());
    let manifest = selected
        .optimized_target()
        .optimized()
        .pre_physical_manifest()
        .record()
        .identity;
    assert_eq!(selected.custody().manifest(), manifest);

    let liveness = stage_optimized_liveness(selected).unwrap();
    assert_eq!(liveness.custody().manifest(), manifest);
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    assert_eq!(ranges.custody().manifest(), manifest);
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    assert_eq!(legality.custody().manifest(), manifest);
    let homes = stage_optimized_register_homes(legality).unwrap();
    assert_eq!(homes.custody().manifest(), manifest);
}

#[test]
fn literal_fold_custody_rejects_every_one_field_substitution() {
    use OptimizedLiteralFoldCustodyFieldForTest::*;
    let fields: [(&str, OptimizedLiteralFoldCustodyFieldForTest); 9] = [
        ("source", Source),
        ("iterations", Iterations),
        ("transformations", Transformations),
        ("final_selected", FinalSelected),
        ("final_liveness", FinalLiveness),
        ("final_ranges", FinalRanges),
        ("final_legality", FinalLegality),
        ("final_virtual_register_count", FinalVirtualRegisterCount),
        ("final_entry_transition_count", FinalEntryTransitionCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let folds = stage_first_optimized_literal_fold(
                staged_single_block_exact_add_fold_legality(target),
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                LiteralFoldPolicy::EXACT_ADD_V1,
                selected_lowering_budget(),
            )
            .unwrap();
            // The second step folds the other arm's stranded literal, after
            // which every range seats in the single allowlisted view.
            stage_next_optimized_literal_fold(
                folds,
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                LiteralFoldPolicy::EXACT_ADD_V1,
                selected_lowering_budget(),
            )
            .unwrap()
        };
        // An authentic foreign fold sequence on the opposite architecture is
        // the donor for the nested source receipt and iteration evidence.
        let donor = build(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            _ => NativeTarget::linux_x64(),
        });
        assert_ne!(
            build(target).custody(),
            donor.custody(),
            "{target:?}: the foreign target must produce a distinct custody receipt",
        );
        for (name, field) in fields {
            let mut substituted = build(target);
            let honest = substituted.custody().clone();
            substituted.corrupt_custody_for_test(field, &donor);
            assert_ne!(
                substituted.custody(),
                &honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            assert_eq!(
                validate_optimized_literal_fold_custody(&substituted),
                Err(OptimizedLiteralFoldCustodyError::StepMismatch { step: 0 }),
                "{target:?}: independent replay must reject substituted literal-fold custody field {name}",
            );
        }
    }
}

#[test]
fn selected_lowering_optimization_custody_rejects_every_one_field_substitution() {
    use OptimizedLiteralFoldCustodyError::{SelectionProjectionMismatch, StepMismatch};
    use SelectedLoweringOptimizationCustodyFieldForTest::*;
    let fields: [(
        &str,
        SelectedLoweringOptimizationCustodyFieldForTest,
        OptimizedLiteralFoldCustodyError,
    ); 16] = [
        ("identity", Identity, StepMismatch { step: 0 }),
        ("source", Source, StepMismatch { step: 0 }),
        // The retained projection facts are checked against the staged source
        // before the whole-receipt comparison runs, so they surface through
        // the projection arm instead of `StepMismatch`.
        ("selections", Selections, SelectionProjectionMismatch),
        (
            "selected_lowering_selections",
            SelectedLoweringSelections,
            StepMismatch { step: 0 },
        ),
        ("budget", Budget, SelectionProjectionMismatch),
        ("usage", Usage, StepMismatch { step: 0 }),
        ("iteration_bound", IterationBound, StepMismatch { step: 0 }),
        ("action_count", ActionCount, StepMismatch { step: 0 }),
        (
            "initial_virtual_register_count",
            InitialVirtualRegisterCount,
            StepMismatch { step: 0 },
        ),
        ("iterations", Iterations, StepMismatch { step: 0 }),
        ("attempt", Attempt, StepMismatch { step: 0 }),
        ("final_selected", FinalSelected, StepMismatch { step: 0 }),
        ("final_liveness", FinalLiveness, StepMismatch { step: 0 }),
        ("final_ranges", FinalRanges, StepMismatch { step: 0 }),
        ("final_legality", FinalLegality, StepMismatch { step: 0 }),
        (
            "final_virtual_register_count",
            FinalVirtualRegisterCount,
            StepMismatch { step: 0 },
        ),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let legality = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(
                        staged_widened_u8_exact_add_conditional_with_selections(
                            target,
                            OptimizationSelections::new([
                                Optimization::CopyPropagation,
                                Optimization::SelectedIncomingU12ExactAddImmediate,
                            ])
                            .unwrap(),
                        ),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            run_selected_lowering_optimizations(legality).unwrap()
        };
        // An authentic foreign run on the opposite architecture is the donor
        // for the nested source, iteration, and attempt receipts.
        let donor = build(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            _ => NativeTarget::linux_x64(),
        });
        assert_ne!(
            build(target).custody(),
            donor.custody(),
            "{target:?}: the foreign target must produce a distinct custody receipt",
        );
        for &(name, field, ref expected) in &fields {
            let mut substituted = build(target);
            let honest = substituted.custody().clone();
            substituted.corrupt_custody_for_test(field, &donor);
            assert_ne!(
                substituted.custody(),
                &honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            assert_eq!(
                validate_selected_lowering_optimization_custody(&substituted),
                Err(expected.clone()),
                "{target:?}: independent replay must reject substituted selected-lowering custody field {name}",
            );
        }
    }
}
