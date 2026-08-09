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
    TerminalExecution, TerminalExecutionStatus, TerminalScalarValue,
    interpret_terminal_artifact_measured, interpret_terminal_measured,
};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractOperation,
    TerminalAbstractOperationPlan, TerminalValueBinding,
};
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_assigned_target_operations::{
    TerminalAssignedBooleanControl, TerminalAssignedIntegerControl, TerminalAssignedOperation,
};
use omega_terminal_image_emission::{
    TerminalObjectArtifact, build_terminal_installation_record, build_terminal_object_artifact,
    decode_terminal_installation_record, emit_terminal_executable_image,
    emit_terminal_object_container, encode_terminal_installation_record,
    validate_terminal_installation_record,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::{lower_artifact_sections, lower_verified_module};
use omega_terminal_target_operations::{
    TerminalTargetBooleanControl, TerminalTargetBooleanExpression, TerminalTargetIntegerControl,
    TerminalTargetIntegerExpression, TerminalTargetOperation,
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
use psi_terminal::{CrashCause, CrashContextMaximum, OperationKind, Terminator, VocabularyMarker};
use psi_terminal_codec::{
    DebugSubject, build_artifact_manifest, decode_debug_map, decode_module, decode_proof_bundle,
    encode_debug_map, encode_module, encode_proof_bundle, terminal_psi_identity,
    validate_artifact_manifest,
};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_verifier::{VerifiedTerminalModule, verify_module};
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

fn assert_guarded_crash_emits(verified: &VerifiedTerminalModule<'_>) {
    let abstract_operations = lower_verified_module(verified)
        .expect("guarded crash should cross the source-independent Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("guarded crash should select as recursive terminal control");
        let assigned =
            assign_registers(&target_operations).expect("guarded crash control should assign");
        let emitted = emit_machine_code(&assigned).expect("guarded crash control should emit");
        let fault = match target.architecture {
            omega_target::Architecture::X86_64 => &[0x0f, 0x0b][..],
            omega_target::Architecture::Aarch64 => &[0x00, 0x00, 0x20, 0xd4][..],
        };
        assert!(
            emitted.functions[0]
                .bytes
                .windows(fault.len())
                .any(|window| window == fault),
            "guarded crash machine code must retain its selected fault leaf"
        );
    }
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
    let abstract_operations = lower_artifact_sections(
        &canonical_bytes,
        &canonical_proof_bytes,
        &AdmissionProfile::default(),
    )
    .expect("canonical artifact sections should lower without producer state");
    let measured = interpret_terminal_artifact_measured(
        &canonical_bytes,
        &canonical_proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("canonical artifact sections should execute with fuel");
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
                        result: ValueId::new(2).expect("jump constant"),
                        scalar_type: ScalarType::Integer(i32_type),
                        value: IntegerValue::Signed(7),
                    },
                    TerminalAbstractOperation::Jump {
                        psi_edge: EdgeId::new(1).expect("jump edge"),
                        target: BlockId::new(2).expect("return block"),
                        bindings: vec![TerminalValueBinding {
                            parameter: ValueId::new(1).expect("block parameter"),
                            argument: ValueId::new(2).expect("jump constant"),
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
fn terminal_scalar_contract_consumes_the_source_independent_checked_plan() {
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
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the checked scalar contract should lower");

    let mut without_contract_expressions = checked.clone();
    let contract_expressions = {
        let machine = without_contract_expressions
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "terminal_constant")
            .expect("terminal constant machine");
        without_contract_expressions
            .machine_contracts(machine)
            .iter()
            .flat_map(|contract| {
                without_contract_expressions
                    .proof_facts
                    .span_or_empty(contract.facts)
            })
            .filter_map(|fact| match fact {
                psi_typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for expression in contract_expressions {
        *without_contract_expressions
            .typed
            .expression_table
            .expression_mut(expression) =
            psi_checked_trees::expression::ExpressionNode::Boolean(false);
    }

    let actual = lower_machine(&without_contract_expressions, "terminal_constant")
        .expect("terminal production must not reopen checked contract expressions");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_contract = checked;
    let terminal_constant = without_checked_contract
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "terminal_constant")
        .expect("terminal constant machine")
        .symbol;
    without_checked_contract
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == terminal_constant)
        .expect("terminal constant contract plan")
        .closed_scalar_values = Default::default();
    assert_eq!(
        lower_machine(&without_checked_contract, "terminal_constant")
            .expect_err("terminal production must fail without checked scalar contract values"),
        LoweringError::Unsupported("machine must have exactly one requires and one ensures clause")
    );
}

#[test]
fn terminal_scalar_body_consumes_the_source_independent_checked_plan() {
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
    let expected =
        lower_machine(&checked, "terminal_constant").expect("the checked scalar body should lower");

    let return_expression = {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "terminal_constant")
            .expect("terminal constant machine");
        checked
            .machine_states(machine)
            .iter()
            .flat_map(|state| {
                checked
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
            })
            .find_map(|statement| match statement {
                psi_checked_trees::statement::StatementNode::Expression(expression) => {
                    Some(*expression)
                }
                _ => None,
            })
            .expect("terminal constant return expression")
    };
    let mut without_typed_return = checked.clone();
    *without_typed_return
        .typed
        .expression_table
        .expression_mut(return_expression) =
        psi_checked_trees::expression::ExpressionNode::Boolean(false);

    let actual = lower_machine(&without_typed_return, "terminal_constant")
        .expect("terminal production must not reopen the checked return expression");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_scalar_body = checked;
    without_checked_scalar_body.facts.values.scalar_expressions = Default::default();
    assert_eq!(
        lower_machine(&without_checked_scalar_body, "terminal_constant")
            .expect_err("terminal production must fail without the checked scalar body"),
        LoweringError::Unsupported(
            "scalar expression has no source-independent checked value plan"
        )
    );
}

#[test]
fn terminal_scalar_control_consumes_the_source_independent_checked_plan() {
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
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the checked scalar control plan should lower");

    let replacement_transition = {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "terminal_path_guarded_trap")
            .expect("guarded trap machine");
        checked
            .machine_states(machine)
            .iter()
            .flat_map(|state| {
                checked
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
            })
            .find(|statement| {
                matches!(
                    statement,
                    psi_checked_trees::statement::StatementNode::Transition(_)
                )
            })
            .expect("a valid replacement transition")
            .clone()
    };
    let constant_statements = {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "terminal_constant")
            .expect("terminal constant machine");
        checked
            .machine_states(machine)
            .first()
            .expect("terminal constant entry state")
            .statement_nodes
    };
    let mut without_typed_control = checked.clone();
    let [statement] = without_typed_control
        .typed
        .statement_table
        .statements_mut(constant_statements)
    else {
        panic!("terminal constant must have one statement");
    };
    *statement = replacement_transition;

    let actual = lower_machine(&without_typed_control, "terminal_constant")
        .expect("terminal production must not reopen checked statement topology");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_control = checked;
    without_checked_control.facts.flow.terminal_scalar_graphs = Default::default();
    assert_eq!(
        lower_machine(&without_checked_control, "terminal_constant")
            .expect_err("terminal production must fail without checked scalar control"),
        LoweringError::Unsupported("machine has no source-independent checked scalar control plan")
    );
}

#[test]
fn terminal_machine_selection_consumes_the_source_independent_checked_plan() {
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
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the checked machine selection should lower");
    let replacement_name = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() != "terminal_constant")
        .expect("a replacement machine name")
        .name
        .clone();

    let mut without_typed_selection = checked.clone();
    let source_machine = without_typed_selection
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "terminal_constant")
        .expect("terminal constant machine");
    source_machine.name = replacement_name;
    source_machine.boundary = true;

    let actual = lower_machine(&without_typed_selection, "terminal_constant")
        .expect("terminal production must not reopen typed machine selection or eligibility");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_selection = checked;
    without_checked_selection.facts.flow.terminal_machines = Default::default();
    assert_eq!(
        lower_machine(&without_checked_selection, "terminal_constant")
            .expect_err("terminal production must fail without checked machine selection"),
        LoweringError::MachineNotFound("terminal_constant".to_owned())
    );
}

#[test]
fn terminal_production_survives_complete_typed_frontend_drop() {
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
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the complete checked terminal plan should lower");

    let mut without_typed_frontend = checked.clone();
    without_typed_frontend.typed = Default::default();
    let actual = lower_machine(&without_typed_frontend, "terminal_constant")
        .expect("terminal production must survive complete typed-tree disposal");
    assert_eq!(actual, expected);

    let mut without_debug_presentation = checked;
    without_debug_presentation.facts.flow.terminal_debug = Default::default();
    let without_debug = lower_machine(&without_debug_presentation, "terminal_constant")
        .expect("debug presentation must be optional at the terminal boundary");
    assert_eq!(without_debug.semantic_module, expected.semantic_module);
    assert_eq!(without_debug.proof_bundle, expected.proof_bundle);
    assert_eq!(without_debug.debug_map, None);
}

#[test]
fn terminal_proposition_vocabulary_consumes_checked_proof_facts() {
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
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the checked proposition vocabulary should lower");
    assert!(!expected.semantic_module.proposition_declarations.is_empty());
    assert!(!expected.semantic_module.proposition_applications.is_empty());

    let mut without_typed_declarations = checked.clone();
    without_typed_declarations.typed.roots.propositions = Default::default();
    let actual = lower_machine(&without_typed_declarations, "terminal_constant")
        .expect("terminal production must not reopen typed proposition declarations");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_vocabulary = checked;
    without_checked_vocabulary
        .facts
        .proof
        .proposition_vocabulary = Default::default();
    let absent = lower_machine(&without_checked_vocabulary, "terminal_constant")
        .expect("an intentionally empty checked proposition vocabulary remains valid");
    assert!(absent.semantic_module.proposition_declarations.is_empty());
    assert!(absent.semantic_module.proposition_applications.is_empty());
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
        ("terminal_exact_literal_narrowing", vec![], 127_u128, 2_u64),
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
fn checked_source_exact_literal_narrowing_relands_before_terminal_psi() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("exact literal narrowing source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_literal_narrowing")
        .expect("exact literal narrowing should lower to terminal Psi");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");

    let operations = &lowered.semantic_module.machines[0].blocks[0].operations;
    assert_eq!(operations.len(), 1);
    assert!(matches!(
        operations[0].kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(127)
        }
    ));
    assert_eq!(
        operations[0].result.scalar_type,
        ScalarType::Integer(u8_type)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("narrowing semantic bytes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("narrowing proof bytes");
    drop(lowered);
    let measured =
        interpret_terminal_artifact_measured(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("decoded narrowing artifact should interpret");
    assert_eq!(
        measured.value(),
        TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(127),
        }
    );
    assert_eq!(measured.usage().total_units(), 2);

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("narrowing artifact should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("narrowing constant should select");
        let assigned = assign_registers(&target_operations).expect("narrowing homes should assign");
        emit_machine_code(&assigned).expect("narrowing constant should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("narrowing host selection");
        let assigned = assign_registers(&target_operations).expect("narrowing host homes");
        let machine_code = emit_machine_code(&assigned).expect("narrowing host emission");
        let object = build_terminal_object_artifact(&machine_code).expect("narrowing host object");
        assert_eq!(
            run_host_machine_code(object.entry_function().bytes(&object)),
            127
        );
    }
}

#[test]
fn checked_source_guarded_exact_narrowing_carries_independently_verified_evidence() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("guarded exact-narrowing source canary should compile");
    let lowered = lower_machine(&checked, "terminal_guarded_exact_narrow")
        .expect("guarded exact narrowing should lower with path evidence");
    let cast_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::IntegerExactCast { .. }))
        .expect("the runtime narrowing remains explicit terminal work");
    let OperationKind::IntegerExactCast {
        obligation: cast_obligation,
        ..
    } = cast_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&cast_operation.kind),
        1
    );
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == cast_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));

    let semantic = encode_module(&lowered.semantic_module).expect("guarded narrowing semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("guarded narrowing proof");
    let module = decode_module(&semantic).expect("decode guarded narrowing semantics");
    let mut missing_cast_proof = decode_proof_bundle(&proof).expect("decode guarded proof");
    missing_cast_proof
        .evidence
        .retain(|evidence| evidence.obligation != cast_obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_cast_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == cast_obligation
    ));

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(0)],
        )
        .expect("verified guarded narrowing should interpret")
    };
    let narrowed = execute(255);
    let rejected = execute(256);
    assert_eq!(
        narrowed.value(),
        TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(255),
        }
    );
    assert_eq!(
        rejected.value(),
        TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(0),
        }
    );
    assert_eq!(
        narrowed.usage().total_units(),
        rejected.usage().total_units()
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("guarded narrowing should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| {
                matches!(
                    operation,
                    TerminalAbstractOperation::IntegerExactCast { .. }
                )
            })
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("guarded narrowing should select");
        let assigned =
            assign_registers(&target_operations).expect("guarded narrowing homes should assign");
        emit_machine_code(&assigned).expect("guarded narrowing should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("guarded narrowing host selection");
        let assigned = assign_registers(&target_operations).expect("guarded narrowing host homes");
        let machine_code = emit_machine_code(&assigned).expect("guarded narrowing host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("guarded narrowing host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 255, 0), 255);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 256, 0), 0);
    }
}

