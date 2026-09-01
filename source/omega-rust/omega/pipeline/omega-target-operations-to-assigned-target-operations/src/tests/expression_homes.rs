use crate::assignment::shared::{
    Architecture, AssignedCallDestination, MachineId, MachineRegister, ScalarParameterLocation,
    TargetOperationPlan, ValueId,
};
use crate::{AssignmentError, assign_registers};
use omega_assigned_target_operations::{
    AssignedIntegerExpression, AssignedOperation, AssignedScalarExpression, AssignedScalarLocation,
};
use omega_calling_conventions::{
    IndirectPointerLocation, ValueLocation, ValuePlacement, ValueShape,
};
use omega_target::NativeTarget;
use omega_target_operations::{
    TargetCallArgument, TargetFunction, TargetIntegerExpression, TargetOperation,
    TargetScalarExpression, TerminalPsiProvenance,
};
use psi_core::{
    EdgeId, IntegerSign, IntegerType, ObligationId, OperationId, PlaceId, ScalarType,
    StructuralFieldId,
};
use psi_terminal::{
    CrashCause, CrashRouteBucket, CrashRouteGuard, SemanticFingerprint, TerminalPsiIdentity,
    VocabularyMarker,
};

#[test]
fn aarch64_expression_registers_receive_stable_frame_spills() {
    let plan = expression_plan(
        NativeTarget::linux_arm64(),
        ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
        ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
    );
    let assigned = assign_registers(&plan).expect("assign AArch64 homes");
    let AssignedOperation::ReturnIntegerExpression {
        frame, expression, ..
    } = &assigned.functions[0].operation
    else {
        panic!("fixture must remain an expression")
    };
    assert_eq!(frame.byte_size, 16);
    assert_eq!(frame.register_spills.len(), 2);
    assert_eq!(frame.register_spills[0].byte_offset, 0);
    assert_eq!(frame.register_spills[1].byte_offset, 8);
    let AssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
        panic!("fixture must remain wrapping addition")
    };
    assert!(matches!(
        left.as_ref(),
        AssignedIntegerExpression::Parameter {
            location: AssignedScalarLocation::FrameSpill { byte_offset: 0 },
            ..
        }
    ));
    assert!(matches!(
        right.as_ref(),
        AssignedIntegerExpression::Parameter {
            location: AssignedScalarLocation::FrameSpill { byte_offset: 8 },
            ..
        }
    ));
}

#[test]
fn x86_expression_registers_remain_explicit_without_a_frame() {
    let plan = expression_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ScalarParameterLocation::IncomingStack { byte_offset: 16 },
    );
    let assigned = assign_registers(&plan).expect("assign x86-64 homes");
    let AssignedOperation::ReturnIntegerExpression {
        frame, expression, ..
    } = &assigned.functions[0].operation
    else {
        panic!("fixture must remain an expression")
    };
    assert_eq!(frame.byte_size, 0);
    assert!(frame.register_spills.is_empty());
    let AssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
        panic!("fixture must remain wrapping addition")
    };
    assert!(matches!(
        left.as_ref(),
        AssignedIntegerExpression::Parameter {
            location: AssignedScalarLocation::Register(MachineRegister::X86Rdi),
            ..
        }
    ));
    assert!(matches!(
        right.as_ref(),
        AssignedIntegerExpression::Parameter {
            location: AssignedScalarLocation::IncomingStack { byte_offset: 16 },
            ..
        }
    ));
}

#[test]
fn exact_arithmetic_obligation_survives_register_assignment() {
    let obligation = ObligationId::new(17).expect("obligation");
    let mut plan = expression_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ScalarParameterLocation::Register(MachineRegister::X86Rsi),
    );
    let TargetOperation::ReturnIntegerExpression { expression, .. } =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    let TargetIntegerExpression::WrappingAdd {
        psi_operation,
        left,
        right,
    } = std::mem::replace(
        expression,
        TargetIntegerExpression::Immediate {
            source_value: ValueId::new(3).expect("result"),
            value: psi_core::IntegerValue::Unsigned(0),
        },
    )
    else {
        unreachable!()
    };
    *expression = TargetIntegerExpression::ExactAdd {
        psi_operation,
        obligation,
        left,
        right,
    };

    let assigned = assign_registers(&plan).expect("assign exact arithmetic homes");
    let AssignedOperation::ReturnIntegerExpression { expression, .. } =
        &assigned.functions[0].operation
    else {
        panic!("fixture must remain an expression")
    };
    assert!(matches!(
        expression,
        AssignedIntegerExpression::ExactAdd {
            obligation: retained,
            ..
        } if *retained == obligation
    ));
}

