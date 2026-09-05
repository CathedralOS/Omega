//! Independent rejection for inclusive-order custody and selected inversion corruption.

use crate::tests::*;
use legalized_operations::LegalizedCondition;

use super::fixture::staged_integer_less_or_equal_conditional;

#[test]
fn reflexive_less_or_equal_is_outside_the_two_distinct_parameter_family() {
    let mut machine = conditional_u64_integer_less_or_equal_parameters_machine(19_500, [7, 9]);
    let OperationKind::IntegerLessOrEqual { left, right } =
        &mut machine.blocks[0].operations[0].kind
    else {
        panic!("fixture must compare its two entry parameters")
    };
    *right = *left;
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
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

#[test]
fn inclusive_order_substitution_and_swap_corruption_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_less_or_equal_conditional(target);
        let original = staged.legalized().plan();
        let validate = |plan| {
            validate_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };

        let mut swapped = original.clone();
        let LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. } =
            &mut swapped.functions[0].condition
        else {
            panic!("fixture must retain authored inclusive order")
        };
        std::mem::swap(left, right);
        assert_eq!(
            validate(swapped),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        for substitute in [0, 1] {
            let mut substituted = original.clone();
            let LegalizedCondition::IntegerLessOrEqualParametersV1 {
                operation,
                result_definition_site,
                fuel,
                left,
                right,
            } = substituted.functions[0].condition.clone()
            else {
                unreachable!()
            };
            substituted.functions[0].condition = if substitute == 0 {
                LegalizedCondition::IntegerEqualParametersV1 {
                    operation,
                    result_definition_site,
                    fuel,
                    left,
                    right,
                }
            } else {
                LegalizedCondition::IntegerLessThanParametersV1 {
                    operation,
                    result_definition_site,
                    fuel,
                    left,
                    right,
                }
            };
            assert_eq!(
                validate(substituted),
                Err(LegalizationError::NonCanonicalLegalizedPlan)
            );
        }
    }
}

#[test]
fn reversed_compare_and_successor_corruption_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_less_or_equal_conditional(target);

        let mut operand_swap = staged.selected().plan().clone();
        operand_swap.functions[0].blocks[0].instructions[0]
            .operands
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, operand_swap),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 0
            })
        ));

        let mut successor_swap = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        } = &mut successor_swap.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        std::mem::swap(when_less, when_not_less);
        assert!(matches!(
            validate_raw_selection(&staged, successor_swap),
            Err(SelectedInstructionError::SuccessorProjectionMismatch {
                function: 0,
                block: 0
            })
        ));
    }
}
