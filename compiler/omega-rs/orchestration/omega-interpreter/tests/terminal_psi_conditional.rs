use omega_interpreter::{TerminalScalarValue, interpret_terminal_measured};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::TerminalAbstractOperation;
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_assigned_target_operations::TerminalAssignedOperation;
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::lower_verified_module;
use omega_terminal_target_operations::{
    TerminalTargetIntegerControl, TerminalTargetIntegerExpression, TerminalTargetOperation,
};
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
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
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("direct-binding conditional should lower for the host");
    let TerminalTargetOperation::ReturnIntegerConditionalControl {
        condition_source,
        when_true,
        when_false,
        ..
    } = &target_plan.functions[0].operation
    else {
        panic!("target plan must retain a conditional return")
    };
    assert_eq!(*condition_source, ValueId::new(1).unwrap());
    assert_eq!(when_true.psi_edge, true_edge);
    assert_eq!(when_false.psi_edge, false_edge);
    assert!(matches!(
        when_true.control.as_ref(),
        TerminalTargetIntegerControl::Return { psi_return_edge, .. }
            if *psi_return_edge == EdgeId::new(3).unwrap()
    ));
    assert!(matches!(
        when_false.control.as_ref(),
        TerminalTargetIntegerControl::Return { psi_return_edge, .. }
            if *psi_return_edge == EdgeId::new(4).unwrap()
    ));
    let assigned = assign_registers(&target_plan).expect("conditional homes assign");
    assert!(matches!(
        assigned.functions[0].operation,
        TerminalAssignedOperation::ReturnIntegerConditionalControl { .. }
    ));
    let machine_code = emit_machine_code(&assigned).expect("conditional machine code emits");
    assert!(!machine_code.functions[0].bytes.is_empty());
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
fn conditional_arms_lower_through_computed_jumps_to_a_shared_tail() {
    let module = conditional_shared_tail_module();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("acyclic shared-tail conditional verifies");
    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("shared-tail conditional has a fixed bound");
    assert_eq!(fixed.ceiling_units(), 5);

    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (condition, expected) in [(true, 12), (false, 32)] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(3),
                },
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(4),
                },
            ],
        )
        .expect("selected shared-tail path executes");
        assert_eq!(measured.usage().total_units(), 5);
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            }
        );
    }

    let abstract_plan = lower_verified_module(&verified).expect("lower shared-tail requirements");
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("acyclic arm chains should lower for the host");
    let function = &target_plan.functions[0];
    assert_eq!(
        function.provenance.operations,
        [
            OperationId::new(1).unwrap(),
            OperationId::new(2).unwrap(),
            OperationId::new(3).unwrap(),
        ]
    );
    assert_eq!(
        function.provenance.edges,
        [
            EdgeId::new(1).unwrap(),
            EdgeId::new(2).unwrap(),
            EdgeId::new(3).unwrap(),
            EdgeId::new(4).unwrap(),
            EdgeId::new(5).unwrap(),
        ]
    );
    let TerminalTargetOperation::ReturnIntegerConditionalControl {
        when_true,
        when_false,
        ..
    } = &function.operation
    else {
        panic!("shared-tail graph must retain its runtime conditional")
    };
    let TerminalTargetIntegerControl::Return {
        expression: true_expression,
        ..
    } = when_true.control.as_ref()
    else {
        panic!("true shared-tail arm must return")
    };
    assert!(matches!(
        true_expression,
        TerminalTargetIntegerExpression::WrappingAdd { left, .. }
            if matches!(left.as_ref(), TerminalTargetIntegerExpression::WrappingAdd { .. })
    ));
    let TerminalTargetIntegerControl::Return {
        expression: false_expression,
        ..
    } = when_false.control.as_ref()
    else {
        panic!("false shared-tail arm must return")
    };
    assert!(matches!(
        false_expression,
        TerminalTargetIntegerExpression::WrappingAdd { left, .. }
            if matches!(left.as_ref(), TerminalTargetIntegerExpression::WrappingMultiply { .. })
    ));
    let assigned = assign_registers(&target_plan).expect("shared-tail expressions assign");
    let machine_code = emit_machine_code(&assigned).expect("shared-tail expressions emit");
    assert!(!machine_code.functions[0].bytes.is_empty());
}

