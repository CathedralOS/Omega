use super::*;
use target_operations::{TargetControlTerminator, TargetScalarExpression};

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
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
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