#[test]
fn checked_source_exact_right_shift_carries_independently_verified_count_evidence() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("exact right-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_right_runtime")
        .expect("exact right shift should lower with path evidence");
    let shift_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerShiftRight { .. }))
        .expect("the proof-gated right shift remains explicit terminal work");
    let OperationKind::ExactIntegerShiftRight {
        obligation: shift_obligation,
        ..
    } = shift_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&shift_operation.kind),
        1
    );
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == shift_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));

    let semantic = encode_module(&lowered.semantic_module).expect("exact shift semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact shift proof");
    let module = decode_module(&semantic).expect("decode exact shift semantics");
    let mut missing_shift_proof = decode_proof_bundle(&proof).expect("decode exact shift proof");
    missing_shift_proof
        .evidence
        .retain(|evidence| evidence.obligation != shift_obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_shift_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == shift_obligation
    ));

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value, count| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(count)],
        )
        .expect("verified exact right shift should interpret")
    };
    let shifted = execute(1u128 << 63, 63);
    let rejected = execute(1u128 << 63, 64);
    assert_eq!(shifted.value(), argument(1));
    assert_eq!(rejected.value(), argument(0));
    assert_eq!(shifted.usage().total_units(), 6);
    assert_eq!(
        shifted.usage().total_units(),
        rejected.usage().total_units()
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact shift should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| {
                matches!(
                    operation,
                    TerminalAbstractOperation::ExactIntegerShiftRight { .. }
                )
            })
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact shift should select");
        let assigned =
            assign_registers(&target_operations).expect("exact shift homes should assign");
        emit_machine_code(&assigned).expect("exact shift should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact shift host selection");
        let assigned = assign_registers(&target_operations).expect("exact shift host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact shift host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact shift host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 1u64 << 63, 63), 1);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 1u64 << 63, 64), 0);
    }
}

#[test]
fn checked_source_exact_left_shift_carries_count_and_value_evidence() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("exact left-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_left_runtime")
        .expect("exact left shift should lower with path evidence");
    let shift_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerShiftLeft { .. }))
        .expect("the proof-gated left shift remains explicit terminal work");
    let OperationKind::ExactIntegerShiftLeft {
        obligation: shift_obligation,
        ..
    } = shift_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&shift_operation.kind),
        1
    );
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == shift_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));

    let semantic = encode_module(&lowered.semantic_module).expect("exact left-shift semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact left-shift proof");
    let module = decode_module(&semantic).expect("decode exact left-shift semantics");
    let mut missing_shift_proof =
        decode_proof_bundle(&proof).expect("decode exact left-shift proof");
    missing_shift_proof
        .evidence
        .retain(|evidence| evidence.obligation != shift_obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_shift_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == shift_obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value, count| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(count)],
        )
        .expect("verified exact left shift should interpret")
    };
    let shifted = execute(1, 31);
    let rejected_value = execute(2, 31);
    let rejected_count = execute(1, 32);
    assert_eq!(shifted.value(), argument(1u128 << 31));
    assert_eq!(rejected_value.value(), argument(0));
    assert_eq!(rejected_count.value(), argument(0));
    assert_eq!(
        shifted.usage().total_units(),
        rejected_value.usage().total_units()
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact left shift should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerShiftLeft { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact left shift should select");
        let assigned =
            assign_registers(&target_operations).expect("exact left-shift homes should assign");
        emit_machine_code(&assigned).expect("exact left shift should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact left-shift host selection");
        let assigned = assign_registers(&target_operations).expect("exact left-shift host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact left-shift host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact left-shift host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 1, 5), 32);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 2, 31), 0);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 1, 32), 0);
    }
}

#[test]
fn checked_source_exact_left_shift_uses_known_count_bounds() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-count exact left-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_left_known_count")
        .expect("known-count exact left shift should use the precise value bound");
    let shift_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerShiftLeft { .. }))
        .expect("known-count exact left shift remains explicit terminal work");
    let OperationKind::ExactIntegerShiftLeft { obligation, .. } = shift_operation.kind else {
        unreachable!()
    };
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic =
        encode_module(&lowered.semantic_module).expect("known-count exact left-shift semantics");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("known-count exact left-shift proof");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value)],
        )
        .expect("verified known-count exact left shift should interpret")
    };
    assert_eq!(execute(536_870_911).value(), argument(4_294_967_288));
    assert_eq!(execute(536_870_912).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("known-count exact left shift should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("known-count exact left shift should select");
        let assigned =
            assign_registers(&target_operations).expect("known-count exact left-shift homes");
        emit_machine_code(&assigned).expect("known-count exact left shift should emit");
    }
}

#[test]
fn checked_source_exact_left_shift_uses_bounded_count_maximum() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("bounded-count exact left-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_left_bounded_count")
        .expect("bounded-count exact left shift should use its proved maximum count");
    let shift_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerShiftLeft { .. }))
        .expect("bounded-count exact left shift remains explicit terminal work");
    let OperationKind::ExactIntegerShiftLeft { obligation, .. } = shift_operation.kind else {
        unreachable!()
    };
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic =
        encode_module(&lowered.semantic_module).expect("bounded-count exact left-shift semantics");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("bounded-count exact left-shift proof");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value, count| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(count)],
        )
        .expect("verified bounded-count exact left shift should interpret")
    };
    assert_eq!(execute(536_870_911, 3).value(), argument(4_294_967_288));
    assert_eq!(execute(536_870_911, 2).value(), argument(2_147_483_644));
    assert_eq!(execute(536_870_912, 3).value(), argument(0));
    assert_eq!(execute(1, 4).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("bounded-count exact left shift should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("bounded-count exact left shift should select");
        let assigned =
            assign_registers(&target_operations).expect("bounded-count exact left-shift homes");
        emit_machine_code(&assigned).expect("bounded-count exact left shift should emit");
    }
}

