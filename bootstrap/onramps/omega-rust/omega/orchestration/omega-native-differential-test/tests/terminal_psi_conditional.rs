use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractOperation, TerminalAbstractOperationPlan,
};
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_assigned_target_operations::TerminalAssignedOperation;
use omega_terminal_image_emission::{build_terminal_object_artifact, derive_terminal_stack_demand};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::{ArtifactLoweringError, lower_artifact_sections};
use omega_terminal_target_operations::{
    TerminalTargetBooleanControl, TerminalTargetIntegerControl, TerminalTargetIntegerExpression,
    TerminalTargetOperation,
};
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{
    decode_module, encode_module, encode_proof_bundle, terminal_psi_identity,
};
use psi_terminal_fixed_fuel::{
    derive_fixed_entry_fuel, derive_fixed_safe_point_segments, validate_fixed_entry_fuel,
    validate_fixed_safe_point_segments,
};
use psi_terminal_fuel::FuelChargeSite;
use psi_terminal_interpreter::{
    MeasuredTerminalExecution, TerminalArtifactInterpretError, TerminalExecutionResult,
    TerminalScalarValue, interpret_terminal_artifact_measured,
};
use psi_terminal_verifier::{ProofBundle, VerifiedTerminalModule, verify_module};

fn interpret_verified_artifact(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let semantic_bytes = encode_module(verified.module()).expect("verified semantics encode");
    let proof_bytes =
        encode_proof_bundle(&ProofBundle::default()).expect("proof-free bundle encodes");
    interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        arguments,
    )
}

fn lower_verified_artifact(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<TerminalAbstractOperationPlan, ArtifactLoweringError> {
    let semantic_bytes = encode_module(verified.module()).expect("verified semantics encode");
    let proof_bytes =
        encode_proof_bundle(&ProofBundle::default()).expect("proof-free bundle encodes");
    lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
}

#[test]
fn conditional_round_trips_executes_and_lowers_both_ordered_successors() {
    let module = conditional_module(VocabularyMarker::CURRENT);
    let identity = terminal_psi_identity(&module).expect("identity");
    assert_eq!(
        identity.program_fingerprint.to_string(),
        "0d74ffe63ae8853ad2815d46a3a4730283706d28f7ff1f12cc188da0aa2ba225"
    );
    let bytes = encode_module(&module).expect("canonical bytes");
    let decoded = decode_module(&bytes).expect("decode canonical module");
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
        let measured = interpret_verified_artifact(
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
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            })
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

    let abstract_plan = lower_verified_artifact(&verified).expect("lower conditional requirements");
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
fn bounded_two_return_conditional_derives_native_stack_by_arm_maximum() {
    let module = conditional_module(VocabularyMarker::CURRENT);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("bounded conditional verifies");
    let abstract_plan = lower_verified_artifact(&verified).expect("bounded conditional lowers");
    for (target, expected_peak) in [
        (NativeTarget::linux_x64(), 0),
        (NativeTarget::linux_arm64(), 16),
    ] {
        let target_plan = lower_to_target_operations(&abstract_plan, target)
            .expect("bounded conditional lowers for stack accounting target");
        let assigned = assign_registers(&target_plan)
            .expect("bounded conditional assigns for stack accounting target");
        let machine_code =
            emit_machine_code(&assigned).expect("bounded conditional emits stack evidence");
        let artifact = build_terminal_object_artifact(&machine_code)
            .expect("bounded conditional object replays each arm");
        assert_eq!(
            derive_terminal_stack_demand(&artifact, MachineId::new(1).unwrap())
                .expect("bounded conditional stack demand")
                .ceiling_bytes(),
            expected_peak
        );
    }
}

#[test]
fn bounded_conditional_call_arm_composes_native_stack_closure() {
    let module = conditional_call_arm_module();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("conditional call-arm module verifies");
    let abstract_plan = lower_verified_artifact(&verified).expect("conditional call arm lowers");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = lower_to_target_operations(&abstract_plan, target)
            .expect("conditional call arm selects target operations");
        let assigned = assign_registers(&target_plan).expect("conditional call arm assigns");
        let machine_code = emit_machine_code(&assigned).expect("conditional call arm emits");
        assert!(machine_code.functions[0].scalar_stack.is_some());
        assert_eq!(machine_code.functions[0].internal_calls.len(), 1);
        let artifact = build_terminal_object_artifact(&machine_code)
            .expect("conditional call arm object validates");
        let demand = derive_terminal_stack_demand(&artifact, MachineId::new(1).unwrap())
            .expect("conditional call arm closure composes");
        assert!(demand.ceiling_bytes() >= 16);
        assert_eq!(demand.contributing_machines().len(), 2);
    }
}

