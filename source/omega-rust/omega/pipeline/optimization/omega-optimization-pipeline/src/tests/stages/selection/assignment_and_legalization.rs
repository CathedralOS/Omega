use crate::tests::*;

#[test]
fn staged_assignment_is_deterministic_and_retains_optimizer_custody() {
    let (semantic, proof) = artifact();
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let stage = || {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections.clone()),
        )
        .unwrap();
        let target =
            lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
        stage_optimized_assignment(target).unwrap()
    };
    let first = stage();
    let second = stage();

    assert_eq!(first.assigned(), second.assigned());
    assert_eq!(first.custody(), second.custody());
    assert_eq!(
        first.optimized_target().optimized().transformation_ledger(),
        second
            .optimized_target()
            .optimized()
            .transformation_ledger()
    );
    assert_eq!(
        first.optimized_target().optimized().pass_manifests(),
        second.optimized_target().optimized().pass_manifests()
    );
}

#[test]
fn independent_assignment_custody_rejects_each_root_and_provenance_corruption() {
    let (semantic, proof) = artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(
            OptimizationSelections::new([
                Optimization::SparseConditionalConstantPropagation,
                Optimization::CopyPropagation,
            ])
            .unwrap(),
        ),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    let staged = stage_optimized_assignment(target).unwrap();

    let wrong_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            &wrong_environment,
            staged.assigned(),
        ),
        Err(OptimizedAssignmentCustodyError::RegisterEnvironmentTargetMismatch)
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.psi.program_fingerprint = psi_terminal::SemanticFingerprint::from_bytes([0x44; 32]);
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::TerminalPsiMismatch)
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.target = NativeTarget::windows_x64();
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::NativeTargetMismatch)
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.entry = MachineId::new(9_001).unwrap();
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::EntryMismatch)
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.functions.push(corrupted.functions[0].clone());
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::FunctionCountMismatch {
            expected: 1,
            actual: 2,
        })
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.functions[0].machine = MachineId::new(9_002).unwrap();
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::FunctionMachineMismatch { position: 0 })
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.functions[0].attachment = Some(psi_core::StructuralTypeId::new(9_003).unwrap());
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::FunctionAttachmentMismatch { position: 0 })
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.functions[0]
        .provenance
        .operations
        .push(OperationId::new(9_004).unwrap());
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::FunctionProvenanceMismatch { position: 0 })
    );
}
#[test]
fn verified_three_block_conditional_selects_typed_vregs_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_conditional(target);
        let plan = staged.selected().plan();
        assert_eq!(plan.functions.len(), 1);
        assert_eq!(plan.functions[0].blocks.len(), 3);
        assert_eq!(plan.functions[0].virtual_registers.len(), 3);
        assert_eq!(staged.selected().receipt().instruction_count(), 6);
        assert_eq!(
            staged.custody().optimization_unit(),
            staged.optimized_target().optimized().unit().identity
        );
        assert_eq!(staged.custody().fuel_schedule(), plan.fuel_schedule);
        assert_eq!(staged.legalized().receipt().target(), target);
        assert_eq!(staged.legalized().receipt().function_count(), 1);
        assert_eq!(staged.legalized().receipt().decomposition_count(), 0);
        assert_eq!(
            staged.custody().legalized(),
            staged.legalized().receipt().identity()
        );
        assert_eq!(
            staged.selected().receipt().legalized(),
            staged.legalized().receipt().identity()
        );
        assert_eq!(
            staged.custody().register_environment(),
            staged.register_environment().identity()
        );
        assert_eq!(
            staged.custody().selected(),
            staged.selected().receipt().identity()
        );
        let mut copy_tagged = plan.clone();
        copy_tagged.functions[0].blocks[1].instructions[0].kind = SelectedInstructionKind::CopyI64;
        assert_ne!(
            selected_instruction_plan_identity(&copy_tagged),
            staged.selected().receipt().identity()
        );

        let entry = &plan.functions[0].blocks[0];
        assert_eq!(
            entry.instructions[0].kind,
            SelectedInstructionKind::CompareI64Zero
        );
        assert!(entry.instructions[0].provenance.fuel.is_empty());
        let SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } = &entry.terminator
        else {
            panic!("entry must branch")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchNonZero
        );
        assert!(instruction.provenance.fuel.is_empty());
        assert_eq!(when_nonzero.fuel.len(), 1);
        assert_eq!(when_zero.fuel.len(), 1);
        assert_ne!(when_nonzero.psi_edge, when_zero.psi_edge);
        for block in &plan.functions[0].blocks[1..] {
            assert!(matches!(
                block.instructions[0].kind,
                SelectedInstructionKind::MaterializeI64 { .. }
            ));
            assert_eq!(block.instructions[0].provenance.operations.len(), 1);
            assert_eq!(block.instructions[0].provenance.fuel.len(), 1);
            let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
                panic!("leaf must return")
            };
            assert!(instruction.operands[0].fixed_view.is_some());
            assert_eq!(instruction.provenance.fuel.len(), 1);
        }
    }
}

