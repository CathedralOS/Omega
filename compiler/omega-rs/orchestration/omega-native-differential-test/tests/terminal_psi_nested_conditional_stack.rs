use omega_target::{Architecture, NativeTarget};
use omega_terminal_image_emission::{
    build_terminal_installation_record, build_terminal_object_artifact,
    decode_terminal_installation_record, derive_terminal_installation_stack_demand,
    derive_terminal_stack_demand, emit_terminal_executable_image,
    encode_terminal_installation_record,
};
use omega_terminal_machine_code::{
    TerminalScalarConditionalArm, TerminalScalarControlFlowEvidence,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_target_operations::{
    MachineRegister, TerminalPsiProvenance, TerminalScalarParameterLocation,
    TerminalTargetConditionalIntegerArm, TerminalTargetFunction, TerminalTargetIntegerControl,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
};
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ProfileDecisionId, ValueId,
};
use psi_terminal::{CrashCause, SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[test]
fn nested_conditional_stack_facts_survive_installation_and_reject_forgery() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for nested_false_arm in [false, true] {
            let assigned = assign_registers(&nested_conditional_plan(target, nested_false_arm))
                .expect("assign nested conditional");
            let emitted = emit_machine_code(&assigned).expect("emit nested conditional");
            let TerminalScalarControlFlowEvidence::TopLevelTwoDecisionThreeReturn {
                nested,
                nested_arm,
                ..
            } = emitted.functions[0]
                .scalar_stack
                .as_ref()
                .expect("nested conditional stack evidence")
                .control_flow
            else {
                panic!("nested conditional must retain three-leaf evidence")
            };
            assert_eq!(
                nested_arm,
                if nested_false_arm {
                    TerminalScalarConditionalArm::False
                } else {
                    TerminalScalarConditionalArm::True
                }
            );
            let nested_branch_offset = nested.branch_offset;
            let nested_false_arm_offset = nested.false_arm_offset;

            let mut forged = emitted.clone();
            {
                let TerminalScalarControlFlowEvidence::TopLevelTwoDecisionThreeReturn {
                    nested,
                    ..
                } = &mut forged.functions[0]
                    .scalar_stack
                    .as_mut()
                    .expect("forged nested stack evidence")
                    .control_flow
                else {
                    unreachable!()
                };
                nested.false_arm_offset += 1;
            }
            assert!(build_terminal_object_artifact(&forged).is_err());

            let artifact = build_terminal_object_artifact(&emitted)
                .expect("object boundary replays all three nested leaves");
            let demand = derive_terminal_stack_demand(&artifact, machine_id(1))
                .expect("derive nested conditional stack demand");
            let image = emit_terminal_executable_image(&artifact, 31 + u16::from(nested_false_arm))
                .expect("emit nested conditional executable image");
            let installation = build_terminal_installation_record(
                &image,
                ProfileDecisionId::new(1).expect("profile decision"),
            )
            .expect("build nested conditional installation record");
            let encoded = encode_terminal_installation_record(&installation)
                .expect("encode nested conditional installation record");
            let decoded = decode_terminal_installation_record(&encoded)
                .expect("decode nested conditional installation record");
            let installed =
                derive_terminal_installation_stack_demand(&decoded, &image, machine_id(1))
                    .expect("recompose installed nested conditional stack demand");
            assert_eq!(installed, demand);
            assert!(nested_branch_offset < nested_false_arm_offset);
        }
    }
}

