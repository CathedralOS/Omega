use omega_image_emission::{
    build_installation_record, build_object_artifact, decode_installation_record,
    derive_installation_stack_demand, derive_stack_demand, emit_executable_image,
    emit_object_container, encode_installation_record,
};
use omega_machine_code::ScalarControlFlowEvidence;
use omega_machine_emission::emit_machine_code;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{
    MachineRegister, ScalarParameterLocation, TargetBooleanExpression, TargetCallArgument,
    TargetConditionalIntegerArm, TargetFunction, TargetIntegerControl, TargetIntegerExpression,
    TargetOperation, TargetOperationPlan, TargetScalarExpression, TerminalPsiProvenance,
};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ProfileDecisionId,
    ValueId,
};
use psi_terminal::{CrashCause, SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[test]
fn division_argument_stack_facts_survive_assignment_emission_object_and_image() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = division_argument_plan(target);
        let assigned = assign_registers(&plan).expect("assign division call argument");
        let emitted = emit_machine_code(&assigned).expect("emit division call argument");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == machine_id(1))
            .expect("caller machine code");
        let stack = caller
            .scalar_stack
            .as_ref()
            .expect("division argument stack evidence");
        assert_eq!(caller.internal_calls.len(), 1);
        assert!(caller.internal_calls[0].scalar_stack.is_some());
        match target.architecture {
            Architecture::X86_64 => {
                let ScalarControlFlowEvidence::LinearWithDivisionBranches { branches } =
                    &stack.control_flow
                else {
                    panic!("signed x86 division argument must retain its generated diamond")
                };
                assert_eq!(branches.len(), 1);
                assert!(branches[0].merge_offset < caller.internal_calls[0].offset);
            }
            Architecture::Aarch64 => {
                assert_eq!(stack.control_flow, ScalarControlFlowEvidence::Linear);
            }
        }

        let artifact = build_object_artifact(&emitted)
            .expect("object boundary replays division argument stack paths");
        let demand = derive_stack_demand(&artifact, machine_id(1))
            .expect("compose division argument call closure");
        assert!(demand.ceiling_bytes() > 0);
        assert_eq!(demand.contributing_machines().len(), 2);
        let object = emit_object_container(&artifact);
        assert_eq!(object.output.relocations, 1);
        let image =
            emit_executable_image(&artifact, 7).expect("resolve division argument call image");
        assert_eq!(
            image.output().final_text_bytes.len(),
            artifact.text_bytes().len()
        );
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("build division argument installation record");
        let encoded = encode_installation_record(&installation)
            .expect("encode division argument installation record");
        let decoded = decode_installation_record(&encoded)
            .expect("decode division argument installation record");
        let installed = derive_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed division argument stack closure");
        assert_eq!(installed, demand);
    }
}

#[test]
fn conditional_arm_division_stack_facts_survive_object_image_and_installation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assign_registers(&conditional_arm_division_plan(target, false))
            .expect("assign conditional division arms");
        let emitted = emit_machine_code(&assigned).expect("emit conditional division arms");
        let stack = emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("conditional division stack evidence");
        assert!(matches!(
            stack.control_flow,
            ScalarControlFlowEvidence::ConditionalTree { .. }
        ));

        let artifact = build_object_artifact(&emitted)
            .expect("object boundary replays both branch-free division arms");
        let demand = derive_stack_demand(&artifact, machine_id(1))
            .expect("derive conditional division stack demand");
        let object = emit_object_container(&artifact);
        assert_eq!(object.output.relocations, 0);
        let image = emit_executable_image(&artifact, 11)
            .expect("emit conditional division executable image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(2).expect("profile decision"))
                .expect("build conditional division installation record");
        let encoded = encode_installation_record(&installation)
            .expect("encode conditional division installation record");
        let decoded = decode_installation_record(&encoded)
            .expect("decode conditional division installation record");
        let installed = derive_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed conditional division stack demand");
        assert_eq!(installed, demand);
    }
}