#[test]
fn compile_known_nested_conditional_folds_inside_a_runtime_arm() {
    let module = nested_constant_conditional_module();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("acyclic nested conditional verifies");
    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("nested conditional has a fixed bound");
    assert_eq!(fixed.ceiling_units(), 5);

    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (condition, expected, fuel) in [(true, 6, 5), (false, 4, 2)] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(3),
                },
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(4),
                },
            ],
        )
        .expect("selected nested path executes");
        assert_eq!(measured.usage().total_units(), fuel);
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            }
        );
    }

    let abstract_plan = lower_verified_module(&verified).expect("lower nested requirements");
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("compile-known nested condition should fold inside its runtime arm");
    let function = &target_plan.functions[0];
    assert_eq!(
        function.provenance.operations,
        [OperationId::new(1).unwrap(), OperationId::new(2).unwrap()]
    );
    assert_eq!(
        function.provenance.edges,
        [
            EdgeId::new(1).unwrap(),
            EdgeId::new(2).unwrap(),
            EdgeId::new(3).unwrap(),
            EdgeId::new(5).unwrap(),
            EdgeId::new(6).unwrap(),
        ]
    );
    let TerminalTargetOperation::ReturnIntegerConditionalControl {
        when_true,
        when_false,
        ..
    } = &function.operation
    else {
        panic!("outer runtime conditional must remain")
    };
    assert_eq!(when_true.psi_edge, EdgeId::new(1).unwrap());
    assert!(matches!(
        when_true.control.as_ref(),
        TerminalTargetIntegerControl::Return {
            psi_return_edge,
            expression: TerminalTargetIntegerExpression::WrappingAdd { .. },
            ..
        } if *psi_return_edge == EdgeId::new(6).unwrap()
    ));
    assert_eq!(when_false.psi_edge, EdgeId::new(2).unwrap());
    assert!(matches!(
        when_false.control.as_ref(),
        TerminalTargetIntegerControl::Return { psi_return_edge, .. }
            if *psi_return_edge == EdgeId::new(5).unwrap()
    ));
    let assigned = assign_registers(&target_plan).expect("folded nested arms assign");
    let machine_code = emit_machine_code(&assigned).expect("folded nested arms emit");
    assert!(!machine_code.functions[0].bytes.is_empty());
}