#[test]
fn legalization_identity_and_replay_reject_target_recipe_provenance_and_fuel_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_conditional(target);
        let original = staged.legalized().plan();
        let identity = legalized_operation_plan_identity(original);
        assert_eq!(identity, staged.legalized().receipt().identity());
        assert_eq!(
            staged.legalized().receipt().validator(),
            legalization_validator_identity()
        );
        assert_eq!(
            staged.selected().receipt().legalization_validator(),
            legalization_validator_identity()
        );
        assert_eq!(
            staged.custody().legalization_validator(),
            legalization_validator_identity()
        );
        assert_eq!(
            identity,
            staged_conditional(target).legalized().receipt().identity()
        );
        assert_eq!(
            original.functions[0].recipe,
            LegalizationRecipe::ReturnU64ImmediateConditionalV1
        );

        let validate = |plan| {
            validate_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };

        let mut corrupted = original.clone();
        corrupted.target = if target.architecture == omega_target::Architecture::X86_64 {
            NativeTarget::linux_arm64()
        } else {
            NativeTarget::linux_x64()
        };
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].recipe = LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1;
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0]
            .provenance
            .operations
            .push(OperationId::new(9_601).unwrap());
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].branch_true_fuel[0].units += 1;
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].condition_definition_site = ValueDefinitionSite::Node {
            block: corrupted.functions[0].entry_block,
            node: 0,
        };
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].branch_true_fuel[0].site =
            omega_optimization_unit::PsiProvenance::Edge(corrupted.functions[0].branch_false_edge);
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].provenance.edges.swap(0, 1);
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert!(matches!(
            validate(corrupted),
            Err(LegalizationError::SourceCustodyMismatch)
                | Err(LegalizationError::NonCanonicalLegalizedPlan)
        ));
    }
}

