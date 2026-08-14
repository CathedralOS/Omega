use omega_target::{Architecture, NativeTarget};
use omega_terminal_image_emission::{
    build_terminal_installation_record, build_terminal_object_artifact,
    decode_terminal_installation_record, derive_terminal_installation_stack_demand,
    derive_terminal_stack_demand, emit_terminal_executable_image, emit_terminal_object_container,
    encode_terminal_installation_record,
};
use omega_terminal_machine_code::TerminalScalarControlFlowEvidence;
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_target_operations::{
    MachineRegister, TerminalPsiProvenance, TerminalScalarParameterLocation,
    TerminalTargetCallArgument, TerminalTargetFunction, TerminalTargetIntegerExpression,
    TerminalTargetOperation, TerminalTargetOperationPlan, TerminalTargetScalarExpression,
};
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ProfileDecisionId,
    ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

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
                let TerminalScalarControlFlowEvidence::LinearWithDivisionBranches { branches } =
                    &stack.control_flow
                else {
                    panic!("signed x86 division argument must retain its generated diamond")
                };
                assert_eq!(branches.len(), 1);
                assert!(branches[0].merge_offset < caller.internal_calls[0].offset);
            }
            Architecture::Aarch64 => {
                assert_eq!(
                    stack.control_flow,
                    TerminalScalarControlFlowEvidence::Linear
                );
            }
        }

        let artifact = build_terminal_object_artifact(&emitted)
            .expect("object boundary replays division argument stack paths");
        let demand = derive_terminal_stack_demand(&artifact, machine_id(1))
            .expect("compose division argument call closure");
        assert!(demand.ceiling_bytes() > 0);
        assert_eq!(demand.contributing_machines().len(), 2);
        let object = emit_terminal_object_container(&artifact);
        assert_eq!(object.output.relocations, 1);
        let image = emit_terminal_executable_image(&artifact, 7)
            .expect("resolve division argument call image");
        assert_eq!(
            image.output().final_text_bytes.len(),
            artifact.text_bytes().len()
        );
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .expect("build division argument installation record");
        let encoded = encode_terminal_installation_record(&installation)
            .expect("encode division argument installation record");
        let decoded = decode_terminal_installation_record(&encoded)
            .expect("decode division argument installation record");
        let installed = derive_terminal_installation_stack_demand(&decoded, &image, machine_id(1))
            .expect("recompose installed division argument stack closure");
        assert_eq!(installed, demand);
    }
}

fn division_argument_plan(target: NativeTarget) -> TerminalTargetOperationPlan {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument_register = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    };
    TerminalTargetOperationPlan {
        terminal_psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([41; 32]),
        },
        target,
        entry: machine_id(1),
        functions: vec![
            TerminalTargetFunction {
                machine: machine_id(1),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1), operation_id(2)],
                    edges: vec![edge_id(1)],
                },
                operation: TerminalTargetOperation::ReturnIntegerExpression {
                    psi_edge: edge_id(1),
                    source_value: value_id(1),
                    scalar_type,
                    expression: TerminalTargetIntegerExpression::Call {
                        psi_operation: operation_id(2),
                        source_value: value_id(1),
                        callee: machine_id(2),
                        arguments: vec![TerminalTargetCallArgument {
                            scalar_type: psi_core::ScalarType::Integer(scalar_type),
                            location: TerminalScalarParameterLocation::Register(argument_register),
                            expression: TerminalTargetScalarExpression::Integer {
                                scalar_type,
                                expression: TerminalTargetIntegerExpression::WrappingDivide {
                                    psi_operation: operation_id(1),
                                    left: Box::new(TerminalTargetIntegerExpression::Immediate {
                                        source_value: value_id(2),
                                        value: IntegerValue::Signed(i64::MIN.into()),
                                    }),
                                    right: Box::new(TerminalTargetIntegerExpression::Immediate {
                                        source_value: value_id(3),
                                        value: IntegerValue::Signed((-1_i64).into()),
                                    }),
                                },
                            },
                        }],
                    },
                },
            },
            TerminalTargetFunction {
                machine: machine_id(2),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(2)],
                },
                operation: TerminalTargetOperation::ReturnIntegerParameter {
                    psi_edge: edge_id(2),
                    source_value: value_id(4),
                    scalar_type,
                    parameter_index: 0,
                    location: TerminalScalarParameterLocation::Register(argument_register),
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
