use omega_target::NativeTarget;
use omega_terminal_abstract_operations::TerminalAbstractOperation;
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_image_emission::{
    TerminalObjectError, build_terminal_object_artifact, derive_terminal_stack_demand,
    emit_terminal_executable_image, emit_terminal_object_container,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::lower_artifact_sections;
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    Block, CrashCause, CrashRouteBucket, CrashRouteGuard, MachineContract, Operation,
    OperationKind, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::derive_fixed_entry_fuel;
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    interpret_terminal_artifact_measured,
};
use psi_terminal_verifier::{ProofBundle, verify_module};

const SCALAR_CALL_V28_FIXTURE: &str = include_str!(
    "../../../../../../../bootstrap/omega-bootstrap/gates/fixtures/omega-bootstrap-scalar-call-v28.hex"
);

#[test]
fn scalar_i32_call_has_exact_exportable_vocabulary_28_bytes() {
    let module = i32_call_module();
    let semantic = encode_module(&module).expect("encode scalar i32 call fixture");

    if std::env::var_os("OMEGA_UPDATE_TERMINAL_FIXTURES").is_some() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../../bootstrap/omega-bootstrap/gates/fixtures/omega-bootstrap-scalar-call-v28.hex",
        );
        std::fs::write(fixture, wrapped_hex(&semantic)).expect("refresh scalar call fixture");
    } else {
        assert_eq!(
            compact_hex(&semantic),
            SCALAR_CALL_V28_FIXTURE
                .split_ascii_whitespace()
                .collect::<String>(),
            "scalar call terminal bytes drifted; reviewed replacement:\n{}",
            wrapped_hex(&semantic)
        );
    }

    assert_eq!(decode_module(&semantic), Ok(module.clone()));
    assert_eq!(
        encode_module(&decode_module(&semantic).expect("decode scalar call fixture")),
        Ok(semantic.clone())
    );
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free scalar i32 call verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1))
            .expect("fixed scalar call fuel")
            .ceiling_units(),
        4
    );
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &encode_proof_bundle(&ProofBundle::default()).expect("empty scalar call proof"),
        &AdmissionProfile::default(),
        &[],
    )
    .expect("interpret scalar i32 call fixture");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: i32_type(),
            value: IntegerValue::Signed(73),
        })
    );

    let abstract_plan = lower_artifact_sections(
        &semantic,
        &encode_proof_bundle(&ProofBundle::default()).expect("empty lowering proof"),
        &AdmissionProfile::default(),
    )
    .expect("lower scalar call fixture");
    let target = lower_to_target_operations(&abstract_plan, NativeTarget::linux_x64())
        .expect("select Linux x86-64 scalar call ABI");
    let assigned = assign_registers(&target).expect("assign scalar call arguments");
    let machine_code = emit_machine_code(&assigned).expect("emit scalar call machine code");
    assert_eq!(machine_code.functions[0].internal_calls.len(), 1);
    assert_eq!(
        machine_code.functions[0].internal_calls[0].target,
        machine_id(2)
    );
    let artifact = build_terminal_object_artifact(&machine_code).expect("build scalar call object");
    derive_terminal_stack_demand(&artifact, machine_id(1)).expect("compose scalar call stack");
    emit_terminal_executable_image(&artifact, 3).expect("resolve scalar call image");

    if let Some(path) = std::env::var_os("OMEGA_BOOTSTRAP_SCALAR_CALL_TERMINAL") {
        std::fs::write(path, &semantic).expect("write requested scalar call terminal reference");
    }

    let mut wrong_arity = module.clone();
    let OperationKind::Call { arguments, .. } =
        &mut wrong_arity.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    arguments.clear();
    assert!(
        verify_module(
            &wrong_arity,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .is_err(),
        "scalar call arity mutation must reject"
    );

    let mut wrong_callee = module;
    let OperationKind::Call { callee, .. } =
        &mut wrong_callee.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *callee = machine_id(3);
    assert!(
        verify_module(
            &wrong_callee,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .is_err(),
        "unknown scalar callee mutation must reject"
    );

    let mut wrong_argument = i32_call_module();
    let OperationKind::Call { arguments, .. } =
        &mut wrong_argument.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    arguments[0] = value_id(99);
    assert!(
        verify_module(
            &wrong_argument,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .is_err(),
        "undefined scalar argument mutation must reject"
    );

    let mut wrong_result_type = i32_call_module();
    wrong_result_type.machines[0].blocks[0].operations[1].result =
        psi_terminal::OperationResult::Scalar(scalar_declaration(value_id(2), ScalarType::Boolean));
    assert!(
        verify_module(
            &wrong_result_type,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .is_err(),
        "scalar call result-type mutation must reject"
    );
}

#[test]
fn scalar_call_executes_resumes_and_reaches_a_relocated_native_image() {
    let module = call_module();
    let semantic = encode_module(&module).expect("encode call semantics");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("encode empty proof");
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free call module verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine_id(1)).expect("fixed call fuel");
    assert_eq!(fixed.ceiling_units(), 4);

    let measured =
        interpret_terminal_artifact_measured(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("interpret direct call");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(measured.usage().total_units(), 4);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Operation(operation_id(2)))
            .expect("call charge")
            .executions(),
        1
    );

    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("start resumable call");
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert_eq!(
        execution.resume(&mut meter).expect("exhaust in callee"),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(edge_id(2)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    meter.replenish(1).expect("fund callee return");
    assert_eq!(
        execution.resume(&mut meter).expect("exhaust in caller"),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(edge_id(1)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    meter.replenish(1).expect("fund caller return");
    assert_eq!(
        execution.resume(&mut meter).expect("complete call"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(2)))
            .expect("call charge remains")
            .executions(),
        1,
        "resumption must not replay the paid call"
    );

    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified call artifact");
    assert!(matches!(
        abstract_plan.functions[0].operations[1],
        TerminalAbstractOperation::Call { .. }
    ));
    let target =
        lower_to_target_operations(&abstract_plan, NativeTarget::host()).expect("select call ABI");
    let assigned = assign_registers(&target).expect("assign call arguments");
    let machine_code = emit_machine_code(&assigned).expect("emit native call");
    assert_eq!(machine_code.functions[0].internal_calls.len(), 1);
    assert_eq!(
        machine_code.functions[0].internal_calls[0].target,
        machine_id(2)
    );
    let artifact = build_terminal_object_artifact(&machine_code).expect("build call object");
    let stack = derive_terminal_stack_demand(&artifact, machine_id(1))
        .expect("compose byte-validated scalar call stack");
    assert!(stack.ceiling_bytes() >= 16);
    assert_eq!(stack.contributing_machines().len(), 2);
    let object = emit_terminal_object_container(&artifact);
    assert_eq!(object.output.relocations, 1);
    let image = emit_terminal_executable_image(&artifact, 3).expect("resolve internal call image");
    assert_eq!(
        image.output().final_text_bytes.len(),
        artifact.text_bytes().len()
    );
}

#[test]
fn unconditional_call_crash_is_explicitly_verified_interpreted_and_lowered() {
    let mut module = call_module();
    let route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    };
    module.machines[0].contract.crash_routes = vec![route.clone()];
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![route.clone()];
    module.machines[1].contract.crash_routes = vec![route];
    module.machines[1].blocks[0].terminator = Terminator::Crash {
        edge: edge_id(2),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };

    let semantic = encode_module(&module).expect("encode crash-capable call semantics");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("encode empty proof");
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("covered unconditional call crash verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1))
            .expect("call crash has bounded acyclic fuel")
            .ceiling_units(),
        3
    );

    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("start crash-capable call");
    let mut meter = TerminalFuelMeter::unbounded();
    let TerminalExecutionStatus::Crashed(crash) = execution
        .resume(&mut meter)
        .expect("interpret crash-capable call")
    else {
        panic!("the callee's explicit crash must escape the caller")
    };
    assert_eq!(crash.edge, edge_id(2));
    assert_eq!(crash.cause, CrashCause::Trap);
    assert_eq!(meter.usage().total_units(), 3);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(2)))
            .expect("crashing call charge")
            .executions(),
        1
    );
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Edge(edge_id(2)))
            .expect("callee crash edge charge")
            .executions(),
        1
    );

    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified crash-capable call artifact");
    assert!(matches!(
        abstract_plan.functions[0].operations[1],
        TerminalAbstractOperation::Call { .. }
    ));
    assert!(
        abstract_plan.functions[1]
            .operations
            .iter()
            .any(|operation| matches!(operation, TerminalAbstractOperation::Crash { .. }))
    );
    let target = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select crash-capable call ABI");
    let assigned = assign_registers(&target).expect("assign crash-capable call arguments");
    let machine_code = emit_machine_code(&assigned).expect("emit call and callee crash leaf");
    assert_eq!(machine_code.functions[0].internal_calls.len(), 1);
    let artifact = build_terminal_object_artifact(&machine_code).expect("build call crash object");
    assert_eq!(
        derive_terminal_stack_demand(&artifact, machine_id(1)),
        Err(TerminalObjectError::UnaccountedTerminalStack(machine_id(2)))
    );
    assert_eq!(
        emit_terminal_object_container(&artifact).output.relocations,
        1
    );
    emit_terminal_executable_image(&artifact, 3)
        .expect("resolve crash-capable internal call image");
}

