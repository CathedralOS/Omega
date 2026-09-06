use crate::tests::*;

use super::fixture::staged_not_equal_zero_parameter;

#[test]
fn u64_parameter_not_equal_zero_selects_exact_compare_zero_graph_on_both_isas() {
    let zero_operation = OperationId::new(20_119).unwrap();
    let equality_operation = OperationId::new(20_111).unwrap();
    let boolean_not_operation = OperationId::new(20_120).unwrap();
    let parameter = ValueId::new(20_105).unwrap();
    let zero = ValueId::new(20_106).unwrap();
    let equality = ValueId::new(20_107).unwrap();
    let not_equal = ValueId::new(20_121).unwrap();
    let true_edge = EdgeId::new(20_114).unwrap();
    let false_edge = EdgeId::new(20_115).unwrap();

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_not_equal_zero_parameter(target);
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            staged.legalized().plan().clone(),
        )
        .unwrap();
        validate_raw_selection(&staged, staged.selected().plan().clone()).unwrap();

        let legalized = staged.legalized().plan().functions[0].conditional();
        assert_eq!(
            legalized.recipe,
            LegalizationRecipe::ReturnU64NotEqualZeroParameterConditionalV1
        );
        let legalized_operations::LegalizedCondition::U64NotEqualZeroParameterV1 {
            equality_operation: legalized_equality,
            equality_result,
            boolean_not_operation: legalized_not,
            boolean_not_result,
            parameter: legalized_parameter,
            zero: legalized_zero,
            ..
        } = &legalized.condition
        else {
            panic!("legalization must retain exact parameter-not-equal-zero custody")
        };
        assert_eq!(*legalized_equality, equality_operation);
        assert_eq!(*equality_result, equality);
        assert_eq!(*legalized_not, boolean_not_operation);
        assert_eq!(*boolean_not_result, not_equal);
        assert_eq!(legalized_parameter.source_value, parameter);
        assert_eq!(legalized_zero.source_value, zero);
        assert_eq!(legalized_zero.value, IntegerValue::Unsigned(0));

        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.virtual_registers.len(), 3);
        assert_eq!(staged.selected().receipt().instruction_count(), 6);
        let [compare] = function.blocks[0].instructions.as_slice() else {
            panic!("entry must contain exactly one compare-zero instruction")
        };
        assert_eq!(compare.kind, SelectedInstructionKind::CompareI64Zero);
        assert_eq!(compare.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(
            compare.provenance.operations,
            [zero_operation, equality_operation]
        );
        assert_eq!(compare.provenance.values, [parameter, zero, equality]);
        assert_eq!(compare.provenance.fuel.len(), 2);

        let SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } = &function.blocks[0].terminator
        else {
            panic!("entry must branch on the compare-zero result")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchNonZero
        );
        assert_eq!(instruction.provenance.operations, [boolean_not_operation]);
        assert_eq!(instruction.provenance.values, [equality, not_equal]);
        assert_eq!(instruction.provenance.fuel.len(), 1);
        assert_eq!(when_nonzero.psi_edge, true_edge);
        assert_eq!(
            when_nonzero.block,
            selected_instructions::SelectedBlockId(1)
        );
        assert_eq!(when_zero.psi_edge, false_edge);
        assert_eq!(when_zero.block, selected_instructions::SelectedBlockId(2));
    }
}

#[test]
fn not_equal_zero_semantics_cover_zero_one_and_u64_max_on_both_isas() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_not_equal_zero_parameter(target);
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &staged.selected().plan().functions[0].blocks[0].terminator
        else {
            panic!("fixture must branch on the nonzero comparison predicate")
        };
        for (input, expected_true) in [(0_u64, false), (1, true), (u64::MAX, true)] {
            let selected_edge = if input != 0 {
                when_nonzero.psi_edge
            } else {
                when_zero.psi_edge
            };
            assert_eq!(
                selected_edge == EdgeId::new(20_114).unwrap(),
                expected_true,
                "{input} != 0 on {target:?}"
            );
        }
    }
}
