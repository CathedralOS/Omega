//! Independent replay rejection for not-equal source, legalization, and selection drift.

use crate::tests::*;
use omega_legalized_operations::LegalizedCondition;

use super::fixture::staged_integer_not_equal_conditional;

#[test]
fn reflexive_integer_not_equal_is_outside_the_two_distinct_parameter_family() {
    let mut machine = conditional_u64_integer_not_equal_parameters_machine(19_700, [7, 9]);
    let OperationKind::IntegerEqual { left, right } = &mut machine.blocks[0].operations[0].kind
    else {
        panic!("fixture must begin with equality of its entry parameters")
    };
    *right = *left;
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&ProofBundle {
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
fn inequality_order_operation_fuel_and_condition_substitution_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_not_equal_conditional(target);
        let original = staged.legalized().plan();
        let validate = |plan| {
            validate_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };

        let mut corrupted = original.clone();
        let LegalizedCondition::IntegerNotEqualParametersV1 { left, right, .. } =
            &mut corrupted.functions[0].condition
        else {
            panic!("fixture must retain inequality custody")
        };
        std::mem::swap(left, right);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        let LegalizedCondition::IntegerNotEqualParametersV1 {
            boolean_not_operation,
            boolean_not_fuel,
            ..
        } = &mut corrupted.functions[0].condition
        else {
            unreachable!()
        };
        *boolean_not_operation = OperationId::new(19_699).unwrap();
        boolean_not_fuel[0].units += 1;
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut substituted = original.clone();
        let LegalizedCondition::IntegerNotEqualParametersV1 {
            equality_operation,
            equality_result_definition_site,
            equality_fuel,
            left,
            right,
            ..
        } = substituted.functions[0].condition.clone()
        else {
            unreachable!()
        };
        substituted.functions[0].condition = LegalizedCondition::IntegerEqualParametersV1 {
            operation: equality_operation,
            result_definition_site: equality_result_definition_site,
            fuel: equality_fuel,
            left,
            right,
        };
        assert_eq!(
            validate(substituted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );
    }
}

#[test]
fn inequality_selected_operand_successor_and_boolean_not_custody_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_not_equal_conditional(target);

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[0]
            .operands
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 0
            })
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
            Err(SelectedInstructionError::SuccessorProjectionMismatch {
                function: 0,
                block: 0
            })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.provenance.operations.clear();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 1
            })
        ));
    }
}
