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
use psi_checked_trees_to_terminal::{
    LoweringError, lower_machine, lower_machine_with_crash_context,
};
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
use psi_terminal::{CrashCause, CrashContextMaximum, OperationKind, SemanticVersion, Terminator};
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
            if evidence_type == "TerminalEvidence"
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

#[test]
fn checked_source_acyclic_branch_graph_reaches_both_native_backends() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("nested terminal-Psi branch source canary should compile");
    let lowered = lower_machine(&checked, "terminal_nested_integer_branch")
        .expect("nested ordered source branches should lower");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 6);
    assert_eq!(
        lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count(),
        2
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("nested source branch tree should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("nested source branch proof should encode canonically");
    let semantic_module = decode_module(&semantic_bytes).expect("decode nested source branch tree");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("decode nested source branch proof");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested source branch tree should verify after frontend drop");
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("nested branch tree should have an exact maximum fuel bound");
    assert_eq!(fixed.ceiling_units(), 6);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected, units) in [
        (true, true, 11_u128, 6_u64),
        (true, false, 21, 6),
        (false, false, 31, 5),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(10),
                integer(20),
                integer(30),
            ],
        )
        .expect("nested branch selection should interpret");
        assert_eq!(measured.value(), integer(expected));
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("nested branch tree should cross the Omega abstract boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("nested branch tree should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("nested branch parameter homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("nested branch machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_nested_jump_expressions_reach_terminal_and_native_lowering() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed nested-jump source canary should compile");
    let lowered = lower_machine(&checked, "terminal_nested_jump_expression")
        .expect("an unconditional nested jump may compute its arguments");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 4);
    assert_eq!(
        lowered.semantic_module.machines[0].blocks[1]
            .operations
            .len(),
        2
    );
    assert_eq!(
        lowered.semantic_module.machines[0].blocks[2]
            .operations
            .len(),
        2
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed nested jump should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed nested-jump proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("decode computed nested-jump module");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("decode computed nested-jump proof");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed nested jump should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed nested jump should have an exact fuel bound")
            .ceiling_units(),
        5
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (choose_add, expected) in [(true, 8_u128), (false, 14)] {
        let measured = interpret_terminal_measured(
            &verified,
            &[TerminalScalarValue::Boolean(choose_add), integer(7)],
        )
        .expect("computed nested jump should interpret");
        assert_eq!(measured.value(), integer(expected));
        assert_eq!(measured.usage().total_units(), 5);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("computed nested jump should cross the Omega abstract boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed nested jump should select for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("computed nested-jump parameter homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("computed nested-jump machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_conditional_edge_expressions_execute_only_on_the_selected_arm() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed conditional-edge source canary should compile");
    let lowered = lower_machine(&checked, "terminal_conditional_edge_expression")
        .expect("conditional edges may compute bindings in selected-arm blocks");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 4);
    assert!(matches!(
        machine.blocks[0].terminator,
        Terminator::Conditional {
            ref when_true,
            ref when_false,
            ..
        } if when_true.target.get() == 3 && when_false.target.get() == 4
    ));
    assert!(matches!(
        &machine.blocks[2].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerAdd { .. },
                ..
            },
        ]
    ));
    assert!(matches!(
        &machine.blocks[3].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerMultiply { .. },
                ..
            },
        ]
    ));

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed conditional edge should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed conditional-edge proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("decode computed conditional-edge module");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("decode computed conditional-edge proof");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed conditional edge should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed conditional edge should have an exact fuel bound")
            .ceiling_units(),
        5
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (choose_add, expected) in [(true, 8_u128), (false, 14)] {
        let measured = interpret_terminal_measured(
            &verified,
            &[TerminalScalarValue::Boolean(choose_add), integer(7)],
        )
        .expect("computed conditional edge should interpret");
        assert_eq!(measured.value(), integer(expected));
        assert_eq!(measured.usage().total_units(), 5);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("computed conditional edge should cross the Omega abstract boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed conditional edge should select for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("computed conditional-edge parameter homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("computed conditional-edge machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_short_circuit_guard_keeps_computed_bindings_arm_local() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit computed-edge source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_edge_expression")
        .expect("short-circuit guards should route into selected binding blocks");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 6);
    assert!(matches!(
        machine.blocks[0].terminator,
        Terminator::Jump { target, .. } if target.get() == 3
    ));
    assert!(matches!(
        &machine.blocks[4].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerAdd { .. },
                ..
            },
        ]
    ));
    assert!(matches!(
        &machine.blocks[5].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerMultiply { .. },
                ..
            },
        ]
    ));

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("short-circuit computed edge should encode");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("short-circuit computed-edge proof should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("short-circuit computed edge should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("short-circuit computed-edge proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short-circuit computed edge should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("short-circuit computed edge should have fixed fuel")
            .ceiling_units(),
        7
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected, units) in [
        (false, true, 14_u128, 6),
        (true, false, 14, 7),
        (true, true, 8, 7),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(7),
            ],
        )
        .expect("short-circuit computed edge should interpret");
        assert_eq!(measured.value(), integer(expected));
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("short-circuit computed edge should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("short-circuit computed edge should select for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("short-circuit computed-edge homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("short-circuit computed-edge machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
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
fn checked_source_boolean_conditional_arms_preserve_short_circuit_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_conditional_short_circuit_arms")
        .expect("Boolean conditional short-circuit arms should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("Boolean conditional arm control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean conditional arm control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean conditional arm control should verify");
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("Boolean conditional arm control should have exact fuel");
    assert_eq!(fixed.ceiling_units(), 6);

    for (condition, when_true, when_false) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected_units = if (condition && !when_true) || (!condition && when_false) {
            4
        } else {
            6
        };
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Boolean(when_true),
                TerminalScalarValue::Boolean(when_false),
            ],
        )
        .expect("Boolean conditional arm control should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(!condition));
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("Boolean conditional arm control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean conditional arm control should lower for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("Boolean conditional arm control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("Boolean conditional arm control should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("Boolean conditional arm control should form an object");
    let entry = object_artifact.entry_function();
    for (condition, when_true, when_false) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        assert_eq!(
            run_host_machine_code_with_three_bools(
                entry.bytes(&object_artifact),
                condition,
                when_true,
                when_false,
            ),
            i32::from(!condition)
        );
    }
}

#[cfg(unix)]
#[test]
fn checked_source_boolean_conditional_guard_preserves_short_circuit_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_conditional_short_circuit_guard")
        .expect("Boolean conditional short-circuit guard should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("Boolean conditional guard control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean conditional guard control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean conditional guard control should verify");
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("Boolean conditional guard control should have exact fuel");
    assert_eq!(fixed.ceiling_units(), 3);

    for (first, second, fallback) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = if first && second { first } else { fallback };
        let expected_units = if first { 3 } else { 2 };
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                TerminalScalarValue::Boolean(fallback),
            ],
        )
        .expect("Boolean conditional guard control should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("Boolean conditional guard control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean conditional guard control should lower for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("Boolean conditional guard control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("Boolean conditional guard control should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("Boolean conditional guard control should form an object");
    let entry = object_artifact.entry_function();
    for (first, second, fallback) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = if first && second { first } else { fallback };
        assert_eq!(
            run_host_machine_code_with_three_bools(
                entry.bytes(&object_artifact),
                first,
                second,
                fallback,
            ),
            i32::from(expected)
        );
    }
}

#[test]
fn checked_source_nested_boolean_control_reaches_both_native_targets() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("nested Boolean source canary should compile");
    let lowered = lower_machine(&checked, "terminal_nested_boolean_control")
        .expect("rooted acyclic Boolean control should lower");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 6);
    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("nested Boolean control should encode");
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("nested Boolean proof should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("nested Boolean control should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("nested Boolean proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested Boolean control should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("nested Boolean control should have fixed fuel")
            .ceiling_units(),
        7
    );

    for (arguments, expected, units) in [
        ([true, true, true, true, false, false], false, 6),
        ([true, false, true, true, false, false], true, 6),
        ([false, false, true, true, false, true], true, 6),
        ([true, true, false, true, false, false], false, 7),
    ] {
        let arguments = arguments.map(TerminalScalarValue::Boolean);
        let measured = interpret_terminal_measured(&verified, &arguments)
            .expect("nested Boolean control should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("nested Boolean control should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("nested Boolean control should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned =
            assign_registers(&target_operations).expect("nested Boolean homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("nested Boolean control should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
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
        SemanticVersion::CURRENT
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

#[test]
fn checked_source_runtime_integer_equality_round_trips_and_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime integer-equality source canary should compile");
    let lowered = lower_machine(&checked, "terminal_integer_equal_runtime")
        .expect("runtime integer equality should lower");
    drop(checked);

    assert_eq!(
        lowered.semantic_module.semantic_version,
        SemanticVersion::CURRENT
    );
    assert!(matches!(
        lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
        OperationKind::IntegerEqual { .. }
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("runtime integer equality should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("runtime integer equality should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime integer equality should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("runtime integer equality should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 2);
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 terminal type");
    for (left, right, expected) in [
        (0_u64, 0_u64, true),
        (0, 1, false),
        (u64::MAX, u64::MAX, true),
        (u64::MAX, u64::MAX - 1, false),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: IntegerValue::Unsigned(u128::from(left)),
                },
                TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: IntegerValue::Unsigned(u128::from(right)),
                },
            ],
        )
        .expect("runtime integer equality should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), 2);
    }

    #[cfg(unix)]
    {
        let abstract_operations = lower_verified_module(&verified)
            .expect("runtime integer equality should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("runtime integer equality should select for the host");
        assert!(matches!(
            &target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanExpression {
                expression: TerminalTargetBooleanExpression::IntegerEqual { .. },
                ..
            }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("runtime integer equality homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("runtime integer equality should emit");
        let object_artifact = build_terminal_object_artifact(&machine_code)
            .expect("runtime integer equality should form an object");
        let entry = object_artifact.entry_function();
        assert_eq!(entry.provenance.operations.len(), 1);
        for (left, right, expected) in [
            (0_u64, 0_u64, 1),
            (0, 1, 0),
            (u64::MAX, u64::MAX, 1),
            (u64::MAX, u64::MAX - 1, 0),
        ] {
            assert_eq!(
                run_host_machine_code_with_two_u64(entry.bytes(&object_artifact), left, right),
                expected
            );
        }
    }
}

#[test]
fn checked_source_runtime_integer_ordering_round_trips_and_preserves_signedness() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime integer-ordering source canary should compile");
    for (machine, inclusive, scalar_type, cases) in [
        (
            "terminal_unsigned_less_runtime",
            false,
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            vec![
                (IntegerValue::Unsigned(0), IntegerValue::Unsigned(1), true),
                (
                    IntegerValue::Unsigned(u64::MAX.into()),
                    IntegerValue::Unsigned(0),
                    false,
                ),
            ],
        ),
        (
            "terminal_signed_less_or_equal_runtime",
            true,
            IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
            vec![
                (IntegerValue::Signed(-1), IntegerValue::Signed(0), true),
                (IntegerValue::Signed(1), IntegerValue::Signed(0), false),
                (IntegerValue::Signed(1), IntegerValue::Signed(1), true),
            ],
        ),
    ] {
        let lowered = lower_machine(&checked, machine).expect("integer ordering should lower");
        assert_eq!(
            lowered.semantic_module.semantic_version,
            SemanticVersion::CURRENT
        );
        assert!(
            matches!(
                lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
                OperationKind::IntegerLessOrEqual { .. } if inclusive
            ) || matches!(
                lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
                OperationKind::IntegerLessThan { .. } if !inclusive
            )
        );
        let bytes = encode_module(&lowered.semantic_module).expect("ordering encodes");
        let decoded = decode_module(&bytes).expect("ordering decodes");
        let verified = verify_module(
            &decoded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("ordering verifies");
        assert_eq!(
            derive_fixed_entry_fuel(&verified, decoded.entry)
                .expect("ordering has fixed fuel")
                .ceiling_units(),
            2
        );
        for (left, right, expected) in &cases {
            let measured = interpret_terminal_measured(
                &verified,
                &[
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: *left,
                    },
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: *right,
                    },
                ],
            )
            .expect("ordering interprets");
            assert_eq!(measured.value(), TerminalScalarValue::Boolean(*expected));
            assert_eq!(measured.usage().total_units(), 2);
        }

        let abstract_operations =
            lower_verified_module(&verified).expect("ordering crosses the Omega boundary");
        let portable_target =
            lower_to_target_operations(&abstract_operations, NativeTarget::linux_x64())
                .expect("ordering selects for x86-64");
        let portable_expression = match &portable_target.functions[0].operation {
            TerminalTargetOperation::ReturnBooleanExpression { expression, .. } => expression,
            operation => panic!("unexpected ordering operation: {operation:?}"),
        };
        assert!(
            matches!(
                portable_expression,
                TerminalTargetBooleanExpression::IntegerLessOrEqual { .. } if inclusive
            ) || matches!(
                portable_expression,
                TerminalTargetBooleanExpression::IntegerLessThan { .. } if !inclusive
            )
        );
        let portable_assigned =
            assign_registers(&portable_target).expect("ordering homes assign for x86-64");
        emit_machine_code(&portable_assigned).expect("ordering emits for x86-64");

        #[cfg(unix)]
        {
            let abstract_operations = lower_verified_module(&verified).expect("Omega lowering");
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .expect("host selection");
            let expected_expression = match &target_operations.functions[0].operation {
                TerminalTargetOperation::ReturnBooleanExpression { expression, .. } => expression,
                operation => panic!("unexpected ordering operation: {operation:?}"),
            };
            assert!(
                matches!(
                    expected_expression,
                    TerminalTargetBooleanExpression::IntegerLessOrEqual { .. } if inclusive
                ) || matches!(
                    expected_expression,
                    TerminalTargetBooleanExpression::IntegerLessThan { .. } if !inclusive
                )
            );
            let assigned = assign_registers(&target_operations).expect("ordering homes assign");
            let machine_code = emit_machine_code(&assigned).expect("ordering emits");
            let object = build_terminal_object_artifact(&machine_code).expect("ordering object");
            let entry = object.entry_function();
            for (left, right, expected) in &cases {
                let left = match left {
                    IntegerValue::Unsigned(value) => *value as u64,
                    IntegerValue::Signed(value) => *value as i64 as u64,
                };
                let right = match right {
                    IntegerValue::Unsigned(value) => *value as u64,
                    IntegerValue::Signed(value) => *value as i64 as u64,
                };
                assert_eq!(
                    run_host_machine_code_with_two_u64(entry.bytes(&object), left, right),
                    i32::from(*expected)
                );
            }
        }
    }
}

#[test]
fn checked_source_computed_integer_comparison_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed integer-comparison source canary should compile");
    let lowered = lower_machine(&checked, "terminal_computed_greater_runtime")
        .expect("computed integer comparison should lower");
    drop(checked);

    assert!(matches!(
        &lowered.semantic_module.machines[0].blocks[0].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerMultiply { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerAdd { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::IntegerLessThan { .. },
                ..
            },
        ]
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed comparison should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed-comparison proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("computed comparison should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("computed-comparison proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed comparison should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed comparison should have fixed fuel")
            .ceiling_units(),
        6
    );

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(u128::from(value)),
    };
    for (left, right, expected) in [(10_u64, 3_u64, true), (5, 3, false), (u64::MAX, 0, false)] {
        let measured = interpret_terminal_measured(&verified, &[integer(left), integer(right)])
            .expect("computed comparison should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), 6);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("computed comparison should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed comparison should select for both native targets");
        assert!(matches!(
            &target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanExpression {
                expression: TerminalTargetBooleanExpression::IntegerLessThan { .. },
                ..
            }
        ));
        let assigned =
            assign_registers(&target_operations).expect("computed comparison homes should assign");
        emit_machine_code(&assigned).expect("computed comparison should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("computed comparison should select for the host");
        let assigned =
            assign_registers(&target_operations).expect("host comparison homes should assign");
        let machine_code = emit_machine_code(&assigned).expect("host comparison should emit");
        let object = build_terminal_object_artifact(&machine_code).expect("comparison object");
        let entry = object.entry_function();
        for (left, right, expected) in [(10_u64, 3_u64, 1), (5, 3, 0), (u64::MAX, 0, 0)] {
            assert_eq!(
                run_host_machine_code_with_two_u64(entry.bytes(&object), left, right),
                expected
            );
        }
    }
}

#[test]
fn checked_source_runtime_integer_bitwise_operations_cross_the_full_pipeline() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime integer-bitwise source canary should compile");
    let cases = [
        (
            "terminal_unsigned_bitwise_and_runtime",
            0b1100_u64,
            0b1010_u64,
            0b1000_u64,
            0_u8,
        ),
        (
            "terminal_unsigned_bitwise_or_runtime",
            0b1100,
            0b0011,
            0b1111,
            1,
        ),
        (
            "terminal_signed_bitwise_xor_runtime",
            u64::MAX,
            (-128_i64) as u64,
            127,
            2,
        ),
    ];
    for (machine, left, right, expected, kind) in cases {
        let lowered = lower_machine(&checked, machine).expect("integer bitwise should lower");
        assert_eq!(
            lowered.semantic_module.semantic_version,
            SemanticVersion::CURRENT
        );
        let operation = lowered.semantic_module.machines[0].blocks[0].operations[0].kind;
        assert!(matches!(
            (kind, operation),
            (0, OperationKind::IntegerBitwiseAnd { .. })
                | (1, OperationKind::IntegerBitwiseOr { .. })
                | (2, OperationKind::IntegerBitwiseXor { .. })
        ));
        let bytes = encode_module(&lowered.semantic_module).expect("bitwise module encodes");
        let decoded = decode_module(&bytes).expect("bitwise module decodes");
        let verified = verify_module(
            &decoded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("bitwise module verifies");
        assert_eq!(
            derive_fixed_entry_fuel(&verified, decoded.entry)
                .expect("bitwise machine has fixed fuel")
                .ceiling_units(),
            2
        );
        let scalar_type = if kind == 2 {
            IntegerType::new(IntegerSign::Signed, 64).expect("i64")
        } else {
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")
        };
        let input = |bits: u64| TerminalScalarValue::Integer {
            scalar_type,
            value: if kind == 2 {
                IntegerValue::Signed(bits as i64 as i128)
            } else {
                IntegerValue::Unsigned(bits.into())
            },
        };
        let measured = interpret_terminal_measured(&verified, &[input(left), input(right)])
            .expect("bitwise operation interprets");
        assert_eq!(measured.value(), input(expected));
        assert_eq!(measured.usage().total_units(), 2);

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let abstract_operations =
                lower_verified_module(&verified).expect("bitwise crosses the Omega boundary");
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("bitwise operation selects on both native architectures");
            let expression = match &target_operations.functions[0].operation {
                TerminalTargetOperation::ReturnIntegerExpression { expression, .. } => expression,
                operation => panic!("unexpected bitwise operation: {operation:?}"),
            };
            assert!(matches!(
                (kind, expression),
                (0, TerminalTargetIntegerExpression::BitwiseAnd { .. })
                    | (1, TerminalTargetIntegerExpression::BitwiseOr { .. })
                    | (2, TerminalTargetIntegerExpression::BitwiseXor { .. })
            ));
            let assigned =
                assign_registers(&target_operations).expect("bitwise parameter homes assign");
            emit_machine_code(&assigned).expect("bitwise operation emits exact native code");
        }

        #[cfg(unix)]
        {
            let abstract_operations = lower_verified_module(&verified).expect("Omega lowering");
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .expect("host selection");
            let assigned = assign_registers(&target_operations).expect("bitwise homes assign");
            let machine_code = emit_machine_code(&assigned).expect("bitwise host emission");
            let object = build_terminal_object_artifact(&machine_code).expect("bitwise object");
            assert_eq!(
                run_host_machine_code_with_two_u64(
                    object.entry_function().bytes(&object),
                    left,
                    right,
                ),
                expected as i32
            );
        }
    }
}

#[test]
fn checked_source_runtime_wrapping_shifts_cross_the_full_pipeline() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime wrapping-shift source canary should compile");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let cases = [
        (
            "terminal_unsigned_wrapping_shift_left_runtime",
            TerminalScalarValue::Integer {
                scalar_type: u64_type,
                value: IntegerValue::Unsigned(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: i64_type,
                value: IntegerValue::Signed(-1),
            },
            TerminalScalarValue::Integer {
                scalar_type: u64_type,
                value: IntegerValue::Unsigned(1_u128 << 63),
            },
            true,
        ),
        (
            "terminal_signed_wrapping_shift_right_runtime",
            TerminalScalarValue::Integer {
                scalar_type: i64_type,
                value: IntegerValue::Signed(-8),
            },
            TerminalScalarValue::Integer {
                scalar_type: u64_type,
                value: IntegerValue::Unsigned(65),
            },
            TerminalScalarValue::Integer {
                scalar_type: i64_type,
                value: IntegerValue::Signed(-4),
            },
            false,
        ),
    ];

    for (machine, value, count, expected, left_shift) in cases {
        let lowered = lower_machine(&checked, machine).expect("wrapping shift should lower");
        assert_eq!(
            lowered.semantic_module.semantic_version,
            SemanticVersion::CURRENT
        );
        assert!(matches!(
            (
                left_shift,
                &lowered.semantic_module.machines[0].blocks[0].operations[0].kind
            ),
            (true, OperationKind::WrappingIntegerShiftLeft { .. })
                | (false, OperationKind::WrappingIntegerShiftRight { .. })
        ));
        let bytes = encode_module(&lowered.semantic_module).expect("shift module encodes");
        let decoded = decode_module(&bytes).expect("shift module decodes");
        let verified = verify_module(
            &decoded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("shift module verifies");
        assert_eq!(
            derive_fixed_entry_fuel(&verified, decoded.entry)
                .expect("shift machine has fixed fuel")
                .ceiling_units(),
            2
        );
        let measured = interpret_terminal_measured(&verified, &[value, count])
            .expect("wrapping shift interprets");
        assert_eq!(measured.value(), expected);
        assert_eq!(measured.usage().total_units(), 2);

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let abstract_operations =
                lower_verified_module(&verified).expect("shift crosses the Omega boundary");
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("shift selects on both native architectures");
            let expression = match &target_operations.functions[0].operation {
                TerminalTargetOperation::ReturnIntegerExpression { expression, .. } => expression,
                operation => panic!("unexpected shift operation: {operation:?}"),
            };
            assert!(matches!(
                (left_shift, expression),
                (
                    true,
                    TerminalTargetIntegerExpression::WrappingShiftLeft { .. }
                ) | (
                    false,
                    TerminalTargetIntegerExpression::WrappingShiftRight { .. }
                )
            ));
            let assigned =
                assign_registers(&target_operations).expect("shift parameter homes assign");
            emit_machine_code(&assigned).expect("shift emits exact native code");
        }

        #[cfg(unix)]
        {
            let abstract_operations = lower_verified_module(&verified).expect("Omega lowering");
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .expect("host selection");
            let assigned = assign_registers(&target_operations).expect("shift homes assign");
            let machine_code = emit_machine_code(&assigned).expect("shift host emission");
            let object = build_terminal_object_artifact(&machine_code).expect("shift object");
            let input_bits = match value {
                TerminalScalarValue::Integer { value, .. } => match value {
                    IntegerValue::Unsigned(value) => value as u64,
                    IntegerValue::Signed(value) => value as i64 as u64,
                },
                TerminalScalarValue::Boolean(_) => unreachable!(),
            };
            let count_bits = match count {
                TerminalScalarValue::Integer { value, .. } => match value {
                    IntegerValue::Unsigned(value) => value as u64,
                    IntegerValue::Signed(value) => value as i64 as u64,
                },
                TerminalScalarValue::Boolean(_) => unreachable!(),
            };
            let expected_bits = match expected {
                TerminalScalarValue::Integer { value, .. } => match value {
                    IntegerValue::Unsigned(value) => value as u64,
                    IntegerValue::Signed(value) => value as i64 as u64,
                },
                TerminalScalarValue::Boolean(_) => unreachable!(),
            };
            assert!(
                host_machine_code_with_two_u64_matches(
                    object.entry_function().bytes(&object),
                    input_bits,
                    count_bits,
                    expected_bits,
                ),
                "emitted wrapping shift should return the complete expected u64"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn checked_source_runtime_boolean_inequality_reuses_terminal_primitives() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime Boolean-inequality source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_not_equal_runtime")
        .expect("runtime Boolean inequality should lower");
    drop(checked);

    let operations = &lowered.semantic_module.machines[0].blocks[0].operations;
    assert_eq!(operations.len(), 2);
    assert!(matches!(
        operations[0].kind,
        OperationKind::BooleanEqual { .. }
    ));
    assert!(matches!(
        operations[1].kind,
        OperationKind::BooleanNot { .. }
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("runtime Boolean inequality should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("runtime Boolean inequality should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime Boolean inequality should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("runtime Boolean inequality should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 3);
    for (left, right, expected) in [
        (false, false, false),
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(left),
                TerminalScalarValue::Boolean(right),
            ],
        )
        .expect("runtime Boolean inequality should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), 3);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("runtime Boolean inequality should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("runtime Boolean inequality should select for the host");
    assert!(matches!(
        &target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanExpression {
            expression: TerminalTargetBooleanExpression::Not { operand, .. },
            ..
        } if matches!(operand.as_ref(), TerminalTargetBooleanExpression::Equal { .. })
    ));
    let assigned = assign_registers(&target_operations)
        .expect("runtime Boolean inequality homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("runtime Boolean inequality should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("runtime Boolean inequality should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 2);
    for (left, right, expected) in [
        (false, false, 0),
        (false, true, 1),
        (true, false, 1),
        (true, true, 0),
    ] {
        assert_eq!(
            run_host_machine_code_with_two_bools(entry.bytes(&object_artifact), left, right),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn checked_source_short_circuit_booleans_lower_to_terminal_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit Boolean source canaries should compile");
    for (machine, is_and) in [
        ("terminal_boolean_and", true),
        ("terminal_boolean_or", false),
    ] {
        let lowered = lower_machine(&checked, machine)
            .expect("short-circuit Boolean expression should lower");
        let conditional_count = lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count();
        assert_eq!(conditional_count, 2);
        let semantic_bytes = encode_module(&lowered.semantic_module)
            .expect("short-circuit Boolean control should encode canonically");
        let semantic_module =
            decode_module(&semantic_bytes).expect("short-circuit Boolean control should decode");
        let verified = verify_module(
            &semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("short-circuit Boolean control should verify");
        let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("short-circuit Boolean control should have fixed fuel");
        assert_eq!(fuel.ceiling_units(), 4);

        for (left, right) in [(false, false), (false, true), (true, false), (true, true)] {
            let expected = if is_and { left && right } else { left || right };
            let expected_units = if (is_and && !left) || (!is_and && left) {
                3
            } else {
                4
            };
            let measured = interpret_terminal_measured(
                &verified,
                &[
                    TerminalScalarValue::Boolean(left),
                    TerminalScalarValue::Boolean(right),
                ],
            )
            .expect("short-circuit Boolean control should interpret");
            assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
            assert_eq!(measured.usage().total_units(), expected_units);
        }

        let abstract_operations = lower_verified_module(&verified)
            .expect("short-circuit Boolean control should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("short-circuit Boolean control should select for the host");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("short-circuit Boolean control homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("short-circuit Boolean control should emit");
        let object_artifact = build_terminal_object_artifact(&machine_code)
            .expect("short-circuit Boolean control should form an object");
        let entry = object_artifact.entry_function();
        for (left, right) in [(false, false), (false, true), (true, false), (true, true)] {
            let expected = i32::from(if is_and { left && right } else { left || right });
            assert_eq!(
                run_host_machine_code_with_two_bools(entry.bytes(&object_artifact), left, right),
                expected
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn checked_source_short_circuit_expression_conditions_reach_native_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit expression-condition canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_equal_and_equal")
        .expect("short-circuit expression conditions should lower");
    drop(checked);
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("expression-condition control should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("expression-condition control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("expression-condition control should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("expression-condition control should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 6);

    for (first, second, third) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = first == second && second == third;
        let expected_units = if first == second { 6 } else { 4 };
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                TerminalScalarValue::Boolean(third),
            ],
        )
        .expect("expression-condition control should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("expression-condition control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("expression-condition control should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanExpressionConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("expression-condition control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("expression-condition control should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("expression-condition control should form an object");
    let entry = object_artifact.entry_function();
    for (first, second, third) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = i32::from(first == second && second == third);
        assert_eq!(
            run_host_machine_code_with_three_bools(
                entry.bytes(&object_artifact),
                first,
                second,
                third,
            ),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn checked_source_short_circuit_operands_preserve_terminal_equality() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit equality canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_short_circuit_equality")
        .expect("short-circuit equality operands should lower");
    drop(checked);
    assert!(
        lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::BooleanEqual { .. })),
        "value-producing decision leaves must retain the v17 equality operation"
    );

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("short-circuit equality control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("short-circuit equality control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short-circuit equality control should verify");
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("short-circuit equality control should have exact fuel");
    assert_eq!(fixed.ceiling_units(), 8);

    for (first, second, third) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = (first && second) == (second || third);
        let expected_units = 4 + if first { 2 } else { 1 } + if second { 1 } else { 2 };
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                TerminalScalarValue::Boolean(third),
            ],
        )
        .expect("short-circuit equality control should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("short-circuit equality control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("short-circuit equality control should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("short-circuit equality control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("short-circuit equality control should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("short-circuit equality control should form an object");
    let entry = object_artifact.entry_function();
    for (first, second, third) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        assert_eq!(
            run_host_machine_code_with_three_bools(
                entry.bytes(&object_artifact),
                first,
                second,
                third,
            ),
            i32::from((first && second) == (second || third))
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

#[cfg(unix)]
#[test]
fn source_boolean_state_chain_return_preserves_short_circuit_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean state-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain_short_circuit_return")
        .expect("Boolean state-chain short-circuit return should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("state-chain short-circuit control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("state-chain short-circuit control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("state-chain short-circuit control should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("state-chain short-circuit control should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 6);

    for (value, expected_units) in [(false, 6), (true, 4)] {
        let measured =
            interpret_terminal_measured(&verified, &[TerminalScalarValue::Boolean(value)])
                .expect("state-chain short-circuit control should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(true));
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("state-chain short-circuit control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("state-chain short-circuit control should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("state-chain short-circuit control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("state-chain short-circuit control should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("state-chain short-circuit control should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), false),
        1
    );
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), true),
        1
    );
}

#[cfg(unix)]
#[test]
fn source_boolean_state_chain_binding_preserves_short_circuit_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean state-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain_short_circuit_binding")
        .expect("Boolean state-chain short-circuit binding should lower");
    drop(checked);

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("state-chain binding control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("state-chain binding control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("state-chain binding control should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("state-chain binding control should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 6);

    for (first, second) in [(false, false), (false, true), (true, false), (true, true)] {
        let expected = first && second;
        let expected_units = if first { 6 } else { 5 };
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("state-chain binding control should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("state-chain binding control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("state-chain binding control should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("state-chain binding control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("state-chain binding control should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("state-chain binding control should form an object");
    let entry = object_artifact.entry_function();
    for (first, second) in [(false, false), (false, true), (true, false), (true, true)] {
        assert_eq!(
            run_host_machine_code_with_two_bools(entry.bytes(&object_artifact), first, second,),
            i32::from(first && second)
        );
    }
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
    for machine in [
        "terminal_unpublished_abort",
        "terminal_narrow_abort",
        "terminal_guarded_abort",
    ] {
        assert_eq!(
            lower_machine(&checked, machine)
                .expect_err("a crash without a uniquely covering bucket must fail"),
            LoweringError::Unsupported(
                "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering bucket"
            )
        );
    }

    let mut missing_site = checked.clone();
    let terminal_abort = missing_site
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "terminal_abort")
        .expect("terminal abort machine")
        .symbol;
    let crash = &mut missing_site
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == terminal_abort)
        .expect("terminal abort contract plan")
        .crash;
    *crash = psi_checked_trees::CrashPlan::published_ceiling(crash.published().to_vec());
    assert_eq!(
        lower_machine(&missing_site, "terminal_abort")
            .expect_err("terminal production must consume checked crash-site evidence"),
        LoweringError::Unsupported("explicit crash has no body-derived checked crash-site row")
    );

    let mut missing_coverage = checked.clone();
    let crash = &mut missing_coverage
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == terminal_abort)
        .expect("terminal abort contract plan")
        .crash;
    let site = crash
        .checked_sites()
        .first()
        .expect("terminal abort checked site");
    let uncovered_site = psi_checked_trees::CheckedCrashSite::new(
        site.location(),
        site.cause(),
        Vec::new(),
        site.frontier_lower_bound().to_vec(),
    );
    *crash = psi_checked_trees::CrashPlan::published_ceiling(crash.published().to_vec())
        .with_checked_sites(vec![uncovered_site])
        .expect("uncovered site still has a valid checked location");
    assert_eq!(
        lower_machine(&missing_coverage, "terminal_abort")
            .expect_err("terminal production must consume checked guard coverage"),
        LoweringError::Unsupported(
            "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering bucket"
        )
    );

    let mut unmapped_frontier = checked.clone();
    let crash = &mut unmapped_frontier
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == terminal_abort)
        .expect("terminal abort contract plan")
        .crash;
    let site = crash
        .checked_sites()
        .first()
        .expect("terminal abort checked site");
    let claim = psi_language_semantics::PermissionClaimIdentity::Established {
        machine_symbol: terminal_abort,
        state_symbol: site.location().state(),
        source: psi_language_semantics::PermissionEventSource::StateEntry,
        ordinal: 0,
    };
    let site_with_frontier = psi_checked_trees::CheckedCrashSite::new(
        site.location(),
        site.cause(),
        site.guard_covering_buckets().to_vec(),
        vec![claim],
    );
    *crash = crash
        .clone()
        .with_checked_sites(vec![site_with_frontier])
        .expect("known claim identity is valid checked crash evidence");
    assert_eq!(
        lower_machine(&unmapped_frontier, "terminal_abort")
            .expect_err("terminal production must map every checked crash-frontier claim"),
        LoweringError::CrashFrontierClaimNotLowered(claim)
    );
}

#[test]
fn explicit_source_crash_lowers_to_verified_nonreturning_terminal() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi source canary should compile");
    let wide_trap = lower_machine(&checked, "terminal_wide_trap")
        .expect("a wider published trap demand should lower");
    assert_eq!(
        wide_trap.semantic_module.machines[0].contract.crash_context,
        CrashContextMaximum::portable_root()
    );
    assert!(matches!(
        lower_machine_with_crash_context(
            &checked,
            "terminal_wide_trap",
            vec![CrashContextMaximum {
                cause: CrashCause::Trap,
                maximum_scope: "Activation".to_owned(),
            }],
        ),
        Err(LoweringError::InvalidTerminalModule(
            psi_terminal_verifier::ModuleError::CrashContextMaximumTooNarrow { .. }
        ))
    ));
    assert!(matches!(
        lower_machine_with_crash_context(&checked, "terminal_wide_trap", Vec::new()),
        Err(LoweringError::InvalidTerminalModule(
            psi_terminal_verifier::ModuleError::MissingCrashContextMaximum {
                cause: CrashCause::Trap,
                ..
            }
        ))
    ));
    assert!(matches!(
        &wide_trap.semantic_module.machines[0].blocks[0].terminator,
        Terminator::Crash {
            cause: CrashCause::Trap,
            damage_minimum,
            containment_demand,
            ..
        } if damage_minimum == "Activation" && containment_demand == "ExecutionDomain"
    ));
    let guarded_trap = lower_machine(&checked, "terminal_path_guarded_trap")
        .expect("checked incoming guard coverage should open a guarded crash branch");
    assert!(matches!(
        &guarded_trap.semantic_module.machines[0].blocks[1].terminator,
        Terminator::Crash {
            cause: CrashCause::Trap,
            damage_minimum,
            containment_demand,
            ..
        } if damage_minimum == "Activation" && containment_demand == "ExecutionDomain"
    ));
    let guarded_semantic_bytes =
        encode_module(&guarded_trap.semantic_module).expect("guarded crash should encode");
    let guarded_proof_bytes =
        encode_proof_bundle(&guarded_trap.proof_bundle).expect("guarded crash proof should encode");
    let guarded_semantic_module =
        decode_module(&guarded_semantic_bytes).expect("guarded crash should decode");
    let guarded_proof_bundle =
        decode_proof_bundle(&guarded_proof_bytes).expect("guarded crash proof should decode");
    let guarded_verified = verify_module(
        &guarded_semantic_module,
        &guarded_proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the guarded crash branch should verify");
    assert_eq!(
        derive_fixed_entry_fuel(&guarded_verified, guarded_semantic_module.entry)
            .expect("guarded crash control should have a fixed entry ceiling")
            .ceiling_units(),
        3
    );
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    for (flag, expected) in [
        (
            true,
            TerminalExecutionStatus::Crashed(omega_interpreter::TerminalCrash {
                cause: CrashCause::Trap,
                damage_minimum: "Activation".to_owned(),
                containment_demand: "ExecutionDomain".to_owned(),
                frontier_lower_bound: Vec::new(),
            }),
        ),
        (
            false,
            TerminalExecutionStatus::Complete(TerminalScalarValue::Integer {
                scalar_type: i32_type,
                value: IntegerValue::Signed(0),
            }),
        ),
    ] {
        let mut execution =
            TerminalExecution::start(&guarded_verified, &[TerminalScalarValue::Boolean(flag)])
                .expect("guarded crash execution should start");
        let mut guarded_meter = TerminalFuelMeter::unbounded();
        assert_eq!(execution.resume(&mut guarded_meter).unwrap(), expected);
    }
    let integer_guarded_trap = lower_machine(&checked, "terminal_integer_guarded_trap")
        .expect("exact-type integer comparison should open a guarded crash branch");
    assert!(matches!(
        &integer_guarded_trap.semantic_module.machines[0].blocks[0].operations[..],
        [
            psi_terminal::Operation {
                kind: psi_terminal::OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: psi_terminal::OperationKind::WrappingIntegerAdd { .. },
                ..
            },
            psi_terminal::Operation {
                kind: psi_terminal::OperationKind::IntegerLessOrEqual { left, right },
                ..
            },
        ] if left.get() == 2 && right.get() == 4
    ));
    let integer_guarded_semantic_bytes = encode_module(&integer_guarded_trap.semantic_module)
        .expect("integer-guarded crash should encode");
    let integer_guarded_proof_bytes = encode_proof_bundle(&integer_guarded_trap.proof_bundle)
        .expect("integer-guarded crash proof should encode");
    let integer_guarded_semantic_module = decode_module(&integer_guarded_semantic_bytes)
        .expect("integer-guarded crash should decode");
    let integer_guarded_proof_bundle = decode_proof_bundle(&integer_guarded_proof_bytes)
        .expect("integer-guarded crash proof should decode");
    let integer_guarded_verified = verify_module(
        &integer_guarded_semantic_module,
        &integer_guarded_proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the integer-guarded crash branch should verify");
    assert_eq!(
        derive_fixed_entry_fuel(
            &integer_guarded_verified,
            integer_guarded_semantic_module.entry
        )
        .expect("integer-guarded crash control should have a fixed entry ceiling")
        .ceiling_units(),
        6
    );
    for (value, limit, expected) in [
        (
            1,
            2,
            TerminalExecutionStatus::Crashed(omega_interpreter::TerminalCrash {
                cause: CrashCause::Trap,
                damage_minimum: "Activation".to_owned(),
                containment_demand: "ExecutionDomain".to_owned(),
                frontier_lower_bound: Vec::new(),
            }),
        ),
        (
            1,
            3,
            TerminalExecutionStatus::Complete(TerminalScalarValue::Integer {
                scalar_type: i32_type,
                value: IntegerValue::Signed(0),
            }),
        ),
    ] {
        let mut execution = TerminalExecution::start(
            &integer_guarded_verified,
            &[
                TerminalScalarValue::Integer {
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(value),
                },
                TerminalScalarValue::Integer {
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(limit),
                },
            ],
        )
        .expect("integer-guarded crash execution should start");
        let mut meter = TerminalFuelMeter::unbounded();
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
    }
    let transitive_trap = lower_machine(&checked, "terminal_transitive_guarded_trap")
        .expect("a transitive integer conjunction should lower as short-circuit control");
    assert_eq!(transitive_trap.semantic_module.machines[0].blocks.len(), 5);
    assert!(matches!(
        transitive_trap.semantic_module.machines[0].blocks[0].terminator,
        Terminator::Jump { target, .. } if target.get() == 4
    ));
    let transitive_semantic_bytes = encode_module(&transitive_trap.semantic_module)
        .expect("transitive guarded crash should encode");
    let transitive_proof_bytes = encode_proof_bundle(&transitive_trap.proof_bundle)
        .expect("transitive guarded crash proof should encode");
    let transitive_semantic_module =
        decode_module(&transitive_semantic_bytes).expect("transitive guarded crash should decode");
    let transitive_proof_bundle = decode_proof_bundle(&transitive_proof_bytes)
        .expect("transitive guarded crash proof should decode");
    let transitive_verified = verify_module(
        &transitive_semantic_module,
        &transitive_proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("transitive guarded crash should verify");
    assert_eq!(
        derive_fixed_entry_fuel(&transitive_verified, transitive_semantic_module.entry)
            .expect("transitive guarded crash should have fixed fuel")
            .ceiling_units(),
        7
    );
    let signed = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    for (left, middle, right, expected, expected_units) in [
        (5, 3, 10, TerminalExecutionStatus::Complete(signed(0)), 5),
        (1, 5, 3, TerminalExecutionStatus::Complete(signed(0)), 7),
        (
            1,
            2,
            3,
            TerminalExecutionStatus::Crashed(omega_interpreter::TerminalCrash {
                cause: CrashCause::Trap,
                damage_minimum: "Activation".to_owned(),
                containment_demand: "ExecutionDomain".to_owned(),
                frontier_lower_bound: Vec::new(),
            }),
            6,
        ),
    ] {
        let mut execution = TerminalExecution::start(
            &transitive_verified,
            &[signed(left), signed(middle), signed(right)],
        )
        .expect("transitive guarded crash execution should start");
        let mut meter = TerminalFuelMeter::unbounded();
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
        assert_eq!(meter.usage().total_units(), expected_units);
    }
    let implied_trap = lower_machine(&checked, "terminal_implied_guarded_trap")
        .expect("structurally implied guard coverage should reach terminal production");
    let implied_verified = verify_module(
        &implied_trap.semantic_module,
        &implied_trap.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the structurally implied crash branch should verify");
    for (flag, crashes) in [(true, true), (false, false)] {
        let mut execution =
            TerminalExecution::start(&implied_verified, &[TerminalScalarValue::Boolean(flag)])
                .expect("implied crash execution should start");
        let mut implied_meter = TerminalFuelMeter::unbounded();
        assert_eq!(
            matches!(
                execution.resume(&mut implied_meter).unwrap(),
                TerminalExecutionStatus::Crashed(_)
            ),
            crashes
        );
    }
    let lowered = lower_machine(&checked, "terminal_abort")
        .expect("an unconditional published crash should lower");
    let explicit_true = lower_machine(&checked, "terminal_explicit_true_abort")
        .expect("an explicit-true crash route should normalize to unconditional coverage");
    assert_eq!(
        lowered.semantic_module, explicit_true.semantic_module,
        "route-less and explicit-true crash ceilings lower identically"
    );
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("source slice should emit one machine");
    };
    let [block] = machine.blocks.as_slice() else {
        panic!("crash-only source should emit one block");
    };
    assert_eq!(
        block.terminator,
        Terminator::Crash {
            edge: EdgeId::new(1).unwrap(),
            cause: CrashCause::Abort,
            damage_minimum: "ExecutionDomain".to_owned(),
            containment_demand: "ExecutionDomain".to_owned(),
            frontier_lower_bound: Vec::new(),
        }
    );

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced crash terminal should verify");
    let mut execution =
        TerminalExecution::start(&verified, &[]).expect("verified crash terminal should start");
    let mut meter = TerminalFuelMeter::with_allowance(1);
    let expected = TerminalExecutionStatus::Crashed(omega_interpreter::TerminalCrash {
        cause: CrashCause::Abort,
        damage_minimum: "ExecutionDomain".to_owned(),
        containment_demand: "ExecutionDomain".to_owned(),
        frontier_lower_bound: Vec::new(),
    });
    assert_eq!(execution.resume(&mut meter).unwrap(), expected);
    let charged = meter.usage().total_units();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        expected,
        "resuming a crashed execution reports the same terminal outcome"
    );
    assert_eq!(
        meter.usage().total_units(),
        charged,
        "resuming a crash must not replay its edge"
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
        TerminalExecutionStatus::Crashed(_) => {
            panic!("the source canary has no crash exit")
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
fn run_host_machine_code_with_two_u64(bytes: &[u8], left: u64, right: u64) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-integer-equality");
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
extern uint8_t terminal_entry(uint64_t, uint64_t);\n\
int main(void) {{ return terminal_entry({left}ULL, {right}ULL); }}\n"
    );
    std::fs::write(&assembly_path, assembly).expect("write integer-equality assembly harness");
    std::fs::write(&driver_path, driver).expect("write integer-equality C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected integer-equality terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal integer-equality canary")
        .code()
        .expect("terminal integer-equality canary exited normally")
}

#[cfg(unix)]
fn host_machine_code_with_two_u64_matches(
    bytes: &[u8],
    left: u64,
    right: u64,
    expected: u64,
) -> bool {
    let directory = fresh_scratch_directory("omega-terminal-integer-result");
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
extern uint64_t terminal_entry(uint64_t, uint64_t);\n\
int main(void) {{ return terminal_entry({left}ULL, {right}ULL) == {expected}ULL ? 0 : 1; }}\n"
    );
    std::fs::write(&assembly_path, assembly).expect("write integer-result assembly harness");
    std::fs::write(&driver_path, driver).expect("write integer-result C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected integer-result terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal integer-result canary")
        .success()
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
fn run_host_machine_code_with_three_bools(
    bytes: &[u8],
    first: bool,
    second: bool,
    third: bool,
) -> i32 {
    let directory = fresh_scratch_directory("omega-terminal-boolean-control-expression");
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
        "#include <stdbool.h>\nextern bool terminal_entry(bool, bool, bool);\nint main(void) {{ return terminal_entry({}, {}, {}); }}\n",
        if first { "true" } else { "false" },
        if second { "true" } else { "false" },
        if third { "true" } else { "false" },
    );
    std::fs::write(&assembly_path, assembly)
        .expect("write Boolean-control-expression assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean-control-expression C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean-control-expression machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal Boolean-control-expression canary")
        .code()
        .expect("terminal Boolean-control-expression canary exited normally")
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