#[test]
fn checked_source_exact_add_uses_known_addend_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-addend exact-add source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_add_known_right")
        .expect("known-addend exact addition should use its path bound");
    let add_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }))
        .expect("proof-gated exact addition remains explicit terminal work");
    let OperationKind::ExactIntegerAdd { obligation, .. } = add_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&add_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-add semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-add proof");
    let module = decode_module(&semantic).expect("decode exact-add semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_add_proof = decode_proof_bundle(&proof).expect("decode exact-add proof");
    missing_add_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(&module, &missing_add_proof, &AdmissionProfile::default()),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(0)],
        )
        .expect("verified exact addition should interpret")
    };
    assert_eq!(execute(4_294_967_290).value(), argument(4_294_967_295));
    assert_eq!(execute(4_294_967_291).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact addition should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerAdd { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact addition should select");
        let assigned = assign_registers(&target_operations).expect("exact-add homes should assign");
        emit_machine_code(&assigned).expect("exact addition should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-add host selection");
        let assigned = assign_registers(&target_operations).expect("exact-add host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-add host emission");
        let object = build_terminal_object_artifact(&machine_code).expect("exact-add host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 100, 0), 105);
        assert_eq!(
            run_host_machine_code_with_two_u64(entry, 4_294_967_291, 0),
            0
        );
    }
}

#[test]
fn checked_source_exact_add_uses_joint_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("joint-bound exact-add source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_add_runtime_bound")
        .expect("joint-bound exact addition should use its path proposition");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let operations = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let subtract_obligation = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("the bound subtraction remains explicit proof-gated work");
    let add_obligation = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("the joint addition remains explicit proof-gated work");

    let semantic = encode_module(&lowered.semantic_module).expect("joint-bound semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("joint-bound proof");
    let module = decode_module(&semantic).expect("decode joint-bound semantics");
    for missing in [subtract_obligation, add_obligation] {
        let mut incomplete = decode_proof_bundle(&proof).expect("decode joint-bound proof");
        incomplete
            .evidence
            .retain(|evidence| evidence.obligation != missing);
        assert!(matches!(
            verify_module(&module, &incomplete, &AdmissionProfile::default()),
            Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == missing
        ));
    }
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified joint-bound exact addition should interpret")
    };
    assert_eq!(execute(20, 22).value(), argument(42));
    assert_eq!(execute(4_294_967_285, 10).value(), argument(4_294_967_295));
    assert_eq!(execute(4_294_967_295, 1).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("joint-bound exact addition should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("joint-bound exact addition should select");
        let assigned = assign_registers(&target_operations)
            .expect("joint-bound exact-add homes should assign");
        emit_machine_code(&assigned).expect("joint-bound exact addition should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("joint-bound exact-add host selection");
        let assigned = assign_registers(&target_operations).expect("joint-bound host homes");
        let machine_code = emit_machine_code(&assigned).expect("joint-bound host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("joint-bound exact-add host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 20, 22), 42);
        assert_eq!(
            run_host_machine_code_with_two_u64(entry, 4_294_967_295, 1),
            0
        );
    }
}

#[test]
fn checked_source_exact_add_uses_signed_nonnegative_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed joint-bound exact-add source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_add_signed_nonnegative_bound")
        .expect("signed joint-bound exact addition should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed joint semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed joint proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified signed joint-bound exact addition should interpret")
    };
    assert_eq!(execute(20, 22).value(), argument(42));
    assert_eq!(execute(2_147_483_637, 10).value(), argument(2_147_483_647));
    assert_eq!(execute(2_147_483_647, 1).value(), argument(0));
    assert_eq!(execute(-5, 3).value(), argument(-2));
    assert_eq!(execute(20, -1).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed joint-bound exact addition should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed joint-bound exact addition should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed joint-bound exact-add homes should assign");
        emit_machine_code(&assigned).expect("signed joint-bound exact addition should emit");
    }
}

#[test]
fn checked_source_exact_add_uses_signed_nonpositive_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed lower joint-bound exact-add source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_add_signed_nonpositive_bound")
        .expect("signed lower joint-bound exact addition should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed lower joint semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed lower joint proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified signed lower joint-bound exact addition should interpret")
    };
    assert_eq!(
        execute(-2_147_483_640, -8).value(),
        argument(i32::MIN as i128)
    );
    assert_eq!(execute(i32::MIN as i128, -1).value(), argument(0));
    assert_eq!(execute(5, -3).value(), argument(2));
    assert_eq!(
        execute(i32::MAX as i128, -1).value(),
        argument(2_147_483_646)
    );
    assert_eq!(execute(20, 1).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed lower joint-bound exact addition should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed lower joint-bound exact addition should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed lower joint-bound exact-add homes should assign");
        emit_machine_code(&assigned).expect("signed lower joint-bound exact addition should emit");
    }
}

#[test]
fn checked_source_exact_subtract_uses_known_subtrahend_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-subtrahend exact-subtract source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_subtract_known_right")
        .expect("known-subtrahend exact subtraction should use its path bound");
    let subtract_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. }))
        .expect("proof-gated exact subtraction remains explicit terminal work");
    let OperationKind::ExactIntegerSubtract { obligation, .. } = subtract_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&subtract_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-subtract semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-subtract proof");
    let module = decode_module(&semantic).expect("decode exact-subtract semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_subtract_proof =
        decode_proof_bundle(&proof).expect("decode exact-subtract proof");
    missing_subtract_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_subtract_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(0)],
        )
        .expect("verified exact subtraction should interpret")
    };
    assert_eq!(execute(5).value(), argument(0));
    assert_eq!(execute(100).value(), argument(95));
    assert_eq!(execute(4).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact subtraction should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerSubtract { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact subtraction should select");
        let assigned =
            assign_registers(&target_operations).expect("exact-subtract homes should assign");
        emit_machine_code(&assigned).expect("exact subtraction should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-subtract host selection");
        let assigned = assign_registers(&target_operations).expect("exact-subtract host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-subtract host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact-subtract host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 100, 0), 95);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 4, 0), 0);
    }
}

#[test]
fn checked_source_exact_subtract_uses_joint_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("joint-bound exact-subtract source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_subtract_joint_bound")
        .expect("joint-bound exact subtraction should use its path proposition");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("joint subtract semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("joint subtract proof");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified joint-bound exact subtraction should interpret")
    };
    assert_eq!(execute(42, 20).value(), argument(22));
    assert_eq!(
        execute(u32::MAX as u128, u32::MAX as u128).value(),
        argument(0)
    );
    assert_eq!(execute(0, 0).value(), argument(0));
    assert_eq!(execute(20, 21).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("joint-bound exact subtraction should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("joint-bound exact subtraction should select");
        let assigned = assign_registers(&target_operations)
            .expect("joint-bound exact-subtract homes should assign");
        emit_machine_code(&assigned).expect("joint-bound exact subtraction should emit");
    }
}

#[test]
fn checked_source_exact_subtract_uses_signed_nonnegative_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed joint-bound exact-subtract source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_subtract_signed_nonnegative_bound")
        .expect("signed joint-bound exact subtraction should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed subtract semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed subtract proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified signed joint-bound exact subtraction should interpret")
    };
    assert_eq!(
        execute(-2_147_483_640, 8).value(),
        argument(i32::MIN as i128)
    );
    assert_eq!(execute(i32::MIN as i128, 1).value(), argument(0));
    assert_eq!(execute(5, 3).value(), argument(2));
    assert_eq!(
        execute(i32::MAX as i128, 0).value(),
        argument(i32::MAX as i128)
    );
    assert_eq!(execute(20, -1).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed joint-bound exact subtraction should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed joint-bound exact subtraction should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed joint-bound exact-subtract homes should assign");
        emit_machine_code(&assigned).expect("signed joint-bound exact subtraction should emit");
    }
}

#[test]
fn checked_source_exact_subtract_uses_signed_nonpositive_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed upper joint-bound exact-subtract source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_subtract_signed_nonpositive_bound")
        .expect("signed upper joint-bound exact subtraction should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed upper semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed upper proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified signed upper joint-bound exact subtraction should interpret")
    };
    assert_eq!(
        execute(2_147_483_640, -7).value(),
        argument(i32::MAX as i128)
    );
    assert_eq!(execute(i32::MAX as i128, -1).value(), argument(0));
    assert_eq!(execute(5, -3).value(), argument(8));
    assert_eq!(
        execute(i32::MIN as i128, 0).value(),
        argument(i32::MIN as i128)
    );
    assert_eq!(execute(20, 1).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed upper joint-bound exact subtraction should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed upper joint-bound exact subtraction should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed upper joint-bound exact-subtract homes should assign");
        emit_machine_code(&assigned)
            .expect("signed upper joint-bound exact subtraction should emit");
    }
}

#[test]
fn checked_source_exact_multiply_uses_known_factor_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-factor exact-multiply source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_multiply_known_right")
        .expect("known-factor exact multiplication should use its path bound");
    let multiply_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerMultiply { .. }))
        .expect("proof-gated exact multiplication remains explicit terminal work");
    let OperationKind::ExactIntegerMultiply { obligation, .. } = multiply_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&multiply_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-multiply semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-multiply proof");
    let module = decode_module(&semantic).expect("decode exact-multiply semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_multiply_proof =
        decode_proof_bundle(&proof).expect("decode exact-multiply proof");
    missing_multiply_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_multiply_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(0)],
        )
        .expect("verified exact multiplication should interpret")
    };
    assert_eq!(execute(858_993_459).value(), argument(4_294_967_295));
    assert_eq!(execute(100).value(), argument(500));
    assert_eq!(execute(858_993_460).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact multiplication should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerMultiply { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact multiplication should select");
        let assigned =
            assign_registers(&target_operations).expect("exact-multiply homes should assign");
        emit_machine_code(&assigned).expect("exact multiplication should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-multiply host selection");
        let assigned = assign_registers(&target_operations).expect("exact-multiply host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-multiply host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact-multiply host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 100, 0, 500));
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            858_993_460,
            0,
            0
        ));
    }
}

#[test]
fn checked_source_exact_multiply_uses_joint_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("joint-bound exact-multiply source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_multiply_joint_bound")
        .expect("joint-bound exact multiplication should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("joint multiply semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("joint multiply proof");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified joint-bound exact multiplication should interpret")
    };
    assert_eq!(execute(21, 2).value(), argument(42));
    assert_eq!(
        execute(u32::MAX as u128, 1).value(),
        argument(u32::MAX as u128)
    );
    assert_eq!(execute(65_535, 65_537).value(), argument(u32::MAX as u128));
    assert_eq!(execute(u32::MAX as u128, 2).value(), argument(0));
    assert_eq!(execute(20, 0).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("joint-bound exact multiplication should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("joint-bound exact multiplication should select");
        let assigned = assign_registers(&target_operations)
            .expect("joint-bound exact-multiply homes should assign");
        emit_machine_code(&assigned).expect("joint-bound exact multiplication should emit");
    }
}