#[test]
fn runtime_nested_conditional_lowers_as_recursive_target_control() {
    let mut module = nested_constant_conditional_module();
    module.machines[0].parameters.swap(0, 1);
    module.machines[0].parameters.push(ValueDeclaration {
        id: ValueId::new(11).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    module.machines[0].blocks[1].operations.clear();
    let Terminator::Conditional { condition, .. } = &mut module.machines[0].blocks[1].terminator
    else {
        unreachable!("fixture has a nested conditional")
    };
    *condition = ValueId::new(11).unwrap();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("runtime nested conditional verifies as terminal Psi");
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (outer, inner, expected, fuel) in
        [(true, true, 6, 4), (true, false, 9, 4), (false, true, 4, 2)]
    {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(3),
                },
                TerminalScalarValue::Boolean(outer),
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(4),
                },
                TerminalScalarValue::Boolean(inner),
            ],
        )
        .expect("selected runtime-nested path executes");
        assert_eq!(measured.usage().total_units(), fuel);
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            }
        );
    }
    let abstract_plan = lower_verified_module(&verified).expect("lower nested requirements");
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("runtime-nested conditional lowers");
    let TerminalTargetOperation::ReturnIntegerConditionalControl {
        when_true,
        when_false,
        ..
    } = &target_plan.functions[0].operation
    else {
        panic!("outer conditional must remain")
    };
    assert!(matches!(
        when_true.control.as_ref(),
        TerminalTargetIntegerControl::Conditional {
            condition_source,
            when_true: nested_true,
            when_false: nested_false,
            ..
        } if *condition_source == ValueId::new(11).unwrap()
            && nested_true.psi_edge == EdgeId::new(3).unwrap()
            && nested_false.psi_edge == EdgeId::new(4).unwrap()
    ));
    assert!(matches!(
        when_false.control.as_ref(),
        TerminalTargetIntegerControl::Return { psi_return_edge, .. }
            if *psi_return_edge == EdgeId::new(5).unwrap()
    ));
    assert_eq!(
        target_plan.functions[0].provenance.operations,
        [OperationId::new(2).unwrap(), OperationId::new(3).unwrap()]
    );
    assert_eq!(
        target_plan.functions[0].provenance.edges,
        (1..=7)
            .map(|raw| EdgeId::new(raw).unwrap())
            .collect::<Vec<_>>()
    );
    let assigned = assign_registers(&target_plan).expect("recursive conditionals assign");
    let machine_code = emit_machine_code(&assigned).expect("recursive conditionals emit");
    assert!(!machine_code.functions[0].bytes.is_empty());
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = lower_to_target_operations(&abstract_plan, target)
            .expect("recursive conditionals lower for each architecture");
        let assigned = assign_registers(&target_plan)
            .expect("recursive conditional homes assign for each architecture");
        let emitted = emit_machine_code(&assigned)
            .expect("recursive conditional machine code emits for each architecture");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
    #[cfg(unix)]
    for (outer, inner, expected) in [(true, true, 6), (true, false, 9), (false, true, 4)] {
        assert_eq!(
            run_host_runtime_nested_conditional(
                &machine_code.functions[0].bytes,
                3,
                outer,
                4,
                inner,
            ),
            expected
        );
    }
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
            content_entry_claims: Vec::new(),
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