#[test]
fn conditional_fixed_bound_uses_the_maximum_path_not_the_sum() {
    let mut module = conditional_module(VocabularyMarker::CURRENT);
    module.machines[0].blocks[1].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(7).unwrap(),
            scalar_type: ScalarType::Boolean,
        }),
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
fn unconditional_entry_prefix_reaches_runtime_conditional_control() {
    let mut module = conditional_module(VocabularyMarker::CURRENT);
    module.machines[0].entry = BlockId::new(8).unwrap();
    module.machines[0].blocks.push(Block {
        id: BlockId::new(8).unwrap(),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Jump {
            edge: EdgeId::new(8).unwrap(),
            target: BlockId::new(1).unwrap(),
            arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    });
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("entry-prefixed conditional verifies");
    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("entry-prefixed conditional has a fixed bound");
    assert_eq!(fixed.ceiling_units(), 3);

    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (condition, expected) in [(true, 17), (false, 29)] {
        let measured = interpret_verified_artifact(
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
        .expect("entry-prefixed conditional executes");
        assert_eq!(measured.usage().total_units(), 3);
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            })
        );
    }

    let abstract_plan = lower_verified_artifact(&verified).expect("lower prefixed requirements");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = lower_to_target_operations(&abstract_plan, target)
            .expect("entry-prefixed conditional lowers for each architecture");
        assert!(
            target_plan.functions[0]
                .provenance
                .edges
                .contains(&EdgeId::new(8).unwrap())
        );
        assert!(matches!(
            target_plan.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_plan)
            .expect("entry-prefixed conditional assigns for each architecture");
        let emitted = emit_machine_code(&assigned)
            .expect("entry-prefixed conditional emits for each architecture");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
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
        let measured = interpret_verified_artifact(
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
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            })
        );
    }

    let abstract_plan = lower_verified_artifact(&verified).expect("lower shared-tail requirements");
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
        let measured = interpret_verified_artifact(
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
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            })
        );
    }

    let abstract_plan = lower_verified_artifact(&verified).expect("lower nested requirements");
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
        let measured = interpret_verified_artifact(
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
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(expected),
            })
        );
    }
    let abstract_plan = lower_verified_artifact(&verified).expect("lower nested requirements");
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
fn nested_boolean_result_conditional_reaches_native_control() {
    let module = nested_boolean_conditional_module();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("nested Boolean-result conditional verifies");
    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("nested Boolean-result conditional has a fixed bound");
    assert_eq!(fixed.ceiling_units(), 3);

    for (outer, inner, expected, fuel) in [
        (true, true, true, 3),
        (true, false, false, 3),
        (false, true, true, 2),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(outer),
                TerminalScalarValue::Boolean(inner),
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
        )
        .expect("nested Boolean-result path executes");
        assert_eq!(measured.usage().total_units(), fuel);
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
    }

    let abstract_plan = lower_verified_artifact(&verified).expect("lower Boolean requirements");
    let host_target = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("nested Boolean-result conditional lowers for the host");
    let TerminalTargetOperation::ReturnBooleanConditionalControl {
        when_true,
        when_false,
        ..
    } = &host_target.functions[0].operation
    else {
        panic!("outer Boolean conditional must remain")
    };
    assert!(matches!(
        when_true.control.as_ref(),
        TerminalTargetBooleanControl::Conditional {
            condition_source,
            when_true: nested_true,
            when_false: nested_false,
            ..
        } if *condition_source == ValueId::new(6).unwrap()
            && nested_true.psi_edge == EdgeId::new(3).unwrap()
            && nested_false.psi_edge == EdgeId::new(4).unwrap()
    ));
    assert!(matches!(
        when_false.control.as_ref(),
        TerminalTargetBooleanControl::ReturnParameter { psi_return_edge, .. }
            if *psi_return_edge == EdgeId::new(5).unwrap()
    ));
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = lower_to_target_operations(&abstract_plan, target)
            .expect("nested Boolean control lowers for each architecture");
        let assigned = assign_registers(&target_plan)
            .expect("nested Boolean control assigns for each architecture");
        let emitted = emit_machine_code(&assigned)
            .expect("nested Boolean control emits for each architecture");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
    let assigned = assign_registers(&host_target).expect("host Boolean control assigns");
    let machine_code = emit_machine_code(&assigned).expect("host Boolean control emits");
    #[cfg(unix)]
    for (outer, inner, expected) in [
        (true, true, true),
        (true, false, false),
        (false, true, true),
    ] {
        assert_eq!(
            run_host_boolean_conditional(
                &machine_code.functions[0].bytes,
                outer,
                inner,
                true,
                false,
                true,
            ),
            i32::from(expected)
        );
    }
}

