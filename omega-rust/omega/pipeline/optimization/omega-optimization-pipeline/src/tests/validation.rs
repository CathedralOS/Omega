use super::*;

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
        corrupted.functions[0].blocks[0].instructions[0]
            .implicit_defs
            .clear();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::ConstraintEffectMismatch { .. })
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
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].virtual_registers[0].entry_fixed_view = None;
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
        instruction.operands[0].fixed_view = None;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[0].operands[0].tied_to = Some(0);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[0].operands[0].early_clobber = true;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::Return { instruction, .. } =
            &mut corrupted.functions[0].blocks[1].terminator
        else {
            unreachable!()
        };
        instruction.operands[0].virtual_register =
            omega_selected_instructions::VirtualRegisterId(2);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::UseBeforeDefinition { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[0].kind =
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(11),
            };
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[0]
            .provenance
            .values[0] = ValueId::new(8_001).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_nonzero.psi_edge = EdgeId::new(8_002).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { when_zero, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_zero.fuel[0].units += 1;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
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
    changed.functions[0].attachment = Some(psi_core::StructuralTypeId::new(8_010).unwrap());
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
    changed.functions[0].virtual_registers[1].scalar_type = ScalarType::Boolean;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].id.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].class.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].origin =
        omega_selected_instructions::VirtualRegisterOrigin::InstructionResult {
            instruction: omega_selected_instructions::SelectedInstructionId(4),
            source_value: ValueId::new(8_012).unwrap(),
        };
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[1].definition_site = ValueDefinitionSite::Node {
        block: BlockId::new(8_013).unwrap(),
        node: 7,
    };
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].virtual_registers[0].entry_fixed_view = None;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].id.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].source_block = BlockId::new(8_020).unwrap();
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
    changed.functions[0].blocks[0].instructions[0]
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
    when_nonzero.bindings.push(ValueBinding {
        parameter: ValueId::new(8_015).unwrap(),
        argument: ValueId::new(8_016).unwrap(),
        scalar_type: ScalarType::Boolean,
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

    for mutation in mutations {
        assert_ne!(selected_instruction_plan_identity(&mutation), identity);
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
    unit.identity = omega_optimization_unit::recompute_psi_optimization_unit_identity(&unit);
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