#[test]
fn signed_x86_conditional_division_diamond_survives_installation_and_rejects_forgery() {
    let target = NativeTarget::linux_x64();
    let assigned = assign_registers(&conditional_arm_division_plan(target, true))
        .expect("assign signed x86 conditional division");
    let emitted = emit_machine_code(&assigned).expect("emit signed x86 conditional division");
    let stack = emitted.functions[0]
        .scalar_stack
        .as_ref()
        .expect("signed x86 conditional division stack evidence");
    let ScalarControlFlowEvidence::ConditionalTree { branches, .. } = &stack.control_flow else {
        panic!("signed x86 conditional division must retain composite evidence")
    };
    assert_eq!(branches.len(), 1);

    let mut forged = emitted.clone();
    let ScalarControlFlowEvidence::ConditionalTree { branches, .. } = &mut forged.functions[0]
        .scalar_stack
        .as_mut()
        .expect("forged stack evidence")
        .control_flow
    else {
        unreachable!()
    };
    branches[0].ordinary_arm_offset += 1;
    assert!(build_object_artifact(&forged).is_err());

    let artifact = build_object_artifact(&emitted)
        .expect("object boundary replays the signed x86 conditional division diamond");
    let demand = derive_stack_demand(&artifact, machine_id(1))
        .expect("derive signed x86 conditional division stack demand");
    let image = emit_executable_image(&artifact, 19)
        .expect("emit signed x86 conditional division executable image");
    let installation =
        build_installation_record(&image, ProfileDecisionId::new(5).expect("profile decision"))
            .expect("build signed x86 conditional division installation record");
    let encoded = encode_installation_record(&installation)
        .expect("encode signed x86 conditional division installation record");
    let decoded = decode_installation_record(&encoded)
        .expect("decode signed x86 conditional division installation record");
    let installed = derive_installation_stack_demand(&decoded, &image, machine_id(1))
        .expect("recompose installed signed x86 conditional division demand");
    assert_eq!(installed, demand);
}

#[test]
fn signed_x86_return_crash_division_stack_facts_survive_installation() {
    let target = NativeTarget::linux_x64();
    let mut plan = conditional_arm_division_plan(target, true);
    let TargetOperation::ReturnIntegerConditionalControl { when_false, .. } =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    *when_false.control = TargetIntegerControl::Crash {
        psi_crash_edge: edge_id(4),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let assigned = assign_registers(&plan).expect("assign signed x86 return/crash division");
    let emitted =
        emit_machine_code(&assigned).expect("emit signed x86 return/crash division evidence");
    let ScalarControlFlowEvidence::ConditionalTree {
        crash_leaves,
        branches,
        ..
    } = &emitted.functions[0]
        .scalar_stack
        .as_ref()
        .expect("signed x86 return/crash division stack evidence")
        .control_flow
    else {
        panic!("signed x86 return/crash division must retain composite evidence")
    };
    assert_eq!(crash_leaves, &[false, true]);
    assert_eq!(branches.len(), 1);

    let artifact = build_object_artifact(&emitted)
        .expect("object boundary replays the return, crash, and division paths");
    let demand = derive_stack_demand(&artifact, machine_id(1))
        .expect("derive signed x86 return/crash division demand");
    let image = emit_executable_image(&artifact, 21)
        .expect("emit signed x86 return/crash division executable image");
    let installation =
        build_installation_record(&image, ProfileDecisionId::new(7).expect("profile decision"))
            .expect("build signed x86 return/crash division installation record");
    let encoded = encode_installation_record(&installation)
        .expect("encode signed x86 return/crash division installation record");
    let decoded = decode_installation_record(&encoded)
        .expect("decode signed x86 return/crash division installation record");
    let installed = derive_installation_stack_demand(&decoded, &image, machine_id(1))
        .expect("recompose installed signed x86 return/crash division demand");
    assert_eq!(installed, demand);
}

#[test]
fn conditional_return_crash_stack_facts_survive_installation_and_reject_forgery() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for crash_false_arm in [false, true] {
            let assigned =
                assign_registers(&conditional_return_crash_plan(target, crash_false_arm))
                    .expect("assign conditional return/crash");
            let emitted = emit_machine_code(&assigned).expect("emit conditional return/crash");
            let ScalarControlFlowEvidence::ConditionalTree {
                decisions,
                crash_leaves,
                ..
            } = &emitted.functions[0]
                .scalar_stack
                .as_ref()
                .expect("conditional return/crash stack evidence")
                .control_flow
            else {
                panic!("conditional return/crash must retain terminal evidence")
            };
            let branch = decisions[0];
            assert_eq!(
                crash_leaves,
                if crash_false_arm {
                    &[false, true]
                } else {
                    &[true, false]
                }
            );

            let mut forged = emitted.clone();
            let crash_region_end = if crash_false_arm {
                forged.functions[0].bytes.len()
            } else {
                branch.false_arm_offset
            };
            forged.functions[0].bytes[crash_region_end - 1] ^= 1;
            assert!(build_object_artifact(&forged).is_err());

            let artifact = build_object_artifact(&emitted)
                .expect("object boundary replays return and crash terminals");
            let demand = derive_stack_demand(&artifact, machine_id(1))
                .expect("derive conditional return/crash stack demand");
            let image = emit_executable_image(&artifact, 23 + u16::from(crash_false_arm))
                .expect("emit conditional return/crash executable image");
            let installation = build_installation_record(
                &image,
                ProfileDecisionId::new(6).expect("profile decision"),
            )
            .expect("build conditional return/crash installation record");
            let encoded = encode_installation_record(&installation)
                .expect("encode conditional return/crash installation record");
            let decoded = decode_installation_record(&encoded)
                .expect("decode conditional return/crash installation record");
            let installed = derive_installation_stack_demand(&decoded, &image, machine_id(1))
                .expect("recompose installed conditional return/crash demand");
            assert_eq!(installed, demand);
            assert!(branch.branch_offset + branch.branch_byte_count <= branch.false_arm_offset);
        }
    }
}