fn call_module() -> TerminalModule {
    scalar_call_module(
        ScalarType::Boolean,
        OperationKind::BooleanConstant { value: true },
    )
}

fn i32_call_module() -> TerminalModule {
    scalar_call_module(
        ScalarType::Integer(i32_type()),
        OperationKind::IntegerConstant {
            value: IntegerValue::Signed(73),
        },
    )
}

fn scalar_call_module(scalar_type: ScalarType, constant_kind: OperationKind) -> TerminalModule {
    let caller_constant = value_id(1);
    let call_result = value_id(2);
    let caller_result = value_id(3);
    let callee_parameter = value_id(4);
    let callee_result = value_id(5);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
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
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(scalar_declaration(
                    caller_result,
                    scalar_type,
                )),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: operation_id(1),
                            result: psi_terminal::OperationResult::Scalar(scalar_declaration(
                                caller_constant,
                                scalar_type,
                            )),
                            kind: constant_kind,
                        },
                        Operation {
                            id: operation_id(2),
                            result: psi_terminal::OperationResult::Scalar(scalar_declaration(
                                call_result,
                                scalar_type,
                            )),
                            kind: OperationKind::Call {
                                callee: machine_id(2),
                                arguments: vec![caller_constant],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(1),
                        value: call_result,
                    },
                }],
                contract: empty_contract(1),
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![scalar_declaration(callee_parameter, scalar_type)],
                result: TerminalMachineResult::Scalar(scalar_declaration(
                    callee_result,
                    scalar_type,
                )),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(2),
                        value: callee_parameter,
                    },
                }],
                contract: empty_contract(2),
            },
        ],
    }
}

fn empty_contract(raw: u64) -> MachineContract {
    MachineContract {
        id: ContractId::new(raw).unwrap(),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
    }
}

fn scalar_declaration(id: ValueId, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration { id, scalar_type }
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("i32 scalar type")
}

fn compact_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn wrapped_hex(bytes: &[u8]) -> String {
    let compact = compact_hex(bytes);
    compact
        .as_bytes()
        .chunks(96)
        .map(|chunk| std::str::from_utf8(chunk).expect("hex is UTF-8"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}