#[test]
fn checked_source_exact_multiply_uses_signed_positive_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed joint-bound exact-multiply source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_multiply_signed_positive_bound")
        .expect("signed joint-bound exact multiplication should use all path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed multiply semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed multiply proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified signed joint-bound exact multiplication should interpret")
    };
    assert_eq!(execute(21, 2).value(), argument(42));
    assert_eq!(
        execute(-1_073_741_824, 2).value(),
        argument(i32::MIN as i128)
    );
    assert_eq!(execute(715_827_882, 3).value(), argument(2_147_483_646));
    assert_eq!(execute(-1_073_741_825, 2).value(), argument(0));
    assert_eq!(execute(1_073_741_824, 2).value(), argument(0));
    assert_eq!(execute(20, 0).value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed joint-bound exact multiplication should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed joint-bound exact multiplication should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed joint-bound exact-multiply homes should assign");
        emit_machine_code(&assigned).expect("signed joint-bound exact multiplication should emit");
    }
}

#[test]
fn checked_source_exact_divide_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor exact-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_divide_known_right")
        .expect("known nonzero exact division should lower");
    let divide_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerDivide { .. }))
        .expect("proof-gated exact division remains explicit terminal work");
    let OperationKind::ExactIntegerDivide { obligation, .. } = divide_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&divide_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-divide proof");
    let module = decode_module(&semantic).expect("decode exact-divide semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_divide_proof = decode_proof_bundle(&proof).expect("decode exact-divide proof");
    missing_divide_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_divide_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(500), argument(0)],
    )
    .expect("verified exact division should interpret");
    assert_eq!(execution.value(), argument(100));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact division should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerDivide { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact division should select");
        let assigned =
            assign_registers(&target_operations).expect("exact-divide homes should assign");
        emit_machine_code(&assigned).expect("exact division should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-divide host selection");
        let assigned = assign_registers(&target_operations).expect("exact-divide host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-divide host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 500, 0, 100));
    }
}

#[test]
fn checked_source_signed_exact_divide_truncates_toward_zero() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed exact-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_signed_divide_known_right")
        .expect("known signed exact division should lower");
    let semantic = encode_module(&lowered.semantic_module).expect("signed exact-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed exact-divide proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(-101), argument(0)],
    )
    .expect("verified signed exact division should interpret");
    assert_eq!(execution.value(), argument(-50));

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed exact division should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed exact-divide host selection");
        let assigned = assign_registers(&target_operations).expect("signed exact-divide homes");
        let machine_code = emit_machine_code(&assigned).expect("signed exact-divide host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("signed exact-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            (-101_i64) as u64,
            0,
            (-50_i64) as u64,
        ));
    }
}

#[test]
fn checked_source_exact_remainder_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor exact-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_remainder_known_right")
        .expect("known nonzero exact remainder should lower");
    let remainder_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerRemainder { .. }))
        .expect("proof-gated exact remainder remains explicit terminal work");
    let OperationKind::ExactIntegerRemainder { obligation, .. } = remainder_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&remainder_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-remainder semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-remainder proof");
    let module = decode_module(&semantic).expect("decode exact-remainder semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_remainder_proof =
        decode_proof_bundle(&proof).expect("decode exact-remainder proof");
    missing_remainder_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_remainder_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(503), argument(0)],
    )
    .expect("verified exact remainder should interpret");
    assert_eq!(execution.value(), argument(3));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact remainder should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerRemainder { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact remainder should select");
        let assigned =
            assign_registers(&target_operations).expect("exact-remainder homes should assign");
        emit_machine_code(&assigned).expect("exact remainder should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-remainder host selection");
        let assigned = assign_registers(&target_operations).expect("exact-remainder host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-remainder host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 503, 0, 3));
    }
}

#[test]
fn checked_source_signed_exact_remainder_is_truncating() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed exact-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_signed_remainder_known_right")
        .expect("known signed exact remainder should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed exact-remainder semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed exact-remainder proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(-101), argument(0)],
    )
    .expect("verified signed exact remainder should interpret");
    assert_eq!(execution.value(), argument(-1));

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed exact remainder should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed exact-remainder host selection");
        let assigned = assign_registers(&target_operations).expect("signed exact-remainder homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed exact-remainder host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed exact-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            (-101_i64) as u64,
            0,
            (-1_i64) as u64,
        ));
    }
}

#[test]
fn checked_source_wrapping_divide_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor wrapping-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_wrapping_divide_known_right")
        .expect("known nonzero wrapping division should lower");
    let divide_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::WrappingIntegerDivide { .. }))
        .expect("proof-gated wrapping division remains explicit terminal work");
    let OperationKind::WrappingIntegerDivide { obligation, .. } = divide_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&divide_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("wrapping-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("wrapping-divide proof");
    let module = decode_module(&semantic).expect("decode wrapping-divide semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_divide_proof =
        decode_proof_bundle(&proof).expect("decode wrapping-divide proof");
    missing_divide_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_divide_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(505), argument(0)],
    )
    .expect("verified wrapping division should interpret");
    assert_eq!(execution.value(), argument(101));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("wrapping division should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerDivide { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("wrapping division should select");
        let assigned =
            assign_registers(&target_operations).expect("wrapping-divide homes should assign");
        emit_machine_code(&assigned).expect("wrapping division should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("wrapping-divide host selection");
        let assigned = assign_registers(&target_operations).expect("wrapping-divide host homes");
        let machine_code = emit_machine_code(&assigned).expect("wrapping-divide host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("wrapping-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 505, 0, 101));
    }
}

#[test]
fn checked_source_signed_wrapping_divide_wraps_minimum_by_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed wrapping-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_signed_wrapping_divide_min")
        .expect("known signed wrapping division should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed wrapping-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed wrapping-divide proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(i64::MIN as i128), argument(0)],
    )
    .expect("verified signed wrapping division should interpret");
    assert_eq!(execution.value(), argument(i64::MIN as i128));

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed wrapping division should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed wrapping-divide host selection");
        let assigned = assign_registers(&target_operations).expect("signed wrapping-divide homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed wrapping-divide host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed wrapping-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            i64::MIN as u64,
            0,
            i64::MIN as u64,
        ));
    }
}

#[test]
fn checked_source_wrapping_remainder_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor wrapping-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_wrapping_remainder_known_right")
        .expect("known nonzero wrapping remainder should lower");
    let remainder_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::WrappingIntegerRemainder { .. }
            )
        })
        .expect("proof-gated wrapping remainder remains explicit terminal work");
    let OperationKind::WrappingIntegerRemainder { obligation, .. } = remainder_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&remainder_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("wrapping-remainder semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("wrapping-remainder proof");
    let module = decode_module(&semantic).expect("decode wrapping-remainder semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_remainder_proof =
        decode_proof_bundle(&proof).expect("decode wrapping-remainder proof");
    missing_remainder_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_remainder_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(503), argument(0)],
    )
    .expect("verified wrapping remainder should interpret");
    assert_eq!(execution.value(), argument(3));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("wrapping remainder should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerRemainder { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("wrapping remainder should select");
        let assigned =
            assign_registers(&target_operations).expect("wrapping-remainder homes should assign");
        emit_machine_code(&assigned).expect("wrapping remainder should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("wrapping-remainder host selection");
        let assigned = assign_registers(&target_operations).expect("wrapping-remainder host homes");
        let machine_code = emit_machine_code(&assigned).expect("wrapping-remainder host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("wrapping-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 503, 0, 3));
    }
}

#[test]
fn checked_source_signed_wrapping_remainder_returns_zero_for_minimum_by_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed wrapping-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_signed_wrapping_remainder_min")
        .expect("known signed wrapping remainder should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed wrapping-remainder semantics");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("signed wrapping-remainder proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(i64::MIN as i128), argument(0)],
    )
    .expect("verified signed wrapping remainder should interpret");
    assert_eq!(execution.value(), argument(0));

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed wrapping remainder should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed wrapping-remainder host selection");
        let assigned =
            assign_registers(&target_operations).expect("signed wrapping-remainder homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed wrapping-remainder host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed wrapping-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            i64::MIN as u64,
            0,
            0,
        ));
    }
}

#[test]
fn checked_source_saturating_divide_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor saturating-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_saturating_divide_known_right")
        .expect("known nonzero saturating division should lower");
    let divide_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::SaturatingIntegerDivide { .. }
            )
        })
        .expect("proof-gated saturating division remains explicit terminal work");
    let OperationKind::SaturatingIntegerDivide { obligation, .. } = divide_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&divide_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("saturating-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("saturating-divide proof");
    let module = decode_module(&semantic).expect("decode saturating-divide semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_divide_proof =
        decode_proof_bundle(&proof).expect("decode saturating-divide proof");
    missing_divide_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_divide_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(505), argument(0)],
    )
    .expect("verified saturating division should interpret");
    assert_eq!(execution.value(), argument(101));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("saturating division should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::SaturatingIntegerDivide { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("saturating division should select");
        let assigned =
            assign_registers(&target_operations).expect("saturating-divide homes should assign");
        emit_machine_code(&assigned).expect("saturating division should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("saturating-divide host selection");
        let assigned = assign_registers(&target_operations).expect("saturating-divide host homes");
        let machine_code = emit_machine_code(&assigned).expect("saturating-divide host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("saturating-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 505, 0, 101));
    }
}

#[test]
fn checked_source_signed_saturating_divide_clamps_minimum_by_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed saturating-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_signed_saturating_divide_min")
        .expect("known signed saturating division should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed saturating-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed saturating-divide proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(i64::MIN as i128), argument(0)],
    )
    .expect("verified signed saturating division should interpret");
    assert_eq!(execution.value(), argument(i64::MAX as i128));

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed saturating division should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed saturating-divide host selection");
        let assigned =
            assign_registers(&target_operations).expect("signed saturating-divide homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed saturating-divide host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed saturating-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            i64::MIN as u64,
            0,
            i64::MAX as u64,
        ));
    }
}