#[test]
fn conditional_two_crash_stack_facts_survive_installation_and_reject_forgery() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut plan = conditional_return_crash_plan(target, false);
        let TargetOperation::ReturnIntegerConditionalControl { when_false, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        *when_false.control = TargetIntegerControl::Crash {
            psi_crash_edge: edge_id(4),
            cause: CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        };
        let assigned = assign_registers(&plan).expect("assign two-crash conditional");
        let emitted = emit_machine_code(&assigned).expect("emit two-crash conditional");
        let ScalarControlFlowEvidence::ConditionalTree {
            decisions,
            crash_leaves,
            branches,
            ..
        } = &emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("two-crash conditional stack evidence")
            .control_flow
        else {
            panic!("two-crash conditional must retain terminal evidence")
        };
        assert_eq!(crash_leaves, &[true, true]);
        assert!(branches.is_empty());

        for crash_region_end in [
            decisions[0].false_arm_offset,
            emitted.functions[0].bytes.len(),
        ] {
            let mut forged = emitted.clone();
            forged.functions[0].bytes[crash_region_end - 1] ^= 1;
            assert!(build_object_artifact(&forged).is_err());
        }

        let artifact =
            build_object_artifact(&emitted).expect("object boundary replays both crash terminals");
        let demand = derive_stack_demand(&artifact, machine_id(1))
            .expect("derive two-crash conditional stack demand");
        let image = emit_executable_image(&artifact, 27)
            .expect("emit two-crash conditional executable image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(8).expect("profile decision"))
                .expect("build two-crash conditional installation record");
        let encoded = encode_installation_record(&installation)
            .expect("encode two-crash conditional installation record");
        let decoded = decode_installation_record(&encoded)
            .expect("decode two-crash conditional installation record");
        let installed = derive_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed two-crash conditional demand");
        assert_eq!(installed, demand);
    }
}

fn conditional_return_crash_plan(
    target: NativeTarget,
    crash_false_arm: bool,
) -> TargetOperationPlan {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let condition_register = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    };
    let returned = |edge, return_edge, source, value| TargetConditionalIntegerArm {
        psi_edge: edge_id(edge),
        control: Box::new(TargetIntegerControl::Return {
            psi_return_edge: edge_id(return_edge),
            source_value: value_id(source),
            expression: TargetIntegerExpression::Immediate {
                source_value: value_id(source),
                value: IntegerValue::Unsigned(value),
            },
        }),
    };
    let crashed = |edge, crash_edge| TargetConditionalIntegerArm {
        psi_edge: edge_id(edge),
        control: Box::new(TargetIntegerControl::Crash {
            psi_crash_edge: edge_id(crash_edge),
            cause: CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        }),
    };
    let (when_true, when_false) = if crash_false_arm {
        (returned(1, 3, 2, 1), crashed(2, 4))
    } else {
        (crashed(1, 3), returned(2, 4, 2, 1))
    };
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([46; 32]),
        },
        target,
        entry: machine_id(1),
        functions: vec![TargetFunction {
            machine: machine_id(1),
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: (1..=4).map(edge_id).collect(),
            },
            operation: TargetOperation::ReturnIntegerConditionalControl {
                condition_source: value_id(1),
                condition_parameter_index: 0,
                condition_location: ScalarParameterLocation::Register(condition_register),
                scalar_type,
                when_true,
                when_false,
            },
        }],
    }
}

