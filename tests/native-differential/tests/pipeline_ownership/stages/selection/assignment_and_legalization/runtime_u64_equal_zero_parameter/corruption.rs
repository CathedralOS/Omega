use crate::tests::*;
use legalized_operations::LegalizedCondition;

use super::fixture::staged_equal_zero_parameter;

#[test]
fn reversed_or_nonzero_source_is_outside_the_equal_zero_family() {
    for mutate in [
        |machine: &mut TerminalMachine| {
            let OperationKind::IntegerEqual { left, right } =
                &mut machine.blocks[0].operations[1].kind
            else {
                panic!("fixture must contain integer equality")
            };
            std::mem::swap(left, right);
        },
        |machine: &mut TerminalMachine| {
            let OperationKind::IntegerConstant { value } =
                &mut machine.blocks[0].operations[0].kind
            else {
                panic!("fixture must begin with the authored zero")
            };
            *value = IntegerValue::Unsigned(1);
        },
    ] {
        let mut machine = conditional_u64_equal_zero_parameter_machine(20_100, [7, 9]);
        mutate(&mut machine);
        let module = conditional_immediate_module(machine.id, vec![machine]);
        let semantic = terminal_codec::encode_module(&module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap();

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                ExplicitOptimizationRequest::new(
                    OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
                    selected_lowering_budget(),
                )
                .unwrap(),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target).unwrap();
            assert!(matches!(
                stage_optimized_instruction_selection(target),
                Err(OptimizedSelectionPipelineError::Legalization(
                    LegalizationError::UnsupportedCondition { function: 0 }
                ))
            ));
        }
    }
}

#[test]
fn equal_zero_legalization_and_selected_corruption_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_equal_zero_parameter(target);
        let validate = |plan| {
            validate_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };

        let mut corrupted = staged.legalized().plan().clone();
        corrupted.functions[0].recipe =
            LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1;
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::UnsupportedSourceShape { function: 0 })
        );

        let mut corrupted = staged.legalized().plan().clone();
        let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } =
            &mut corrupted.functions[0].condition
        else {
            panic!("fixture must retain equal-zero custody")
        };
        zero.value = IntegerValue::Unsigned(1);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &mut corrupted.functions[0].blocks[0].terminator
        else {
            panic!("fixture must retain conditional control")
        };
        std::mem::swap(when_nonzero, when_zero);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SuccessorProjectionMismatch {
                function: 0,
                block: 0
            })
        ));
    }
}

#[test]
fn equal_zero_compare_shape_corruption_fails_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_equal_zero_parameter(target);

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::CompareI64;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 0
            })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[0].operands[0].access =
            RegisterOperandAccess::Def;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 0
            }) | Err(SelectedInstructionError::ConstraintOperandMismatch {
                function: 0,
                instruction: 0
            })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[0].constraint =
            staged.register_environment().selected_keys().copy_i64;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 0
            }) | Err(SelectedInstructionError::ConstraintOperandMismatch {
                function: 0,
                instruction: 0
            })
        ));
    }
}

#[test]
fn equal_zero_compare_provenance_and_fuel_corruption_fails_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_equal_zero_parameter(target);

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[0]
            .provenance
            .operations
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 0
            })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[0]
            .provenance
            .fuel[0]
            .units += 1;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 0
            }) | Err(SelectedInstructionError::ProvenancePartitionMismatch { function: 0 })
        ));
    }
}

#[test]
fn equal_zero_branch_and_virtual_register_custody_corruption_fails_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_equal_zero_parameter(target);

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            panic!("fixture must retain conditional control")
        };
        instruction.kind = SelectedInstructionKind::ConditionalBranchU64LessThan;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 1
            })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            panic!("fixture must retain conditional control")
        };
        instruction.provenance.values[0] = ValueId::new(20_006).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 1
            })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let VirtualRegisterOrigin::EntryParameter {
            parameter_index, ..
        } = &mut corrupted.functions[0].virtual_registers[0].origin
        else {
            panic!("fixture must retain parameter virtual-register custody")
        };
        *parameter_index = 1;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(
                SelectedInstructionError::VirtualRegisterProjectionMismatch {
                    function: 0,
                    register: 0
                }
            )
        ));
    }
}
