use crate::assignment::shared::*;
use crate::{AssignmentError, assign_registers};
use omega_assigned_target_operations::{
    AssignedBooleanControl, AssignedIntegerExpression, AssignedOperation, AssignedScalarExpression,
    AssignedScalarLocation,
};
use omega_target::NativeTarget;
use omega_target_operations::{
    TargetBooleanControl, TargetCallArgument, TargetConditionalBooleanArm, TargetFunction,
    TargetIntegerExpression, TargetOperation, TargetScalarExpression, TargetStructuralParameter,
    TerminalPsiProvenance,
};
use psi_core::{
    EdgeId, IntegerSign, IntegerType, ObligationId, OperationId, PlaceId, ScalarType,
    StructuralTypeId,
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    SemanticFingerprint, StructuralMultiplicity, StructuralPathSegment,
    TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const RANKED_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

const RANKED_RECEIVER_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root { token: Token; }

    machine Root::countdown(&mut self, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(remaining - 1)
            _ -> done()
        }
        state done(&mut self) {}
    }
"#;

fn ranked_target(target: NativeTarget) -> omega_target_operations::TargetOperationPlan {
    ranked_target_from(RANKED_COUNTDOWN_SOURCE, target)
}

fn ranked_target_from(
    source: &str,
    target: NativeTarget,
) -> omega_target_operations::TargetOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::countdown")
        .expect("lower terminal countdown");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("semantic");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("proof");
    let ranked =
        omega_psi_to_abstract_operations::lower_artifact_sections_for_native_ranked_countdown(
            &semantic,
            &proof,
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .expect("admit native ranked countdown");
    omega_abstract_operations_to_target_operations::lower_ranked_to_target_operations(
        &ranked, target,
    )
    .expect("lower ranked target")
}

#[test]
fn ranked_receiver_assignment_replays_semantic_identity_and_pointer_placement() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = ranked_target_from(RANKED_RECEIVER_COUNTDOWN_SOURCE, target);
        let assigned = assign_registers(&target_plan).expect("assign persistent receiver");
        let AssignedOperation::RankedU32Countdown(countdown) = &assigned.functions[0].operation
        else {
            panic!("assigned ranked carrier")
        };
        let [replay] = countdown.custody.semantic_replay.machines[0]
            .structural_parameters
            .as_slice()
        else {
            panic!("one replay receiver")
        };
        let [physical] = countdown.structural_parameters.as_slice() else {
            panic!("one physical receiver")
        };
        assert!(replay.is_self);
        assert_eq!(physical.place, replay.place);
        assert_eq!(physical.structural_type, replay.structural_type);
        assert_eq!(physical.multiplicity, replay.multiplicity);
        assert_eq!(physical.access, replay.access);
        assert_eq!(physical.shape, ValueShape::integer(8, 8));
        assert!(countdown.cleanup_actions.is_empty());

        let mut forged = target_plan.clone();
        let TargetOperation::RankedU32Countdown(candidate) = &mut forged.functions[0].operation
        else {
            unreachable!()
        };
        candidate.structural_parameters[0].access = psi_terminal::StructuralAccess::Owned;
        let source = candidate.custody.graph.initial_value;
        assert_eq!(
            assign_registers(&forged),
            Err(AssignmentError::RankedCountdownAbiMismatch(source))
        );
    }
}

#[test]
fn ranked_countdown_assignment_preserves_custody_and_uses_the_initial_register() {
    for (target, expected_register) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let target_plan = ranked_target(target);
        let TargetOperation::RankedU32Countdown(target_countdown) =
            &target_plan.functions[0].operation
        else {
            panic!("target ranked carrier")
        };
        let assigned = assign_registers(&target_plan).expect("assign ranked countdown");
        assert_eq!(
            assigned.functions[0].provenance,
            target_plan.functions[0].provenance
        );
        let AssignedOperation::RankedU32Countdown(countdown) = &assigned.functions[0].operation
        else {
            panic!("assigned ranked carrier")
        };
        assert_eq!(countdown.custody, target_countdown.custody);
        assert_eq!(countdown.call_plan, target_countdown.call_plan);
        assert_eq!(
            countdown.structural_types,
            target_countdown.structural_types
        );
        assert_eq!(
            countdown.structural_parameters,
            target_countdown.structural_parameters
        );
        assert_eq!(countdown.cleanup_actions, target_countdown.cleanup_actions);
        assert_eq!(countdown.rank_home, expected_register);
    }
}