#[test]
fn conditional_condition_division_stack_facts_survive_object_image_and_installation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assign_registers(&conditional_condition_division_plan(target))
            .expect("assign conditional division condition");
        let emitted = emit_machine_code(&assigned).expect("emit conditional division condition");
        let stack = emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("conditional division condition stack evidence");
        assert!(matches!(
            stack.control_flow,
            ScalarControlFlowEvidence::ConditionalTree { .. }
        ));

        let artifact = build_object_artifact(&emitted)
            .expect("object boundary replays the branch-free division condition");
        let demand = derive_stack_demand(&artifact, machine_id(1))
            .expect("derive conditional division condition stack demand");
        let object = emit_object_container(&artifact);
        assert_eq!(object.output.relocations, 0);
        let image = emit_executable_image(&artifact, 13)
            .expect("emit conditional division condition executable image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(3).expect("profile decision"))
                .expect("build conditional division condition installation record");
        let encoded = encode_installation_record(&installation)
            .expect("encode conditional division condition installation record");
        let decoded = decode_installation_record(&encoded)
            .expect("decode conditional division condition installation record");
        let installed = derive_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed conditional division condition stack demand");
        assert_eq!(installed, demand);
    }
}

#[test]
fn conditional_call_argument_division_stack_facts_survive_installation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assign_registers(&conditional_call_argument_division_plan(target))
            .expect("assign conditional division call arguments");
        let emitted = emit_machine_code(&assigned).expect("emit conditional division calls");
        let caller = &emitted.functions[0];
        let stack = caller
            .scalar_stack
            .as_ref()
            .expect("conditional division call stack evidence");
        assert!(matches!(
            stack.control_flow,
            ScalarControlFlowEvidence::ConditionalTree { .. }
        ));
        assert_eq!(caller.internal_calls.len(), 2);
        assert!(
            caller
                .internal_calls
                .iter()
                .all(|call| call.scalar_stack.is_some())
        );

        let artifact = build_object_artifact(&emitted)
            .expect("object boundary replays conditional division calls");
        let demand = derive_stack_demand(&artifact, machine_id(1))
            .expect("compose conditional division call closure");
        assert_eq!(demand.contributing_machines().len(), 3);
        let object = emit_object_container(&artifact);
        assert_eq!(object.output.relocations, 2);
        let image = emit_executable_image(&artifact, 17)
            .expect("emit conditional division call executable image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(4).expect("profile decision"))
                .expect("build conditional division call installation record");
        let encoded = encode_installation_record(&installation)
            .expect("encode conditional division call installation record");
        let decoded = decode_installation_record(&encoded)
            .expect("decode conditional division call installation record");
        let installed = derive_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed conditional division call closure");
        assert_eq!(installed, demand);
    }
}

