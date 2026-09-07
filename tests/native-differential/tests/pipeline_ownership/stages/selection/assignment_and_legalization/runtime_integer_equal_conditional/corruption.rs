//! Independent replay rejection for equality-condition and selected-graph corruption.

use crate::tests::*;
use legalized_operations::LegalizedScalarInstructionKind;

use super::fixture::staged_integer_equal_conditional;

#[test]
fn reflexive_equality_uses_one_semantic_parameter_in_the_ordinary_graph() {
    let mut machine = conditional_u64_integer_equal_parameters_machine(19_100, [7, 9]);
    let OperationKind::IntegerEqual { left, right } = &mut machine.blocks[0].operations[0].kind
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
fn equality_condition_custody_corruption_fails_closed_on_both_isas() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_equal_conditional(target);
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
        corrupted.scalar_functions[0].blocks[0].instructions[0].operation =
            OperationId::new(19_099).unwrap();
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        let LegalizedScalarInstructionKind::Compare { left, right, .. } =
            &mut corrupted.scalar_functions[0].blocks[0].instructions[0].kind
        else {
            panic!("fixture must retain equality custody")
        };
        std::mem::swap(left, right);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.scalar_functions[0].blocks[0].instructions[0].fuel[0].units += 1;
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );
    }
}

#[test]
fn equality_selected_graph_corruption_fails_closed_on_both_isas() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_equal_conditional(target);

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &mut corrupted.functions[0].blocks[0].terminator
        else {
            panic!("fixture must retain conditional branch")
        };
        std::mem::swap(when_nonzero, when_zero);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SourceCustodyMismatch)
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| instruction.kind == SelectedInstructionKind::CompareI64)
            .unwrap()
            .provenance
            .operations[0] = OperationId::new(19_099).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));
    }
}