#[test]
fn checked_source_saturating_remainder_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor saturating-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_saturating_remainder_known_right")
        .expect("known nonzero saturating remainder should lower");
    let remainder_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::SaturatingIntegerRemainder { .. }
            )
        })
        .expect("proof-gated saturating remainder remains explicit terminal work");
    let OperationKind::SaturatingIntegerRemainder { obligation, .. } = remainder_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&remainder_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("saturating-remainder semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("saturating-remainder proof");
    let module = decode_module(&semantic).expect("decode saturating-remainder semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_remainder_proof =
        decode_proof_bundle(&proof).expect("decode saturating-remainder proof");
    missing_remainder_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_remainder_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(507), argument(0)],
    )
    .expect("verified saturating remainder should interpret");
    assert_eq!(execution.value(), argument(2));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("saturating remainder should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::SaturatingIntegerRemainder { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("saturating remainder should select");
        let assigned =
            assign_registers(&target_operations).expect("saturating-remainder homes should assign");
        emit_machine_code(&assigned).expect("saturating remainder should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("saturating-remainder host selection");
        let assigned =
            assign_registers(&target_operations).expect("saturating-remainder host homes");
        let machine_code =
            emit_machine_code(&assigned).expect("saturating-remainder host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("saturating-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 507, 0, 2));
    }
}

#[test]
fn checked_source_signed_saturating_remainder_returns_zero_for_minimum_by_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed saturating-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_signed_saturating_remainder_min")
        .expect("known signed saturating remainder should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed saturating-remainder semantics");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("signed saturating-remainder proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(i64::MIN as i128), argument(0)],
    )
    .expect("verified signed saturating remainder should interpret");
    assert_eq!(execution.value(), argument(0));

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed saturating remainder should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed saturating-remainder host selection");
        let assigned =
            assign_registers(&target_operations).expect("signed saturating-remainder homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed saturating-remainder host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed saturating-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            i64::MIN as u64,
            0,
            0,
        ));
    }
}

#[test]
fn checked_source_guarded_runtime_divisors_cross_every_fixed_integer_policy() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("guarded runtime-divisor source canaries should compile");
    let cases = [
        ("terminal_exact_divide_guarded_right", 4_u128),
        ("terminal_exact_remainder_guarded_right", 3_u128),
        ("terminal_wrapping_divide_guarded_right", 4_u128),
        ("terminal_wrapping_remainder_guarded_right", 3_u128),
        ("terminal_saturating_divide_guarded_right", 4_u128),
        ("terminal_saturating_remainder_guarded_right", 3_u128),
    ];
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };

    for (machine, expected) in cases {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        let obligation = lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ExactIntegerDivide { obligation, .. }
                | OperationKind::ExactIntegerRemainder { obligation, .. }
                | OperationKind::WrappingIntegerDivide { obligation, .. }
                | OperationKind::WrappingIntegerRemainder { obligation, .. }
                | OperationKind::SaturatingIntegerDivide { obligation, .. }
                | OperationKind::SaturatingIntegerRemainder { obligation, .. } => Some(obligation),
                _ => None,
            })
            .expect("guarded operation owns a divisor obligation");
        assert!(
            lowered
                .proof_bundle
                .evidence
                .iter()
                .any(|evidence| evidence.obligation == obligation)
        );

        let semantic = encode_module(&lowered.semantic_module).expect("guarded semantics");
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("guarded proof");
        verify_module(
            &decode_module(&semantic).expect("decode guarded semantics"),
            &decode_proof_bundle(&proof).expect("decode guarded proof"),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} artifact should verify: {error:?}"));
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(23), argument(5)],
        )
        .unwrap_or_else(|error| panic!("{machine} should interpret: {error:?}"));
        assert_eq!(execution.value(), argument(expected));
        let zero_path = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(23), argument(0)],
        )
        .unwrap_or_else(|error| panic!("{machine} zero path should bypass arithmetic: {error:?}"));
        assert_eq!(zero_path.value(), argument(0));

        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .unwrap_or_else(|error| panic!("{machine} should cross Omega: {error:?}"));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} should assign: {error:?}"));
            emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        }
    }
}

#[test]
fn checked_source_guarded_negative_runtime_divisor_excludes_zero_and_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("guarded negative runtime-divisor source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_divide_guarded_negative_right")
        .expect("divisor <= -2 should lower exact signed division");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("negative-divisor semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("negative-divisor proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(23), argument(-5)],
    )
    .expect("negative guarded divisor should interpret");
    assert_eq!(execution.value(), argument(-4));
    let bypassed = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(23), argument(-1)],
    )
    .expect("negative one should take the bypass arm");
    assert_eq!(bypassed.value(), argument(0));

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("negative-divisor artifact should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("negative-divisor control should select");
        let assigned =
            assign_registers(&target_operations).expect("negative-divisor control should assign");
        emit_machine_code(&assigned).expect("negative-divisor control should emit");
    }
}

