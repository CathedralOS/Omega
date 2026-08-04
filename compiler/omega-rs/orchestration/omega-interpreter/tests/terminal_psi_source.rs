//! Real-source proof that the transitional producer emits a self-contained
//! terminal-Psi module: frontend trees are dropped before verification and
//! execution.

use omega_compiler::compile_to_checked;
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
    ArtifactId, CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use omega_external_roots::{
    FixedFuelProviderSummary, ProviderFuelSummaryId, RootProviderId,
    bind_installed_terminal_entry_fuel, compose_fixed_fuel, validate_installed_terminal_entry_fuel,
};
use omega_interpreter::{
    TerminalExecution, TerminalExecutionStatus, TerminalScalarValue, interpret_terminal_measured,
};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractOperation,
    TerminalAbstractOperationPlan, TerminalValueBinding,
};
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_image_emission::{
    TerminalObjectArtifact, build_terminal_installation_record, build_terminal_object_artifact,
    decode_terminal_installation_record, emit_terminal_executable_image,
    emit_terminal_object_container, encode_terminal_installation_record,
    validate_terminal_installation_record,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::lower_verified_module;
use omega_terminal_target_operations::{
    TerminalTargetBooleanExpression, TerminalTargetIntegerControl, TerminalTargetIntegerExpression,
    TerminalTargetOperation,
};
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_checked_trees_to_terminal::{LoweringError, lower_machine};
use psi_core::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ProfileDecisionId, ScalarType, ValueId,
};
use psi_extents::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{OperationKind, SemanticVersion};
use psi_terminal_codec::{
    DebugSubject, build_artifact_manifest, decode_debug_map, decode_module, decode_proof_bundle,
    encode_debug_map, encode_module, encode_proof_bundle, terminal_psi_identity,
    validate_artifact_manifest,
};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_verifier::verify_module;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

#[cfg(unix)]
static NEXT_SCRATCH_DIRECTORY: AtomicU64 = AtomicU64::new(1);

fn source_canary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-interpreter lives under compiler/omega-rs/orchestration")
        .join("canaries/pass/terminal_psi/integer_control_contract/main.omg")
}

#[test]
fn checked_source_survives_frontend_drop_as_verified_terminal_psi() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let lowered = lower_machine(&checked, "terminal_constant")
        .expect("accepted source slice should lower to terminal Psi");

    assert_eq!(lowered.semantic_module.proposition_declarations.len(), 2);
    assert_eq!(lowered.semantic_module.proposition_applications.len(), 1);
    let relation = lowered
        .semantic_module
        .proposition_declarations
        .iter()
        .find(|declaration| declaration.name == "terminal_relation")
        .expect("primitive proposition should retain terminal identity");
    assert_eq!(relation.binders.len(), 3);
    assert!(matches!(
        relation.evidence,
        psi_terminal::PropositionEvidence::FactOnly
    ));
    let witness = lowered
        .semantic_module
        .proposition_declarations
        .iter()
        .find(|declaration| declaration.name == "terminal_witness")
        .expect("witness-bearing proposition should retain terminal identity");
    assert!(matches!(
        &witness.evidence,
        psi_terminal::PropositionEvidence::Witness { evidence_type }
            if evidence_type == "dyn TerminalEvidence"
    ));
    let application = &lowered.semantic_module.proposition_applications[0];
    assert_eq!(application.declaration, relation.id);
    assert_eq!(application.binder_arguments.len(), 3);
    assert_eq!(application.arguments, ["7"]);

    drop(checked);

    let canonical_bytes = encode_module(&lowered.semantic_module)
        .expect("source-produced terminal Psi should encode canonically");
    let original_identity = terminal_psi_identity(&lowered.semantic_module)
        .expect("source-produced terminal Psi should have a semantic identity");
    let canonical_proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("source-produced proof bundle should encode canonically");
    let canonical_debug_bytes = encode_debug_map(
        &lowered.semantic_module,
        lowered
            .debug_map
            .as_ref()
            .expect("the source producer should retain its debug map"),
    )
    .expect("source-produced debug map should encode canonically");
    let artifact_manifest = build_artifact_manifest(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        None,
        Some(&canonical_debug_bytes),
    )
    .expect("source-produced terminal sections should have a manifest");
    drop(lowered);
    let semantic_module = decode_module(&canonical_bytes)
        .expect("canonical source-produced terminal Psi should decode");
    let proof_bundle = decode_proof_bundle(&canonical_proof_bytes)
        .expect("canonical source-produced proof bundle should decode");
    let debug_map = decode_debug_map(&semantic_module, &canonical_debug_bytes)
        .expect("canonical source-produced debug map should decode");
    validate_artifact_manifest(
        &semantic_module,
        &proof_bundle,
        None,
        Some(&canonical_debug_bytes),
        artifact_manifest,
    )
    .expect("decoded source-produced sections should match their manifest");
    assert_eq!(artifact_manifest.semantic(), original_identity);
    assert!(artifact_manifest.debug().is_some());
    assert_eq!(debug_map.semantic, original_identity);
    assert_eq!(debug_map.files.len(), 1);
    assert!(debug_map.files[0].path.ends_with("main.omg"));
    assert!(
        debug_map
            .sites
            .iter()
            .any(|site| matches!(site.subject, DebugSubject::Machine(_)))
    );
    assert!(
        debug_map
            .sites
            .iter()
            .any(|site| matches!(site.subject, DebugSubject::Operation(_)))
    );
    let source_text = std::fs::read_to_string(source_canary()).expect("read source debug canary");
    let snippets = |subject: fn(DebugSubject) -> bool| {
        debug_map
            .sites
            .iter()
            .filter(|site| subject(site.subject))
            .map(|site| {
                &source_text[usize::try_from(site.span.start).unwrap()
                    ..usize::try_from(site.span.end).unwrap()]
            })
            .collect::<Vec<_>>()
    };
    assert!(
        snippets(|subject| matches!(subject, DebugSubject::Operation(_)))
            .iter()
            .all(|snippet| *snippet == "7i32")
    );
    let edge_snippets = snippets(|subject| matches!(subject, DebugSubject::Edge(_)));
    assert!(edge_snippets.iter().any(|snippet| *snippet == "7i32"));
    assert!(edge_snippets.contains(&"->"));
    assert_eq!(
        terminal_psi_identity(&semantic_module).unwrap(),
        original_identity
    );

    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced terminal Psi and its proof should verify");
    let fixed_fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("straight-line source module should have a fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed_fuel)
        .expect("source-independent consumer should recompute the certificate");
    assert_eq!(fixed_fuel.terminal_psi(), original_identity);
    assert_eq!(fixed_fuel.ceiling_units(), 4);
    let abstract_operations = lower_verified_module(&verified)
        .expect("verified terminal Psi should lower without source state");
    let measured = interpret_terminal_measured(&verified, &[])
        .expect("verified source-produced terminal Psi should execute with fuel");
    assert_eq!(measured.usage().schedule().schedule_version(), 1);
    assert_eq!(measured.usage().total_units(), fixed_fuel.ceiling_units());
    assert_eq!(
        terminal_psi_identity(&semantic_module).unwrap(),
        original_identity,
        "fuel accounting must not change semantic identity"
    );
    let result = measured.value();
    drop(verified);
    drop(semantic_module);
    drop(proof_bundle);

    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    assert_eq!(
        abstract_operations,
        TerminalAbstractOperationPlan {
            terminal_psi: original_identity,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalAbstractFunction {
                machine: MachineId::new(1).expect("machine"),
                entry: BlockId::new(1).expect("entry block"),
                parameters: Vec::new(),
                result: omega_terminal_abstract_operations::TerminalAbstractResult {
                    value: ValueId::new(4).expect("machine result"),
                    scalar_type: ScalarType::Integer(i32_type),
                },
                block_entries: vec![
                    TerminalAbstractBlockEntry {
                        block: BlockId::new(1).expect("entry block"),
                        operation_offset: 0,
                    },
                    TerminalAbstractBlockEntry {
                        block: BlockId::new(2).expect("return block"),
                        operation_offset: 2,
                    },
                ],
                operations: vec![
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(1).expect("operation"),
                        result: ValueId::new(1).expect("jump constant"),
                        scalar_type: ScalarType::Integer(i32_type),
                        value: IntegerValue::Signed(7),
                    },
                    TerminalAbstractOperation::Jump {
                        psi_edge: EdgeId::new(1).expect("jump edge"),
                        target: BlockId::new(2).expect("return block"),
                        bindings: vec![TerminalValueBinding {
                            parameter: ValueId::new(2).expect("block parameter"),
                            argument: ValueId::new(1).expect("jump constant"),
                            scalar_type: ScalarType::Integer(i32_type),
                        }],
                    },
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(2).expect("operation"),
                        result: ValueId::new(3).expect("return constant"),
                        scalar_type: ScalarType::Integer(i32_type),
                        value: IntegerValue::Signed(7),
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: EdgeId::new(2).expect("return edge"),
                        result: ValueId::new(4).expect("machine result"),
                        value: ValueId::new(3).expect("return constant"),
                        scalar_type: ScalarType::Integer(i32_type),
                    },
                ],
            }],
        }
    );
    assert_eq!(
        result,
        TerminalScalarValue::Integer {
            scalar_type: i32_type,
            value: IntegerValue::Signed(7),
        }
    );
}