#[test]
fn widened_u8_exact_add_legalization_retains_theorem_temporaries_and_exact_custody() {
    let u8_integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u64_integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let expected_function_operations = vec![
        OperationId::new(5_121).unwrap(),
        OperationId::new(5_122).unwrap(),
        OperationId::new(5_123).unwrap(),
        OperationId::new(5_124).unwrap(),
        OperationId::new(5_125).unwrap(),
        OperationId::new(5_126).unwrap(),
        OperationId::new(5_127).unwrap(),
        OperationId::new(5_128).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_widened_u8_exact_add_conditional(target);
        let target_plan = staged.optimized_target().target_operations();
        let target_function = &target_plan.functions[0];
        let TargetOperation::ReturnIntegerConditionalControl {
            scalar_type,
            when_true,
            when_false,
            ..
        } = &target_function.operation
        else {
            panic!("fixture must lower to bounded integer conditional control")
        };
        assert_eq!(*scalar_type, u64_integer);
        assert_eq!(
            target_function.provenance.operations,
            expected_function_operations
        );
        for (
            arm,
            expected_wide,
            expected_widen_operation,
            expected_add_operation,
            expected_obligation,
            expected_left,
            expected_left_value,
            expected_right,
            expected_right_value,
        ) in [
            (
                when_true,
                ValueId::new(5_109).unwrap(),
                OperationId::new(5_124).unwrap(),
                OperationId::new(5_123).unwrap(),
                ObligationId::new(5_131).unwrap(),
                ValueId::new(5_106).unwrap(),
                IntegerValue::Unsigned(200),
                ValueId::new(5_107).unwrap(),
                IntegerValue::Unsigned(55),
            ),
            (
                when_false,
                ValueId::new(5_113).unwrap(),
                OperationId::new(5_128).unwrap(),
                OperationId::new(5_127).unwrap(),
                ObligationId::new(5_132).unwrap(),
                ValueId::new(5_110).unwrap(),
                IntegerValue::Unsigned(254),
                ValueId::new(5_111).unwrap(),
                IntegerValue::Unsigned(1),
            ),
        ] {
            let TargetIntegerControl::Return {
                source_value,
                expression,
                ..
            } = arm.control.as_ref()
            else {
                panic!("conditional arm must return its widened value")
            };
            assert_eq!(*source_value, expected_wide);
            let TargetIntegerExpression::IntegerWiden {
                psi_operation: widen_operation,
                source_type,
                operand,
            } = expression
            else {
                panic!("exact u8 addition must remain nested under its widening")
            };
            assert_eq!(*widen_operation, expected_widen_operation);
            assert_eq!(*source_type, u8_integer);
            let TargetIntegerExpression::ExactAdd {
                psi_operation: add_operation,
                obligation,
                left,
                right,
            } = operand.as_ref()
            else {
                panic!("proof-bearing exact addition must remain explicit")
            };
            assert_eq!(*add_operation, expected_add_operation);
            assert_eq!(*obligation, expected_obligation);
            assert_eq!(
                left.as_ref(),
                &TargetIntegerExpression::Immediate {
                    source_value: expected_left,
                    value: expected_left_value,
                }
            );
            assert_eq!(
                right.as_ref(),
                &TargetIntegerExpression::Immediate {
                    source_value: expected_right,
                    value: expected_right_value,
                }
            );
        }

        let legalized = staged.legalized();
        assert_eq!(legalized.receipt().target(), target);
        assert_eq!(legalized.receipt().function_count(), 1);
        assert_eq!(legalized.receipt().decomposition_count(), 2);
        assert_eq!(
            legalized.receipt().validator(),
            legalization_validator_identity()
        );
        assert_eq!(
            legalized.receipt().identity(),
            staged_widened_u8_exact_add_conditional(target)
                .legalized()
                .receipt()
                .identity()
        );
        let legalized_function = &legalized.plan().functions[0];
        assert_eq!(
            legalized_function.recipe,
            LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1
        );
        assert_eq!(
            legalized_function.provenance.operations,
            expected_function_operations
        );
        assert_eq!(
            legalized_function.provenance.edges,
            vec![
                EdgeId::new(5_141).unwrap(),
                EdgeId::new(5_142).unwrap(),
                EdgeId::new(5_143).unwrap(),
                EdgeId::new(5_144).unwrap(),
            ]
        );
        assert_eq!(
            legalized_function.branch_true_fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_141).unwrap()),
                units: 1,
            }]
        );
        assert_eq!(
            legalized_function.branch_false_fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_142).unwrap()),
                units: 1,
            }]
        );
        assert_eq!(
            legalized_function.when_true.return_fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_143).unwrap()),
                units: 1,
            }]
        );
        assert_eq!(
            legalized_function.when_false.return_fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_144).unwrap()),
                units: 1,
            }]
        );
        let accepted = &staged
            .optimized_target()
            .optimized()
            .unit()
            .accepted_obligation_facts;
        assert_eq!(accepted.len(), 2);
        for (
            leaf,
            expected_left_temporary,
            expected_right_temporary,
            expected_left,
            expected_right,
            expected_narrow,
            expected_wide,
            expected_add_operation,
            expected_widen_operation,
            expected_obligation,
            expected_block,
        ) in [
            (
                &legalized_function.when_true,
                LegalizedTemporaryId(0),
                LegalizedTemporaryId(1),
                ValueId::new(5_106).unwrap(),
                ValueId::new(5_107).unwrap(),
                ValueId::new(5_108).unwrap(),
                ValueId::new(5_109).unwrap(),
                OperationId::new(5_123).unwrap(),
                OperationId::new(5_124).unwrap(),
                ObligationId::new(5_131).unwrap(),
                BlockId::new(5_103).unwrap(),
            ),
            (
                &legalized_function.when_false,
                LegalizedTemporaryId(2),
                LegalizedTemporaryId(3),
                ValueId::new(5_110).unwrap(),
                ValueId::new(5_111).unwrap(),
                ValueId::new(5_112).unwrap(),
                ValueId::new(5_113).unwrap(),
                OperationId::new(5_127).unwrap(),
                OperationId::new(5_128).unwrap(),
                ObligationId::new(5_132).unwrap(),
                BlockId::new(5_104).unwrap(),
            ),
        ] {
            assert_eq!(leaf.source_value, expected_wide);
            let LegalizedLeafValue::WidenedExactAdd {
                source_type,
                target_type,
                theorem,
                obligation,
                accepted_fact,
                add_operation,
                narrow_result,
                add_definition_site,
                add_fuel,
                widen_operation,
                widen_definition_site,
                widen_fuel,
                left_temporary,
                right_temporary,
                left,
                right,
            } = &leaf.value
            else {
                panic!("legalizer must publish its proof-aware widening recipe")
            };
            assert_eq!(*source_type, u8_integer);
            assert_eq!(*target_type, u64_integer);
            assert_eq!(
                *theorem,
                LegalizationTheorem::UnsignedExactAddCommutesWithWidenV1
            );
            assert_eq!(*obligation, expected_obligation);
            assert_eq!(*add_operation, expected_add_operation);
            assert_eq!(*narrow_result, expected_narrow);
            assert_eq!(
                *add_definition_site,
                ValueDefinitionSite::Node {
                    block: expected_block,
                    node: 2,
                }
            );
            assert_eq!(*widen_operation, expected_widen_operation);
            assert_eq!(
                *widen_definition_site,
                ValueDefinitionSite::Node {
                    block: expected_block,
                    node: 3,
                }
            );
            assert_eq!(*left_temporary, expected_left_temporary);
            assert_eq!(*right_temporary, expected_right_temporary);
            assert_eq!(left.source_value, expected_left);
            assert_eq!(right.source_value, expected_right);
            assert_eq!(
                add_fuel,
                &vec![FuelSettlement {
                    site: PsiProvenance::Operation(expected_add_operation),
                    units: 1,
                }]
            );
            assert_eq!(
                widen_fuel,
                &vec![FuelSettlement {
                    site: PsiProvenance::Operation(expected_widen_operation),
                    units: 1,
                }]
            );
            let fact = accepted
                .iter()
                .find(|fact| fact.identity == *accepted_fact)
                .expect("legalized fact remains verifier-owned");
            assert_eq!(fact.operation, expected_add_operation);
            assert_eq!(fact.obligation, expected_obligation);

            let narrow_sum = source_type.exact_add(left.value, right.value).unwrap();
            let widened_sum = source_type
                .widen_value_to(*target_type, narrow_sum)
                .unwrap();
            let widened_left = source_type
                .widen_value_to(*target_type, left.value)
                .unwrap();
            let widened_right = source_type
                .widen_value_to(*target_type, right.value)
                .unwrap();
            assert_eq!(
                target_type.exact_add(widened_left, widened_right),
                Some(widened_sum)
            );
        }

        let replayed = validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            legalized.plan().clone(),
        )
        .unwrap();
        assert_eq!(replayed.receipt(), legalized.receipt());
    }
}

