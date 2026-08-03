use omega_interpreter::{TerminalScalarValue, interpret_terminal_measured};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::TerminalAbstractOperation;
use omega_terminal_abstract_operations_to_target_operations::{
    LoweringError, lower_to_target_operations,
};
use omega_terminal_psi_to_abstract_operations::lower_verified_module;
use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, SemanticVersion, SuccessorEdge,
    TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_codec::{decode_module, encode_module, terminal_psi_identity};
use psi_terminal_fixed_fuel::{
    derive_fixed_entry_fuel, derive_fixed_safe_point_segments, validate_fixed_entry_fuel,
    validate_fixed_safe_point_segments,
};
use psi_terminal_fuel::FuelChargeSite;
use psi_terminal_verifier::{ProofBundle, verify_module};

#[test]
fn v13_conditional_round_trips_executes_and_lowers_both_ordered_successors() {
    let module = conditional_module(SemanticVersion::V13);
    let identity = terminal_psi_identity(&module).expect("v13 identity");
    assert_eq!(
        identity.program_fingerprint.to_string(),
        "0b851f3c9aae5523434ab415e1b14b9d1f7c4d37def9023879aa3f24ea11ed0f"
    );
    let bytes = encode_module(&module).expect("canonical v13 bytes");
    let decoded = decode_module(&bytes).expect("decode canonical v13 module");
    assert_eq!(terminal_psi_identity(&decoded).unwrap(), identity);
    let verified = verify_module(
        &decoded,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free conditional module verifies");
    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("acyclic conditional has a maximum fuel bound");
    assert_eq!(fixed.ceiling_units(), 2);
    validate_fixed_entry_fuel(&verified, &fixed).expect("conditional bound recomputes");
    let segments = derive_fixed_safe_point_segments(&verified, MachineId::new(1).unwrap())
        .expect("conditional graph has a complete safe-point partition");
    assert_eq!(
        segments
            .iter()
            .map(|segment| (
                segment.start_block().get(),
                segment.end_edge().get(),
                segment.ceiling_units()
            ))
            .collect::<Vec<_>>(),
        [(1, 1, 1), (1, 2, 1), (2, 3, 1), (3, 4, 1)]
    );
    validate_fixed_safe_point_segments(&verified, MachineId::new(1).unwrap(), &segments)
        .expect("conditional safe-point partition recomputes");

    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let true_edge = EdgeId::new(1).expect("true edge");
    let false_edge = EdgeId::new(2).expect("false edge");
    for (condition, expected, selected, unselected) in [
        (true, 17, true_edge, false_edge),
        (false, 29, false_edge, true_edge),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(17),
                },
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(29),
                },
            ],
        )
        .expect("selected conditional arm executes");
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            }
        );
        assert_eq!(measured.usage().total_units(), 2);
        assert_eq!(
            measured
                .usage()
                .at(FuelChargeSite::Edge(selected))
                .expect("selected edge is charged")
                .executions(),
            1
        );
        assert_eq!(measured.usage().at(FuelChargeSite::Edge(unselected)), None);
    }

    let abstract_plan = lower_verified_module(&verified).expect("lower conditional requirements");
    let function = &abstract_plan.functions[0];
    assert_eq!(
        function
            .block_entries
            .iter()
            .map(|entry| (entry.block.get(), entry.operation_offset))
            .collect::<Vec<_>>(),
        [(1, 0), (2, 1), (3, 2)]
    );
    let TerminalAbstractOperation::Conditional {
        condition,
        when_true,
        when_false,
    } = &function.operations[0]
    else {
        panic!("entry operation must retain the conditional")
    };
    assert_eq!(*condition, ValueId::new(1).unwrap());
    assert_eq!(when_true.psi_edge, true_edge);
    assert_eq!(when_true.target, BlockId::new(2).unwrap());
    assert_eq!(when_true.bindings[0].argument, ValueId::new(2).unwrap());
    assert_eq!(when_false.psi_edge, false_edge);
    assert_eq!(when_false.target, BlockId::new(3).unwrap());
    assert_eq!(when_false.bindings[0].argument, ValueId::new(3).unwrap());
    assert_eq!(
        lower_to_target_operations(&abstract_plan, NativeTarget::host()),
        Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            MachineId::new(1).unwrap()
        ))
    );
}

