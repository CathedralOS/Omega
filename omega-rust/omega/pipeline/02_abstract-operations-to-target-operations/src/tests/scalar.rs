use super::super::{LoweringError, TargetLoweringRequest, lower_to_target_operations};
use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractParameter, AbstractResult, BlockId, EdgeId, IntegerType, MachineId, NativeTarget,
    ObligationId, OperationId, PlaceId, ScalarType, TargetIntegerExpression, TargetUnitOperation,
    TerminalAffineCleanupAction, ValueId, identity,
};
mod fixtures;
mod shared_values;
use fixtures::{direct_call_plan, parameter_return_plan};

#[test]
fn common_graph_rejects_unsupported_ieee_float_fma_realization() {
    use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (format, constant) in [
            (IeeeFloatFormat::Binary32, IeeeFloatValue::Binary32(0)),
            (IeeeFloatFormat::Binary64, IeeeFloatValue::Binary64(0)),
        ] {
            let mut source = parameter_return_plan(1);
            let function = &mut source.functions[0];
            function.parameters.clear();
            function.result = AbstractFunctionResult::Unit;
            function.operations = vec![AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(10).unwrap(),
                cleanup_actions: Vec::new(),
            }];
            let lowered =
                lower_to_target_operations(&source, TargetLoweringRequest::new(target)).unwrap();
            let value = ValueId::new(1).unwrap();
            let operation = OperationId::new(2).unwrap();
            source.functions[0].operations.splice(
                0..0,
                [
                    AbstractOperation::IeeeFloatConstant {
                        psi_operation: OperationId::new(1).unwrap(),
                        result: value,
                        value: constant,
                    },
                    AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                        psi_operation: operation,
                        result: ValueId::new(2).unwrap(),
                        format,
                        left: value,
                        right: value,
                        addend: value,
                    },
                ],
            );
            assert_eq!(
                lower_to_target_operations(&source, TargetLoweringRequest::new(target)),
                Err(LoweringError::UnsupportedIeeeFloatFmaRealization(operation))
            );
            // A target graph that omits the unsupported source occurrence
            // cannot turn independently replayed translation into admission.
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, target, &lowered),
                Err(
                    crate::AbstractToTargetTranslationValidationError::UnsupportedIeeeFloatFmaRealization(
                        operation
                    )
                )
            );
        }
    }
}

#[test]
fn scalar_graph_call_keeps_callee_abi_and_effect_custody() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = direct_call_plan(9);
        let lowered =
            lower_to_target_operations(&source, TargetLoweringRequest::new(target)).unwrap();
        let graph = &lowered.functions[0].graph;
        let [
            TargetUnitOperation::Call {
                scalar_arguments: arguments,
                call_plan,
                requirement_obligations,
                crash_continuations,
                result: target_operations::TargetCallResult::Scalar(result_home),
                ..
            },
        ] = graph.blocks[0].operations.as_slice()
        else {
            panic!("one scalar call")
        };
        assert_eq!(arguments.len(), 9);
        for (argument, placement) in arguments.iter().zip(&call_plan.parameters) {
            assert_eq!(&argument.placement, placement);
        }
        assert_eq!(requirement_obligations, &[ObligationId::new(700).unwrap()]);
        assert_eq!(crash_continuations.len(), 1);
        assert!(matches!(&graph.blocks[0].terminator,
            target_operations::TargetControlTerminator::ReturnScalar {
                expression: target_operations::TargetScalarExpression::Integer {
                    expression: TargetIntegerExpression::ScalarHome(home), ..
                }, ..
            } if home == result_home));
    }
}

#[test]
fn scalar_graph_rejects_missing_block_ownership() {
    let mut source = parameter_return_plan(1);
    source.functions[0].block_entries.clear();
    assert!(matches!(
        lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::UnsupportedControlFlow { .. })
    ));
}

#[test]
fn scalar_graph_rejects_undefined_returns_and_unimplemented_cleanup() {
    let mut source = parameter_return_plan(1);
    let unknown = ValueId::new(999).unwrap();
    let AbstractOperation::Return { value, .. } = &mut source.functions[0].operations[0] else {
        panic!("return")
    };
    *value = unknown;
    assert_eq!(
        lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::UnknownValue(unknown))
    );
    let mut source = parameter_return_plan(1);
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut source.functions[0].operations[0]
    else {
        panic!("return")
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
        PlaceId::new(999).unwrap(),
    ));
    assert!(
        lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        )
        .is_err()
    );
}