#[test]
fn checked_source_negative_one_range_uses_policy_appropriate_dividend_evidence() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("negative-one-range source canaries should compile");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    for (machine, value, expected) in [
        (
            "terminal_exact_divide_guarded_negative_one_range",
            23_i128,
            -23_i128,
        ),
        (
            "terminal_wrapping_divide_guarded_negative_one_range",
            i32::MIN as i128,
            i32::MIN as i128,
        ),
    ] {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        let semantic = encode_module(&lowered.semantic_module).expect("range semantics");
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("range proof");
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(-1)],
        )
        .unwrap_or_else(|error| panic!("{machine} should interpret: {error:?}"));
        assert_eq!(execution.value(), argument(expected));
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .unwrap_or_else(|error| panic!("{machine} should cross Omega: {error:?}"));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} should assign: {error:?}"));
            emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        }
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
    assert_eq!(semantic_module.vocabulary_marker, VocabularyMarker::CURRENT);
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
fn checked_source_integer_graph_computes_boolean_jump_bindings() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed Boolean integer-graph source canary should compile");
    let lowered = lower_machine(&checked, "terminal_integer_computed_boolean_binding")
        .expect("integer graphs should lower non-short-circuit Boolean bindings");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 3);
    assert!(matches!(
        &machine.blocks[0].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::BooleanEqual { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::BooleanNot { .. },
                ..
            },
        ]
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed Boolean integer graph should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed Boolean integer-graph proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("computed Boolean integer graph should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("computed Boolean integer-graph proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed Boolean integer graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed Boolean integer graph should have exact fuel")
            .ceiling_units(),
        5
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected) in [
        (false, false, 20_u128),
        (false, true, 10),
        (true, false, 10),
        (true, true, 20),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(10),
                integer(20),
            ],
        )
        .expect("computed Boolean integer graph should interpret");
        assert_eq!(measured.value(), integer(expected));
        assert_eq!(measured.usage().total_units(), 5);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("computed Boolean integer graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed Boolean integer graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerExpressionConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("computed Boolean integer-graph homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("computed Boolean integer-graph machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_integer_graph_stages_short_circuit_boolean_jump_bindings() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit Boolean integer-graph source canary should compile");
    let lowered = lower_machine(&checked, "terminal_integer_short_circuit_boolean_binding")
        .expect("integer graphs should stage short-circuit Boolean bindings");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 11);
    assert_eq!(
        machine
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count(),
        3
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("short-circuit Boolean integer graph should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("short-circuit Boolean integer-graph proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("short-circuit Boolean integer graph should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("short-circuit Boolean integer-graph proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short-circuit Boolean integer graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("short-circuit Boolean integer graph should have exact fuel")
            .ceiling_units(),
        10
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected, units) in [
        (false, false, 20_u128, 9_u64),
        (false, true, 20, 9),
        (true, false, 20, 10),
        (true, true, 10, 10),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(10),
                integer(20),
            ],
        )
        .expect("short-circuit Boolean integer graph should interpret");
        assert_eq!(measured.value(), integer(expected));
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("short-circuit Boolean integer graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("short-circuit Boolean integer graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("short-circuit Boolean integer-graph homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("short-circuit Boolean integer-graph machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_integer_graph_localizes_short_circuit_boolean_edge_bindings() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("selected short-circuit Boolean source canary should compile");
    let lowered = lower_machine(
        &checked,
        "terminal_integer_conditional_short_circuit_boolean_binding",
    )
    .expect("integer graphs should localize short-circuit Boolean edge bindings");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 12);
    assert_eq!(
        machine
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count(),
        4
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("selected short-circuit Boolean graph should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("selected short-circuit Boolean graph proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("selected short-circuit Boolean graph should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("selected short-circuit Boolean graph proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("selected short-circuit Boolean graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("selected short-circuit Boolean graph should have exact fuel")
            .ceiling_units(),
        10
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (select, first, second, expected, units) in [
        (false, true, true, 20_u128, 3_u64),
        (true, false, true, 20, 9),
        (true, true, false, 20, 10),
        (true, true, true, 10, 10),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(select),
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(10),
                integer(20),
            ],
        )
        .expect("selected short-circuit Boolean graph should interpret");
        assert_eq!(measured.value(), integer(expected));
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("selected short-circuit Boolean graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("selected short-circuit Boolean graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("selected short-circuit Boolean graph homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("selected short-circuit Boolean graph machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_unconditional_mixed_scalar_graph_uses_general_lowering() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("unconditional mixed-scalar source canary should compile");
    let lowered = lower_machine(&checked, "terminal_unconditional_mixed_scalar_graph")
        .expect("unconditional mixed-scalar graphs should use general lowering");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 10);
    assert_eq!(
        machine
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count(),
        2
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("unconditional mixed-scalar graph should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("unconditional mixed-scalar graph proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("unconditional mixed-scalar graph should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("unconditional mixed-scalar graph proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("unconditional mixed-scalar graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("unconditional mixed-scalar graph should have exact fuel")
            .ceiling_units(),
        11
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, units) in [
        (false, false, 10_u64),
        (false, true, 10),
        (true, false, 11),
        (true, true, 11),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(30),
            ],
        )
        .expect("unconditional mixed-scalar graph should interpret");
        assert_eq!(measured.value(), integer(31));
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("unconditional mixed-scalar graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("unconditional mixed-scalar graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("unconditional mixed-scalar graph homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("unconditional mixed-scalar graph machine code should emit");
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
    assert_eq!(machine.blocks.len(), 5);
    assert!(matches!(
        machine.blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    assert!(matches!(
        &machine.blocks[3].operations[..],
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
        &machine.blocks[4].operations[..],
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
        6
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected, units) in [
        (false, true, 14_u128, 5),
        (true, false, 14, 6),
        (true, true, 8, 6),
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

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 13);
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
        12
    );

    for (arguments, expected, units) in [
        ([true, true, true, true, false, false], true, 6),
        ([true, false, true, true, false, false], true, 8),
        ([false, false, true, true, false, true], true, 7),
        ([true, true, false, true, false, false], true, 10),
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

#[test]
fn checked_source_short_circuit_tuple_binding_is_staged_left_to_right() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit tuple-binding source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain_short_circuit_tuple")
        .expect("short-circuit tuple bindings should lower in ordered stages");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 14);
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("short-circuit tuple binding should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("short-circuit tuple-binding proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("short-circuit tuple binding should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("short-circuit tuple-binding proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short-circuit tuple binding should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("short-circuit tuple binding should have exact fuel")
            .ceiling_units(),
        13
    );

    for (arguments, expected, units) in [
        ([true, false, false, true], false, 11),
        ([false, true, false, false], false, 12),
        ([true, false, true, true], true, 12),
        ([false, false, true, false], true, 13),
    ] {
        let measured =
            interpret_terminal_measured(&verified, &arguments.map(TerminalScalarValue::Boolean))
                .expect("short-circuit tuple binding should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("short-circuit tuple binding should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("short-circuit tuple binding should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("short-circuit tuple-binding homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("short-circuit tuple-binding machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_boolean_conditional_edges_compute_only_on_the_selected_arm() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed Boolean conditional-edge source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_computed_conditional_edges")
        .expect("computed Boolean conditional edges should lower into selected-arm blocks");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 17);
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed Boolean conditional edges should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed Boolean conditional-edge proof should encode canonically");
    let semantic_module = decode_module(&semantic_bytes)
        .expect("computed Boolean conditional-edge module should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("computed Boolean conditional-edge proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed Boolean conditional edges should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed Boolean conditional edges should have exact fuel")
            .ceiling_units(),
        14
    );

    for (arguments, expected, units) in [
        ([false, true, true, true, false], false, 6),
        ([true, false, true, true, true], false, 7),
        ([true, true, false, true, false], false, 12),
        ([true, true, true, true, false], true, 13),
    ] {
        let measured =
            interpret_terminal_measured(&verified, &arguments.map(TerminalScalarValue::Boolean))
                .expect("computed Boolean conditional edges should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("computed Boolean conditional edges should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed Boolean conditional edges should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("computed Boolean conditional-edge homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("computed Boolean conditional-edge machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_mixed_scalar_boolean_graph_uses_the_typed_dag() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("mixed-scalar Boolean source canary should compile");
    let lowered = lower_machine(&checked, "terminal_mixed_scalar_boolean_graph")
        .expect("mixed-scalar Boolean graph should lower through terminal Psi");
    drop(checked);

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("mixed-scalar Boolean graph should encode");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("mixed-scalar Boolean proof should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("mixed-scalar Boolean graph should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("mixed-scalar Boolean proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed-scalar Boolean graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("mixed-scalar Boolean graph should have fixed fuel")
            .ceiling_units(),
        9
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (choose_less, left, right, expected) in [
        (true, 1, 2, true),
        (true, 5, 2, false),
        (false, 3, 2, true),
        (false, 1, 2, false),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(choose_less),
                integer(left),
                integer(right),
            ],
        )
        .expect("mixed-scalar Boolean graph should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), 9);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("mixed-scalar Boolean graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("mixed-scalar Boolean graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned =
            assign_registers(&target_operations).expect("mixed-scalar Boolean homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("mixed-scalar Boolean graph should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_mixed_scalar_boolean_short_circuit_preserves_selected_fuel() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("mixed-scalar Boolean short-circuit canary should compile");
    let lowered = lower_machine(&checked, "terminal_mixed_scalar_boolean_short_circuit")
        .expect("mixed-scalar Boolean short-circuit graph should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed-scalar Boolean short-circuit graph should verify");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("mixed-scalar Boolean short-circuit graph should have fixed fuel")
            .ceiling_units(),
        15
    );
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, value, limit, expected, expected_units) in [
        (false, true, 1, 4, false, 12),
        (true, false, 1, 4, false, 13),
        (true, true, 1, 4, true, 15),
        (true, true, 4, 4, false, 15),
    ] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(value),
                integer(limit),
            ],
        )
        .expect("mixed-scalar Boolean short-circuit graph should interpret");
        assert_eq!(measured.value(), TerminalScalarValue::Boolean(expected));
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("mixed-scalar Boolean short-circuit graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("mixed-scalar Boolean short-circuit graph should select natively");
        let assigned = assign_registers(&target_operations)
            .expect("mixed-scalar Boolean short-circuit homes should assign");
        assert!(
            !emit_machine_code(&assigned)
                .expect("mixed-scalar Boolean short-circuit graph should emit")
                .functions[0]
                .bytes
                .is_empty()
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
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
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
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
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
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
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
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
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
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
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
fn checked_source_runtime_integer_bitwise_not_crosses_canonical_artifacts_and_native_targets() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime integer-bitwise-not source canary should compile");
    let lowered = lower_machine(&checked, "terminal_unsigned_bitwise_not_runtime")
        .expect("integer bitwise-not should lower");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    assert!(matches!(
        lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
        OperationKind::IntegerBitwiseNot { .. }
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("not module encodes");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("not proof encodes");
    drop(checked);
    drop(lowered);

    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let input = |value| TerminalScalarValue::Integer {
        scalar_type,
        value: IntegerValue::Unsigned(value),
    };
    let expected = input(!0x0f0f_u64 as u128 & u64::MAX as u128);
    let measured = interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[input(0x0f0f), input(0)],
    )
    .expect("canonical bitwise-not artifact should interpret");
    assert_eq!(measured.value(), expected);
    assert_eq!(measured.usage().total_units(), 2);

    let abstract_operations =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("canonical bitwise-not artifact should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("bitwise-not should select for both native architectures");
        assert!(matches!(
            &target_operations.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerExpression {
                expression: TerminalTargetIntegerExpression::BitwiseNot { .. },
                ..
            }
        ));
        let assigned = assign_registers(&target_operations).expect("bitwise-not homes assign");
        emit_machine_code(&assigned).expect("bitwise-not emits native code");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("host bitwise-not selection");
        let assigned = assign_registers(&target_operations).expect("host bitwise-not homes assign");
        let machine_code = emit_machine_code(&assigned).expect("host bitwise-not emission");
        let object = build_terminal_object_artifact(&machine_code).expect("bitwise-not object");
        assert_eq!(
            run_host_machine_code_with_two_u64(object.entry_function().bytes(&object), 0x0f0f, 0,),
            0xf0,
        );
    }
}

#[test]
fn checked_source_same_carrier_policy_casts_retag_without_terminal_work() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("explicit arithmetic-policy cast source canary should compile");
    let wrapping = lower_machine(&checked, "terminal_explicit_wrapping_cast_add_runtime")
        .expect("same-carrier wrapping casts should select terminal wrapping addition");
    let erasure = lower_machine(&checked, "terminal_explicit_policy_erasure_runtime")
        .expect("same-carrier policy erasure should lower as an identity");
    assert!(matches!(
        &wrapping.semantic_module.machines[0].blocks[0].operations[..],
        [psi_terminal::Operation {
            kind: OperationKind::WrappingIntegerAdd { .. },
            ..
        }]
    ));
    assert!(
        erasure.semantic_module.machines[0].blocks[0]
            .operations
            .is_empty(),
        "a same-carrier policy erasure must not invent executable terminal work"
    );
    let wrapping_semantic =
        encode_module(&wrapping.semantic_module).expect("wrapping-cast module encodes");
    let wrapping_proof =
        encode_proof_bundle(&wrapping.proof_bundle).expect("wrapping-cast proof encodes");
    let erasure_semantic =
        encode_module(&erasure.semantic_module).expect("policy-erasure module encodes");
    let erasure_proof =
        encode_proof_bundle(&erasure.proof_bundle).expect("policy-erasure proof encodes");
    drop(checked);
    drop(wrapping);
    drop(erasure);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u8_value = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    let measured = interpret_terminal_artifact_measured(
        &wrapping_semantic,
        &wrapping_proof,
        &AdmissionProfile::default(),
        &[u8_value(250), u8_value(10)],
    )
    .expect("canonical wrapping-cast artifact should interpret");
    assert_eq!(measured.value(), u8_value(4));
    assert_eq!(measured.usage().total_units(), 2);

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_value = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(value),
    };
    let measured = interpret_terminal_artifact_measured(
        &erasure_semantic,
        &erasure_proof,
        &AdmissionProfile::default(),
        &[u64_value(73), u64_value(0)],
    )
    .expect("canonical policy-erasure artifact should interpret");
    assert_eq!(measured.value(), u64_value(73));
    assert_eq!(measured.usage().total_units(), 1);

    let wrapping_abstract = lower_artifact_sections(
        &wrapping_semantic,
        &wrapping_proof,
        &AdmissionProfile::default(),
    )
    .expect("wrapping-cast artifact should cross the Omega boundary");
    let erasure_abstract = lower_artifact_sections(
        &erasure_semantic,
        &erasure_proof,
        &AdmissionProfile::default(),
    )
    .expect("policy-erasure artifact should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let wrapping_target = lower_to_target_operations(&wrapping_abstract, target)
            .expect("wrapping-cast expression should select on both native targets");
        assert!(matches!(
            &wrapping_target.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerExpression {
                expression: TerminalTargetIntegerExpression::WrappingAdd { .. },
                ..
            }
        ));
        let wrapping_assigned =
            assign_registers(&wrapping_target).expect("wrapping-cast homes should assign");
        emit_machine_code(&wrapping_assigned).expect("wrapping-cast expression should emit");

        let erasure_target = lower_to_target_operations(&erasure_abstract, target)
            .expect("policy erasure should select on both native targets");
        assert!(matches!(
            erasure_target.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerParameter { .. }
        ));
        let erasure_assigned =
            assign_registers(&erasure_target).expect("policy-erasure homes should assign");
        emit_machine_code(&erasure_assigned).expect("policy erasure should emit");
    }

    #[cfg(unix)]
    {
        let wrapping_target = lower_to_target_operations(&wrapping_abstract, NativeTarget::host())
            .expect("host wrapping-cast selection");
        let wrapping_assigned =
            assign_registers(&wrapping_target).expect("host wrapping-cast homes should assign");
        let wrapping_code =
            emit_machine_code(&wrapping_assigned).expect("host wrapping-cast emission");
        let wrapping_object =
            build_terminal_object_artifact(&wrapping_code).expect("wrapping-cast object");
        assert_eq!(
            run_host_machine_code_with_two_u64(
                wrapping_object.entry_function().bytes(&wrapping_object),
                250,
                10,
            ),
            4,
        );

        let erasure_target = lower_to_target_operations(&erasure_abstract, NativeTarget::host())
            .expect("host policy-erasure selection");
        let erasure_assigned =
            assign_registers(&erasure_target).expect("host policy-erasure homes should assign");
        let erasure_code =
            emit_machine_code(&erasure_assigned).expect("host policy-erasure emission");
        let erasure_object =
            build_terminal_object_artifact(&erasure_code).expect("policy-erasure object");
        assert_eq!(
            run_host_machine_code_with_two_u64(
                erasure_object.entry_function().bytes(&erasure_object),
                73,
                0,
            ),
            73,
        );
    }
}

#[test]
fn checked_source_total_integer_widening_crosses_canonical_artifacts_and_native_targets() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("total integer-widening source canaries should compile");
    let cases = [
        (
            "terminal_unsigned_widen_runtime",
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            IntegerValue::Unsigned(250),
            IntegerValue::Unsigned(250),
            2_u64,
            false,
        ),
        (
            "terminal_signed_widen_runtime",
            IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
            IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
            IntegerValue::Signed(-128),
            IntegerValue::Signed(-128),
            2,
            false,
        ),
        (
            "terminal_unsigned_to_signed_widen_runtime",
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerType::new(IntegerSign::Signed, 16).expect("i16"),
            IntegerValue::Unsigned(255),
            IntegerValue::Signed(255),
            2,
            false,
        ),
        (
            "terminal_unsigned_widen_then_wrapping_add",
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            IntegerValue::Unsigned(250),
            IntegerValue::Unsigned(251),
            4,
            true,
        ),
    ];

    for (machine, source_type, target_type, input, expected, fuel, nested_add) in cases {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        assert!(
            lowered.semantic_module.machines[0]
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(operation.kind, OperationKind::IntegerWiden { .. })),
            "{machine} must retain widening as terminal work"
        );
        let semantic = encode_module(&lowered.semantic_module)
            .unwrap_or_else(|error| panic!("{machine} semantic module should encode: {error:?}"));
        let proof = encode_proof_bundle(&lowered.proof_bundle)
            .unwrap_or_else(|error| panic!("{machine} proof should encode: {error:?}"));
        drop(lowered);

        let argument = |value| TerminalScalarValue::Integer {
            scalar_type: source_type,
            value,
        };
        let zero = match source_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        };
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(input), argument(zero)],
        )
        .unwrap_or_else(|error| panic!("{machine} artifact should interpret: {error:?}"));
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: target_type,
                value: expected,
            },
            "{machine} result"
        );
        assert_eq!(measured.usage().total_units(), fuel, "{machine} fuel");

        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .unwrap_or_else(|error| panic!("{machine} should cross Omega: {error:?}"));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
            let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } =
                &target_operations.functions[0].operation
            else {
                panic!("{machine} should return an integer expression");
            };
            if nested_add {
                let TerminalTargetIntegerExpression::WrappingAdd { left, .. } = expression else {
                    panic!("{machine} should retain its wrapping add");
                };
                assert!(matches!(
                    left.as_ref(),
                    TerminalTargetIntegerExpression::IntegerWiden { .. }
                ));
            } else {
                assert!(matches!(
                    expression,
                    TerminalTargetIntegerExpression::IntegerWiden { .. }
                ));
            }
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} homes should assign: {error:?}"));
            emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        }

        #[cfg(unix)]
        {
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .unwrap_or_else(|error| panic!("{machine} host selection: {error:?}"));
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} host homes: {error:?}"));
            let machine_code = emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} host emission: {error:?}"));
            let object = build_terminal_object_artifact(&machine_code)
                .unwrap_or_else(|error| panic!("{machine} host object: {error:?}"));
            let bits = |value: IntegerValue| match value {
                IntegerValue::Unsigned(value) => value as u64,
                IntegerValue::Signed(value) => value as i64 as u64,
            };
            assert!(
                host_machine_code_with_two_u64_matches(
                    object.entry_function().bytes(&object),
                    bits(input),
                    0,
                    bits(expected),
                ),
                "{machine} native result"
            );
        }
    }
}

#[test]
fn checked_source_address_identity_survives_artifacts_and_native_realization() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("address identity source canary should compile");
    let lowered = lower_machine(&checked, "terminal_address_reflexive")
        .expect("address identity should lower to terminal Psi");
    let address = IntegerType::address(64).expect("addr");
    let address_scalar = ScalarType::Integer(address);
    let u64_scalar = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).expect("ordinary u64 carrier"),
    );

    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    assert_eq!(
        lowered.semantic_module.machines[0].parameters[0].scalar_type,
        address_scalar
    );
    assert_eq!(
        lowered.semantic_module.machines[0].result.scalar_type,
        ScalarType::Boolean
    );
    assert_ne!(address_scalar, u64_scalar);

    let semantic = encode_module(&lowered.semantic_module).expect("address semantic bytes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("address proof bytes");
    drop(lowered);
    let decoded = decode_module(&semantic).expect("decode address semantic bytes");
    assert!(matches!(
        decoded.machines[0].parameters[0].scalar_type,
        ScalarType::Integer(integer_type) if integer_type.is_address()
    ));

    let input = IntegerValue::Unsigned(0xfedc_ba98_7654_3210);
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Integer {
            scalar_type: address,
            value: input,
        }],
    )
    .expect("decoded address artifact should interpret");
    assert_eq!(measured.value(), TerminalScalarValue::Boolean(true));
    assert_eq!(measured.usage().total_units(), 2);

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("address artifact should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("address identity should select");
        let TerminalTargetOperation::ReturnBooleanExpression { expression, .. } =
            &target_operations.functions[0].operation
        else {
            panic!("address comparison should return a Boolean expression");
        };
        let TerminalTargetBooleanExpression::IntegerEqual { scalar_type, .. } = expression else {
            panic!("address comparison should retain integer equality");
        };
        assert!(scalar_type.is_address());
        let assigned = assign_registers(&target_operations).expect("address homes should assign");
        emit_machine_code(&assigned).expect("address identity should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("address host selection");
        let assigned = assign_registers(&target_operations).expect("address host homes");
        let machine_code = emit_machine_code(&assigned).expect("address host emission");
        let object = build_terminal_object_artifact(&machine_code).expect("address host object");
        assert!(host_machine_code_with_two_u64_matches(
            object.entry_function().bytes(&object),
            0xfedc_ba98_7654_3210,
            0,
            1,
        ));
    }
}

#[test]
fn checked_source_policy_retags_and_unary_negation_reuse_terminal_arithmetic() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("policy-retag and unary-negation source canaries should compile");
    let cases = [
        (
            "terminal_explicit_saturating_cast_add_runtime",
            0_u8,
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerValue::Unsigned(250),
            IntegerValue::Unsigned(10),
            IntegerValue::Unsigned(255),
            2,
        ),
        (
            "terminal_wrapping_negate_runtime",
            1,
            IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
            IntegerValue::Signed(-128),
            IntegerValue::Signed(0),
            IntegerValue::Signed(-128),
            3,
        ),
        (
            "terminal_saturating_negate_runtime",
            2,
            IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
            IntegerValue::Signed(-128),
            IntegerValue::Signed(0),
            IntegerValue::Signed(127),
            3,
        ),
    ];

    for (machine, expected_kind, scalar_type, left, right, expected, expected_fuel) in cases {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        let operation = lowered.semantic_module.machines[0].blocks[0]
            .operations
            .last()
            .expect("policy arithmetic should retain one terminal operation");
        assert!(
            matches!(
                (expected_kind, operation.kind),
                (0, OperationKind::SaturatingIntegerAdd { .. })
                    | (1, OperationKind::WrappingIntegerSubtract { .. })
                    | (2, OperationKind::SaturatingIntegerSubtract { .. })
            ),
            "{machine} terminal operation kind"
        );
        let semantic = encode_module(&lowered.semantic_module)
            .unwrap_or_else(|error| panic!("{machine} semantic module should encode: {error:?}"));
        let proof = encode_proof_bundle(&lowered.proof_bundle)
            .unwrap_or_else(|error| panic!("{machine} proof should encode: {error:?}"));
        drop(lowered);

        let argument = |value| TerminalScalarValue::Integer { scalar_type, value };
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .unwrap_or_else(|error| panic!("{machine} artifact should interpret: {error:?}"));
        assert_eq!(measured.value(), argument(expected), "{machine} result");
        assert_eq!(
            measured.usage().total_units(),
            expected_fuel,
            "{machine} fuel"
        );

        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .unwrap_or_else(|error| panic!("{machine} should cross Omega: {error:?}"));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
            let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } =
                &target_operations.functions[0].operation
            else {
                panic!("{machine} should remain an integer expression return");
            };
            assert!(
                matches!(
                    (machine, expression),
                    (
                        "terminal_explicit_saturating_cast_add_runtime",
                        TerminalTargetIntegerExpression::SaturatingAdd { .. }
                    ) | (
                        "terminal_wrapping_negate_runtime",
                        TerminalTargetIntegerExpression::WrappingSubtract { .. }
                    ) | (
                        "terminal_saturating_negate_runtime",
                        TerminalTargetIntegerExpression::SaturatingSubtract { .. }
                    )
                ),
                "{machine} target expression kind"
            );
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} homes should assign: {error:?}"));
            emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        }

        #[cfg(unix)]
        {
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .unwrap_or_else(|error| panic!("{machine} host selection: {error:?}"));
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} host homes: {error:?}"));
            let machine_code = emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} host emission: {error:?}"));
            let object = build_terminal_object_artifact(&machine_code)
                .unwrap_or_else(|error| panic!("{machine} host object: {error:?}"));
            let argument_bits = |value: IntegerValue| match value {
                IntegerValue::Unsigned(value) => value as u64,
                IntegerValue::Signed(value) => value as i64 as u64,
            };
            let expected_bits = argument_bits(expected);
            let actual = run_host_machine_code_with_two_u64(
                object.entry_function().bytes(&object),
                argument_bits(left),
                argument_bits(right),
            ) as u32 as u64;
            let mask = if scalar_type.bits() == 64 {
                u64::MAX
            } else {
                (1_u64 << scalar_type.bits()) - 1
            };
            assert_eq!(
                actual & mask,
                expected_bits & mask,
                "{machine} native result"
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
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
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
        "value-producing decision leaves must retain the equality operation"
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
        lower_machine(&checked, "terminal_known_integer_graph_wrong_contract")
            .expect_err("compile-known integer graph with an unrelated contract must fail closed"),
        LoweringError::Unsupported("contract literals must equal the executed literal")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_known_boolean_binding_wrong_contract").expect_err(
            "compile-known Boolean binding with an unrelated integer contract must fail closed"
        ),
        LoweringError::Unsupported("contract literals must equal the executed literal")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_boolean_chain_wrong_contract")
            .expect_err("closed Boolean chain with an unrelated contract must fail closed"),
        LoweringError::Unsupported("Boolean contract literal must match the compile-known result")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_boolean_tuple_wrong_contract")
            .expect_err("compile-known general graph with an unrelated contract must fail closed"),
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
fn boolean_result_graph_retains_guarded_crash_exit() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("Boolean guarded-crash source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_guarded_trap")
        .expect("Boolean-result graph should retain its guarded crash exit");
    drop(checked);

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("Boolean guarded crash should encode");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("Boolean guarded-crash proof should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean guarded crash should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("Boolean guarded-crash proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean guarded crash should verify after frontend drop");
    assert!(matches!(
        semantic_module.machines[0].blocks[1].terminator,
        Terminator::Crash {
            cause: CrashCause::Trap,
            ..
        }
    ));

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
            TerminalExecutionStatus::Complete(TerminalScalarValue::Boolean(true)),
        ),
    ] {
        let mut execution =
            TerminalExecution::start(&verified, &[TerminalScalarValue::Boolean(flag)])
                .expect("Boolean guarded-crash execution should start");
        assert_eq!(
            execution
                .resume(&mut TerminalFuelMeter::unbounded())
                .expect("Boolean guarded-crash execution should finish"),
            expected
        );
    }

    let abstract_operations = lower_verified_module(&verified)
        .expect("guarded crash should remain represented at the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(operation, TerminalAbstractOperation::Crash { .. }))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("guarded Boolean crash should select as mixed terminal control");
        let TerminalTargetOperation::ReturnBooleanConditionalControl {
            when_true,
            when_false,
            ..
        } = &target_operations.functions[0].operation
        else {
            panic!("direct Boolean guard should retain target conditional control");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TerminalTargetBooleanControl::Crash {
                cause: CrashCause::Trap,
                damage_minimum,
                containment_demand,
                frontier_lower_bound,
                ..
            } if damage_minimum == "Activation"
                && containment_demand == "ExecutionDomain"
                && frontier_lower_bound.is_empty()
        ));
        assert!(matches!(
            when_false.control.as_ref(),
            TerminalTargetBooleanControl::ReturnImmediate { value: true, .. }
        ));

        let assigned = assign_registers(&target_operations)
            .expect("guarded Boolean crash control should assign");
        let TerminalAssignedOperation::ReturnBooleanConditionalControl { when_true, .. } =
            &assigned.functions[0].operation
        else {
            panic!("assigned Boolean control should retain its shape");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TerminalAssignedBooleanControl::Crash {
                cause: CrashCause::Trap,
                ..
            }
        ));
        let emitted = emit_machine_code(&assigned).expect("guarded Boolean crash should emit");
        let branch_to_false_over_fault = match target.architecture {
            omega_target::Architecture::X86_64 => {
                &[0x0f, 0x84, 0x02, 0x00, 0x00, 0x00, 0x0f, 0x0b][..]
            }
            omega_target::Architecture::Aarch64 => {
                &[0x40, 0x00, 0x00, 0x34, 0x00, 0x00, 0x20, 0xd4][..]
            }
        };
        assert!(
            emitted.functions[0]
                .bytes
                .windows(branch_to_false_over_fault.len())
                .any(|window| window == branch_to_false_over_fault),
            "the false return arm must branch over the true crash leaf"
        );
    }
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
    let guarded_abstract = lower_verified_module(&guarded_verified)
        .expect("guarded integer crash should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&guarded_abstract, target)
            .expect("guarded integer crash should select as mixed terminal control");
        let TerminalTargetOperation::ReturnIntegerConditionalControl {
            when_true,
            when_false,
            ..
        } = &target_operations.functions[0].operation
        else {
            panic!("direct Boolean guard should retain integer target control");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TerminalTargetIntegerControl::Crash {
                cause: CrashCause::Trap,
                damage_minimum,
                containment_demand,
                frontier_lower_bound,
                ..
            } if damage_minimum == "Activation"
                && containment_demand == "ExecutionDomain"
                && frontier_lower_bound.is_empty()
        ));
        assert!(matches!(
            when_false.control.as_ref(),
            TerminalTargetIntegerControl::Return { .. }
        ));

        let assigned = assign_registers(&target_operations)
            .expect("guarded integer crash control should assign");
        let TerminalAssignedOperation::ReturnIntegerConditionalControl { when_true, .. } =
            &assigned.functions[0].operation
        else {
            panic!("assigned integer control should retain its shape");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TerminalAssignedIntegerControl::Crash {
                cause: CrashCause::Trap,
                ..
            }
        ));
        let emitted = emit_machine_code(&assigned).expect("guarded integer crash should emit");
        let fault = match target.architecture {
            omega_target::Architecture::X86_64 => &[0x0f, 0x0b][..],
            omega_target::Architecture::Aarch64 => &[0x00, 0x00, 0x20, 0xd4][..],
        };
        assert!(
            emitted.functions[0]
                .bytes
                .windows(fault.len())
                .any(|window| window == fault)
        );
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
    assert_guarded_crash_emits(&integer_guarded_verified);
    let transitive_trap = lower_machine(&checked, "terminal_transitive_guarded_trap")
        .expect("a transitive integer conjunction should lower as short-circuit control");
    assert_eq!(transitive_trap.semantic_module.machines[0].blocks.len(), 4);
    assert!(matches!(
        transitive_trap.semantic_module.machines[0].blocks[0].terminator,
        Terminator::Conditional { .. }
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
        6
    );
    let signed = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    for (left, middle, right, expected, expected_units) in [
        (5, 3, 10, TerminalExecutionStatus::Complete(signed(0)), 4),
        (1, 5, 3, TerminalExecutionStatus::Complete(signed(0)), 6),
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
            5,
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
    assert_guarded_crash_emits(&transitive_verified);
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
    assert_guarded_crash_emits(&implied_verified);
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

    for (source, expected_cause, expected_damage, expected_demand) in [
        (
            &wide_trap,
            CrashCause::Trap,
            "Activation",
            "ExecutionDomain",
        ),
        (
            &lowered,
            CrashCause::Abort,
            "ExecutionDomain",
            "ExecutionDomain",
        ),
    ] {
        let semantic =
            encode_module(&source.semantic_module).expect("crash semantics should encode");
        let proof = encode_proof_bundle(&source.proof_bundle).expect("crash proof should encode");
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("verified unconditional crash should cross the Omega boundary");
        assert!(matches!(
            abstract_operations.functions[0].operations.as_slice(),
            [TerminalAbstractOperation::Crash {
                cause,
                damage_minimum,
                containment_demand,
                frontier_lower_bound,
                ..
            }] if *cause == expected_cause
                && damage_minimum == expected_damage
                && containment_demand == expected_demand
                && frontier_lower_bound.is_empty()
        ));

        for (target, expected_bytes) in [
            (NativeTarget::linux_x64(), &[0x0f, 0x0b][..]),
            (NativeTarget::linux_arm64(), &[0x00, 0x00, 0x20, 0xd4][..]),
        ] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("unconditional crash should select");
            assert!(matches!(
                &target_operations.functions[0].operation,
                TerminalTargetOperation::Crash {
                    cause,
                    damage_minimum,
                    containment_demand,
                    frontier_lower_bound,
                    ..
                } if *cause == expected_cause
                    && damage_minimum == expected_damage
                    && containment_demand == expected_demand
                    && frontier_lower_bound.is_empty()
            ));
            let assigned = assign_registers(&target_operations)
                .expect("unconditional crash should require no register homes");
            assert!(matches!(
                &assigned.functions[0].operation,
                TerminalAssignedOperation::Crash { cause, .. } if *cause == expected_cause
            ));
            let emitted = emit_machine_code(&assigned).expect("unconditional crash should emit");
            assert_eq!(emitted.functions[0].bytes, expected_bytes);
            assert_eq!(
                emitted.functions[0].provenance.edges,
                vec![EdgeId::new(1).unwrap()]
            );
        }
    }
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