#[test]
fn four_leaf_conditional_stack_facts_survive_installation_and_reject_forgery() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assign_registers(&four_leaf_conditional_plan(target))
            .expect("assign four-leaf conditional");
        let emitted = emit_machine_code(&assigned).expect("emit four-leaf conditional");
        let TerminalScalarControlFlowEvidence::TopLevelThreeDecisionFourReturn {
            root,
            true_nested,
            false_nested,
        } = emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("four-leaf conditional stack evidence")
            .control_flow
        else {
            panic!("four-leaf conditional must retain three branch records")
        };
        assert!(root.branch_offset < true_nested.branch_offset);
        assert!(true_nested.false_arm_offset < root.false_arm_offset);
        assert!(root.false_arm_offset <= false_nested.branch_offset);

        let mut forged = emitted.clone();
        let TerminalScalarControlFlowEvidence::TopLevelThreeDecisionFourReturn {
            false_nested, ..
        } = &mut forged.functions[0]
            .scalar_stack
            .as_mut()
            .expect("forged four-leaf stack evidence")
            .control_flow
        else {
            unreachable!()
        };
        false_nested.false_arm_offset += 1;
        assert!(build_terminal_object_artifact(&forged).is_err());

        let artifact = build_terminal_object_artifact(&emitted)
            .expect("object boundary replays all four leaves");
        let demand = derive_terminal_stack_demand(&artifact, machine_id(1))
            .expect("derive four-leaf conditional stack demand");
        let image = emit_terminal_executable_image(&artifact, 33)
            .expect("emit four-leaf conditional executable image");
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(2).expect("profile decision"),
        )
        .expect("build four-leaf conditional installation record");
        let encoded = encode_terminal_installation_record(&installation)
            .expect("encode four-leaf conditional installation record");
        let decoded = decode_terminal_installation_record(&encoded)
            .expect("decode four-leaf conditional installation record");
        let installed = derive_terminal_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed four-leaf conditional stack demand");
        assert_eq!(installed, demand);
    }
}

#[test]
fn nested_crash_leaf_stack_facts_survive_installation_and_reject_forgery() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut plan = nested_conditional_plan(target, false);
        let TerminalTargetOperation::ReturnIntegerConditionalControl { when_true, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        let TerminalTargetIntegerControl::Conditional { when_false, .. } =
            when_true.control.as_mut()
        else {
            unreachable!()
        };
        when_false.control = Box::new(TerminalTargetIntegerControl::Crash {
            psi_crash_edge: edge_id(8),
            cause: CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        });
        let assigned = assign_registers(&plan).expect("assign nested crash conditional");
        let emitted = emit_machine_code(&assigned).expect("emit nested crash conditional");
        let TerminalScalarControlFlowEvidence::TopLevelTwoDecisionThreeTerminal {
            root,
            crash_leaves,
            ..
        } = emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("nested crash conditional stack evidence")
            .control_flow
        else {
            panic!("nested crash conditional must retain terminal evidence")
        };
        assert_eq!(crash_leaves, [false, true, false]);

        let mut forged = emitted.clone();
        forged.functions[0].bytes[root.false_arm_offset - 1] ^= 1;
        assert!(build_terminal_object_artifact(&forged).is_err());

        let artifact = build_terminal_object_artifact(&emitted)
            .expect("object boundary replays nested return and crash leaves");
        let demand = derive_terminal_stack_demand(&artifact, machine_id(1))
            .expect("derive nested crash conditional stack demand");
        let image = emit_terminal_executable_image(&artifact, 35)
            .expect("emit nested crash conditional executable image");
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(3).expect("profile decision"),
        )
        .expect("build nested crash conditional installation record");
        let encoded = encode_terminal_installation_record(&installation)
            .expect("encode nested crash conditional installation record");
        let decoded = decode_terminal_installation_record(&encoded)
            .expect("decode nested crash conditional installation record");
        let installed = derive_terminal_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed nested crash conditional demand");
        assert_eq!(installed, demand);
    }
}

#[test]
fn four_leaf_crash_stack_facts_survive_installation_and_reject_forgery() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut plan = four_leaf_conditional_plan(target);
        let TerminalTargetOperation::ReturnIntegerConditionalControl { when_false, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        let TerminalTargetIntegerControl::Conditional { when_true, .. } =
            when_false.control.as_mut()
        else {
            unreachable!()
        };
        when_true.control = Box::new(TerminalTargetIntegerControl::Crash {
            psi_crash_edge: edge_id(11),
            cause: CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        });
        let assigned = assign_registers(&plan).expect("assign four-leaf crash conditional");
        let emitted = emit_machine_code(&assigned).expect("emit four-leaf crash conditional");
        let TerminalScalarControlFlowEvidence::TopLevelThreeDecisionFourTerminal {
            false_nested,
            crash_leaves,
            ..
        } = emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("four-leaf crash stack evidence")
            .control_flow
        else {
            panic!("four-leaf crash conditional must retain terminal evidence")
        };
        assert_eq!(crash_leaves, [false, false, true, false]);

        let mut forged = emitted.clone();
        forged.functions[0].bytes[false_nested.false_arm_offset - 1] ^= 1;
        assert!(build_terminal_object_artifact(&forged).is_err());

        let artifact = build_terminal_object_artifact(&emitted)
            .expect("object boundary replays four return/crash leaves");
        let demand = derive_terminal_stack_demand(&artifact, machine_id(1))
            .expect("derive four-leaf crash stack demand");
        let image = emit_terminal_executable_image(&artifact, 37)
            .expect("emit four-leaf crash executable image");
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(4).expect("profile decision"),
        )
        .expect("build four-leaf crash installation record");
        let encoded = encode_terminal_installation_record(&installation)
            .expect("encode four-leaf crash installation record");
        let decoded = decode_terminal_installation_record(&encoded)
            .expect("decode four-leaf crash installation record");
        let installed = derive_terminal_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed four-leaf crash demand");
        assert_eq!(installed, demand);
    }
}

