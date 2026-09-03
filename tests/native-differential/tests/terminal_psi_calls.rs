use omega_abstract_operations::AbstractOperation;
use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_image_emission::{
    ObjectError, build_object_artifact, derive_stack_demand, emit_executable_image,
    emit_object_container, emit_scalar_call_reference_linux_x86_64_image,
};
use omega_machine_emission::emit_machine_code;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::NativeTarget;
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use psi_proof_admission::AdmissionProfile;
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

const SCALAR_CALL_FIXTURE: &str = include_str!("../../fixtures/terminal-psi/scalar-call.hex");

#[test]
fn frontend_generated_scalar_terminals_are_product_valid() {
    let Some(case_directory) = std::env::var_os("OMEGA_TERMINAL_SCALAR_CALL_CASE_DIR") else {
        return;
    };
    let cases = [
        ("renamed-permuted.terminal", 73_i128),
        ("nested-three-hop.terminal", 7),
        ("four-arguments.terminal", 4),
        ("signed-minimum.terminal", i128::from(i32::MIN)),
        ("signed-maximum.terminal", i128::from(i32::MAX)),
    ];
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("encode empty scalar proof");

    for (file_name, expected) in cases {
        let path = std::path::Path::new(&case_directory).join(file_name);
        let semantic = std::fs::read(&path).unwrap_or_else(|error| {
            panic!("read frontend scalar case {}: {error}", path.display())
        });
        let module = decode_module(&semantic)
            .unwrap_or_else(|error| panic!("decode frontend scalar case {file_name}: {error:?}"));
        assert_eq!(
            encode_module(&module),
            Ok(semantic.clone()),
            "frontend scalar case {file_name} must be canonical Terminal Psi"
        );
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("verify frontend scalar case {file_name}: {error:?}"));
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap_or_else(|error| panic!("interpret frontend scalar case {file_name}: {error:?}"));
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: i32_type(),
                value: IntegerValue::Signed(expected),
            }),
            "frontend scalar case {file_name} returned the wrong value"
        );
    }
}

