use super::{
    AbstractFunctionResult, AbstractOperation, AbstractParameter, AbstractResult, EdgeId,
    OperationId, ScalarType, ValueId, fixture,
};
use crate::legalize_target_operations;
use crate::validate_legalized_operations;
use target_operations::{TargetControlTerminator, TargetScalarExpression};

#[test]
fn literal_negation_retains_its_operation_and_exact_operand() {
    use target_operations::{TargetBooleanExpression, TargetUnitOperation};

    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        for literal in [false, true] {
            let (mut source, _, previous) = fixture(Some(7), native);
            let function = &mut source.functions[0];
            let operand = ValueId::new(2).unwrap();
            let negated = ValueId::new(4).unwrap();
            let result = function.result.scalar().unwrap().value;
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type: ScalarType::Boolean,
            });
            function.operations = vec![
                AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: operand,
                    value: literal,
                },
                AbstractOperation::BooleanNot {
                    psi_operation: OperationId::new(2).unwrap(),
                    result: negated,
                    operand,
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(1).unwrap(),
                    result,
                    value: negated,
                    scalar_type: ScalarType::Boolean,
                    cleanup_actions: Vec::new(),
                },
            ];
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                previous.fuel_schedule,
            )
            .unwrap();
            let legal = legalize_target_operations(&target, &source, &unit)
                .expect("lowering preserves the authored negation, including a literal operand");
            for replacement in [
                // Equal results do not authorize folding during target lowering.
                TargetBooleanExpression::Immediate {
                    source_value: negated,
                    value: !literal,
                },
                TargetBooleanExpression::Not {
                    psi_operation: OperationId::new(1).unwrap(),
                    operand: Box::new(TargetBooleanExpression::Immediate {
                        source_value: operand,
                        value: literal,
                    }),
                },
                TargetBooleanExpression::Not {
                    psi_operation: OperationId::new(2).unwrap(),
                    operand: Box::new(TargetBooleanExpression::Immediate {
                        source_value: negated,
                        value: literal,
                    }),
                },
            ] {
                let mut changed = target.clone();
                let TargetUnitOperation::ScalarDefinition { expression, .. } =
                    &mut changed.functions[0].graph.blocks[0].operations[1]
                else {
                    panic!("negation definition");
                };
                *expression = TargetScalarExpression::Boolean(replacement);
                assert!(
                    validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn operation_operands_and_returns_reject_reconstructed_boolean_trees() {
    let native = target::NativeTarget::linux_x64();
    let (mut source, _, previous) = fixture(Some(7), native);
    let function = &mut source.functions[0];
    let parameter = ValueId::new(2).unwrap();
    let first = ValueId::new(4).unwrap();
    let second = ValueId::new(5).unwrap();
    let result = function.result.scalar().unwrap().value;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: result,
        scalar_type: ScalarType::Boolean,
    });
    function.parameters.push(AbstractParameter {
        value: parameter,
        scalar_type: ScalarType::Boolean,
    });
    function.operations = vec![
        AbstractOperation::BooleanNot {
            psi_operation: OperationId::new(2).unwrap(),
            result: first,
            operand: parameter,
        },
        AbstractOperation::BooleanNot {
            psi_operation: OperationId::new(3).unwrap(),
            result: second,
            operand: first,
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result,
            value: second,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: Vec::new(),
        },
    ];
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let unit =
        optimization_unit::reconstruct_psi_optimization_unit_seed(&source, previous.fuel_schedule)
            .unwrap();
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    for replace_return in [false, true] {
        let mut changed = target.clone();
        let graph = &mut changed.functions[0].graph;
        let observations = graph.blocks[0]
            .operations
            .iter()
            .filter_map(|operation| match operation {
                target_operations::TargetUnitOperation::ScalarDefinition {
                    expression: TargetScalarExpression::Boolean(expression),
                    ..
                } => Some(expression.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(observations.len(), 2);
        if replace_return {
            let TargetControlTerminator::ReturnScalar { expression, .. } =
                &mut graph.blocks[0].terminator
            else {
                panic!("scalar return")
            };
            *expression = TargetScalarExpression::Boolean(observations[1].clone());
        } else {
            let second = graph.blocks[0]
                .operations
                .iter_mut()
                .filter_map(|operation| match operation {
                    target_operations::TargetUnitOperation::ScalarDefinition {
                        expression: TargetScalarExpression::Boolean(expression),
                        ..
                    } => Some(expression),
                    _ => None,
                })
                .nth(1)
                .unwrap();
            let target_operations::TargetBooleanExpression::Not { operand, .. } = second else {
                panic!("Boolean not")
            };
            **operand = observations[0].clone();
        }
        assert!(legalize_target_operations(&changed, &source, &unit).is_err());
        assert!(
            validate_legalized_operations(&changed, &source, &unit, legal.plan().clone()).is_err()
        );
    }
}