#[test]
fn ranked_countdown_assignment_rejects_stack_and_wrong_architecture_rank_homes() {
    let mut stacked = ranked_target(NativeTarget::linux_x64());
    let TargetOperation::RankedU32Countdown(countdown) = &mut stacked.functions[0].operation else {
        unreachable!()
    };
    let source = countdown.custody.graph.initial_value;
    countdown.call_plan.parameters[0].locations = vec![ValueLocation::Stack {
        stack_byte_offset: 0,
        value_byte_offset: 0,
        byte_size: 4,
        alignment: 4,
    }];
    assert_eq!(
        assign_registers(&stacked),
        Err(AssignmentError::RankedCountdownRequiresRegister(source))
    );

    let mut wrong_arch = ranked_target(NativeTarget::linux_x64());
    let TargetOperation::RankedU32Countdown(countdown) = &mut wrong_arch.functions[0].operation
    else {
        unreachable!()
    };
    countdown.call_plan.parameters[0].locations = vec![ValueLocation::Register {
        register: MachineRegister::Aarch64X(0),
        value_byte_offset: 0,
        byte_size: 4,
    }];
    assert_eq!(
        assign_registers(&wrong_arch),
        Err(AssignmentError::ParameterRegisterArchitectureMismatch {
            value: source,
            register: MachineRegister::Aarch64X(0),
            architecture: omega_target::Architecture::X86_64,
        })
    );

    let mut wrong_same_arch = ranked_target(NativeTarget::linux_x64());
    let TargetOperation::RankedU32Countdown(countdown) =
        &mut wrong_same_arch.functions[0].operation
    else {
        unreachable!()
    };
    countdown.call_plan.parameters[0].locations = vec![ValueLocation::Register {
        register: MachineRegister::X86Rsi,
        value_byte_offset: 0,
        byte_size: 4,
    }];
    assert_eq!(
        assign_registers(&wrong_same_arch),
        Err(AssignmentError::RankedCountdownAbiMismatch(source))
    );
}

#[test]
fn three_leaf_boolean_cleanup_assignment_retains_exact_edges() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = boolean_cleanup_plan(target);
        let assigned = assign_registers(&plan).expect("assign bounded Boolean cleanup");
        let AssignedOperation::BooleanControlWithCleanup {
            control,
            structural_parameters,
            cleanup_actions,
            ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must retain its Boolean cleanup carrier")
        };
        assert_eq!(structural_parameters.len(), 1);
        assert_eq!(cleanup_actions.len(), 1);
        let AssignedBooleanControl::Conditional {
            when_true,
            when_false,
            ..
        } = control
        else {
            panic!("root decision must survive assignment")
        };
        let AssignedBooleanControl::Conditional {
            when_true: nested_true,
            when_false: nested_false,
            ..
        } = when_true.control.as_ref()
        else {
            panic!("true arm must retain the nested decision")
        };
        assert!(matches!(
            nested_true.control.as_ref(),
            AssignedBooleanControl::ReturnImmediate {
                psi_return_edge,
                ..
            } if *psi_return_edge == EdgeId::new(10).unwrap()
        ));
        assert!(matches!(
            nested_false.control.as_ref(),
            AssignedBooleanControl::ReturnParameter {
                psi_return_edge,
                ..
            } if *psi_return_edge == EdgeId::new(11).unwrap()
        ));
        assert!(matches!(
            when_false.control.as_ref(),
            AssignedBooleanControl::ReturnNotParameter {
                psi_return_edge,
                ..
            } if *psi_return_edge == EdgeId::new(12).unwrap()
        ));
    }
}

#[test]
fn finite_boolean_cleanup_accepts_two_leaf_and_wider_trees() {
    let mut two_leaf = boolean_cleanup_plan(NativeTarget::linux_x64());
    let TargetOperation::BooleanControlWithCleanup { control, .. } =
        &mut two_leaf.functions[0].operation
    else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional { when_true, .. } = control else {
        unreachable!()
    };
    when_true.control = Box::new(boolean_immediate_return(13));
    assign_registers(&two_leaf).expect("assign two-leaf Boolean cleanup");

    let mut wider = boolean_cleanup_plan(NativeTarget::linux_x64());
    let location = boolean_cleanup_condition_location(&wider);
    let TargetOperation::BooleanControlWithCleanup { control, .. } =
        &mut wider.functions[0].operation
    else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional { when_true, .. } = control else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional {
        when_true: nested_true,
        ..
    } = when_true.control.as_mut()
    else {
        unreachable!()
    };
    nested_true.control = Box::new(TargetBooleanControl::Conditional {
        condition_source: ValueId::new(1).unwrap(),
        condition_parameter_index: 0,
        condition_location: location,
        when_true: boolean_arm(20, boolean_immediate_return(20)),
        when_false: boolean_arm(21, boolean_immediate_return(21)),
    });
    assign_registers(&wider).expect("assign wider Boolean cleanup");
}

#[test]
fn finite_boolean_cleanup_requires_distinct_return_edges() {
    let mut plan = boolean_cleanup_plan(NativeTarget::linux_x64());
    let TargetOperation::BooleanControlWithCleanup { control, .. } =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional { when_true, .. } = control else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional { when_false, .. } = when_true.control.as_mut() else {
        unreachable!()
    };
    when_false.control = Box::new(boolean_immediate_return(10));
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::UnsupportedScalarCleanup(_))
    ));
}

