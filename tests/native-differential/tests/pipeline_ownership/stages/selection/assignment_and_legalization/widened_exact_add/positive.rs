//! Widened exact-add theorem temporaries, authored order, and exact custody.

use crate::tests::*;

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
        let legalized_function = legalized.plan().functions[0].conditional();
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