#[test]
fn widened_u8_exact_subtract_legalization_preserves_authored_order_and_exact_custody() {
    let u8_integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u64_integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let expected_function_operations = vec![
        OperationId::new(5_121).unwrap(),
        OperationId::new(5_122).unwrap(),
        OperationId::new(5_123).unwrap(),
        OperationId::new(5_124).unwrap(),
        OperationId::new(5_125).unwrap(),
        OperationId::new(5_126).unwrap(),
        OperationId::new(5_127).unwrap(),
        OperationId::new(5_128).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_widened_u8_exact_subtract_conditional(target);
        let target_function = &staged.optimized_target().target_operations().functions[0];
        let TargetOperation::ReturnIntegerConditionalControl {
            scalar_type,
            when_true,
            when_false,
            ..
        } = &target_function.operation
        else {
            panic!("fixture must lower to bounded integer conditional control")
        };
        assert_eq!(*scalar_type, u64_integer);
        assert_eq!(
            target_function.provenance.operations,
            expected_function_operations
        );
        for (
            arm,
            expected_wide,
            expected_widen_operation,
            expected_subtract_operation,
            expected_obligation,
            expected_left,
            expected_left_value,
            expected_right,
            expected_right_value,
        ) in [
            (
                when_true,
                ValueId::new(5_109).unwrap(),
                OperationId::new(5_124).unwrap(),
                OperationId::new(5_123).unwrap(),
                ObligationId::new(5_131).unwrap(),
                ValueId::new(5_106).unwrap(),
                IntegerValue::Unsigned(255),
                ValueId::new(5_107).unwrap(),
                IntegerValue::Unsigned(0),
            ),
            (
                when_false,
                ValueId::new(5_113).unwrap(),
                OperationId::new(5_128).unwrap(),
                OperationId::new(5_127).unwrap(),
                ObligationId::new(5_132).unwrap(),
                ValueId::new(5_110).unwrap(),
                IntegerValue::Unsigned(200),
                ValueId::new(5_111).unwrap(),
                IntegerValue::Unsigned(55),
            ),
        ] {
            let TargetIntegerControl::Return {
                source_value,
                expression,
                ..
            } = arm.control.as_ref()
            else {
                panic!("conditional arm must return its widened value")
            };
            assert_eq!(*source_value, expected_wide);
            let TargetIntegerExpression::IntegerWiden {
                psi_operation: widen_operation,
                source_type,
                operand,
            } = expression
            else {
                panic!("exact u8 subtraction must remain nested under its widening")
            };
            assert_eq!(*widen_operation, expected_widen_operation);
            assert_eq!(*source_type, u8_integer);
            let TargetIntegerExpression::ExactSubtract {
                psi_operation: subtract_operation,
                obligation,
                left,
                right,
            } = operand.as_ref()
            else {
                panic!("proof-bearing exact subtraction must remain explicit")
            };
            assert_eq!(*subtract_operation, expected_subtract_operation);
            assert_eq!(*obligation, expected_obligation);
            assert_eq!(
                left.as_ref(),
                &TargetIntegerExpression::Immediate {
                    source_value: expected_left,
                    value: expected_left_value,
                }
            );
            assert_eq!(
                right.as_ref(),
                &TargetIntegerExpression::Immediate {
                    source_value: expected_right,
                    value: expected_right_value,
                }
            );
        }

        let legalized = staged.legalized();
        assert_eq!(legalized.receipt().target(), target);
        assert_eq!(legalized.receipt().function_count(), 1);
        assert_eq!(legalized.receipt().decomposition_count(), 2);
        assert_eq!(
            legalized.receipt().validator(),
            legalization_validator_identity()
        );
        assert_eq!(
            legalized.receipt().identity(),
            staged_widened_u8_exact_subtract_conditional(target)
                .legalized()
                .receipt()
                .identity()
        );
        let function = &legalized.plan().functions[0];
        assert_eq!(
            function.recipe,
            LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1
        );
        assert_eq!(function.provenance.operations, expected_function_operations);
        assert_eq!(
            function.provenance.edges,
            vec![
                EdgeId::new(5_141).unwrap(),
                EdgeId::new(5_142).unwrap(),
                EdgeId::new(5_143).unwrap(),
                EdgeId::new(5_144).unwrap(),
            ]
        );
        assert_eq!(
            function.branch_true_fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_141).unwrap()),
                units: 1,
            }]
        );
        assert_eq!(
            function.branch_false_fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_142).unwrap()),
                units: 1,
            }]
        );
        assert_eq!(
            function.when_true.return_fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_143).unwrap()),
                units: 1,
            }]
        );
        assert_eq!(
            function.when_false.return_fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_144).unwrap()),
                units: 1,
            }]
        );
        let accepted = &staged
            .optimized_target()
            .optimized()
            .unit()
            .accepted_obligation_facts;
        assert_eq!(accepted.len(), 2);
        for (
            leaf,
            expected_temporaries,
            expected_values,
            expected_operations,
            expected_obligation,
            expected_block,
            expected_constants,
        ) in [
            (
                &function.when_true,
                [LegalizedTemporaryId(0), LegalizedTemporaryId(1)],
                [
                    ValueId::new(5_106).unwrap(),
                    ValueId::new(5_107).unwrap(),
                    ValueId::new(5_108).unwrap(),
                    ValueId::new(5_109).unwrap(),
                ],
                [
                    OperationId::new(5_123).unwrap(),
                    OperationId::new(5_124).unwrap(),
                ],
                ObligationId::new(5_131).unwrap(),
                BlockId::new(5_103).unwrap(),
                [IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)],
            ),
            (
                &function.when_false,
                [LegalizedTemporaryId(2), LegalizedTemporaryId(3)],
                [
                    ValueId::new(5_110).unwrap(),
                    ValueId::new(5_111).unwrap(),
                    ValueId::new(5_112).unwrap(),
                    ValueId::new(5_113).unwrap(),
                ],
                [
                    OperationId::new(5_127).unwrap(),
                    OperationId::new(5_128).unwrap(),
                ],
                ObligationId::new(5_132).unwrap(),
                BlockId::new(5_104).unwrap(),
                [IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)],
            ),
        ] {
            assert_eq!(leaf.source_value, expected_values[3]);
            let LegalizedLeafValue::WidenedExactSubtract {
                source_type,
                target_type,
                theorem,
                obligation,
                accepted_fact,
                subtract_operation,
                narrow_result,
                subtract_definition_site,
                subtract_fuel,
                widen_operation,
                widen_definition_site,
                widen_fuel,
                left_temporary,
                right_temporary,
                left,
                right,
            } = &leaf.value
            else {
                panic!("legalizer must publish its ordered proof-aware subtraction recipe")
            };
            assert_eq!(*source_type, u8_integer);
            assert_eq!(*target_type, u64_integer);
            assert_eq!(
                *theorem,
                LegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1
            );
            assert_eq!(*obligation, expected_obligation);
            assert_eq!(*subtract_operation, expected_operations[0]);
            assert_eq!(*narrow_result, expected_values[2]);
            assert_eq!(
                *subtract_definition_site,
                ValueDefinitionSite::Node {
                    block: expected_block,
                    node: 2,
                }
            );
            assert_eq!(*widen_operation, expected_operations[1]);
            assert_eq!(
                *widen_definition_site,
                ValueDefinitionSite::Node {
                    block: expected_block,
                    node: 3,
                }
            );
            assert_eq!(*left_temporary, expected_temporaries[0]);
            assert_eq!(*right_temporary, expected_temporaries[1]);
            assert_eq!(left.source_value, expected_values[0]);
            assert_eq!(right.source_value, expected_values[1]);
            assert_eq!(left.value, expected_constants[0]);
            assert_eq!(right.value, expected_constants[1]);
            let constant_operations = if expected_block == BlockId::new(5_103).unwrap() {
                [
                    OperationId::new(5_121).unwrap(),
                    OperationId::new(5_122).unwrap(),
                ]
            } else {
                [
                    OperationId::new(5_125).unwrap(),
                    OperationId::new(5_126).unwrap(),
                ]
            };
            assert_eq!(left.constant_operation, constant_operations[0]);
            assert_eq!(right.constant_operation, constant_operations[1]);
            assert_eq!(
                left.definition_site,
                ValueDefinitionSite::Node {
                    block: expected_block,
                    node: 0,
                }
            );
            assert_eq!(
                right.definition_site,
                ValueDefinitionSite::Node {
                    block: expected_block,
                    node: 1,
                }
            );
            assert_eq!(
                left.fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Operation(constant_operations[0]),
                    units: 1,
                }]
            );
            assert_eq!(
                right.fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Operation(constant_operations[1]),
                    units: 1,
                }]
            );
            assert_eq!(
                subtract_fuel,
                &vec![FuelSettlement {
                    site: PsiProvenance::Operation(expected_operations[0]),
                    units: 1,
                }]
            );
            assert_eq!(
                widen_fuel,
                &vec![FuelSettlement {
                    site: PsiProvenance::Operation(expected_operations[1]),
                    units: 1,
                }]
            );
            let fact = accepted
                .iter()
                .find(|fact| fact.identity == *accepted_fact)
                .expect("legalized fact remains verifier-owned");
            assert_eq!(fact.operation, expected_operations[0]);
            assert_eq!(fact.obligation, expected_obligation);

            let narrow = source_type.exact_sub(left.value, right.value).unwrap();
            let widened = source_type.widen_value_to(*target_type, narrow).unwrap();
            let widened_left = source_type
                .widen_value_to(*target_type, left.value)
                .unwrap();
            let widened_right = source_type
                .widen_value_to(*target_type, right.value)
                .unwrap();
            assert_eq!(
                target_type.exact_sub(widened_left, widened_right),
                Some(widened)
            );
            assert_ne!(
                target_type.exact_sub(widened_right, widened_left),
                Some(widened),
                "the theorem must not commute subtraction operands"
            );
        }

        let replayed = validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            legalized.plan().clone(),
        )
        .unwrap();
        assert_eq!(replayed.receipt(), legalized.receipt());
    }
}