fn four_leaf_conditional_plan(target: NativeTarget) -> TerminalTargetOperationPlan {
    let mut plan = nested_conditional_plan(target, false);
    let condition_register = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdx,
        Architecture::Aarch64 => MachineRegister::Aarch64X(2),
    };
    let returned = |edge, return_edge, source, value| TerminalTargetConditionalIntegerArm {
        psi_edge: edge_id(edge),
        control: Box::new(TerminalTargetIntegerControl::Return {
            psi_return_edge: edge_id(return_edge),
            source_value: value_id(source),
            expression: TerminalTargetIntegerExpression::Immediate {
                source_value: value_id(source),
                value: IntegerValue::Unsigned(value),
            },
        }),
    };
    let TerminalTargetOperation::ReturnIntegerConditionalControl { when_false, .. } =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    when_false.control = Box::new(TerminalTargetIntegerControl::Conditional {
        condition_source: value_id(6),
        condition_parameter_index: 2,
        condition_location: TerminalScalarParameterLocation::Register(condition_register),
        when_true: returned(9, 11, 7, 13),
        when_false: returned(10, 12, 8, 15),
    });
    plan.functions[0].provenance.edges = (1..=12).map(edge_id).collect();
    plan
}

fn nested_conditional_plan(
    target: NativeTarget,
    nested_false_arm: bool,
) -> TerminalTargetOperationPlan {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let registers = match target.architecture {
        Architecture::X86_64 => [
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdx,
        ],
        Architecture::Aarch64 => [
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(1),
            MachineRegister::Aarch64X(2),
        ],
    };
    let returned = |edge, return_edge, source, value| TerminalTargetConditionalIntegerArm {
        psi_edge: edge_id(edge),
        control: Box::new(TerminalTargetIntegerControl::Return {
            psi_return_edge: edge_id(return_edge),
            source_value: value_id(source),
            expression: TerminalTargetIntegerExpression::Immediate {
                source_value: value_id(source),
                value: IntegerValue::Unsigned(value),
            },
        }),
    };
    let nested = TerminalTargetConditionalIntegerArm {
        psi_edge: edge_id(if nested_false_arm { 2 } else { 1 }),
        control: Box::new(TerminalTargetIntegerControl::Conditional {
            condition_source: value_id(2),
            condition_parameter_index: if nested_false_arm { 2 } else { 1 },
            condition_location: TerminalScalarParameterLocation::Register(if nested_false_arm {
                registers[2]
            } else {
                registers[1]
            }),
            when_true: returned(5, 7, 3, 7),
            when_false: returned(6, 8, 4, 9),
        }),
    };
    let outer_leaf = returned(
        if nested_false_arm { 1 } else { 2 },
        if nested_false_arm { 3 } else { 4 },
        5,
        11,
    );
    let (when_true, when_false) = if nested_false_arm {
        (outer_leaf, nested)
    } else {
        (nested, outer_leaf)
    };
    TerminalTargetOperationPlan {
        terminal_psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([47; 32]),
        },
        target,
        entry: machine_id(1),
        functions: vec![TerminalTargetFunction {
            machine: machine_id(1),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: (1..=8).map(edge_id).collect(),
            },
            operation: TerminalTargetOperation::ReturnIntegerConditionalControl {
                condition_source: value_id(1),
                condition_parameter_index: 0,
                condition_location: TerminalScalarParameterLocation::Register(registers[0]),
                scalar_type,
                when_true,
                when_false,
            },
        }],
    }
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine id")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("edge id")
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).expect("value id")
}