#[test]
fn conditional_fixed_bound_uses_the_maximum_path_not_the_sum() {
    let mut module = conditional_module(SemanticVersion::V13);
    module.machines[0].blocks[1].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        result: ValueDeclaration {
            id: ValueId::new(7).unwrap(),
            scalar_type: ScalarType::Boolean,
        },
        kind: OperationKind::BooleanConstant { value: true },
    });
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("unequal acyclic branch costs verify");

    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("maximum branch cost derives");
    assert_eq!(fixed.ceiling_units(), 3);
    let segments = derive_fixed_safe_point_segments(&verified, MachineId::new(1).unwrap())
        .expect("unequal branch segments derive");
    assert_eq!(segments[2].ceiling_units(), 2);
    assert_eq!(segments[3].ceiling_units(), 1);
}

#[test]
fn conditional_requires_semantic_v13() {
    let module = conditional_module(SemanticVersion::V12);
    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::Module(
            psi_terminal_verifier::ModuleError::ConditionalRequiresSemanticVersion {
                required: SemanticVersion::V13,
                actual: SemanticVersion::V12,
                ..
            }
        ))
    ));
}

#[test]
fn conditional_requires_boolean_condition_and_dominating_values() {
    let mut wrong_condition = conditional_module(SemanticVersion::V13);
    let integer = wrong_condition.machines[0].parameters[1].scalar_type;
    wrong_condition.machines[0].parameters[0].scalar_type = integer;
    assert!(matches!(
        verify_module(
            &wrong_condition,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::Module(
            psi_terminal_verifier::ModuleError::ConditionalConditionTypeMismatch { .. }
        ))
    ));

    let mut branch_local_leak = conditional_module(SemanticVersion::V13);
    let true_parameter = branch_local_leak.machines[0].blocks[1].parameters[0].id;
    let Terminator::Return { value, .. } = &mut branch_local_leak.machines[0].blocks[2].terminator
    else {
        unreachable!("fixture's false block returns")
    };
    *value = true_parameter;
    assert!(matches!(
        verify_module(
            &branch_local_leak,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::Module(
            psi_terminal_verifier::ModuleError::ValueUsedBeforeDefinition(value)
        )) if value == true_parameter
    ));
}

fn conditional_module(semantic_version: SemanticVersion) -> TerminalModule {
    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 terminal type"));
    let declaration = |raw, scalar_type| ValueDeclaration {
        id: ValueId::new(raw).expect("nonzero value"),
        scalar_type,
    };
    TerminalModule {
        semantic_version,
        entry: MachineId::new(1).unwrap(),
        machines: vec![TerminalMachine {
            id: MachineId::new(1).unwrap(),
            parameters: vec![
                declaration(1, ScalarType::Boolean),
                declaration(2, integer),
                declaration(3, integer),
            ],
            result: declaration(4, integer),
            structural_places: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![
                Block {
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition: ValueId::new(1).unwrap(),
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(1).unwrap(),
                            target: BlockId::new(2).unwrap(),
                            arguments: vec![ValueId::new(2).unwrap()],
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(2).unwrap(),
                            target: BlockId::new(3).unwrap(),
                            arguments: vec![ValueId::new(3).unwrap()],
                        },
                    },
                },
                Block {
                    id: BlockId::new(2).unwrap(),
                    parameters: vec![declaration(5, integer)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(3).unwrap(),
                        value: ValueId::new(5).unwrap(),
                    },
                },
                Block {
                    id: BlockId::new(3).unwrap(),
                    parameters: vec![declaration(6, integer)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(4).unwrap(),
                        value: ValueId::new(6).unwrap(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    }
}