#[test]
fn widened_u8_exact_add_independent_replay_rejects_corrupted_bridge_custody() {
    let staged = staged_widened_u8_exact_add_conditional(NativeTarget::linux_x64());
    let original = staged.legalized().plan();
    let validate = |plan| {
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            plan,
        )
    };
    let false_fact = match original.functions[0].when_false.value {
        LegalizedLeafValue::WidenedExactAdd { accepted_fact, .. } => accepted_fact,
        _ => panic!("fixture must retain its false-arm proof fact"),
    };

    macro_rules! corrupt_true_leaf {
        (|$value:ident| $body:block) => {{
            let mut corrupted = original.clone();
            let $value = &mut corrupted.functions[0].when_true.value;
            $body
            assert_eq!(
                validate(corrupted),
                Err(LegalizationError::NonCanonicalLegalizedPlan)
            );
        }};
    }

    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { source_type, .. } = value else {
            unreachable!()
        };
        *source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { target_type, .. } = value else {
            unreachable!()
        };
        *target_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { accepted_fact, .. } = value else {
            unreachable!()
        };
        *accepted_fact = false_fact;
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { narrow_result, .. } = value else {
            unreachable!()
        };
        *narrow_result = ValueId::new(9_601).unwrap();
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd {
            add_operation,
            widen_operation,
            ..
        } = value
        else {
            unreachable!()
        };
        std::mem::swap(add_operation, widen_operation);
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd {
            add_definition_site,
            widen_definition_site,
            ..
        } = value
        else {
            unreachable!()
        };
        std::mem::swap(add_definition_site, widen_definition_site);
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { add_fuel, .. } = value else {
            unreachable!()
        };
        add_fuel[0].units += 1;
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { widen_fuel, .. } = value else {
            unreachable!()
        };
        widen_fuel[0].units += 1;
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd {
            left_temporary,
            right_temporary,
            ..
        } = value
        else {
            unreachable!()
        };
        *left_temporary = *right_temporary;
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { left, right, .. } = value else {
            unreachable!()
        };
        std::mem::swap(&mut left.constant_operation, &mut right.constant_operation);
    });
}