#[test]
fn checked_source_integer_policy_operations_survive_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi integer policy source canary should compile");
    let cases = [
        ("terminal_wrapping_add", 44_u128),
        ("terminal_saturating_add", 255),
        ("terminal_wrapping_subtract", 251),
        ("terminal_saturating_subtract", 0),
        ("terminal_wrapping_multiply", 4),
        ("terminal_saturating_multiply", 255),
    ];
    let lowered = cases
        .iter()
        .map(|(machine, expected)| {
            (
                *machine,
                *expected,
                lower_machine(&checked, machine)
                    .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
            )
        })
        .collect::<Vec<_>>();
    drop(checked);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (machine, expected, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} terminal Psi should verify: {error:?}"));
        let fixed_fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .unwrap_or_else(|error| panic!("{machine} should have fixed fuel: {error:?}"));
        assert_eq!(fixed_fuel.ceiling_units(), 5, "{machine} fuel");
        let measured = interpret_terminal_measured(&verified, &[])
            .unwrap_or_else(|error| panic!("{machine} should execute: {error:?}"));
        assert_eq!(measured.usage().total_units(), 5, "{machine} usage");
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: u8_type,
                value: IntegerValue::Unsigned(expected),
            },
            "{machine} result"
        );
    }
}

#[cfg(unix)]
#[test]
fn source_wrapping_add_matches_emitted_host_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi integer policy source canary should compile");
    let lowered = lower_machine(&checked, "terminal_wrapping_add")
        .expect("source wrapping add should lower to terminal Psi");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source wrapping add terminal Psi should verify");
    let abstract_operations = lower_verified_module(&verified)
        .expect("verified source wrapping add should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("source wrapping add should select for the host");
    let assigned = assign_registers(&target_operations).expect("source target homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("source wrapping add machine code should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("source wrapping add should form an owned object artifact");
    let entry = object_artifact.entry_function();
    assert_eq!(
        entry.provenance.operations,
        [
            OperationId::new(1).expect("jump constant"),
            OperationId::new(2).expect("right constant"),
            OperationId::new(3).expect("wrapping add"),
        ]
    );
    assert_eq!(run_host_machine_code(entry.bytes(&object_artifact)), 44);
}

#[cfg(unix)]
#[test]
fn checked_source_ninth_parameter_reaches_the_host_stack_abi() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi runtime-parameter source canary should compile");
    let lowered = lower_machine(&checked, "terminal_ninth_parameter")
        .expect("nine-parameter source machine should lower to terminal Psi");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced nine-parameter terminal Psi should verify");
    let fixed_fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("direct parameter return should have fixed fuel");
    assert_eq!(fixed_fuel.ceiling_units(), 1);
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let arguments = [1_u128, 2, 3, 4, 5, 6, 7, 8, 77]
        .into_iter()
        .map(|value| TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(value),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_measured(&verified, &arguments)
        .expect("source-produced ninth parameter should execute");
    assert_eq!(measured.usage().total_units(), 1);
    assert_eq!(measured.value(), arguments[8]);

    let abstract_operations = lower_verified_module(&verified)
        .expect("verified source parameters should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("source parameters should select host ABI locations");
    let assigned =
        assign_registers(&target_operations).expect("parameter target homes should assign");
    let machine_code = emit_machine_code(&assigned).expect("source parameter return should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("source parameter return should form an object artifact");
    let entry = object_artifact.entry_function();
    assert!(entry.provenance.operations.is_empty());
    assert_eq!(
        entry.provenance.edges,
        [EdgeId::new(1).expect("return edge")]
    );
    assert_eq!(
        run_host_machine_code_with_nine_u8(entry.bytes(&object_artifact), 1, 2, 77),
        77
    );
}

#[test]
fn checked_source_runtime_integer_policy_operations_survive_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi runtime arithmetic source canary should compile");
    let cases = [
        ("terminal_direct_integer_constant", vec![], 42_u128, 2_u64),
        ("terminal_closed_integer_chain", vec![], 42_u128, 8_u64),
        (
            "terminal_runtime_wrapping_add",
            vec![100_u128, 2, 3, 4, 5, 6, 7, 8, 200],
            44_u128,
            2_u64,
        ),
        (
            "terminal_runtime_nested_wrapping",
            vec![100_u128, 3, 3, 4, 5, 6, 7, 8, 200],
            132,
            3,
        ),
        (
            "terminal_runtime_jump_wrapping",
            vec![5_u128, 2, 3, 4, 5, 6, 7, 8, 40],
            135,
            5,
        ),
        (
            "terminal_runtime_chain_wrapping",
            vec![5_u128, 2, 3, 4, 5, 6, 7, 8, 40],
            134,
            8,
        ),
        (
            "terminal_runtime_multi_binding",
            vec![5_u128, 2, 3, 4, 5, 6, 7, 8, 40],
            137,
            10,
        ),
        ("terminal_runtime_saturating_add", vec![200], 255, 3),
        ("terminal_runtime_wrapping_subtract", vec![5], 251, 3),
        ("terminal_runtime_saturating_subtract", vec![5], 0, 3),
        ("terminal_runtime_wrapping_multiply", vec![20], 4, 3),
        ("terminal_runtime_saturating_multiply", vec![20], 255, 3),
    ];
    let lowered = cases
        .into_iter()
        .map(|(machine, arguments, expected, fuel)| {
            (
                machine,
                arguments,
                expected,
                fuel,
                lower_machine(&checked, machine)
                    .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
            )
        })
        .collect::<Vec<_>>();
    drop(checked);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (machine, arguments, expected, fuel, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} terminal Psi should verify: {error:?}"));
        let fixed_fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .unwrap_or_else(|error| panic!("{machine} should have fixed fuel: {error:?}"));
        assert_eq!(fixed_fuel.ceiling_units(), fuel, "{machine} fuel");
        let arguments = arguments
            .into_iter()
            .map(|value| TerminalScalarValue::Integer {
                scalar_type: u8_type,
                value: IntegerValue::Unsigned(value),
            })
            .collect::<Vec<_>>();
        let measured = interpret_terminal_measured(&verified, &arguments)
            .unwrap_or_else(|error| panic!("{machine} should execute: {error:?}"));
        assert_eq!(measured.usage().total_units(), fuel, "{machine} usage");
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: u8_type,
                value: IntegerValue::Unsigned(expected),
            },
            "{machine} result"
        );
    }
}