#[test]
fn scalar_i32_call_has_exact_exportable_terminal_bytes() {
    let module = i32_call_module();
    let semantic = encode_module(&module).expect("encode scalar i32 call fixture");

    if std::env::var_os("OMEGA_UPDATE_TERMINAL_FIXTURES").is_some() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../tests/fixtures/terminal-psi/scalar-call.hex");
        std::fs::write(fixture, wrapped_hex(&semantic)).expect("refresh scalar call fixture");
    } else {
        assert_eq!(
            compact_hex(&semantic),
            SCALAR_CALL_FIXTURE
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
    let artifact = build_object_artifact(&machine_code).expect("build scalar call object");
    derive_stack_demand(&artifact, machine_id(1)).expect("compose scalar call stack");
    assert_eq!(artifact.functions()[0].text_offset, 0);
    assert_eq!(artifact.functions()[0].byte_count, 48);
    assert_eq!(artifact.functions()[1].text_offset, 48);
    assert_eq!(artifact.functions()[1].byte_count, 3);
    let image = emit_scalar_call_reference_linux_x86_64_image(&artifact)
        .expect("resolve runnable scalar call image");
    let repeated_image = emit_scalar_call_reference_linux_x86_64_image(&artifact)
        .expect("repeat runnable scalar call image");
    assert_eq!(repeated_image.output().bytes, image.output().bytes);
    let shim = image.linux_x86_scalar_exit_shim();
    assert_eq!(shim.text_offset, 51);
    assert_eq!(shim.byte_count, 16);
    assert_eq!(shim.relocation_offset, 52);
    assert_eq!(
        image.output().final_text_bytes,
        [
            0x48, 0x83, 0xec, 0x10, // sub rsp, 16
            0x48, 0xb8, 0x49, 0, 0, 0, 0, 0, 0, 0, // mov rax, 73
            0x48, 0x63, 0xc0, // movsxd rax, eax
            0x48, 0x89, 0x44, 0x24, 0, // spill argument
            0x48, 0x83, 0xec, 8, // align call
            0x48, 0x8b, 0x7c, 0x24, 8, // load rdi
            0xe8, 0x0c, 0, 0, 0, // call machine 2
            0x48, 0x83, 0xc4, 8, // release call area
            0x48, 0x63, 0xc0, // normalize i32 result
            0x48, 0x83, 0xc4, 0x10, // release expression frame
            0xc3, // return from machine 1
            0x89, 0xf8, 0xc3, // machine 2: mov eax, edi; ret
            0xe8, 0xc8, 0xff, 0xff, 0xff, // shim call machine 1
            0x89, 0xc7, // mov edi, eax
            0xb8, 0xe7, 0, 0, 0, // mov eax, exit_group
            0x0f, 0x05, // syscall
            0x0f, 0x0b, // ud2
        ]
    );
    assert_eq!(image.output().bytes.len(), 8192);
    assert_eq!(
        u64::from_le_bytes(image.output().bytes[24..32].try_into().unwrap()),
        0x401033
    );

    if let Some(path) = std::env::var_os("OMEGA_TERMINAL_SCALAR_CALL_FIXTURE") {
        std::fs::write(path, &semantic).expect("write requested scalar call terminal reference");
    }
    if let Some(path) = std::env::var_os("OMEGA_TERMINAL_SCALAR_CALL_X64_IMAGE") {
        std::fs::write(path, &image.output().bytes)
            .expect("write requested runnable scalar call image reference");
    }

    // Ordinary scalar arity is not retained in the object function carrier.
    // This unused entry parameter deliberately leaves native bytes unchanged;
    // the fixture-specific image API must reject it by semantic identity.
    let mut parameterized_entry = i32_call_module();
    parameterized_entry.machines[0]
        .parameters
        .push(scalar_declaration(
            value_id(6),
            ScalarType::Integer(i32_type()),
        ));
    let parameterized_semantic =
        encode_module(&parameterized_entry).expect("encode parameterized scalar entry");
    verify_module(
        &parameterized_entry,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("unused scalar entry parameter remains semantically valid");
    let parameterized_abstract = lower_artifact_sections(
        &parameterized_semantic,
        &encode_proof_bundle(&ProofBundle::default()).expect("empty parameterized proof"),
        &AdmissionProfile::default(),
    )
    .expect("lower parameterized scalar entry");
    let parameterized_target =
        lower_to_target_operations(&parameterized_abstract, NativeTarget::linux_x64())
            .expect("select parameterized scalar entry ABI");
    let parameterized_assigned =
        assign_registers(&parameterized_target).expect("assign parameterized scalar entry");
    let parameterized_code =
        emit_machine_code(&parameterized_assigned).expect("emit parameterized scalar entry");
    let parameterized_artifact = build_object_artifact(&parameterized_code)
        .expect("build parameterized scalar entry object");
    assert_eq!(parameterized_artifact.text_bytes(), artifact.text_bytes());
    assert!(
        emit_scalar_call_reference_linux_x86_64_image(&parameterized_artifact).is_err(),
        "byte-identical parameterized entry must not acquire zero-argument process semantics"
    );

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
        AbstractOperation::Call { .. }
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
    let artifact = build_object_artifact(&machine_code).expect("build call object");
    let stack = derive_stack_demand(&artifact, machine_id(1))
        .expect("compose byte-validated scalar call stack");
    assert!(stack.ceiling_bytes() >= 16);
    assert_eq!(stack.contributing_machines().len(), 2);
    let object = emit_object_container(&artifact);
    assert_eq!(object.output.relocations, 1);
    let image = emit_executable_image(&artifact, 3).expect("resolve internal call image");
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
        AbstractOperation::Call { .. }
    ));
    assert!(
        abstract_plan.functions[1]
            .operations
            .iter()
            .any(|operation| matches!(operation, AbstractOperation::Crash { .. }))
    );
    let target = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select crash-capable call ABI");
    let assigned = assign_registers(&target).expect("assign crash-capable call arguments");
    let machine_code = emit_machine_code(&assigned).expect("emit call and callee crash leaf");
    assert_eq!(machine_code.functions[0].internal_calls.len(), 1);
    let artifact = build_object_artifact(&machine_code).expect("build call crash object");
    assert_eq!(
        derive_stack_demand(&artifact, machine_id(1)),
        Err(ObjectError::UnaccountedTerminalStack(machine_id(2)))
    );
    assert_eq!(emit_object_container(&artifact).output.relocations, 1);
    emit_executable_image(&artifact, 3).expect("resolve crash-capable internal call image");
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
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
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
                ranked_scc: None,
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
        outcome_specific_ensures: Vec::new(),
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