fn conditional_call_argument_division_plan(target: NativeTarget) -> TargetOperationPlan {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let argument_register = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    };
    let argument_location = ScalarParameterLocation::Register(argument_register);
    let immediate = |source, value| TargetIntegerExpression::Immediate {
        source_value: value_id(source),
        value: IntegerValue::Unsigned(value),
    };
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([45; 32]),
        },
        target,
        entry: machine_id(1),
        functions: vec![
            TargetFunction {
                machine: machine_id(1),
                attachment: None,
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: (1..=5).map(operation_id).collect(),
                    edges: (1..=4).map(edge_id).collect(),
                },
                operation: TargetOperation::ReturnIntegerExpressionConditionalControl {
                    condition_source: value_id(1),
                    condition: TargetBooleanExpression::Call {
                        psi_operation: operation_id(3),
                        source_value: value_id(1),
                        callee: machine_id(2),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                        arguments: vec![TargetCallArgument {
                            scalar_type: psi_core::ScalarType::Boolean,
                            location: argument_location,
                            expression: TargetScalarExpression::Boolean(
                                TargetBooleanExpression::IntegerEqual {
                                    psi_operation: operation_id(2),
                                    scalar_type,
                                    left: Box::new(TargetIntegerExpression::WrappingDivide {
                                        psi_operation: operation_id(1),
                                        obligation: psi_core::ObligationId::new(1).unwrap(),
                                        left: Box::new(immediate(2, 24)),
                                        right: Box::new(immediate(3, 3)),
                                    }),
                                    right: Box::new(immediate(4, 8)),
                                },
                            ),
                        }],
                    },
                    scalar_type,
                    when_true: TargetConditionalIntegerArm {
                        psi_edge: edge_id(1),
                        control: Box::new(TargetIntegerControl::Return {
                            psi_return_edge: edge_id(3),
                            source_value: value_id(5),
                            expression: TargetIntegerExpression::Call {
                                psi_operation: operation_id(5),
                                source_value: value_id(5),
                                callee: machine_id(3),
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                                arguments: vec![TargetCallArgument {
                                    scalar_type: psi_core::ScalarType::Integer(scalar_type),
                                    location: argument_location,
                                    expression: TargetScalarExpression::Integer {
                                        scalar_type,
                                        expression: TargetIntegerExpression::ExactRemainder {
                                            psi_operation: operation_id(4),
                                            obligation: psi_core::ObligationId::new(1).unwrap(),
                                            left: Box::new(immediate(6, 43)),
                                            right: Box::new(immediate(7, 6)),
                                        },
                                    },
                                }],
                            },
                        }),
                    },
                    when_false: TargetConditionalIntegerArm {
                        psi_edge: edge_id(2),
                        control: Box::new(TargetIntegerControl::Return {
                            psi_return_edge: edge_id(4),
                            source_value: value_id(8),
                            expression: immediate(8, 2),
                        }),
                    },
                },
            },
            TargetFunction {
                machine: machine_id(2),
                attachment: None,
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(5)],
                },
                operation: TargetOperation::ReturnBooleanParameter {
                    psi_edge: edge_id(5),
                    source_value: value_id(9),
                    parameter_index: 0,
                    location: argument_location,
                },
            },
            TargetFunction {
                machine: machine_id(3),
                attachment: None,
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(6)],
                },
                operation: TargetOperation::ReturnIntegerParameter {
                    psi_edge: edge_id(6),
                    source_value: value_id(10),
                    scalar_type,
                    parameter_index: 0,
                    location: argument_location,
                },
            },
        ],
    }
}

fn conditional_condition_division_plan(target: NativeTarget) -> TargetOperationPlan {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let immediate = |source, value| TargetIntegerExpression::Immediate {
        source_value: value_id(source),
        value: IntegerValue::Unsigned(value),
    };
    let arm = |edge, return_edge, source, value| TargetConditionalIntegerArm {
        psi_edge: edge_id(edge),
        control: Box::new(TargetIntegerControl::Return {
            psi_return_edge: edge_id(return_edge),
            source_value: value_id(source),
            expression: immediate(source, value),
        }),
    };
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
        },
        target,
        entry: machine_id(1),
        functions: vec![TargetFunction {
            machine: machine_id(1),
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![operation_id(1), operation_id(2)],
                edges: vec![edge_id(1), edge_id(2), edge_id(3), edge_id(4)],
            },
            operation: TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition_source: value_id(1),
                condition: TargetBooleanExpression::IntegerEqual {
                    psi_operation: operation_id(2),
                    scalar_type,
                    left: Box::new(TargetIntegerExpression::WrappingDivide {
                        psi_operation: operation_id(1),
                        obligation: psi_core::ObligationId::new(1).unwrap(),
                        left: Box::new(immediate(2, 24)),
                        right: Box::new(immediate(3, 3)),
                    }),
                    right: Box::new(immediate(4, 8)),
                },
                scalar_type,
                when_true: arm(1, 3, 5, 1),
                when_false: arm(2, 4, 6, 2),
            },
        }],
    }
}

