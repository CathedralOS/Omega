//! Independent rejection for inclusive-order custody and selected inversion corruption.

use crate::tests::*;
use legalized_operations::{LegalizedScalarComparison, LegalizedScalarInstructionKind};

use super::fixture::staged_integer_less_or_equal_conditional;

#[test]
fn reflexive_less_or_equal_uses_one_semantic_parameter_in_the_ordinary_graph() {
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
        let staged = stage_optimized_instruction_selection(target).unwrap();
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            staged.legalized().plan().clone(),
        )
        .unwrap();
        validate_raw_selection(&staged, staged.selected().plan().clone()).unwrap();
        let comparison = staged.selected().plan().functions[0].blocks[0]
            .instructions
            .iter()
            .find(|instruction| instruction.kind == SelectedInstructionKind::CompareI64)
            .unwrap();
        assert_eq!(
            comparison.operands[0].virtual_register,
            comparison.operands[1].virtual_register
        );
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
        let LegalizedScalarInstructionKind::Compare { left, right, .. } =
            &mut swapped.scalar_functions[0].blocks[0].instructions[0].kind
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
            let LegalizedScalarInstructionKind::Compare { predicate, .. } =
                &mut substituted.scalar_functions[0].blocks[0].instructions[0].kind
            else {
                unreachable!()
            };
            *predicate = if substitute == 0 {
                LegalizedScalarComparison::Equal
            } else {
                LegalizedScalarComparison::LessThan
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
        operand_swap.functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| instruction.kind == SelectedInstructionKind::CompareI64)
            .unwrap()
            .operands
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, operand_swap),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
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
            Err(SelectedInstructionError::SourceCustodyMismatch)
        ));
    }
}
