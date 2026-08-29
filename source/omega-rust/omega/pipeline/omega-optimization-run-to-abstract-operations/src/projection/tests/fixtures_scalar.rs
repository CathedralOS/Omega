use super::*;

pub(super) fn dead_scalar_literals_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_081).unwrap();
    let block = BlockId::new(1_082).unwrap();
    let boolean = ValueId::new(1_083).unwrap();
    let integer = ValueId::new(1_084).unwrap();
    verified(
        module_with_blocks(
            machine,
            block,
            TerminalMachineResult::Unit,
            vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(1_085).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: boolean,
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: true },
                    },
                    Operation {
                        id: OperationId::new(1_086).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: integer,
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                            ),
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1_087).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
        ),
        ProofBundle::default(),
    )
}

pub(super) fn dead_wrapping_add_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_091).unwrap();
    let block = BlockId::new(1_092).unwrap();
    let left = ValueId::new(1_093).unwrap();
    let right = ValueId::new(1_094).unwrap();
    let sum = ValueId::new(1_095).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let declaration = |id| {
        OperationResult::Scalar(ValueDeclaration {
            id,
            scalar_type: ScalarType::Integer(integer),
        })
    };
    verified(
        module_with_blocks(
            machine,
            block,
            TerminalMachineResult::Unit,
            vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(1_096).unwrap(),
                        result: declaration(left),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(250),
                        },
                    },
                    Operation {
                        id: OperationId::new(1_097).unwrap(),
                        result: declaration(right),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(10),
                        },
                    },
                    Operation {
                        id: OperationId::new(1_098).unwrap(),
                        result: declaration(sum),
                        kind: OperationKind::WrappingIntegerAdd { left, right },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1_099).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
        ),
        ProofBundle::default(),
    )
}