#[test]
fn checked_source_conditional_survives_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_runtime_conditional")
        .expect("ordered source conditional should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("source conditional should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("source conditional proof should encode canonically");
    drop(lowered);
    let semantic_module = decode_module(&semantic_bytes).expect("decode source conditional");
    let proof_bundle = decode_proof_bundle(&proof_bytes).expect("decode source conditional proof");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source conditional should verify after frontend drop");
    assert_eq!(semantic_module.semantic_version, SemanticVersion::CURRENT);
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("source conditional should have an exact maximum fuel bound");
    assert_eq!(fixed.ceiling_units(), 5);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (condition, expected, selected, unselected) in [
        (
            true,
            49_u128,
            EdgeId::new(1).unwrap(),
            EdgeId::new(2).unwrap(),
        ),
        (false, 239, EdgeId::new(2).unwrap(), EdgeId::new(1).unwrap()),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Integer {
                    scalar_type: u8_type,
                    value: IntegerValue::Unsigned(17),
                },
                TerminalScalarValue::Integer {
                    scalar_type: u8_type,
                    value: IntegerValue::Unsigned(29),
                },
            ],
        )
        .expect("selected source conditional arm should execute");
        assert_eq!(measured.usage().total_units(), 5);
        assert!(
            measured
                .usage()
                .at(FuelChargeSite::Edge(selected))
                .is_some()
        );
        assert_eq!(measured.usage().at(FuelChargeSite::Edge(unselected)), None);
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: u8_type,
                value: IntegerValue::Unsigned(expected),
            }
        );
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("source conditional should cross the Omega abstract boundary");
    let TerminalAbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &abstract_operations.functions[0].operations[0]
    else {
        panic!("abstract plan must retain the conditional")
    };
    assert_eq!(when_true.bindings.len(), 2);
    assert_eq!(when_false.bindings.len(), 2);
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("computed source conditional should lower for the host");
    let TerminalTargetOperation::ReturnIntegerConditionalControl {
        when_true,
        when_false,
        ..
    } = &target_operations.functions[0].operation
    else {
        panic!("target plan must retain both computed conditional expressions")
    };
    let TerminalTargetIntegerControl::Return {
        expression: true_expression,
        ..
    } = when_true.control.as_ref()
    else {
        panic!("true source arm must return")
    };
    assert!(matches!(
        true_expression,
        TerminalTargetIntegerExpression::WrappingAdd { .. }
    ));
    let TerminalTargetIntegerControl::Return {
        expression: false_expression,
        ..
    } = when_false.control.as_ref()
    else {
        panic!("false source arm must return")
    };
    assert!(matches!(
        false_expression,
        TerminalTargetIntegerExpression::WrappingAdd { left, .. }
            if matches!(
                left.as_ref(),
                TerminalTargetIntegerExpression::WrappingMultiply { .. }
            )
    ));
    assert_eq!(
        target_operations.functions[0].provenance.operations.len(),
        6
    );
    let assigned = assign_registers(&target_operations)
        .expect("source conditional parameter homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("source conditional machine code should emit");
    #[cfg(unix)]
    for (condition, expected) in [(true, 49), (false, 239)] {
        assert_eq!(
            run_host_machine_code_with_conditional_u8(
                &machine_code.functions[0].bytes,
                condition,
                17,
                29,
            ),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn checked_source_literal_conditional_emits_only_its_selected_arm() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi literal conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_literal_conditional")
        .expect("literal source conditional should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("literal source conditional should verify");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let measured = interpret_terminal_measured(
        &verified,
        &[TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(17),
        }],
    )
    .expect("literal conditional should interpret");
    assert_eq!(measured.usage().total_units(), 5);
    assert!(
        measured
            .usage()
            .at(FuelChargeSite::Edge(EdgeId::new(1).unwrap()))
            .is_some()
    );
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Edge(EdgeId::new(2).unwrap())),
        None
    );
    assert_eq!(
        measured.value(),
        TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(20),
        }
    );

    let abstract_operations = lower_verified_module(&verified)
        .expect("literal conditional should cross the Omega abstract boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("literal conditional should select its known arm");
    let function = &target_operations.functions[0];
    assert_eq!(
        function.provenance.edges,
        [EdgeId::new(1).unwrap(), EdgeId::new(3).unwrap()]
    );
    assert!(matches!(
        function.operation,
        TerminalTargetOperation::ReturnIntegerExpression {
            psi_edge,
            expression: TerminalTargetIntegerExpression::WrappingAdd { .. },
            ..
        } if psi_edge == EdgeId::new(3).unwrap()
    ));
    let assigned = assign_registers(&target_operations)
        .expect("literal conditional parameter homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("literal conditional machine code should emit");
    assert_eq!(
        run_host_machine_code_with_nine_u8(&machine_code.functions[0].bytes, 17, 0, 0),
        20
    );
}

#[test]
fn checked_source_boolean_conditional_reaches_native_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_conditional")
        .expect("Boolean source conditional should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean source conditional should verify after frontend drop");
    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("Boolean source conditional should have an exact fuel bound");
    assert_eq!(fixed.ceiling_units(), 2);
    for (condition, expected) in [(true, true), (false, false)] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
            ],
        )
        .expect("Boolean source conditional should interpret");
        assert_eq!(measured.usage().total_units(), 2);
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("Boolean source conditional should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean source conditional should lower for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("Boolean source conditional parameter homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("Boolean source conditional machine code should emit");
    #[cfg(unix)]
    for (condition, expected) in [(true, 1), (false, 0)] {
        assert_eq!(
            run_host_machine_code_with_conditional_u8(
                &machine_code.functions[0].bytes,
                condition,
                1,
                0,
            ),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn source_closed_integer_chain_matches_emitted_host_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi closed integer-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_closed_integer_chain")
        .expect("closed integer state chain should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("closed integer state chain should verify");
    let abstract_operations = lower_verified_module(&verified)
        .expect("closed integer state chain should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("closed integer state chain should select for the host");
    let assigned =
        assign_registers(&target_operations).expect("closed chain target homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("closed integer state chain should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("closed integer state chain should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 5);
    assert_eq!(entry.provenance.edges.len(), 3);
    assert_eq!(run_host_machine_code(entry.bytes(&object_artifact)), 42);
}

#[cfg(unix)]
#[test]
fn source_runtime_arithmetic_combines_register_and_stack_parameters() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi runtime arithmetic source canary should compile");
    let lowered = [
        (
            "terminal_runtime_wrapping_add",
            100_u8,
            2_u8,
            200_u8,
            44_i32,
            1_usize,
        ),
        ("terminal_runtime_nested_wrapping", 100, 3, 200, 132, 2),
        ("terminal_runtime_jump_wrapping", 5, 2, 40, 135, 3),
        ("terminal_runtime_chain_wrapping", 5, 2, 40, 134, 5),
        ("terminal_runtime_multi_binding", 5, 2, 40, 137, 7),
    ]
    .into_iter()
    .map(
        |(machine, first, second, ninth, expected, operation_count)| {
            (
                machine,
                first,
                second,
                ninth,
                expected,
                operation_count,
                lower_machine(&checked, machine)
                    .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
            )
        },
    )
    .collect::<Vec<_>>();
    drop(checked);

    for (machine, first, second, ninth, expected, operation_count, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} terminal Psi should verify: {error:?}"));
        let abstract_operations = lower_verified_module(&verified)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
        let assigned =
            assign_registers(&target_operations).expect("integer target homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        let object_artifact = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{machine} should form an object: {error:?}"));
        let entry = object_artifact.entry_function();
        assert_eq!(entry.provenance.operations.len(), operation_count);
        assert_eq!(
            run_host_machine_code_with_nine_u8(entry.bytes(&object_artifact), first, second, ninth,),
            expected,
            "{machine} native result"
        );
    }
}

#[test]
fn checked_source_booleans_survive_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean source canary should compile");
    let constant = lower_machine(&checked, "terminal_boolean_constant")
        .expect("Boolean constant source should lower");
    let parameter = lower_machine(&checked, "terminal_ninth_boolean")
        .expect("Boolean parameter source should lower");
    let chain = lower_machine(&checked, "terminal_boolean_chain")
        .expect("Boolean state chain should lower");
    drop(checked);

    let constant_verified = verify_module(
        &constant.semantic_module,
        &constant.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean constant should verify");
    let constant_fuel = derive_fixed_entry_fuel(&constant_verified, constant.semantic_module.entry)
        .expect("Boolean constant should have fixed fuel");
    assert_eq!(constant_fuel.ceiling_units(), 2);
    let constant_result = interpret_terminal_measured(&constant_verified, &[])
        .expect("source Boolean constant should execute");
    assert_eq!(constant_result.value(), TerminalScalarValue::Boolean(true));
    assert_eq!(constant_result.usage().total_units(), 2);

    let parameter_verified = verify_module(
        &parameter.semantic_module,
        &parameter.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean parameter should verify");
    let parameter_fuel =
        derive_fixed_entry_fuel(&parameter_verified, parameter.semantic_module.entry)
            .expect("Boolean parameter should have fixed fuel");
    assert_eq!(parameter_fuel.ceiling_units(), 1);
    let arguments = [false, false, false, false, false, false, false, false, true]
        .into_iter()
        .map(TerminalScalarValue::Boolean)
        .collect::<Vec<_>>();
    let parameter_result = interpret_terminal_measured(&parameter_verified, &arguments)
        .expect("source Boolean parameter should execute");
    assert_eq!(parameter_result.value(), TerminalScalarValue::Boolean(true));
    assert_eq!(parameter_result.usage().total_units(), 1);

    let chain_verified = verify_module(
        &chain.semantic_module,
        &chain.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean state chain should verify");
    let chain_fuel = derive_fixed_entry_fuel(&chain_verified, chain.semantic_module.entry)
        .expect("Boolean state chain should have fixed fuel");
    assert_eq!(chain_fuel.ceiling_units(), 3);
    let chain_result = interpret_terminal_measured(&chain_verified, &arguments)
        .expect("source Boolean state chain should execute");
    assert_eq!(chain_result.value(), TerminalScalarValue::Boolean(true));
    assert_eq!(chain_result.usage().total_units(), 3);
}

#[cfg(unix)]
#[test]
fn checked_source_boolean_not_round_trips_and_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean-not source canary should compile");
    let lowered =
        lower_machine(&checked, "terminal_boolean_not").expect("Boolean logical not should lower");
    drop(checked);

    assert_eq!(
        lowered.semantic_module.semantic_version,
        SemanticVersion::CURRENT
    );
    assert!(matches!(
        lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
        OperationKind::BooleanNot { .. }
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("Boolean-not terminal Psi should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean-not terminal Psi should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean-not terminal Psi should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("Boolean not should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 2);
    for (input, expected) in [(false, true), (true, false)] {
        let measured =
            interpret_terminal_measured(&verified, &[TerminalScalarValue::Boolean(input)])
                .expect("Boolean not should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), 2);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("Boolean not should cross the source-independent Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean not should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanNotParameter { .. }
    ));
    let assigned =
        assign_registers(&target_operations).expect("Boolean-not parameter home should assign");
    let machine_code = emit_machine_code(&assigned).expect("Boolean not should emit");
    let object_artifact =
        build_terminal_object_artifact(&machine_code).expect("Boolean not should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 1);
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), false),
        1
    );
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), true),
        0
    );
}

#[cfg(unix)]
#[test]
fn checked_source_boolean_equality_round_trips_and_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean-equality source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_equal_false")
        .expect("Boolean equality should lower");
    drop(checked);

    assert_eq!(
        lowered.semantic_module.semantic_version,
        SemanticVersion::V17
    );
    assert!(
        lowered.semantic_module.machines[0].blocks[0]
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::BooleanEqual { .. }))
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("Boolean-equality terminal Psi should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean-equality terminal Psi should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean-equality terminal Psi should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("Boolean equality should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 3);
    for (input, expected) in [(false, true), (true, false)] {
        let measured =
            interpret_terminal_measured(&verified, &[TerminalScalarValue::Boolean(input)])
                .expect("Boolean equality should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), 3);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("Boolean equality should cross the source-independent Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean equality against false should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanNotParameter { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("Boolean-equality parameter home should assign");
    let machine_code = emit_machine_code(&assigned).expect("Boolean equality should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("Boolean equality should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 2);
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), false),
        1
    );
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), true),
        0
    );
}