#[test]
fn finite_boolean_cleanup_rejects_misaligned_cleanup_signature() {
    let mut plan = boolean_cleanup_plan(NativeTarget::linux_x64());
    let TargetOperation::BooleanControlWithCleanup {
        cleanup_actions, ..
    } = &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::UnsupportedScalarCleanup(_))
    ));
}

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
    let AssignedIntegerExpression::Call { arguments, .. } = left.as_ref() else {
        unreachable!()
    };
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

#[test]
fn unit_assignment_retains_typed_structural_argument_paths() {
    let target = NativeTarget::linux_x64();
    let shape = omega_calling_conventions::ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let place = PlaceId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let path = vec![StructuralPathSegment::FixedIndex(1)];
    let plan = TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
        },
        target,
        entry: MachineId::new(1).unwrap(),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::UnitBody(omega_target_operations::TargetUnitBody {
                structural_types: Vec::new(),
                call_plan: call_plan.clone(),
                parameters: Vec::new(),
                operations: vec![TargetUnitOperation::Call {
                    psi_operation: OperationId::new(1).unwrap(),
                    callee: MachineId::new(2).unwrap(),
                    arguments: vec![omega_target_operations::TargetStructuralArgument {
                        place,
                        access: psi_terminal::StructuralAccess::Owned,
                        path: path.clone(),
                        root_structural_type: structural_type,
                        structural_type,
                        shape,
                        source_byte_offset: 0,
                        fixed_array_length: None,
                        element_stride: None,
                        source: call_plan.parameters[0].clone(),
                        destination: call_plan.parameters[0].clone(),
                    }],
                    claim_transfers: Vec::new(),
                }],
            }),
        }],
    };

    let assigned = assign_registers(&plan).unwrap();
    let AssignedOperation::UnitBody(body) = &assigned.functions[0].operation else {
        panic!("Unit body")
    };
    let AssignedUnitOperation::Call { copies, .. } = &body.operations[0] else {
        panic!("Unit call")
    };
    assert_eq!(copies[0].path, path);
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

fn boolean_cleanup_plan(target: NativeTarget) -> TargetOperationPlan {
    let scalar_shape = omega_calling_conventions::ValueShape::integer(1, 1);
    let structural_shape = omega_calling_conventions::ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![scalar_shape, structural_shape],
            result: Some(scalar_shape),
        },
    )
    .expect("bounded Boolean cleanup ABI");
    let [ValueLocation::Register { register, .. }] = call_plan.parameters[0].locations.as_slice()
    else {
        panic!("first Boolean input must have one direct register home")
    };
    let condition_location = ScalarParameterLocation::Register(*register);
    let nested = TargetBooleanControl::Conditional {
        condition_source: ValueId::new(1).unwrap(),
        condition_parameter_index: 0,
        condition_location,
        when_true: boolean_arm(4, boolean_immediate_return(10)),
        when_false: boolean_arm(
            5,
            TargetBooleanControl::ReturnParameter {
                psi_return_edge: EdgeId::new(11).unwrap(),
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
                location: condition_location,
            },
        ),
    };
    let place = PlaceId::new(1).unwrap();
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        target,
        entry: MachineId::new(1).unwrap(),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: (1..=5)
                    .chain(10..=12)
                    .map(|edge| EdgeId::new(edge).unwrap())
                    .collect(),
            },
            operation: TargetOperation::BooleanControlWithCleanup {
                control: TargetBooleanControl::Conditional {
                    condition_source: ValueId::new(1).unwrap(),
                    condition_parameter_index: 0,
                    condition_location,
                    when_true: boolean_arm(2, nested),
                    when_false: boolean_arm(
                        3,
                        TargetBooleanControl::ReturnNotParameter {
                            psi_return_edge: EdgeId::new(12).unwrap(),
                            source_value: ValueId::new(1).unwrap(),
                            parameter_index: 0,
                            location: condition_location,
                        },
                    ),
                },
                structural_types: Vec::new(),
                call_plan: call_plan.clone(),
                structural_parameters: vec![TargetStructuralParameter {
                    place,
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: psi_terminal::StructuralAccess::Owned,
                    shape: structural_shape,
                    placement: call_plan.parameters[1].clone(),
                }],
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place)],
            },
        }],
    }
}

fn boolean_arm(edge: u64, control: TargetBooleanControl) -> TargetConditionalBooleanArm {
    TargetConditionalBooleanArm {
        psi_edge: EdgeId::new(edge).unwrap(),
        control: Box::new(control),
    }
}

fn boolean_immediate_return(edge: u64) -> TargetBooleanControl {
    TargetBooleanControl::ReturnImmediate {
        psi_return_edge: EdgeId::new(edge).unwrap(),
        source_value: ValueId::new(edge).unwrap(),
        value: edge % 2 == 0,
    }
}

fn boolean_cleanup_condition_location(plan: &TargetOperationPlan) -> ScalarParameterLocation {
    let TargetOperation::BooleanControlWithCleanup { control, .. } = &plan.functions[0].operation
    else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional {
        condition_location, ..
    } = control
    else {
        unreachable!()
    };
    *condition_location
}
