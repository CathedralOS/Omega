use super::*;
use target_operations::TargetScalarExpression;

#[test]
fn scalar_graph_retains_distinct_branch_definitions_and_arrivals() {
    for selected in [false, true] {
        let mut plan = constant_conditional_plan(selected);
        let function = &mut plan.functions[0];
        for (position, value) in [(1, 3), (2, 4)] {
            function.block_entries[position].parameters = vec![AbstractParameter {
                value: ValueId::new(value).unwrap(),
                scalar_type: function.parameters[0].scalar_type,
            }];
        }
        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
            panic!("branch definitions retain the source graph");
        };
        assert_eq!(graph.blocks.len(), 3);
        for block in graph.blocks.iter().skip(1) {
            let [
                target_operations::TargetUnitOperation::ScalarDefinition {
                    expression: TargetScalarExpression::Integer { expression, .. },
                    ..
                },
            ] = block.operations.as_slice()
            else {
                panic!("each arm defines its result once");
            };
            let (TargetIntegerExpression::WrappingAdd { left, right, .. }
            | TargetIntegerExpression::SaturatingMultiply { left, right, .. }) = expression
            else {
                panic!("each arm retains its own arithmetic");
            };
            assert_eq!(left, right);
            assert!(
                matches!(left.as_ref(), TargetIntegerExpression::BlockParameter(parameter) if parameter.block == block.block)
            );
        }
    }
}

#[test]
fn scalar_graph_retains_repeated_value_definitions_once() {
    for (with_call, wrapping) in [(false, false), (true, false), (false, true)] {
        let mut plan = if with_call {
            direct_call_plan(1)
        } else {
            parameter_return_plan(1)
        };
        for function in &mut plan.functions {
            function.block_entries = vec![abstract_operations::AbstractBlockEntry {
                block: function.entry,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operation_offset: 0,
            }];
        }
        let function = &mut plan.functions[0];
        let mut returned = function.operations.pop().unwrap();
        let mut previous = if with_call {
            let AbstractOperation::Call {
                result,
                requirement_obligations,
                crash_continuations,
                ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            requirement_obligations.clear();
            crash_continuations.clear();
            *result
        } else {
            function.parameters[0].value
        };
        let ScalarType::Integer(integer) = function.parameters[0].scalar_type else {
            unreachable!()
        };
        let depth = 64;
        for position in 0..depth {
            let result = ValueId::new(1000 + position).unwrap();
            let operation = if wrapping {
                AbstractOperation::WrappingIntegerAdd {
                    psi_operation: OperationId::new(1000 + position).unwrap(),
                    result,
                    scalar_type: integer,
                    left: previous,
                    right: previous,
                }
            } else {
                AbstractOperation::ExactIntegerAdd {
                    psi_operation: OperationId::new(1000 + position).unwrap(),
                    obligation: ObligationId::new(1000 + position).unwrap(),
                    result,
                    scalar_type: integer,
                    left: previous,
                    right: previous,
                }
            };
            function.operations.push(operation);
            previous = result;
        }
        let AbstractOperation::Return { value, .. } = &mut returned else {
            unreachable!()
        };
        *value = previous;
        function.operations.push(returned);
        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        let receipt =
            validate_abstract_to_target_translation(&plan, NativeTarget::linux_x64(), &lowered)
                .expect("ordinary graphs do not enter legacy expression validators");
        assert!(receipt.function_roster().iter().all(|function| matches!(
            function.translation(),
            AbstractToTargetFunctionTranslationDisposition::Uncovered
        )));
        let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
            panic!("computed scalars must retain the source graph");
        };
        let definitions = graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        assert_eq!(definitions.len(), depth as usize + usize::from(with_call));
        assert_eq!(
            definitions
                .iter()
                .filter(|operation| matches!(
                    operation,
                    target_operations::TargetUnitOperation::ScalarCall { .. }
                ))
                .count(),
            usize::from(with_call)
        );
        for operation in definitions.into_iter().skip(usize::from(with_call)) {
            let target_operations::TargetUnitOperation::ScalarDefinition {
                expression:
                    TargetScalarExpression::Integer {
                        expression:
                            TargetIntegerExpression::ExactAdd { left, right, .. }
                            | TargetIntegerExpression::WrappingAdd { left, right, .. },
                        ..
                    },
                ..
            } = operation
            else {
                panic!("one scalar definition per addition");
            };
            assert_eq!(left, right);
            assert!(matches!(
                left.as_ref(),
                TargetIntegerExpression::Parameter { .. } | TargetIntegerExpression::ScalarHome(_)
            ));
        }
    }
}