#[test]
fn integer_structural_field_custody_survives_register_assignment() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let psi_operation = OperationId::new(19).expect("field operation");
    let source_value = ValueId::new(20).expect("field value");
    let source = PlaceId::new(21).expect("structural source");
    let field = StructuralFieldId::new(22).expect("field identity");
    let source_placement = ValuePlacement {
        shape: ValueShape::integer(24, 8),
        locations: vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdi),
            copy_stack_byte_offset: None,
            byte_size: 24,
            alignment: 8,
        }],
    };
    let mut plan = expression_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ScalarParameterLocation::Register(MachineRegister::X86Rsi),
    );
    let TargetOperation::ReturnIntegerExpression {
        scalar_type,
        expression,
        ..
    } = &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    *scalar_type = integer_type;
    *expression = TargetIntegerExpression::StructuralField {
        psi_operation,
        source_value,
        source,
        field,
        source_placement: source_placement.clone(),
        field_byte_offset: 12,
        integer_type,
    };

    let assigned = assign_registers(&plan).expect("assign structural integer field");
    let AssignedOperation::ReturnIntegerExpression {
        frame, expression, ..
    } = &assigned.functions[0].operation
    else {
        panic!("fixture must remain an integer expression")
    };
    assert_eq!(frame.byte_size, 0);
    assert!(frame.register_spills.is_empty());
    assert_eq!(
        expression,
        &AssignedIntegerExpression::StructuralField {
            psi_operation,
            source_value,
            source,
            field,
            source_placement,
            field_byte_offset: 12,
            integer_type,
        }
    );
}

#[test]
fn x86_scratch_conflicting_parameter_receives_a_frame_spill() {
    let plan = expression_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86R10),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
    );
    let assigned = assign_registers(&plan).expect("assign x86-64 scratch conflict");
    let AssignedOperation::ReturnIntegerExpression {
        frame, expression, ..
    } = &assigned.functions[0].operation
    else {
        panic!("fixture must remain an expression")
    };
    assert_eq!(frame.byte_size, 16);
    assert_eq!(frame.register_spills.len(), 1);
    assert_eq!(frame.register_spills[0].register, MachineRegister::X86R10);
    let AssignedIntegerExpression::WrappingAdd { left, .. } = expression else {
        panic!("fixture must remain wrapping addition")
    };
    assert!(matches!(
        left.as_ref(),
        AssignedIntegerExpression::Parameter {
            location: AssignedScalarLocation::FrameSpill { byte_offset: 0 },
            ..
        }
    ));
}