fn conditional_shared_tail_module() -> TerminalModule {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 terminal type");
    let integer = ScalarType::Integer(integer_type);
    let declaration = |raw, scalar_type| ValueDeclaration {
        id: ValueId::new(raw).expect("nonzero value"),
        scalar_type,
    };
    TerminalModule {
        semantic_version: SemanticVersion::CURRENT,
        entry: MachineId::new(1).unwrap(),
        machines: vec![TerminalMachine {
            id: MachineId::new(1).unwrap(),
            parameters: vec![
                declaration(1, ScalarType::Boolean),
                declaration(2, integer),
                declaration(3, integer),
            ],
            result: declaration(10, integer),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
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
                    parameters: vec![declaration(4, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(1).unwrap(),
                        result: declaration(6, integer),
                        kind: OperationKind::WrappingIntegerAdd {
                            left: ValueId::new(4).unwrap(),
                            right: ValueId::new(4).unwrap(),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(3).unwrap(),
                        target: BlockId::new(4).unwrap(),
                        arguments: vec![ValueId::new(6).unwrap()],
                    },
                },
                Block {
                    id: BlockId::new(3).unwrap(),
                    parameters: vec![declaration(5, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(2).unwrap(),
                        result: declaration(7, integer),
                        kind: OperationKind::WrappingIntegerMultiply {
                            left: ValueId::new(5).unwrap(),
                            right: ValueId::new(5).unwrap(),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4).unwrap(),
                        target: BlockId::new(4).unwrap(),
                        arguments: vec![ValueId::new(7).unwrap()],
                    },
                },
                Block {
                    id: BlockId::new(4).unwrap(),
                    parameters: vec![declaration(8, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(3).unwrap(),
                        result: declaration(9, integer),
                        kind: OperationKind::WrappingIntegerAdd {
                            left: ValueId::new(8).unwrap(),
                            right: ValueId::new(8).unwrap(),
                        },
                    }],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(5).unwrap(),
                        value: ValueId::new(9).unwrap(),
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

fn nested_constant_conditional_module() -> TerminalModule {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 terminal type");
    let integer = ScalarType::Integer(integer_type);
    let declaration = |raw, scalar_type| ValueDeclaration {
        id: ValueId::new(raw).expect("nonzero value"),
        scalar_type,
    };
    TerminalModule {
        semantic_version: SemanticVersion::CURRENT,
        entry: MachineId::new(1).unwrap(),
        machines: vec![TerminalMachine {
            id: MachineId::new(1).unwrap(),
            parameters: vec![
                declaration(1, ScalarType::Boolean),
                declaration(2, integer),
                declaration(3, integer),
            ],
            result: declaration(10, integer),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
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
                    parameters: vec![declaration(4, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(1).unwrap(),
                        result: declaration(5, ScalarType::Boolean),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Conditional {
                        condition: ValueId::new(5).unwrap(),
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(3).unwrap(),
                            target: BlockId::new(4).unwrap(),
                            arguments: vec![ValueId::new(4).unwrap()],
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(4).unwrap(),
                            target: BlockId::new(5).unwrap(),
                            arguments: vec![ValueId::new(4).unwrap()],
                        },
                    },
                },
                Block {
                    id: BlockId::new(3).unwrap(),
                    parameters: vec![declaration(6, integer)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(5).unwrap(),
                        value: ValueId::new(6).unwrap(),
                    },
                },
                Block {
                    id: BlockId::new(4).unwrap(),
                    parameters: vec![declaration(7, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(2).unwrap(),
                        result: declaration(8, integer),
                        kind: OperationKind::WrappingIntegerAdd {
                            left: ValueId::new(7).unwrap(),
                            right: ValueId::new(7).unwrap(),
                        },
                    }],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(6).unwrap(),
                        value: ValueId::new(8).unwrap(),
                    },
                },
                Block {
                    id: BlockId::new(5).unwrap(),
                    parameters: vec![declaration(9, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(3).unwrap(),
                        result: declaration(12, integer),
                        kind: OperationKind::WrappingIntegerMultiply {
                            left: ValueId::new(9).unwrap(),
                            right: ValueId::new(9).unwrap(),
                        },
                    }],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(7).unwrap(),
                        value: ValueId::new(12).unwrap(),
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

#[cfg(unix)]
static NEXT_SCRATCH_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
fn run_host_runtime_nested_conditional(
    bytes: &[u8],
    first: u8,
    outer: bool,
    third: u8,
    inner: bool,
) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-runtime-nested");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _terminal_entry\n.p2align 2\n_terminal_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl terminal_entry\n.type terminal_entry,@function\nterminal_entry:\n.byte {bytes}\n.size terminal_entry, .-terminal_entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = format!(
        "#include <stdbool.h>\n#include <stdint.h>\n\
extern uint8_t terminal_entry(uint8_t, bool, uint8_t, bool);\n\
int main(void) {{ return terminal_entry({first}, {}, {third}, {}); }}\n",
        if outer { "true" } else { "false" },
        if inner { "true" } else { "false" },
    );
    std::fs::write(&assembly_path, assembly).expect("write runtime-nested assembly harness");
    std::fs::write(&driver_path, driver).expect("write runtime-nested C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected runtime-nested terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute runtime-nested terminal canary")
        .code()
        .expect("runtime-nested terminal canary exited normally")
}

#[cfg(unix)]
fn fresh_scratch_directory(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let sequence = NEXT_SCRATCH_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "{prefix}-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create unique terminal test directory");
    directory
}

#[cfg(unix)]
struct ScratchDirectory(PathBuf);

#[cfg(unix)]
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::process::Command;
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::time::SystemTime;