#[test]
fn conditional_requires_boolean_condition_and_dominating_values() {
    let mut wrong_condition = conditional_module(VocabularyMarker::CURRENT);
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

    let mut branch_local_leak = conditional_module(VocabularyMarker::CURRENT);
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

fn conditional_module(vocabulary_marker: VocabularyMarker) -> TerminalModule {
    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 terminal type"));
    let declaration = |raw, scalar_type| ValueDeclaration {
        id: ValueId::new(raw).expect("nonzero value"),
        scalar_type,
    };
    TerminalModule {
        vocabulary_marker,
        entry: MachineId::new(1).unwrap(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                declaration(1, ScalarType::Boolean),
                declaration(2, integer),
                declaration(3, integer),
            ],
            result: TerminalMachineResult::Scalar(declaration(4, integer)),
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
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(2).unwrap(),
                            target: BlockId::new(3).unwrap(),
                            arguments: vec![ValueId::new(3).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: BlockId::new(2).unwrap(),
                    parameters: vec![declaration(5, integer)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(3).unwrap(),
                        value: ValueId::new(5).unwrap(),
                    },
                },
                Block {
                    id: BlockId::new(3).unwrap(),
                    parameters: vec![declaration(6, integer)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(4).unwrap(),
                        value: ValueId::new(6).unwrap(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    }
}

fn conditional_call_arm_module() -> TerminalModule {
    let mut module = conditional_module(VocabularyMarker::CURRENT);
    let integer = module.machines[0].parameters[1].scalar_type;
    module.machines[0].blocks[1].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(7).unwrap(),
            scalar_type: integer,
        }),
        kind: OperationKind::Call {
            callee: MachineId::new(2).unwrap(),
            arguments: vec![ValueId::new(5).unwrap()],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let Terminator::Return { value, .. } = &mut module.machines[0].blocks[1].terminator else {
        unreachable!()
    };
    *value = ValueId::new(7).unwrap();
    module.machines.push(TerminalMachine {
        id: MachineId::new(2).unwrap(),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![ValueDeclaration {
            id: ValueId::new(8).unwrap(),
            scalar_type: integer,
        }],
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: ValueId::new(9).unwrap(),
            scalar_type: integer,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(4).unwrap(),
        blocks: vec![Block {
            id: BlockId::new(4).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(5).unwrap(),
                value: ValueId::new(8).unwrap(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(2).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    });
    module
}

fn conditional_shared_tail_module() -> TerminalModule {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 terminal type");
    let integer = ScalarType::Integer(integer_type);
    let declaration = |raw, scalar_type| ValueDeclaration {
        id: ValueId::new(raw).expect("nonzero value"),
        scalar_type,
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(1).unwrap(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                declaration(1, ScalarType::Boolean),
                declaration(2, integer),
                declaration(3, integer),
            ],
            result: TerminalMachineResult::Scalar(declaration(10, integer)),
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
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(2).unwrap(),
                            target: BlockId::new(3).unwrap(),
                            arguments: vec![ValueId::new(3).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: BlockId::new(2).unwrap(),
                    parameters: vec![declaration(4, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(1).unwrap(),
                        result: psi_terminal::OperationResult::Scalar(declaration(6, integer)),
                        kind: OperationKind::WrappingIntegerAdd {
                            left: ValueId::new(4).unwrap(),
                            right: ValueId::new(4).unwrap(),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(3).unwrap(),
                        target: BlockId::new(4).unwrap(),
                        arguments: vec![ValueId::new(6).unwrap()],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: BlockId::new(3).unwrap(),
                    parameters: vec![declaration(5, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(2).unwrap(),
                        result: psi_terminal::OperationResult::Scalar(declaration(7, integer)),
                        kind: OperationKind::WrappingIntegerMultiply {
                            left: ValueId::new(5).unwrap(),
                            right: ValueId::new(5).unwrap(),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4).unwrap(),
                        target: BlockId::new(4).unwrap(),
                        arguments: vec![ValueId::new(7).unwrap()],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: BlockId::new(4).unwrap(),
                    parameters: vec![declaration(8, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(3).unwrap(),
                        result: psi_terminal::OperationResult::Scalar(declaration(9, integer)),
                        kind: OperationKind::WrappingIntegerAdd {
                            left: ValueId::new(8).unwrap(),
                            right: ValueId::new(8).unwrap(),
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(5).unwrap(),
                        value: ValueId::new(9).unwrap(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
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
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(1).unwrap(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                declaration(1, ScalarType::Boolean),
                declaration(2, integer),
                declaration(3, integer),
            ],
            result: TerminalMachineResult::Scalar(declaration(10, integer)),
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
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(2).unwrap(),
                            target: BlockId::new(3).unwrap(),
                            arguments: vec![ValueId::new(3).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: BlockId::new(2).unwrap(),
                    parameters: vec![declaration(4, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(1).unwrap(),
                        result: psi_terminal::OperationResult::Scalar(declaration(
                            5,
                            ScalarType::Boolean,
                        )),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Conditional {
                        condition: ValueId::new(5).unwrap(),
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(3).unwrap(),
                            target: BlockId::new(4).unwrap(),
                            arguments: vec![ValueId::new(4).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(4).unwrap(),
                            target: BlockId::new(5).unwrap(),
                            arguments: vec![ValueId::new(4).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: BlockId::new(3).unwrap(),
                    parameters: vec![declaration(6, integer)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(5).unwrap(),
                        value: ValueId::new(6).unwrap(),
                    },
                },
                Block {
                    id: BlockId::new(4).unwrap(),
                    parameters: vec![declaration(7, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(2).unwrap(),
                        result: psi_terminal::OperationResult::Scalar(declaration(8, integer)),
                        kind: OperationKind::WrappingIntegerAdd {
                            left: ValueId::new(7).unwrap(),
                            right: ValueId::new(7).unwrap(),
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(6).unwrap(),
                        value: ValueId::new(8).unwrap(),
                    },
                },
                Block {
                    id: BlockId::new(5).unwrap(),
                    parameters: vec![declaration(9, integer)],
                    operations: vec![Operation {
                        id: OperationId::new(3).unwrap(),
                        result: psi_terminal::OperationResult::Scalar(declaration(12, integer)),
                        kind: OperationKind::WrappingIntegerMultiply {
                            left: ValueId::new(9).unwrap(),
                            right: ValueId::new(9).unwrap(),
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(7).unwrap(),
                        value: ValueId::new(12).unwrap(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    }
}

fn nested_boolean_conditional_module() -> TerminalModule {
    let declaration = |raw| ValueDeclaration {
        id: ValueId::new(raw).expect("nonzero value"),
        scalar_type: ScalarType::Boolean,
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(1).unwrap(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: (1..=5).map(declaration).collect(),
            result: TerminalMachineResult::Scalar(declaration(10)),
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
                            arguments: vec![
                                ValueId::new(2).unwrap(),
                                ValueId::new(3).unwrap(),
                                ValueId::new(4).unwrap(),
                            ],
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(2).unwrap(),
                            target: BlockId::new(3).unwrap(),
                            arguments: vec![ValueId::new(5).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: BlockId::new(2).unwrap(),
                    parameters: vec![declaration(6), declaration(7), declaration(8)],
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition: ValueId::new(6).unwrap(),
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(3).unwrap(),
                            target: BlockId::new(4).unwrap(),
                            arguments: vec![ValueId::new(7).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(4).unwrap(),
                            target: BlockId::new(5).unwrap(),
                            arguments: vec![ValueId::new(8).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: BlockId::new(3).unwrap(),
                    parameters: vec![declaration(9)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(5).unwrap(),
                        value: ValueId::new(9).unwrap(),
                    },
                },
                Block {
                    id: BlockId::new(4).unwrap(),
                    parameters: vec![declaration(11)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(6).unwrap(),
                        value: ValueId::new(11).unwrap(),
                    },
                },
                Block {
                    id: BlockId::new(5).unwrap(),
                    parameters: vec![declaration(12)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(7).unwrap(),
                        value: ValueId::new(12).unwrap(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    }
}

#[cfg(unix)]
static NEXT_SCRATCH_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
fn run_host_boolean_conditional(
    bytes: &[u8],
    outer: bool,
    inner: bool,
    when_true: bool,
    when_false: bool,
    outer_false: bool,
) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-boolean-conditional");
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
    let boolean = |value| if value { "true" } else { "false" };
    let driver = format!(
        "#include <stdbool.h>\n\
extern bool terminal_entry(bool, bool, bool, bool, bool);\n\
int main(void) {{ return terminal_entry({}, {}, {}, {}, {}); }}\n",
        boolean(outer),
        boolean(inner),
        boolean(when_true),
        boolean(when_false),
        boolean(outer_false),
    );
    std::fs::write(&assembly_path, assembly).expect("write Boolean conditional assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean conditional C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean conditional machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute Boolean conditional canary")
        .code()
        .expect("Boolean conditional canary exited normally")
}

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