#[test]
fn widened_u8_exact_subtract_independent_replay_rejects_corrupted_order_and_custody() {
    let staged = staged_widened_u8_exact_subtract_conditional(NativeTarget::linux_x64());
    let original = staged.legalized().plan();
    let identity = legalized_operation_plan_identity(original);
    let validate = |plan| {
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            plan,
        )
    };
    let false_fact = match original.functions[0].when_false.value {
        LegalizedLeafValue::WidenedExactSubtract { accepted_fact, .. } => accepted_fact,
        _ => panic!("fixture must retain its false-arm proof fact"),
    };

    macro_rules! corrupt_true_subtract_leaf {
        (|$value:ident| $body:block) => {{
            let mut corrupted = original.clone();
            let $value = &mut corrupted.functions[0].when_true.value;
            $body
            assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
            assert_eq!(
                validate(corrupted),
                Err(LegalizationError::NonCanonicalLegalizedPlan)
            );
        }};
    }

    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { source_type, .. } = value else {
            unreachable!()
        };
        *source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { target_type, .. } = value else {
            unreachable!()
        };
        *target_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { theorem, .. } = value else {
            unreachable!()
        };
        *theorem = LegalizationTheorem::UnsignedExactAddCommutesWithWidenV1;
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { accepted_fact, .. } = value else {
            unreachable!()
        };
        *accepted_fact = false_fact;
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { obligation, .. } = value else {
            unreachable!()
        };
        *obligation = ObligationId::new(9_611).unwrap();
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { narrow_result, .. } = value else {
            unreachable!()
        };
        *narrow_result = ValueId::new(9_612).unwrap();
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract {
            subtract_operation,
            widen_operation,
            ..
        } = value
        else {
            unreachable!()
        };
        std::mem::swap(subtract_operation, widen_operation);
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract {
            subtract_definition_site,
            widen_definition_site,
            ..
        } = value
        else {
            unreachable!()
        };
        std::mem::swap(subtract_definition_site, widen_definition_site);
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { subtract_fuel, .. } = value else {
            unreachable!()
        };
        subtract_fuel[0].units += 1;
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { widen_fuel, .. } = value else {
            unreachable!()
        };
        widen_fuel[0].units += 1;
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract {
            left_temporary,
            right_temporary,
            ..
        } = value
        else {
            unreachable!()
        };
        *left_temporary = *right_temporary;
    });
    corrupt_true_subtract_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactSubtract { left, right, .. } = value else {
            unreachable!()
        };
        std::mem::swap(left, right);
    });

    let mut corrupted = original.clone();
    corrupted.functions[0].recipe =
        LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1;
    assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
    assert_eq!(
        validate(corrupted),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );
}