#[cfg(unix)]
#[test]
fn checked_source_runtime_boolean_equality_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime Boolean-equality source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_equal_runtime")
        .expect("runtime Boolean equality should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("runtime Boolean equality should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("runtime Boolean equality should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime Boolean equality should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("runtime Boolean equality should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 2);
    for (left, right, expected) in [
        (false, false, true),
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(left),
                TerminalScalarValue::Boolean(right),
            ],
        )
        .expect("runtime Boolean equality should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), 2);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("runtime Boolean equality should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("runtime Boolean equality should select for the host");
    assert!(matches!(
        &target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanExpression {
            expression: TerminalTargetBooleanExpression::Equal { .. },
            ..
        }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("runtime Boolean expression homes should assign");
    let machine_code = emit_machine_code(&assigned).expect("runtime Boolean equality should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("runtime Boolean equality should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 1);
    for (left, right, expected) in [
        (false, false, 1),
        (false, true, 0),
        (true, false, 0),
        (true, true, 1),
    ] {
        assert_eq!(
            run_host_machine_code_with_two_bools(entry.bytes(&object_artifact), left, right,),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn source_booleans_reach_constant_and_stack_parameter_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean source canary should compile");
    let lowered = [
        ("terminal_boolean_constant", true),
        ("terminal_ninth_boolean", false),
    ]
    .into_iter()
    .map(|(machine, has_operation)| {
        (
            machine,
            has_operation,
            lower_machine(&checked, machine)
                .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
        )
    })
    .collect::<Vec<_>>();
    drop(checked);

    for (machine, has_operation, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} should verify: {error:?}"));
        let abstract_operations = lower_verified_module(&verified)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
        let assigned =
            assign_registers(&target_operations).expect("Boolean target homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        let object_artifact = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{machine} should form an object: {error:?}"));
        let entry = object_artifact.entry_function();
        assert_eq!(!entry.provenance.operations.is_empty(), has_operation);
        let exit = if has_operation {
            run_host_machine_code(entry.bytes(&object_artifact))
        } else {
            run_host_machine_code_with_nine_bool(entry.bytes(&object_artifact))
        };
        assert_eq!(exit, 1, "{machine} native Boolean result");
    }
}

#[cfg(unix)]
#[test]
fn source_boolean_jump_bindings_reach_stack_parameter_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean state-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain")
        .expect("Boolean state chain should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean state chain should verify");
    let abstract_operations = lower_verified_module(&verified)
        .expect("Boolean jump bindings should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean jump bindings should select for the host");
    let assigned =
        assign_registers(&target_operations).expect("Boolean jump target homes should assign");
    let machine_code = emit_machine_code(&assigned).expect("Boolean jump bindings should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("Boolean state chain should form an object");
    let entry = object_artifact.entry_function();
    assert!(entry.provenance.operations.is_empty());
    assert_eq!(
        entry.provenance.edges,
        [
            EdgeId::new(1).expect("first jump edge"),
            EdgeId::new(2).expect("second jump edge"),
            EdgeId::new(3).expect("return edge"),
        ]
    );
    assert_eq!(
        run_host_machine_code_with_nine_bool(entry.bytes(&object_artifact)),
        1
    );
}

#[test]
fn psi_terminal_producer_rejects_source_outside_its_declared_slice() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi source canary should compile");
    assert_eq!(
        lower_machine(&checked, "Main::main").expect_err("attached main must fail closed"),
        LoweringError::Unsupported(
            "attached machines are not in the first terminal-Psi source slice"
        )
    );
    assert_eq!(
        lower_machine(&checked, "terminal_closed_integer_chain_wrong_contract")
            .expect_err("closed chain with an unrelated contract must fail closed"),
        LoweringError::Unsupported("contract literals must equal the executed literal")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_boolean_chain_wrong_contract")
            .expect_err("closed Boolean chain with an unrelated contract must fail closed"),
        LoweringError::Unsupported("Boolean contract literal must match the compile-known result")
    );
}

#[cfg(unix)]
#[test]
fn interpreted_terminal_source_matches_emitted_host_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi source canary should compile");
    let lowered = lower_machine(&checked, "terminal_constant")
        .expect("accepted source slice should lower to terminal Psi");
    drop(checked);

    let canonical_bytes = encode_module(&lowered.semantic_module)
        .expect("source-produced terminal Psi should encode canonically");
    let original_identity = terminal_psi_identity(&lowered.semantic_module)
        .expect("source-produced terminal Psi should have a semantic identity");
    let canonical_proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("source-produced proof bundle should encode canonically");
    let artifact_manifest =
        build_artifact_manifest(&lowered.semantic_module, &lowered.proof_bundle, None, None)
            .expect("source-produced terminal sections should have a manifest");
    drop(lowered);
    let semantic_module = decode_module(&canonical_bytes)
        .expect("canonical source-produced terminal Psi should decode");
    let proof_bundle = decode_proof_bundle(&canonical_proof_bytes)
        .expect("canonical source-produced proof bundle should decode");
    validate_artifact_manifest(
        &semantic_module,
        &proof_bundle,
        None,
        None,
        artifact_manifest,
    )
    .expect("decoded source-produced sections should match their manifest");
    assert_eq!(artifact_manifest.semantic(), original_identity);
    assert_eq!(
        terminal_psi_identity(&semantic_module).unwrap(),
        original_identity
    );

    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced terminal Psi and its proof should verify");
    let fixed_fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("straight-line source module should have a fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed_fuel)
        .expect("source-independent consumer should recompute the certificate");
    assert_eq!(fixed_fuel.terminal_psi(), original_identity);
    assert_eq!(fixed_fuel.ceiling_units(), 4);
    let mut execution = TerminalExecution::start(&verified, &[])
        .expect("verified source-produced terminal Psi should start");
    let mut meter = TerminalFuelMeter::with_allowance(3);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(EdgeId::new(2).unwrap()),
            required_units: 1,
            remaining_units: 0,
        })
    );
    meter.replenish(1).unwrap();
    let interpreted = match execution.resume(&mut meter).unwrap() {
        TerminalExecutionStatus::Complete(value) => value,
        TerminalExecutionStatus::SponsorExhausted(_) => {
            panic!("one replenished unit should complete the source canary")
        }
    };
    assert_eq!(meter.usage().schedule().schedule_version(), 1);
    assert_eq!(meter.usage().total_units(), fixed_fuel.ceiling_units());
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(OperationId::new(1).unwrap()))
            .unwrap()
            .executions(),
        1,
        "resume must not replay source-produced operations"
    );
    assert_eq!(
        terminal_psi_identity(&semantic_module).unwrap(),
        original_identity,
        "fuel accounting must not change semantic identity"
    );
    let abstract_operations = lower_verified_module(&verified)
        .expect("verified terminal Psi should lower without source state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("constant terminal requirements should select for the host");
    let assigned = assign_registers(&target_operations).expect("host target homes should assign");
    let machine_code = emit_machine_code(&assigned).expect("host machine code should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("source-produced machine code should form an owned object artifact");
    assert_eq!(object_artifact.terminal_psi(), original_identity);
    let entry = object_artifact.entry_function();
    assert_eq!(
        entry.provenance.operations,
        [
            OperationId::new(1).expect("entry constant"),
            OperationId::new(2).expect("return constant"),
        ]
    );
    assert_eq!(
        entry.provenance.edges,
        [
            EdgeId::new(1).expect("jump edge"),
            EdgeId::new(2).expect("return edge"),
        ]
    );
    let entry_bytes = entry.bytes(&object_artifact).to_vec();
    let entry_offset = u64::try_from(entry.text_offset).expect("terminal entry offset");
    let (installed_code, entry_stub) = install_terminal_object(
        &object_artifact,
        object_artifact.text_bytes().to_vec(),
        entry_offset,
    );
    let wrong_entry =
        EntryStubId::from_normalized_identity(0x5302).expect("different entry stub identity");
    let error = bind_installed_terminal_entry_fuel(
        fixed_fuel.clone(),
        &object_artifact,
        &installed_code,
        wrong_entry,
    )
    .expect_err("terminal fuel binding must reject a different installed entry");
    assert!(error.0.contains("selected installed entry"));
    let installed_fixed_fuel = bind_installed_terminal_entry_fuel(
        fixed_fuel.clone(),
        &object_artifact,
        &installed_code,
        entry_stub,
    )
    .expect("terminal fuel theorem should bind the exact installed source artifact");
    validate_installed_terminal_entry_fuel(&installed_fixed_fuel, &installed_code, entry_stub)
        .expect("external-root recheck should accept the exact installed code and entry");
    assert!(
        validate_installed_terminal_entry_fuel(&installed_fixed_fuel, &installed_code, wrong_entry)
            .is_err(),
        "external-root recheck must reject a different selected entry"
    );
    let fuel_summary_identity =
        ProviderFuelSummaryId::from_normalized_identity(0x5100).expect("fuel summary identity");
    let certified_summary = FixedFuelProviderSummary::from_terminal_entry(
        fuel_summary_identity,
        RootProviderId::from_normalized_identity(0x5200).expect("root provider identity"),
        installed_fixed_fuel,
        BTreeSet::new(),
    );
    let certified_demand = compose_fixed_fuel(fuel_summary_identity, [&certified_summary])
        .expect("installed terminal Psi should supply its hard-root local fuel demand");
    assert_eq!(certified_demand.schedule(), fixed_fuel.schedule());
    assert_eq!(certified_demand.units(), fixed_fuel.ceiling_units());
    assert!(
        certified_demand.provider_receipts().is_empty(),
        "a recomputable terminal-Psi certificate is not an opaque provider receipt"
    );
    let mut changed_bytes = object_artifact.text_bytes().to_vec();
    changed_bytes[0] ^= 1;
    let (changed_code, changed_entry) =
        install_terminal_object(&object_artifact, changed_bytes, entry_offset);
    assert!(
        bind_installed_terminal_entry_fuel(
            fixed_fuel.clone(),
            &object_artifact,
            &changed_code,
            changed_entry,
        )
        .is_err(),
        "terminal fuel evidence must reject different installed bytes"
    );
    let wrong_offset = if entry_offset == 0 { 4 } else { 0 };
    let (wrong_entry_code, wrong_entry) = install_terminal_object(
        &object_artifact,
        object_artifact.text_bytes().to_vec(),
        wrong_offset,
    );
    assert!(
        bind_installed_terminal_entry_fuel(
            fixed_fuel.clone(),
            &object_artifact,
            &wrong_entry_code,
            wrong_entry,
        )
        .is_err(),
        "terminal fuel evidence must reject a stub at the wrong function offset"
    );

    drop(machine_code);
    drop(target_operations);
    drop(abstract_operations);
    drop(verified);
    drop(semantic_module);
    drop(proof_bundle);

    let object = emit_terminal_object_container(&object_artifact);
    assert_eq!(object.terminal_psi, original_identity);
    assert_eq!(&object.output.bytes[..8], b"OMGOBJ\0\0");
    assert_eq!(object.output.text_bytes, object_artifact.text_bytes().len());
    assert_eq!(object.output.relocations, 0);
    let image = emit_terminal_executable_image(&object_artifact, 3)
        .expect("source-produced owned artifact should emit a standalone host image");
    assert_eq!(image.terminal_psi(), original_identity);
    assert_eq!(
        image.output().final_text_bytes,
        object_artifact.text_bytes()
    );
    assert!(
        image
            .output()
            .executable_regions
            .unclassified_gaps
            .is_empty()
    );
    let installation = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(1).expect("source installation profile decision"),
        [],
    )
    .expect("source image should produce a typed installation record");
    validate_terminal_installation_record(&installation, &image)
        .expect("installation record should bind the exact source image");
    let installation_bytes =
        encode_terminal_installation_record(&installation).expect("canonical installation bytes");
    assert_eq!(
        decode_terminal_installation_record(&installation_bytes),
        Ok(installation)
    );

    let manifest_module = decode_module(&canonical_bytes)
        .expect("redecode semantic bytes after image realization state is dropped");
    let manifest_proof = decode_proof_bundle(&canonical_proof_bytes)
        .expect("redecode proof bytes after image realization state is dropped");
    let installed_manifest = build_artifact_manifest(
        &manifest_module,
        &manifest_proof,
        Some(&installation_bytes),
        None,
    )
    .expect("typed installation bytes should enter the artifact manifest");
    validate_artifact_manifest(
        &manifest_module,
        &manifest_proof,
        Some(&installation_bytes),
        None,
        installed_manifest,
    )
    .expect("installed artifact manifest should recompute from canonical sections");
    assert_eq!(installed_manifest.semantic(), original_identity);
    assert!(installed_manifest.installation().is_some());
    assert_ne!(installed_manifest.identity(), artifact_manifest.identity());

    let expected_exit = match interpreted {
        TerminalScalarValue::Integer {
            value: IntegerValue::Signed(value),
            ..
        } => i32::try_from(value).expect("source canary exit fits i32"),
        other => panic!("source canary returned unexpected value {other:?}"),
    };
    assert_eq!(run_host_machine_code(&entry_bytes), expected_exit);
    #[cfg(target_os = "macos")]
    assert_eq!(
        run_host_executable_image(&image.output().bytes),
        expected_exit
    );
}