fn conditional_arm_division_plan(target: NativeTarget, signed: bool) -> TargetOperationPlan {
    let scalar_type = IntegerType::new(
        if signed {
            IntegerSign::Signed
        } else {
            IntegerSign::Unsigned
        },
        64,
    )
    .expect("64-bit integer");
    let condition_register = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    };
    let immediate = |source, value: i64| TargetIntegerExpression::Immediate {
        source_value: value_id(source),
        value: if signed {
            IntegerValue::Signed(value.into())
        } else {
            IntegerValue::Unsigned(value as u128)
        },
    };
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([43; 32]),
        },
        target,
        entry: machine_id(1),
        functions: vec![TargetFunction {
            machine: machine_id(1),
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![operation_id(1), operation_id(2)],
                edges: vec![edge_id(1), edge_id(2), edge_id(3), edge_id(4)],
            },
            operation: TargetOperation::ReturnIntegerConditionalControl {
                condition_source: value_id(1),
                condition_parameter_index: 0,
                condition_location: ScalarParameterLocation::Register(condition_register),
                scalar_type,
                when_true: TargetConditionalIntegerArm {
                    psi_edge: edge_id(1),
                    control: Box::new(TargetIntegerControl::Return {
                        psi_return_edge: edge_id(3),
                        source_value: value_id(2),
                        expression: TargetIntegerExpression::WrappingDivide {
                            psi_operation: operation_id(1),
                            obligation: psi_core::ObligationId::new(1).unwrap(),
                            left: Box::new(immediate(3, if signed { i64::MIN } else { 24 })),
                            right: Box::new(immediate(4, if signed { -1 } else { 3 })),
                        },
                    }),
                },
                when_false: TargetConditionalIntegerArm {
                    psi_edge: edge_id(2),
                    control: Box::new(TargetIntegerControl::Return {
                        psi_return_edge: edge_id(4),
                        source_value: value_id(5),
                        expression: TargetIntegerExpression::ExactRemainder {
                            psi_operation: operation_id(2),
                            obligation: psi_core::ObligationId::new(1).unwrap(),
                            left: Box::new(immediate(6, 29)),
                            right: Box::new(immediate(7, 5)),
                        },
                    }),
                },
            },
        }],
    }
}

fn division_argument_plan(target: NativeTarget) -> TargetOperationPlan {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument_register = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    };
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([41; 32]),
        },
        target,
        entry: machine_id(1),
        functions: vec![
            TargetFunction {
                machine: machine_id(1),
                attachment: None,
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1), operation_id(2)],
                    edges: vec![edge_id(1)],
                },
                operation: TargetOperation::ReturnIntegerExpression {
                    psi_edge: edge_id(1),
                    source_value: value_id(1),
                    scalar_type,
                    expression: TargetIntegerExpression::Call {
                        psi_operation: operation_id(2),
                        source_value: value_id(1),
                        callee: machine_id(2),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                        arguments: vec![TargetCallArgument {
                            scalar_type: psi_core::ScalarType::Integer(scalar_type),
                            location: ScalarParameterLocation::Register(argument_register),
                            expression: TargetScalarExpression::Integer {
                                scalar_type,
                                expression: TargetIntegerExpression::WrappingDivide {
                                    psi_operation: operation_id(1),
                                    obligation: psi_core::ObligationId::new(1).unwrap(),
                                    left: Box::new(TargetIntegerExpression::Immediate {
                                        source_value: value_id(2),
                                        value: IntegerValue::Signed(i64::MIN.into()),
                                    }),
                                    right: Box::new(TargetIntegerExpression::Immediate {
                                        source_value: value_id(3),
                                        value: IntegerValue::Signed((-1_i64).into()),
                                    }),
                                },
                            },
                        }],
                    },
                },
            },
            TargetFunction {
                machine: machine_id(2),
                attachment: None,
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(2)],
                },
                operation: TargetOperation::ReturnIntegerParameter {
                    psi_edge: edge_id(2),
                    source_value: value_id(4),
                    scalar_type,
                    parameter_index: 0,
                    location: ScalarParameterLocation::Register(argument_register),
                },
            },
        ],
    }
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine id")
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).expect("operation id")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("edge id")
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).expect("value id")
}
