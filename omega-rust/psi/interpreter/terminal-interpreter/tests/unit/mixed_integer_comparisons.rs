use super::*;

fn comparison_module(
    left_type: IntegerType,
    left_value: IntegerValue,
    right_type: IntegerType,
    right_value: IntegerValue,
    relation: usize,
) -> TerminalModule {
    let mut module = unit_module();
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(5),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    machine.blocks[0].operations = [(left_type, left_value), (right_type, right_value)]
        .into_iter()
        .enumerate()
        .map(|(ordinal, (integer_type, value))| Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(ordinal as u64 + 1),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(ordinal as u64 + 1),
                scalar_type: ScalarType::Integer(integer_type),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant { value },
        })
        .collect();
    let (left, right) = if relation >= 4 {
        (value_id(2), value_id(1))
    } else {
        (value_id(1), value_id(2))
    };
    let kind = match relation {
        0 | 1 => OperationKind::IntegerEqual { left, right },
        2 | 4 => OperationKind::IntegerLessThan { left, right },
        _ => OperationKind::IntegerLessOrEqual { left, right },
    };
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(3),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(3),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind,
    });
    let result = if relation == 1 {
        machine.blocks[0].operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(4),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(4),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            }),
            kind: OperationKind::BooleanNot {
                operand: value_id(3),
            },
        });
        value_id(4)
    } else {
        value_id(3)
    };
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: result,
        cleanup_actions: Vec::new(),
    };
    module
}

#[test]
fn heterogeneous_integer_relations_round_trip_and_execute_exact_values() {
    let signed = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let narrower = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    for (left_type, left, right_type, right, expected) in [
        (
            narrower,
            IntegerValue::Signed(-1),
            unsigned,
            IntegerValue::Unsigned(10),
            [false, true, true, true, false, false],
        ),
        (
            signed,
            IntegerValue::Signed(-1),
            unsigned,
            IntegerValue::Unsigned(u64::MAX as u128),
            [false, true, true, true, false, false],
        ),
        (
            narrower,
            IntegerValue::Signed(10),
            unsigned,
            IntegerValue::Unsigned(10),
            [true, false, false, true, false, true],
        ),
        (
            signed,
            IntegerValue::Signed(i64::MAX as i128),
            unsigned,
            IntegerValue::Unsigned(u64::MAX as u128),
            [false, true, true, true, false, false],
        ),
        (
            unsigned,
            IntegerValue::Unsigned(u64::MAX as u128),
            signed,
            IntegerValue::Signed(-1),
            [false, true, false, false, true, true],
        ),
    ] {
        for (relation, expected) in expected.into_iter().enumerate() {
            let module = comparison_module(left_type, left, right_type, right, relation);
            let semantic = encode_module(&module).unwrap();
            assert_eq!(decode_module(&semantic).unwrap(), module);
            let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
            let measured = interpret_terminal_artifact_measured(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &[],
                TerminalStructuralInputs::default(),
                &mut AcceptTerminalEffects,
            )
            .unwrap();
            assert_eq!(
                measured.value(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
                "{left_type:?} {left:?}, {right_type:?} {right:?}, relation {relation}"
            );
            assert_eq!(
                measured.usage().total_units(),
                if relation == 1 { 5 } else { 4 }
            );
        }
    }
}

#[test]
fn heterogeneous_integer_relations_reject_foreign_operands_and_carriers() {
    let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let original = comparison_module(
        signed,
        IntegerValue::Signed(-1),
        unsigned,
        IntegerValue::Unsigned(10),
        2,
    );
    for mutation in 0..3 {
        let mut module = original.clone();
        let operations = &mut module.machines[0].blocks[0].operations;
        match mutation {
            0 => {
                operations[1].kind = OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(10),
                }
            }
            1 => {
                operations[1].kind = OperationKind::BooleanConstant { value: true };
                operations[1].result = OperationResult::Scalar(ValueDeclaration {
                    id: value_id(2),
                    scalar_type: ScalarType::Boolean,
                    qualifications: Default::default(),
                });
            }
            _ => {
                operations[2].kind = OperationKind::IntegerLessThan {
                    left: value_id(1),
                    right: value_id(99),
                }
            }
        }
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}