fn install_terminal_object(
    terminal: &TerminalObjectArtifact,
    code: Vec<u8>,
    entry_offset: u64,
) -> (InstalledCode, EntryStubId) {
    fn installation_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    let entry = EntryStubId::from_normalized_identity(0x5300).expect("entry stub");
    let scope =
        ArtifactInstallationScopeId::from_normalized_identity(0x5301).expect("artifact scope");
    let constraints = PlacementConstraints::new(None, 16, PlacementPhase::Load, None, Some(scope))
        .expect("terminal placement constraints");
    let artifact = Artifact::from_canonical_decode(
        installation_id(0x5310, ArtifactId::from_normalized_identity),
        installation_id(0x5311, ArtifactContentId::from_normalized_identity),
        terminal.target().architecture,
        code,
        installation_id(0x5312, MachineContractSetId::from_normalized_identity),
        installation_id(0x5313, MachineFootprintId::from_normalized_identity),
        installation_id(0x5314, PlacementPlanId::from_normalized_identity),
        constraints,
        installation_id(0x5315, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, entry_offset)],
        installation_id(0x5316, RelocationSetId::from_normalized_identity),
        Vec::new(),
    )
    .expect("terminal text should decode as an executable artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            installation_id(0x5320, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("terminal artifact admission");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        0x5330,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        extent_id(0x5331, ExtentLineageId::from_normalized_identity),
        extent_id(0x5332, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(0x5333, ExtentProvenanceId::from_normalized_identity),
        extent_id(0x5334, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 4096)
    .expect("terminal placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        installation_id(0x5340, CodePlacementId::from_normalized_identity),
        installation_id(0x5301, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("terminal code placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("relocation-free terminal text should materialize exactly");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            installation_id(0x5341, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("terminal placement freeze");
    let validation = FinalValidationCertificate::from_validator(
        installation_id(0x5342, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated =
        validate_final_placement(frozen, &validation).expect("terminal final validation");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        installation_id(0x5343, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    (
        install_validated(validated, authority, receipt).expect("terminal code installation"),
        entry,
    )
}

#[cfg(target_os = "macos")]
fn run_host_executable_image(bytes: &[u8]) -> i32 {
    use std::os::unix::fs::PermissionsExt;

    let directory = fresh_scratch_directory("omega-terminal-source-image");
    let _cleanup = ScratchDirectory(directory.clone());
    let executable_path = directory.join("omega-program");
    std::fs::write(&executable_path, bytes).expect("write direct source terminal image");
    let mut permissions = std::fs::metadata(&executable_path)
        .expect("source terminal image metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable_path, permissions)
        .expect("mark source terminal image executable");
    Command::new(&executable_path)
        .status()
        .expect("execute direct source terminal image")
        .code()
        .expect("direct source terminal image exited normally")
}

#[cfg(unix)]
fn run_host_machine_code(bytes: &[u8]) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-native");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _main\n.p2align 2\n_main:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl main\n.type main,@function\nmain:\n.byte {bytes}\n.size main, .-main\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    std::fs::write(&assembly_path, assembly).expect("write native linker harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal native canary")
        .code()
        .expect("terminal native canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_nine_u8(bytes: &[u8], first: u8, second: u8, ninth: u8) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-nine-parameter");
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
        "#include <stdint.h>\n\
extern uint8_t terminal_entry(uint8_t, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t);\n\
int main(void) {{ return terminal_entry({first}, {second}, 3, 4, 5, 6, 7, 8, {ninth}); }}\n"
    );
    std::fs::write(&assembly_path, assembly).expect("write parameter assembly harness");
    std::fs::write(&driver_path, driver).expect("write parameter C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected parameter terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal parameter canary")
        .code()
        .expect("terminal parameter canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_nine_bool(bytes: &[u8]) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-nine-boolean");
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
    let driver = "#include <stdbool.h>\n\
extern bool terminal_entry(bool, bool, bool, bool, bool, bool, bool, bool, bool);\n\
int main(void) { return terminal_entry(false, false, false, false, false, false, false, false, true); }\n";
    std::fs::write(&assembly_path, assembly).expect("write Boolean assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal Boolean canary")
        .code()
        .expect("terminal Boolean canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_bool(bytes: &[u8], value: bool) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-boolean-not");
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
        "#include <stdbool.h>\nextern bool terminal_entry(bool);\nint main(void) {{ return terminal_entry({}); }}\n",
        if value { "true" } else { "false" }
    );
    std::fs::write(&assembly_path, assembly).expect("write Boolean-not assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean-not C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean-not terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal Boolean-not canary")
        .code()
        .expect("terminal Boolean-not canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_two_bools(bytes: &[u8], left: bool, right: bool) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-boolean-equality");
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
        "#include <stdbool.h>\nextern bool terminal_entry(bool, bool);\nint main(void) {{ return terminal_entry({}, {}); }}\n",
        if left { "true" } else { "false" },
        if right { "true" } else { "false" },
    );
    std::fs::write(&assembly_path, assembly).expect("write Boolean-equality assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean-equality C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean-equality machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal Boolean-equality canary")
        .code()
        .expect("terminal Boolean-equality canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_conditional_u8(
    bytes: &[u8],
    condition: bool,
    when_true: u8,
    when_false: u8,
) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-conditional");
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
extern uint8_t terminal_entry(bool, uint8_t, uint8_t);\n\
int main(void) {{ return terminal_entry({}, {when_true}, {when_false}); }}\n",
        if condition { "true" } else { "false" }
    );
    std::fs::write(&assembly_path, assembly).expect("write conditional assembly harness");
    std::fs::write(&driver_path, driver).expect("write conditional C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected conditional terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal conditional canary")
        .code()
        .expect("terminal conditional canary exited normally")
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
