//! Literal sources retain their defining occurrence, SSA identity and IEEE payload.
use super::*;
use semantic_vocabulary::IeeeFloatValue;

fn literal_plan(value: IeeeFloatValue, call: bool) -> AbstractOperationPlan {
    let mut source = plan(value.format(), true);
    if call {
        let mut caller = source.functions[0].clone();
        caller.machine = MachineId::new(2).unwrap();
        caller.operations[0] = AbstractOperation::CallUnit {
            psi_operation: OperationId::new(2).unwrap(),
            callee: source.entry,
            arguments: vec![ValueId::new(1).unwrap()],
            structural_arguments: vec![StructuralArgument {
                place: caller.structural_parameters[0].place,
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        source.entry = caller.machine;
        source.functions.push(caller);
    }
    let function = source.functions.last_mut().unwrap();
    function.parameters[0].value = ValueId::new(99).unwrap();
    function.operations.insert(
        0,
        AbstractOperation::IeeeFloatConstant {
            psi_operation: OperationId::new(3).unwrap(),
            result: ValueId::new(1).unwrap(),
            value,
        },
    );
    source
}

#[test]
fn field_and_unit_call_literals_retain_exact_ieee_sources_on_four_targets() {
    for value in [
        IeeeFloatValue::Binary32(0x7fa0_0041),
        IeeeFloatValue::Binary32(0x8000_0000),
        IeeeFloatValue::Binary32(1),
        IeeeFloatValue::Binary64(0x7ff0_0000_0000_0041),
        IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
        IeeeFloatValue::Binary64(1),
    ] {
        for call in [false, true] {
            let source = literal_plan(value, call);
            for target in [
                NativeTarget::linux_x64(),
                NativeTarget::windows_x64(),
                NativeTarget::linux_arm64(),
                NativeTarget::macos_arm64(),
            ] {
                let lowered = lower_to_target_operations(&source, target).unwrap();
                let TargetOperation::UnitBody(body) = &lowered.functions.last().unwrap().operation
                else {
                    panic!("literal uses ordinary Unit body")
                };
                assert!(
                    matches!(body.operations[0], TargetUnitOperation::IeeeFloatConstant {
                    psi_operation, result, value: actual
                } if psi_operation == OperationId::new(3).unwrap()
                    && result == ValueId::new(1).unwrap() && actual == value)
                );
                let literal = match &body.operations[1] {
                    TargetUnitOperation::Call {
                        scalar_arguments,
                        call_plan,
                        arguments,
                        ..
                    } => {
                        assert_eq!(
                            scalar_arguments[0].placement.shape,
                            ValueShape::float(if value.format() == IeeeFloatFormat::Binary32 {
                                4
                            } else {
                                8
                            })
                        );
                        assert_eq!(scalar_arguments[0].placement, call_plan.parameters[0]);
                        assert_eq!(arguments[0].access, StructuralAccess::WriteOnlyBorrow);
                        &scalar_arguments[0].source
                    }
                    TargetUnitOperation::StructuralScalarFieldStore { source, .. } => source,
                    _ => panic!("literal consumer"),
                };
                assert_eq!(
                    *literal,
                    TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                        defining_operation: OperationId::new(3).unwrap(),
                        source_value: ValueId::new(1).unwrap(),
                        value,
                    }
                );
            }
        }
    }
}

#[test]
fn literal_consumers_reject_wrong_format_missing_or_late_definition() {
    for value in [IeeeFloatValue::Binary32(1), IeeeFloatValue::Binary64(1)] {
        for call in [false, true] {
            let valid = literal_plan(value, call);
            lower_to_target_operations(&valid, NativeTarget::macos_arm64()).unwrap();
            for mutation in 0..4 {
                let mut source = valid.clone();
                let function = source.functions.last_mut().unwrap();
                match mutation {
                    0 => {
                        let AbstractOperation::IeeeFloatConstant { value: actual, .. } =
                            &mut function.operations[0]
                        else {
                            unreachable!()
                        };
                        *actual = match value {
                            IeeeFloatValue::Binary32(_) => IeeeFloatValue::Binary64(1),
                            IeeeFloatValue::Binary64(_) => IeeeFloatValue::Binary32(1),
                        };
                    }
                    1 => {
                        function.operations.remove(0);
                    }
                    2 => {
                        function.operations.swap(0, 1);
                    }
                    3 => {
                        let AbstractOperation::IeeeFloatConstant { result, .. } =
                            &mut function.operations[0]
                        else {
                            unreachable!()
                        };
                        *result = ValueId::new(2).unwrap();
                    }
                    _ => unreachable!(),
                }
                assert!(
                    lower_to_target_operations(&source, NativeTarget::macos_arm64()).is_err(),
                    "mutation {mutation}, call {call}"
                );
            }
        }
    }
}