#[test]
fn x86_calling_expression_spills_live_caller_registers() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let mut plan = expression_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ScalarParameterLocation::Register(MachineRegister::X86Rsi),
    );
    let TargetOperation::ReturnIntegerExpression { expression, .. } =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    *expression = TargetIntegerExpression::WrappingAdd {
        psi_operation: OperationId::new(8).unwrap(),
        left: Box::new(TargetIntegerExpression::Call {
            psi_operation: OperationId::new(7).unwrap(),
            source_value: ValueId::new(4).unwrap(),
            callee: MachineId::new(2).unwrap(),
            arguments: vec![TargetCallArgument {
                scalar_type: ScalarType::Integer(scalar_type),
                location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                expression: TargetScalarExpression::Integer {
                    scalar_type,
                    expression: TargetIntegerExpression::Parameter {
                        source_value: ValueId::new(1).unwrap(),
                        parameter_index: 0,
                        location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    },
                },
            }],
            requirement_obligations: vec![ObligationId::new(18).unwrap()],
            crash_continuations: vec![CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
        }),
        right: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).unwrap(),
            parameter_index: 0,
            location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        }),
    };

    let assigned = assign_registers(&plan).expect("assign call-preserved parameter");
    let AssignedOperation::ReturnIntegerExpression {
        frame, expression, ..
    } = &assigned.functions[0].operation
    else {
        unreachable!()
    };
    assert_eq!(frame.byte_size, 32);
    assert_eq!(frame.register_spills.len(), 1);
    let AssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
        unreachable!()
    };
    let AssignedIntegerExpression::Call {
        arguments,
        requirement_obligations,
        crash_continuations,
        ..
    } = left.as_ref()
    else {
        unreachable!()
    };
    assert_eq!(requirement_obligations, &[ObligationId::new(18).unwrap()]);
    assert_eq!(
        crash_continuations,
        &[CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        }]
    );
    assert!(matches!(
        &arguments[0].expression,
        AssignedScalarExpression::Integer {
            expression: AssignedIntegerExpression::Parameter {
                location: AssignedScalarLocation::FrameSpill { byte_offset: 0 },
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        right.as_ref(),
        AssignedIntegerExpression::Parameter {
            location: AssignedScalarLocation::FrameSpill { byte_offset: 0 },
            ..
        }
    ));
}

#[test]
fn call_stack_arguments_receive_concrete_outgoing_homes() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let mut plan = expression_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ScalarParameterLocation::Register(MachineRegister::X86Rsi),
    );
    let TargetOperation::ReturnIntegerExpression { expression, .. } =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    *expression = TargetIntegerExpression::Call {
        psi_operation: OperationId::new(7).unwrap(),
        source_value: ValueId::new(4).unwrap(),
        callee: MachineId::new(2).unwrap(),
        arguments: vec![TargetCallArgument {
            scalar_type: ScalarType::Integer(scalar_type),
            location: ScalarParameterLocation::IncomingStack { byte_offset: 8 },
            expression: TargetScalarExpression::Integer {
                scalar_type,
                expression: TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(5).unwrap(),
                    value: psi_core::IntegerValue::Unsigned(9),
                },
            },
        }],
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };

    let assigned = assign_registers(&plan).expect("assign outgoing stack argument");
    let AssignedOperation::ReturnIntegerExpression {
        frame, expression, ..
    } = &assigned.functions[0].operation
    else {
        unreachable!()
    };
    assert_eq!(frame.byte_size, 16);
    let AssignedIntegerExpression::Call { arguments, .. } = expression else {
        unreachable!()
    };
    assert_eq!(arguments[0].spill_byte_offset, 0);
    assert_eq!(
        arguments[0].destination,
        AssignedCallDestination::OutgoingStack { byte_offset: 8 }
    );
}

#[test]
fn x86_stack_pointer_cannot_be_an_expression_parameter_home() {
    let plan = expression_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rsp),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
    );
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::ExpressionRegisterCannotHoldParameter {
            register: MachineRegister::X86Rsp,
            ..
        })
    ));
}

#[test]
fn repeated_parameter_location_drift_rejects_before_emission() {
    let mut plan = expression_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ScalarParameterLocation::Register(MachineRegister::X86Rsi),
    );
    let TargetOperation::ReturnIntegerExpression { expression, .. } =
        &mut plan.functions[0].operation
    else {
        panic!("fixture must contain an expression")
    };
    let TargetIntegerExpression::WrappingAdd { right, .. } = expression else {
        panic!("fixture must contain wrapping addition")
    };
    let TargetIntegerExpression::Parameter {
        parameter_index, ..
    } = right.as_mut()
    else {
        panic!("right operand must be a parameter")
    };
    *parameter_index = 0;
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::ExpressionParameterLocationConflict {
            parameter_index: 0,
            ..
        })
    ));
}

#[test]
fn cross_architecture_register_rejects_during_assignment() {
    let plan = expression_plan(
        NativeTarget::linux_arm64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
    );
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::ParameterRegisterArchitectureMismatch {
            architecture: Architecture::Aarch64,
            ..
        })
    ));
}

fn expression_plan(
    target: NativeTarget,
    left_location: ScalarParameterLocation,
    right_location: ScalarParameterLocation,
) -> TargetOperationPlan {
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
        },
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnIntegerExpression {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value: ValueId::new(3).expect("result"),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                expression: TargetIntegerExpression::WrappingAdd {
                    psi_operation: OperationId::new(1).expect("operation"),
                    left: Box::new(TargetIntegerExpression::Parameter {
                        source_value: ValueId::new(1).expect("left"),
                        parameter_index: 0,
                        location: left_location,
                    }),
                    right: Box::new(TargetIntegerExpression::Parameter {
                        source_value: ValueId::new(2).expect("right"),
                        parameter_index: 1,
                        location: right_location,
                    }),
                },
            },
        }],
    }
}
