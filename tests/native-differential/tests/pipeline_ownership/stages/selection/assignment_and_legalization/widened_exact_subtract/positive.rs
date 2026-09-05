//! Widened exact-subtract authored order, theorem temporaries, and exact custody.

use crate::tests::*;

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
