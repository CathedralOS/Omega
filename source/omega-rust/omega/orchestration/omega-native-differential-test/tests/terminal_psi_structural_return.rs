//! Focused source-to-native canary for the first whole-root structural return.

use omega_calling_conventions::{
    CallingPolicy, MachineRegister, ValueLocation, ValuePlacement, ValueShape,
};
use omega_optimization_validation::validate_verified_psi_optimization_unit;
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractFunctionResult, TerminalAbstractOperation,
};
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_assigned_target_operations::TerminalAssignedOperation;
use omega_terminal_image_emission::{
    TerminalInstallationError, build_terminal_installation_record, build_terminal_object_artifact,
    decode_terminal_installation_record, emit_terminal_executable_image,
    emit_terminal_object_container, encode_terminal_installation_record,
    validate_terminal_installation_record,
};
use omega_terminal_machine_code::{TerminalNativeFuelSite, TerminalScalarControlFlowEvidence};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::{
    build_verified_psi_optimization_unit, lower_artifact_sections,
    lower_artifact_sections_for_optimization,
};
use omega_terminal_target_operations::{
    TerminalCallSiteOwner, TerminalScalarParameterLocation, TerminalTargetIntegerExpression,
    TerminalTargetOperation,
};
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::{
    IntegerSign, IntegerType, IntegerValue, PlaceId, ProfileDecisionId, ScalarType,
    StructuralFieldId, StructuralPlaceKind,
};
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{OperationKind, TerminalAffineCleanupAction, TerminalMachineResult, Terminator};
use psi_terminal_codec::{
    decode_module, encode_module, encode_proof_bundle, terminal_psi_identity,
};
use psi_terminal_fuel::{TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralResult, TerminalStructuralValue,
};
use psi_terminal_verifier::verify_module;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

const OPAQUE_REGION_IDENTITY: u64 = 0x5eed_cafe_dead_beef;

#[cfg(unix)]
static NEXT_SCRATCH_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TargetCase {
    target: NativeTarget,
    policy: CallingPolicy,
    parameter: MachineRegister,
    second_parameter: MachineRegister,
    result: MachineRegister,
    bytes: &'static [u8],
}

const SYSV_RETURN: &[u8] = &[0x48, 0x89, 0xf8, 0xc3];
const MICROSOFT_RETURN: &[u8] = &[0x48, 0x89, 0xc8, 0xc3];
const AAPCS64_RETURN: &[u8] = &[0xc0, 0x03, 0x5f, 0xd6];

fn target_cases() -> [TargetCase; 5] {
    [
        TargetCase {
            target: NativeTarget::linux_x64(),
            policy: CallingPolicy::SystemVAMD64,
            parameter: MachineRegister::X86Rdi,
            second_parameter: MachineRegister::X86Rsi,
            result: MachineRegister::X86Rax,
            bytes: SYSV_RETURN,
        },
        TargetCase {
            target: NativeTarget::windows_x64(),
            policy: CallingPolicy::MicrosoftX64,
            parameter: MachineRegister::X86Rcx,
            second_parameter: MachineRegister::X86Rdx,
            result: MachineRegister::X86Rax,
            bytes: MICROSOFT_RETURN,
        },
        TargetCase {
            target: NativeTarget::uefi_x64(),
            policy: CallingPolicy::MicrosoftX64,
            parameter: MachineRegister::X86Rcx,
            second_parameter: MachineRegister::X86Rdx,
            result: MachineRegister::X86Rax,
            bytes: MICROSOFT_RETURN,
        },
        TargetCase {
            target: NativeTarget::linux_arm64(),
            policy: CallingPolicy::Aapcs64,
            parameter: MachineRegister::Aarch64X(0),
            second_parameter: MachineRegister::Aarch64X(1),
            result: MachineRegister::Aarch64X(0),
            bytes: AAPCS64_RETURN,
        },
        TargetCase {
            target: NativeTarget::macos_arm64(),
            policy: CallingPolicy::Aapcs64,
            parameter: MachineRegister::Aarch64X(0),
            second_parameter: MachineRegister::Aarch64X(1),
            result: MachineRegister::Aarch64X(0),
            bytes: AAPCS64_RETURN,
        },
    ]
}

fn assert_direct_register_placement(placement: &ValuePlacement, register: MachineRegister) {
    assert_eq!(placement.shape, ValueShape::integer(8, 8));
    assert!(matches!(
        placement.locations.as_slice(),
        [ValueLocation::Register {
            register: actual,
            value_byte_offset: 0,
            byte_size: 8,
        }] if *actual == register
    ));
}

fn source_canary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("omega-native-differential-test lives under source/omega-rust/omega/orchestration")
        .join("tests/omega/pass/terminal_psi/structural_content_passthrough/main.omg")
}

fn checked_source() -> psi_checked_trees::CheckedTrees {
    let source = std::fs::read_to_string(source_canary()).expect("read structural source canary");
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn source_scalar_result_precedes_nominal_cleanup_through_all_native_artifacts() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}

        data Token { value: u64; }
        machine Token::drop(&mut self) { Helper::touch(); }

        data Root {}
        machine Root::measure(token: Token) -> u64 { 7u64 }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize scalar cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse scalar cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve scalar cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type scalar cleanup");
    let checked = lower_typed_trees(typed).expect("check scalar cleanup");
    let lowered = lower_machine(&checked, "Root::measure")
        .expect("source scalar nominal cleanup reaches terminal Psi");
    let original_identity =
        terminal_psi_identity(&lowered.semantic_module).expect("terminal identity");
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof artifact");
    let entry_machine = lowered.semantic_module.entry;
    drop(checked);
    drop(lowered);

    let module = decode_module(&semantic_bytes).expect("semantic artifact decodes");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == entry_machine)
        .expect("scalar cleanup entry");
    assert!(matches!(entry.result, TerminalMachineResult::Scalar(_)));
    let Terminator::Return {
        cleanup_actions, ..
    } = &entry.blocks[0].terminator
    else {
        panic!("scalar cleanup entry returns a scalar")
    };
    assert!(matches!(
        cleanup_actions.as_slice(),
        [TerminalAffineCleanupAction::InvokeNominal(_)]
    ));

    let abstract_plan =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("verified scalar cleanup crosses the Omega boundary");
    let abstract_entry = abstract_plan
        .functions
        .iter()
        .find(|function| function.machine == entry_machine)
        .expect("abstract scalar cleanup entry");
    assert!(matches!(
        abstract_entry.operations.as_slice(),
        [TerminalAbstractOperation::IntegerConstant { .. }, TerminalAbstractOperation::Return {
            cleanup_actions,
            ..
        }] if matches!(cleanup_actions.as_slice(), [TerminalAffineCleanupAction::InvokeNominal(_)])
    ));

    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering failed: {error:?}", case.target));
        let target_entry = target_plan
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("target scalar cleanup entry");
        assert!(matches!(
            &target_entry.operation,
            TerminalTargetOperation::ScalarReturnWithCleanup {
                cleanup_actions,
                scalar,
                ..
            } if matches!(cleanup_actions.as_slice(), [TerminalAffineCleanupAction::InvokeNominal(_)])
                && matches!(scalar.as_ref(), TerminalTargetOperation::ReturnIntegerImmediate { .. })
        ));

        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment failed: {error:?}", case.target));
        assert!(matches!(
            assigned
                .functions
                .iter()
                .find(|function| function.machine == entry_machine)
                .map(|function| &function.operation),
            Some(TerminalAssignedOperation::ScalarReturnWithCleanup { .. })
        ));
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission failed: {error:?}", case.target));
        let emitted_entry = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("emitted scalar cleanup entry");
        let cleanup = emitted_entry
            .scalar_affine_cleanup
            .as_ref()
            .expect("emitted scalar cleanup custody");
        assert!(emitted_entry.unit_affine_cleanup.is_none());
        assert_eq!(emitted_entry.scalar_structural_parameter_homes.len(), 1);
        assert_eq!(emitted_entry.internal_unit_calls.len(), 1);
        assert!(cleanup.code_offset > 0, "result bytes precede cleanup");
        assert!(matches!(
            cleanup.actions.as_slice(),
            [TerminalAffineCleanupAction::InvokeNominal(_)]
        ));

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} object failed: {error:?}", case.target));
        let object_entry = object
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("object scalar cleanup entry");
        assert_eq!(object_entry.scalar_affine_cleanup.as_ref(), Some(cleanup));
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} image failed: {error:?}", case.target));
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} installation failed: {error:?}", case.target));
        let installed_entry = installation
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("installed scalar cleanup entry");
        assert_eq!(
            installed_entry.scalar_affine_cleanup.as_ref(),
            Some(cleanup)
        );
        let installation_bytes = encode_terminal_installation_record(&installation)
            .expect("canonical scalar cleanup installation");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation.clone())
        );
        validate_terminal_installation_record(&installation, &image)
            .expect("installed scalar cleanup binds its exact image");
    }
}

#[test]
fn contextual_short_circuit_boolean_cleans_every_leaf_through_all_native_artifacts() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}

        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        { Helper::touch(); }
        data Plain { observed: bool; }

        data Root {}
        machine Root::measure(
            token: Token,
            left: bool,
            plain: Plain,
            right: bool
        ) -> bool
        requires token.ready, plain.observed
        {
            let inverted: bool = !right;
            let staged: bool = left && inverted;
            let reused: bool = staged == staged;
            let repeated: bool = reused && left;
            repeated
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize bounded Boolean cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse bounded Boolean cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve bounded Boolean cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type bounded Boolean cleanup");
    let checked = lower_typed_trees(typed).expect("check bounded Boolean cleanup");
    let lowered = lower_machine(&checked, "Root::measure")
        .expect("bounded Boolean nominal cleanup reaches terminal Psi");
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof artifact");
    let entry_machine = lowered.semantic_module.entry;
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == entry_machine)
        .expect("bounded Boolean cleanup entry");
    assert_eq!(entry.contract.requires.len(), 2);
    assert_eq!(
        entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::BooleanEqual { .. }))
            .count(),
        3,
        "the reused value is decided once and its continuation is distributed over the leaves"
    );
    let mut return_obligations = Vec::new();
    let return_edges = entry
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Return {
                edge,
                cleanup_actions,
                ..
            } => {
                let [
                    TerminalAffineCleanupAction::DiscardRoot(_),
                    TerminalAffineCleanupAction::InvokeNominal(cleanup),
                ] = cleanup_actions.as_slice()
                else {
                    panic!("every Boolean leaf retains contextual nominal cleanup")
                };
                assert!(cleanup.cleanup_receiver.is_some());
                let [obligation] = cleanup.requirement_obligations.as_slice() else {
                    panic!("every Boolean leaf owns one contextual obligation")
                };
                return_obligations.push(*obligation);
                Some(*edge)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(return_edges.len() > 3);
    assert_eq!(
        return_edges
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        return_edges.len()
    );
    assert_eq!(
        return_obligations
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        return_edges.len()
    );
    assert_eq!(lowered.proof_bundle.evidence.len(), return_edges.len());
    drop(checked);
    drop(lowered);

    let abstract_plan =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("verified bounded Boolean cleanup crosses the Omega boundary");
    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering failed: {error:?}", case.target));
        assert!(matches!(
            target_plan
                .functions
                .iter()
                .find(|function| function.machine == entry_machine)
                .map(|function| &function.operation),
            Some(TerminalTargetOperation::BooleanControlWithCleanup { .. })
        ));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment failed: {error:?}", case.target));
        assert!(matches!(
            assigned
                .functions
                .iter()
                .find(|function| function.machine == entry_machine)
                .map(|function| &function.operation),
            Some(TerminalAssignedOperation::BooleanControlWithCleanup { .. })
        ));

        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission failed: {error:?}", case.target));
        let emitted_entry = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("emitted bounded Boolean cleanup entry");
        assert!(emitted_entry.scalar_affine_cleanup.is_none());
        let emitted_leaf_count = emitted_entry.scalar_control_affine_cleanups.len();
        assert!(emitted_leaf_count > 3);
        assert_eq!(emitted_entry.internal_unit_calls.len(), emitted_leaf_count);
        assert_eq!(emitted_entry.scalar_structural_parameter_homes.len(), 2);
        assert!(matches!(
            emitted_entry
                .scalar_stack
                .as_ref()
                .map(|stack| stack.control_flow.clone()),
            Some(TerminalScalarControlFlowEvidence::ConditionalTree { .. })
        ));
        let emitted_edges = emitted_entry
            .scalar_control_affine_cleanups
            .iter()
            .map(|record| record.cleanup.psi_edge)
            .collect::<Vec<_>>();
        assert_eq!(
            emitted_edges
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            emitted_leaf_count
        );
        assert_eq!(
            emitted_entry
                .internal_unit_calls
                .iter()
                .map(|call| call.owner)
                .collect::<std::collections::BTreeSet<_>>(),
            emitted_edges
                .iter()
                .copied()
                .map(|edge| TerminalCallSiteOwner::CleanupAction {
                    edge,
                    action_ordinal: 1,
                })
                .collect()
        );
        for record in &emitted_entry.scalar_control_affine_cleanups {
            let [
                TerminalAffineCleanupAction::DiscardRoot(_),
                TerminalAffineCleanupAction::InvokeNominal(cleanup),
            ] = record.cleanup.actions.as_slice()
            else {
                panic!("each emitted edge retains the contextual cleanup action")
            };
            assert!(cleanup.cleanup_receiver.is_none());
            assert!(cleanup.requirement_obligations.is_empty());
            assert!(record.cleanup.byte_count > 0);
        }

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} object failed: {error:?}", case.target));
        let object_entry = object
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("object bounded Boolean cleanup entry");
        assert_eq!(
            object_entry.scalar_control_affine_cleanups,
            emitted_entry.scalar_control_affine_cleanups
        );
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} image failed: {error:?}", case.target));
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} installation failed: {error:?}", case.target));
        let installed_entry = installation
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("installed bounded Boolean cleanup entry");
        assert_eq!(
            installed_entry.scalar_control_affine_cleanups,
            emitted_entry
                .scalar_control_affine_cleanups
                .iter()
                .map(|record| record.cleanup.clone())
                .collect::<Vec<_>>()
        );
        let installation_bytes = encode_terminal_installation_record(&installation)
            .expect("canonical bounded Boolean cleanup installation");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation.clone())
        );
        validate_terminal_installation_record(&installation, &image)
            .expect("installed bounded Boolean cleanup binds its exact image");
    }
}

#[test]
fn nominal_boolean_convergence_has_one_physical_cleanup_tail_on_all_targets() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}
        data Token { padding: bool; ready: bool; }
        machine Token::drop(&mut self) { Helper::touch(); }
        data Root {}
        machine Root::measure(token: Token, left: bool) -> bool {
            let staged: bool = token.ready && !left;
            staged
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize shared convergence");
    let syntax = parse_syntax_trees(&tokens).expect("parse shared convergence");
    let resolved = lower_syntax_trees(&syntax).expect("resolve shared convergence");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type shared convergence");
    let checked = lower_typed_trees(typed).expect("check shared convergence");
    let lowered = lower_machine(&checked, "Root::measure").expect("lower shared convergence");
    let terminal_entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("terminal shared convergence entry");
    assert!(terminal_entry.blocks.len() > 4);
    assert!(terminal_entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::BooleanNot { .. }))
    }));
    assert!(terminal_entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::BooleanStructuralField { .. }))
    }));
    assert!(terminal_entry.blocks.iter().all(|block| {
        block
            .operations
            .iter()
            .all(|operation| !matches!(operation.kind, OperationKind::BooleanEqual { .. }))
    }));
    assert_eq!(
        terminal_entry
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Return { .. }))
            .count(),
        1
    );
    let semantics = encode_module(&lowered.semantic_module).expect("shared semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("shared proof");
    let entry_machine = lowered.semantic_module.entry;
    let abstract_plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("shared convergence crosses Omega boundary");

    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering: {error:?}", case.target));
        let target_entry = target_plan
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("target shared convergence entry");
        assert!(matches!(
            &target_entry.operation,
            TerminalTargetOperation::ScalarReturnWithCleanup { scalar, .. }
                if matches!(scalar.as_ref(),
                    TerminalTargetOperation::ReturnBooleanSharedConvergence { .. })
        ));
        let target_debug = format!("{:?}", target_entry.operation);
        assert!(target_debug.contains("StructuralField"));
        assert!(target_debug.contains("field_byte_offset: 1"));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment: {error:?}", case.target));
        assert!(matches!(
            assigned
                .functions
                .iter()
                .find(|function| function.machine == entry_machine)
                .map(|function| &function.operation),
            Some(TerminalAssignedOperation::ScalarReturnWithCleanup { scalar, .. })
                if matches!(scalar.as_ref(),
                    TerminalAssignedOperation::ReturnBooleanSharedConvergence { .. })
        ));
        let assigned_debug = format!(
            "{:?}",
            assigned
                .functions
                .iter()
                .find(|function| function.machine == entry_machine)
                .expect("assigned shared convergence entry")
                .operation
        );
        assert!(assigned_debug.contains("StructuralField"));
        assert!(assigned_debug.contains("field_byte_offset: 1"));
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission: {error:?}", case.target));
        let emitted = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("emitted shared convergence entry");
        let cleanup = emitted
            .scalar_affine_cleanup
            .as_ref()
            .expect("one shared scalar cleanup record");
        assert!(emitted.scalar_control_affine_cleanups.is_empty());
        assert_eq!(emitted.internal_unit_calls.len(), 1);
        let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
            decisions,
            joins,
            structural_conditions,
            merge_offset,
        } = &emitted
            .scalar_stack
            .as_ref()
            .expect("shared convergence stack evidence")
            .control_flow
        else {
            panic!("native shared convergence must retain its exact join")
        };
        assert!(decisions.len() >= 2);
        assert_eq!(joins.len(), decisions.len());
        assert!(joins.iter().all(|join| join.join_offset < *merge_offset));
        assert!(!structural_conditions.is_empty());
        assert!(structural_conditions.iter().all(|condition| {
            !condition.reads.is_empty()
                && condition.byte_count == condition.bytes.len()
                && condition.reads.iter().all(|read| {
                    read.field_byte_offset == 1
                        && read.byte_count != 0
                        && read.code_offset >= condition.code_offset
                        && read.code_offset + read.byte_count
                            <= condition.code_offset + condition.byte_count
                })
        }));
        assert_eq!(*merge_offset, cleanup.code_offset);
        let forged_join_offset = joins[0].join_offset;
        let forged_structural_offset = structural_conditions[0].code_offset;

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} object replay: {error:?}", case.target));
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} image: {error:?}", case.target));
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} installation: {error:?}", case.target));
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("installed shared convergence entry");
        assert!(installed.scalar_affine_cleanup.is_some());
        assert!(installed.scalar_control_affine_cleanups.is_empty());
        let bytes = encode_terminal_installation_record(&installation)
            .expect("encode shared convergence installation");
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation.clone())
        );
        validate_terminal_installation_record(&installation, &image)
            .expect("installed shared convergence binds its exact image");

        let mut forged = machine_code.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.machine == entry_machine)
            .expect("forged shared convergence entry");
        function.bytes[forged_join_offset] ^= 1;
        assert!(build_terminal_object_artifact(&forged).is_err());

        let mut forged_structural_read = machine_code.clone();
        let function = forged_structural_read
            .functions
            .iter_mut()
            .find(|function| function.machine == entry_machine)
            .expect("forged structural read entry");
        function.bytes[forged_structural_offset] ^= 1;
        assert!(build_terminal_object_artifact(&forged_structural_read).is_err());

        let mut forged_structural_source = machine_code.clone();
        let function = forged_structural_source
            .functions
            .iter_mut()
            .find(|function| function.machine == entry_machine)
            .expect("forged structural source entry");
        let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
            structural_conditions,
            ..
        } = &mut function
            .scalar_stack
            .as_mut()
            .expect("forged structural source stack")
            .control_flow
        else {
            unreachable!("shared convergence evidence shape was already checked")
        };
        let read = &mut structural_conditions[0].reads[0];
        read.source = PlaceId::new(read.source.get() + 1_000).expect("forged source place");
        assert!(build_terminal_object_artifact(&forged_structural_source).is_err());

        let mut forged_structural_field = machine_code.clone();
        let function = forged_structural_field
            .functions
            .iter_mut()
            .find(|function| function.machine == entry_machine)
            .expect("forged structural field entry");
        let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
            structural_conditions,
            ..
        } = &mut function
            .scalar_stack
            .as_mut()
            .expect("forged structural field stack")
            .control_flow
        else {
            unreachable!("shared convergence evidence shape was already checked")
        };
        let read = &mut structural_conditions[0].reads[0];
        read.field =
            StructuralFieldId::new(read.field.get() + 1_000).expect("forged structural field");
        assert!(build_terminal_object_artifact(&forged_structural_field).is_err());

        let mut forged_structural_field_offset = machine_code.clone();
        let function = forged_structural_field_offset
            .functions
            .iter_mut()
            .find(|function| function.machine == entry_machine)
            .expect("forged structural field offset entry");
        let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
            structural_conditions,
            ..
        } = &mut function
            .scalar_stack
            .as_mut()
            .expect("forged structural field offset stack")
            .control_flow
        else {
            unreachable!("shared convergence evidence shape was already checked")
        };
        structural_conditions[0].reads[0].field_byte_offset += 1;
        assert!(build_terminal_object_artifact(&forged_structural_field_offset).is_err());

        let mut coherently_forged_structural_bytes = machine_code.clone();
        let function = coherently_forged_structural_bytes
            .functions
            .iter_mut()
            .find(|function| function.machine == entry_machine)
            .expect("coherently forged structural bytes entry");
        let (condition_offset, read_offset, read_byte_count) = {
            let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
                structural_conditions,
                ..
            } = &function
                .scalar_stack
                .as_ref()
                .expect("coherently forged structural bytes stack")
                .control_flow
            else {
                unreachable!("shared convergence evidence shape was already checked")
            };
            let condition = &structural_conditions[0];
            let read = &condition.reads[0];
            (condition.code_offset, read.code_offset, read.byte_count)
        };
        let mutation_offset = match case.target.architecture {
            omega_target::Architecture::X86_64 => {
                assert!(read_byte_count >= 3);
                read_offset + 2
            }
            omega_target::Architecture::Aarch64 => {
                assert!(read_byte_count >= 4);
                read_offset
            }
        };
        function.bytes[mutation_offset] ^= match case.target.architecture {
            omega_target::Architecture::X86_64 => 0x08,
            omega_target::Architecture::Aarch64 => 0x20,
        };
        let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
            structural_conditions,
            ..
        } = &mut function
            .scalar_stack
            .as_mut()
            .expect("coherently forged structural bytes stack")
            .control_flow
        else {
            unreachable!("shared convergence evidence shape was already checked")
        };
        structural_conditions[0].bytes[mutation_offset - condition_offset] =
            function.bytes[mutation_offset];
        assert!(build_terminal_object_artifact(&coherently_forged_structural_bytes).is_err());

        let mut missing_join = machine_code.clone();
        let function = missing_join
            .functions
            .iter_mut()
            .find(|function| function.machine == entry_machine)
            .expect("missing-join shared convergence entry");
        let TerminalScalarControlFlowEvidence::BooleanSharedConvergence { joins, .. } =
            &mut function
                .scalar_stack
                .as_mut()
                .expect("missing-join stack evidence")
                .control_flow
        else {
            unreachable!("shared convergence evidence shape was already checked")
        };
        joins.pop();
        assert!(build_terminal_object_artifact(&missing_join).is_err());
    }
}

#[test]
fn arbitrary_exact_mixed_shift_chains_emit_on_every_native_target() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}

        data Token { value: u64; }
        machine Token::drop(&mut self) { Helper::touch(); }

        data Root {}
        machine Root::measure(
            token: Token,
            value: u8,
            signed: i8,
            wide: u16,
            signed_wide: i16,
            post_signed: i16,
            post_unsigned: u16,
            affine_unsigned: u8,
            affine_signed: i8,
            zero_root: u8,
            shift_affine_unsigned: u8,
            shift_affine_signed: i8,
            shift_zero_root: u8,
            sandwich_unsigned: u16,
            sandwich_signed: i16,
            sandwich_right_only: u16,
            affine_cast_shift_unsigned: u16,
            affine_cast_shift_signed: i16,
            affine_cast_shift_zero: u16,
            shift_cast_affine_unsigned: u16,
            shift_cast_affine_signed: i16,
            shift_cast_affine_zero: u16,
            divide_cast_affine: u16,
            divide_cast_shift: u16,
            divide_cast_shift_signed: i16,
            affine_cast_divide: u16,
            shift_cast_remainder: u16,
            divide_affine_direct: u8,
            divide_shift_direct: u8,
            affine_divide_direct: u8,
            shift_remainder_direct: u8,
            divide_cast_divide: u16,
            signed_divide_cast_remainder: i16,
            signed_multiply_chain: i8,
            signed_multiply_cast: i16,
            signed_cast_multiply: u16,
            signed_minimum_factor: i64,
            exact_cast_chain: i64,
            computed_affine_cast_chain: i64,
            computed_signed_product_cast_chain: i64,
            computed_shift_cast_chain: i64,
            computed_divide_cast_chain: u32,
            cast_chain_affine_suffix: i64,
            cast_chain_signed_product_suffix: i64,
            cast_chain_shift_suffix: i64,
            cast_chain_divide_suffix: u32,
            affine_cast_chain_shift_suffix: i64,
            shift_cast_chain_affine_suffix: i64,
            signed_product_cast_chain_signed_product_suffix: i64,
            divide_cast_chain_affine_suffix: u32,
            affine_cast_chain_divide_suffix: i64,
            affine_widen_chain_shift_suffix: i8,
            shift_widen_chain_affine_suffix: u8,
            signed_product_widen_chain_signed_product_suffix: i8,
            remainder_widen_chain_affine_suffix: u8,
            affine_widen_chain_divide_suffix: i8,
            affine_widen_cast_shift_suffix: i8,
            shift_cast_widen_affine_suffix: u16,
            signed_product_widen_cast_signed_product_suffix: i8,
            remainder_cast_widen_affine_suffix: u16,
            affine_widen_cast_widen_divide_suffix: i8,
            signed_affine_direct: i8,
            signed_affine_cast: i8,
            cast_signed_affine: i16,
            signed_affine_cast_affine_source: i16,
            affine_cast_signed_affine_source: i16,
            signed_affine_cast_signed_affine_source: i16,
            affine_fork_add_join: i16,
            affine_fork_subtract_join: i16,
            distinct_affine_fork_left: i16,
            distinct_affine_fork_right: i16,
            affine_product_join_left: i16,
            affine_product_join_right: i16,
            affine_quadratic_join_root: i16,
            affine_divide_remainder_join_root: i16,
            enabled: bool
        ) -> bool
        requires value <= 127u8, value <= 63u8, value <= 31u8,
            -32i8 <= signed, signed <= 31i8, 0i8 <= signed,
            wide <= 32767u16, wide <= 16383u16, wide <= 63u16,
            -16384i16 <= signed_wide, signed_wide <= 16383i16,
            0i16 <= signed_wide, signed_wide <= 127i16,
            0i16 <= post_signed, post_signed <= 255i16,
            post_signed <= 127i16, post_signed <= 63i16,
            post_unsigned <= 127u16, post_unsigned <= 63u16,
            affine_unsigned <= 252u8, affine_unsigned <= 124u8,
            affine_unsigned <= 60u8,
            affine_signed <= 124i8, -67i8 <= affine_signed,
            affine_signed <= 60i8, -35i8 <= affine_signed, affine_signed <= 28i8,
            zero_root <= 0u8,
            shift_affine_unsigned <= 127u8, shift_affine_unsigned <= 63u8,
            -64i8 <= shift_affine_signed, shift_affine_signed <= 63i8,
            -32i8 <= shift_affine_signed, shift_affine_signed <= 31i8,
            shift_zero_root <= 127u8,
            sandwich_unsigned <= 32767u16, sandwich_unsigned <= 127u16,
            sandwich_unsigned <= 63u16,
            -16384i16 <= sandwich_signed, sandwich_signed <= 16383i16,
            0i16 <= sandwich_signed, sandwich_signed <= 127i16,
            sandwich_signed <= 63i16,
            sandwich_right_only <= 32767u16, sandwich_right_only <= 127u16,
            affine_cast_shift_unsigned <= 65534u16,
            affine_cast_shift_unsigned <= 32766u16,
            affine_cast_shift_unsigned <= 126u16,
            affine_cast_shift_unsigned <= 62u16,
            affine_cast_shift_signed <= 32764i16,
            -16387i16 <= affine_cast_shift_signed,
            affine_cast_shift_signed <= 16380i16,
            -3i16 <= affine_cast_shift_signed,
            affine_cast_shift_signed <= 124i16,
            affine_cast_shift_signed <= 60i16,
            affine_cast_shift_zero <= 0u16,
            shift_cast_affine_unsigned <= 32767u16,
            shift_cast_affine_unsigned <= 127u16,
            shift_cast_affine_unsigned <= 63u16,
            -16384i16 <= shift_cast_affine_signed,
            shift_cast_affine_signed <= 16383i16,
            0i16 <= shift_cast_affine_signed,
            shift_cast_affine_signed <= 127i16,
            shift_cast_affine_signed <= 63i16,
            shift_cast_affine_zero <= 32767u16,
            shift_cast_affine_zero <= 127u16,
            affine_cast_divide <= 65534u16,
            affine_cast_divide <= 32766u16,
            affine_cast_divide <= 126u16,
            shift_cast_remainder <= 32767u16,
            shift_cast_remainder <= 127u16,
            affine_divide_direct <= 254u8,
            affine_divide_direct <= 126u8,
            shift_remainder_direct <= 127u8,
            -63i8 <= signed_multiply_chain, signed_multiply_chain <= 64i8,
            -21i8 <= signed_multiply_chain, signed_multiply_chain <= 21i8,
            -63i16 <= signed_multiply_cast, signed_multiply_cast <= 64i16,
            0i16 <= signed_multiply_cast, signed_multiply_cast <= 0i16,
            signed_cast_multiply <= 127u16, signed_cast_multiply <= 64u16,
            0i64 <= signed_minimum_factor, signed_minimum_factor <= 1i64,
            0i64 <= exact_cast_chain,
            exact_cast_chain <= 2147483647i64,
            exact_cast_chain <= 255i64,
            -4611686018427387904i64 <= computed_affine_cast_chain,
            computed_affine_cast_chain <= 4611686018427387903i64,
            0i64 <= computed_affine_cast_chain,
            computed_affine_cast_chain <= 1073741823i64,
            computed_affine_cast_chain <= 127i64,
            -4611686018427387903i64 <= computed_signed_product_cast_chain,
            computed_signed_product_cast_chain <= 4611686018427387904i64,
            -1073741823i64 <= computed_signed_product_cast_chain,
            -127i64 <= computed_signed_product_cast_chain,
            -9223372036854775807i64 <= computed_signed_product_cast_chain,
            computed_signed_product_cast_chain <= 0i64,
            -4611686018427387904i64 <= computed_shift_cast_chain,
            computed_shift_cast_chain <= 4611686018427387903i64,
            0i64 <= computed_shift_cast_chain,
            computed_shift_cast_chain <= 2147483647i64,
            computed_shift_cast_chain <= 255i64,
            0i64 <= cast_chain_affine_suffix,
            cast_chain_affine_suffix <= 2147483647i64,
            cast_chain_affine_suffix <= 2147483646i64,
            cast_chain_affine_suffix <= 1073741822i64,
            0i64 <= cast_chain_signed_product_suffix,
            cast_chain_signed_product_suffix <= 2147483647i64,
            cast_chain_signed_product_suffix <= 1073741824i64,
            0i64 <= cast_chain_shift_suffix,
            cast_chain_shift_suffix <= 2147483647i64,
            cast_chain_shift_suffix <= 1073741823i64,
            cast_chain_divide_suffix <= 127u32,
            affine_cast_chain_shift_suffix <= 9223372036854775806i64,
            -1i64 <= affine_cast_chain_shift_suffix,
            affine_cast_chain_shift_suffix <= 2147483646i64,
            affine_cast_chain_shift_suffix <= 1073741822i64,
            0i64 <= shift_cast_chain_affine_suffix,
            shift_cast_chain_affine_suffix <= 4294967295i64,
            shift_cast_chain_affine_suffix <= 4294967293i64,
            -4611686018427387903i64 <= signed_product_cast_chain_signed_product_suffix,
            signed_product_cast_chain_signed_product_suffix <= 4611686018427387904i64,
            -9223372036854775807i64 <= signed_product_cast_chain_signed_product_suffix,
            -1073741823i64 <= signed_product_cast_chain_signed_product_suffix,
            -536870912i64 <= signed_product_cast_chain_signed_product_suffix,
            signed_product_cast_chain_signed_product_suffix <= 0i64,
            affine_cast_chain_divide_suffix <= 9223372036854775806i64,
            -1i64 <= affine_cast_chain_divide_suffix,
            affine_cast_chain_divide_suffix <= 2147483646i64,
            affine_widen_chain_shift_suffix <= 126i8,
            -63i8 <= signed_product_widen_chain_signed_product_suffix,
            signed_product_widen_chain_signed_product_suffix <= 64i8,
            affine_widen_chain_divide_suffix <= 126i8,
            -1i8 <= affine_widen_cast_shift_suffix,
            affine_widen_cast_shift_suffix <= 126i8,
            -63i8 <= signed_product_widen_cast_signed_product_suffix,
            signed_product_widen_cast_signed_product_suffix <= 64i8,
            -32i8 <= signed_product_widen_cast_signed_product_suffix,
            signed_product_widen_cast_signed_product_suffix <= 31i8,
            -1i8 <= affine_widen_cast_widen_divide_suffix,
            affine_widen_cast_widen_divide_suffix <= 126i8,
            signed_affine_direct <= 124i8,
            -66i8 <= signed_affine_direct, signed_affine_direct <= 61i8,
            -67i8 <= signed_affine_direct, signed_affine_direct <= 60i8,
            signed_affine_cast <= 124i8,
            -66i8 <= signed_affine_cast, signed_affine_cast <= 61i8,
            -67i8 <= signed_affine_cast, signed_affine_cast <= 60i8,
            signed_affine_cast <= -4i8,
            -128i16 <= cast_signed_affine, cast_signed_affine <= 127i16,
            -131i16 <= cast_signed_affine, cast_signed_affine <= 124i16,
            -66i16 <= cast_signed_affine, cast_signed_affine <= 61i16,
            -67i16 <= cast_signed_affine, cast_signed_affine <= 60i16,
            signed_affine_cast_affine_source <= 32764i16,
            -16386i16 <= signed_affine_cast_affine_source,
            signed_affine_cast_affine_source <= 16381i16,
            -16387i16 <= signed_affine_cast_affine_source,
            signed_affine_cast_affine_source <= 16380i16,
            -67i16 <= signed_affine_cast_affine_source,
            signed_affine_cast_affine_source <= 60i16,
            -66i16 <= signed_affine_cast_affine_source,
            -34i16 <= signed_affine_cast_affine_source,
            signed_affine_cast_affine_source <= 29i16,
            affine_cast_signed_affine_source <= 32764i16,
            -16387i16 <= affine_cast_signed_affine_source,
            affine_cast_signed_affine_source <= 16380i16,
            -67i16 <= affine_cast_signed_affine_source,
            affine_cast_signed_affine_source <= 60i16,
            affine_cast_signed_affine_source <= 59i16,
            -36i16 <= affine_cast_signed_affine_source,
            affine_cast_signed_affine_source <= 27i16,
            signed_affine_cast_signed_affine_source <= 32764i16,
            -16386i16 <= signed_affine_cast_signed_affine_source,
            signed_affine_cast_signed_affine_source <= 16381i16,
            -16387i16 <= signed_affine_cast_signed_affine_source,
            signed_affine_cast_signed_affine_source <= 16380i16,
            -67i16 <= signed_affine_cast_signed_affine_source,
            signed_affine_cast_signed_affine_source <= 60i16,
            -65i16 <= signed_affine_cast_signed_affine_source,
            -34i16 <= signed_affine_cast_signed_affine_source,
            signed_affine_cast_signed_affine_source <= 29i16,
            -33i16 <= signed_affine_cast_signed_affine_source,
            signed_affine_cast_signed_affine_source <= 30i16,
            affine_fork_add_join <= 32766i16,
            -16385i16 <= affine_fork_add_join,
            affine_fork_add_join <= 16382i16,
            -32767i16 <= affine_fork_add_join,
            -10921i16 <= affine_fork_add_join,
            affine_fork_add_join <= 10923i16,
            -6553i16 <= affine_fork_add_join,
            affine_fork_add_join <= 6553i16,
            affine_fork_subtract_join <= 32764i16,
            -16386i16 <= affine_fork_subtract_join,
            affine_fork_subtract_join <= 16381i16,
            -32764i16 <= affine_fork_subtract_join,
            -16379i16 <= affine_fork_subtract_join,
            affine_fork_subtract_join <= 16388i16,
            -100i16 <= affine_fork_subtract_join,
            affine_fork_subtract_join <= 100i16,
            distinct_affine_fork_left <= 32766i16,
            -16385i16 <= distinct_affine_fork_left,
            distinct_affine_fork_left <= 16382i16,
            distinct_affine_fork_left <= 32764i16,
            -16386i16 <= distinct_affine_fork_left,
            distinct_affine_fork_left <= 16381i16,
            -32767i16 <= distinct_affine_fork_right,
            -10921i16 <= distinct_affine_fork_right,
            distinct_affine_fork_right <= 10923i16,
            -32764i16 <= distinct_affine_fork_right,
            -16379i16 <= distinct_affine_fork_right,
            distinct_affine_fork_right <= 16388i16,
            -100i16 <= distinct_affine_fork_left,
            distinct_affine_fork_left <= 100i16,
            -100i16 <= distinct_affine_fork_right,
            distinct_affine_fork_right <= 100i16,
            affine_product_join_left <= 32766i16,
            -16385i16 <= affine_product_join_left,
            affine_product_join_left <= 16382i16,
            affine_product_join_left <= 32764i16,
            -16386i16 <= affine_product_join_left,
            affine_product_join_left <= 16381i16,
            -32767i16 <= affine_product_join_right,
            -10921i16 <= affine_product_join_right,
            affine_product_join_right <= 10923i16,
            -32764i16 <= affine_product_join_right,
            -16379i16 <= affine_product_join_right,
            affine_product_join_right <= 16388i16,
            -10i16 <= affine_product_join_left,
            affine_product_join_left <= 10i16,
            -10i16 <= affine_product_join_right,
            affine_product_join_right <= 10i16,
            affine_quadratic_join_root <= 32766i16,
            -16385i16 <= affine_quadratic_join_root,
            affine_quadratic_join_root <= 16382i16,
            -32767i16 <= affine_quadratic_join_root,
            -10921i16 <= affine_quadratic_join_root,
            affine_quadratic_join_root <= 10923i16,
            affine_quadratic_join_root <= 32764i16,
            -16386i16 <= affine_quadratic_join_root,
            affine_quadratic_join_root <= 16381i16,
            -32764i16 <= affine_quadratic_join_root,
            -16380i16 <= affine_quadratic_join_root,
            affine_quadratic_join_root <= 16387i16,
            -10i16 <= affine_quadratic_join_root,
            affine_quadratic_join_root <= 10i16,
            affine_divide_remainder_join_root <= 16383i16,
            -32767i16 <= affine_divide_remainder_join_root,
            affine_divide_remainder_join_root <= 0i16,
            -16384i16 <= affine_divide_remainder_join_root,
            -16385i16 <= affine_divide_remainder_join_root,
            -1i16 <= affine_divide_remainder_join_root,
            affine_divide_remainder_join_root <= 32766i16,
            -16383i16 <= affine_divide_remainder_join_root,
            affine_divide_remainder_join_root <= 16384i16,
            affine_divide_remainder_join_root <= 0i16
        {
            ((((((value >> 1i8) >> 2u16) << 1i32) << 1u64) < 255u8)
                && (((value >> 1i8) << 4u16) < 255u8))
                && ((((signed >> 1u8) << 3i16) < 127i8)
                    && (((((signed >> 7i8) >> 1u16) << 7i32) << 1u64) < 127i8))
                && (((((value >> 7i8) >> 1u16) << 7i32) << 7u64) < 255u8)
                && (((value << 1i8) >> 2u16) < 255u8)
                && (((((value << 1i8) >> 2u16) << 3i32) >> 1u64) < 255u8)
                && (((((wide << 1i8) >> 2u16) << 3i32) as u8) < 255u8)
                && ((((signed_wide >> 1u8) << 2i16) as u8) < 255u8)
                && ((((((post_signed as u8) << 1i8) >> 2u16) << 3i32) < 255u8))
                && (((((post_unsigned as i8) << 1u8) >> 2i16) < 127i8))
                && ((((((affine_unsigned + 3u8) * 2u8) >> 1i8) << 2u16) < 255u8))
                && ((((((affine_signed - -3i8) * 2i8) >> 1u16) << 2i32) < 127i8))
                && ((((((zero_root + 255u8) * 0u8) << 1u8) >> 1i16) < 255u8))
                && ((((((shift_affine_unsigned >> 1i8) << 2u16) + 3u8) * 2u8) < 255u8))
                && ((((((shift_affine_signed >> 1u8) << 2i16) - -3i8) * 2i8) < 127i8))
                && (((((shift_zero_root << 1u8) * 0u8) + 255u8) <= 255u8))
                && (((((sandwich_unsigned >> 1i8) << 2u16) as u8) >> 1i32) << 2u64) < 255u8
                && (((((sandwich_signed >> 1u8) << 2i16) as u8) >> 1u32) << 2i64) < 255u8
                && (((sandwich_right_only << 1u8) as u8) >> 1i16) < 255u8
                && ((((((affine_cast_shift_unsigned + 1u16) * 2u16) as u8) >> 1i8) << 2u32) < 255u8)
                && ((((((affine_cast_shift_signed - -3i16) * 2i16) as u8) >> 1u16) << 2i32) < 255u8)
                && (((((affine_cast_shift_zero + 65535u16) * 0u16) as u8) << 2u8) < 255u8)
                && ((((((shift_cast_affine_unsigned >> 1i8) << 2u16) as u8) + 3u8) * 2u8) < 255u8)
                && ((((((shift_cast_affine_signed >> 1u8) << 2i16) as u8) + 3u8) * 2u8) < 255u8)
                && (((((shift_cast_affine_zero << 1u8) as u8) * 0u8) + 255u8) <= 255u8)
                && ((((divide_cast_affine % 64u16) as i8) + 1i8) < 127i8)
                && ((((divide_cast_shift % 64u16) as u8) << 2u8) < 255u8)
                && (((((divide_cast_shift_signed / 512i16) as i8) >> 1u16) << 1i32) < 127i8)
                && ((((((affine_cast_divide + 1u16) * 2u16) as u8) / 2u8) % 3u8) < 3u8)
                && ((((((shift_cast_remainder >> 1i8) << 2u16) as u8) / 2u8) % 3u8) < 3u8)
                && (((((divide_affine_direct / 2u8) % 64u8) + 1u8) * 2u8) < 255u8)
                && (((((divide_shift_direct / 2u8) % 64u8) >> 1i16) << 2u32) < 255u8)
                && (((((affine_divide_direct + 1u8) * 2u8) / 2u8) % 3u8) < 3u8)
                && (((((shift_remainder_direct >> 1i8) << 2u16) / 2u8) % 3u8) < 3u8)
                && (((((divide_cast_divide % 64u16) as i8) / 2i8) % 3i8) < 3i8)
                && (((((signed_divide_cast_remainder / 512i16) as i8) / 2i8) % 3i8) < 3i8)
                && ((((signed_multiply_chain * -2i8) * 3i8) < 127i8))
                && ((((signed_multiply_cast * -512i16) as i8) < 127i8))
                && (((((signed_cast_multiply as i8) * -2i8) * 0i8) <= 0i8))
                && (((signed_minimum_factor * -9223372036854775808i64) * 1i64) <= 0i64)
                && (((((exact_cast_chain as u64) as i32) as u8) < 255u8))
                && (((((((computed_affine_cast_chain * 2i64) + 1i64) as u64) as i32) as u8) < 255u8))
                && ((((((computed_signed_product_cast_chain * -2i64) as u64) as i32) as u8) < 255u8))
                && ((((((((computed_shift_cast_chain << 1u8) >> 1u16) as u64) as i32) as u8) < 255u8)))
                && ((((((computed_divide_cast_chain / 2u32) % 3u32) as i8) as u8) < 3u8))
                && (((((cast_chain_affine_suffix as u64) as i32) + 1i32) * 2i32) < 2147483647i32)
                && ((((cast_chain_signed_product_suffix as u64) as i32) * -2i32) < 2147483647i32)
                && ((((cast_chain_shift_suffix as u64) as i32) << 1u8) < 2147483647i32)
                && (((((cast_chain_divide_suffix as i8) as u8) / 2u8) % 3u8) < 3u8)
                && (((((affine_cast_chain_shift_suffix + 1i64) as u64) as i32) << 1u8) < 2147483647i32)
                && (((((shift_cast_chain_affine_suffix >> 1u8) as u64) as i32) + 1i32) < 2147483647i32)
                && (((((signed_product_cast_chain_signed_product_suffix * -2i64) as u64) as i32) * -2i32) < 2147483647i32)
                && (((((divide_cast_chain_affine_suffix % 3u32) as u8) as i8) + 1i8) < 127i8)
                && ((((((affine_cast_chain_divide_suffix + 1i64) as u64) as i32) / 2i32) % 3i32) < 3i32)
                && (((((affine_widen_chain_shift_suffix + 1i8) as i16) as i32) << 1u8) < 2147483647i32)
                && (((((shift_widen_chain_affine_suffix >> 1u8) as i16) as i32) + 1i32) < 2147483647i32)
                && (((((signed_product_widen_chain_signed_product_suffix * -2i8) as i16) as i32) * -2i32) < 2147483647i32)
                && (((((remainder_widen_chain_affine_suffix % 3u8) as i16) as i32) + 1i32) < 2147483647i32)
                && ((((((affine_widen_chain_divide_suffix + 1i8) as i16) as i32) / 2i32) % 3i32) < 3i32)
                && (((((affine_widen_cast_shift_suffix + 1i8) as i16) as u8) << 1u8) < 255u8)
                && (((((shift_cast_widen_affine_suffix >> 1u8) as i16) as i32) + 1i32) < 2147483647i32)
                && (((((signed_product_widen_cast_signed_product_suffix * -2i8) as i16) as i8) * -2i8) < 127i8)
                && (((((remainder_cast_widen_affine_suffix % 3u16) as i16) as i32) + 1i32) < 2147483647i32)
                && ((((((((affine_widen_cast_widen_divide_suffix + 1i8) as i16) as u8) as i16) as u8) / 2u8) % 3u8) < 3u8)
                && ((((signed_affine_direct + 3i8) * -2i8) - 1i8) < 127i8)
                && ((((((signed_affine_cast + 3i8) * -2i8) - 1i8) as u8) < 255u8))
                && (((((cast_signed_affine as i8) + 3i8) * -2i8) - 1i8) < 127i8)
                && (((((((signed_affine_cast_affine_source + 3i16) * -2i16) - 1i16) as i8) + 1i8) * 2i8) < 127i8)
                && (((((((affine_cast_signed_affine_source + 3i16) * 2i16) as i8) + 3i8) * -2i8) - 1i8) < 127i8)
                && ((((((((signed_affine_cast_signed_affine_source + 3i16) * -2i16) - 1i16) as i8) + 3i8) * -2i8) - 1i8) < 127i8)
                && (((affine_fork_add_join + 1i16) * 2i16) + ((affine_fork_add_join - 1i16) * 3i16) < 32767i16)
                && (((affine_fork_subtract_join + 3i16) * -2i16) - ((affine_fork_subtract_join - 4i16) * -2i16) < 32767i16)
                && (((distinct_affine_fork_left + 1i16) * 2i16) + ((distinct_affine_fork_right - 1i16) * 3i16) < 32767i16)
                && (((distinct_affine_fork_left + 3i16) * -2i16) - ((distinct_affine_fork_right - 4i16) * -2i16) < 32767i16)
                && ((((affine_product_join_left + 1i16) * 2i16) * ((affine_product_join_right - 1i16) * 3i16)) < 32767i16)
                && ((((affine_product_join_left + 3i16) * -2i16) * ((affine_product_join_right - 4i16) * -2i16)) < 32767i16)
                && ((((affine_quadratic_join_root + 1i16) * 2i16) * ((affine_quadratic_join_root - 1i16) * 3i16)) < 32767i16)
                && ((((affine_quadratic_join_root + 3i16) * -2i16) * ((affine_quadratic_join_root - 4i16) * 2i16)) < 32767i16)
                && ((((affine_divide_remainder_join_root + 16384i16) * -2i16) / ((affine_divide_remainder_join_root * 2i16) + 1i16)) < 32767i16)
                && ((((affine_divide_remainder_join_root - 16383i16) * 2i16) % ((affine_divide_remainder_join_root * 2i16) - 1i16)) < 32767i16)
                && enabled
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize mixed shifts");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed shifts");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed shifts");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type mixed shifts");
    let checked = lower_typed_trees(typed).expect("check mixed shifts");
    let lowered = lower_machine(&checked, "Root::measure")
        .expect("mixed shifts reach terminal Psi with nominal cleanup");
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftRight { .. }
            ))
            .count(),
        37,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftLeft { .. }
            ))
            .count(),
        44,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::IntegerExactCast { .. }))
            .count(),
        65,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::IntegerWiden { .. }))
            .count(),
        16,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }))
            .count(),
        48,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerSubtract { .. }
            ))
            .count(),
        22,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerMultiply { .. }
            ))
            .count(),
        65,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::ExactIntegerDivide { .. }))
            .count(),
        16,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerRemainder { .. }
            ))
            .count(),
        20,
    );
    verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed-shift proofs verify independently");
    let semantics = encode_module(&lowered.semantic_module).expect("mixed-shift semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("mixed-shift proof");
    let abstract_plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("mixed shifts cross the Omega boundary");
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerShiftRight { .. }
            ))
            .count(),
        37,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerShiftLeft { .. }
            ))
            .count(),
        44,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::IntegerExactCast { .. }
            ))
            .count(),
        65,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(operation, TerminalAbstractOperation::IntegerWiden { .. }))
            .count(),
        16,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| {
                matches!(
                    operation,
                    TerminalAbstractOperation::WrappingIntegerAdd { .. }
                )
            })
            .count(),
        48,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| {
                matches!(
                    operation,
                    TerminalAbstractOperation::WrappingIntegerSubtract { .. }
                )
            })
            .count(),
        22,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| {
                matches!(
                    operation,
                    TerminalAbstractOperation::WrappingIntegerMultiply { .. }
                )
            })
            .count(),
        65,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerDivide { .. }
            ))
            .count(),
        16,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerRemainder { .. }
            ))
            .count(),
        20,
    );
    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering: {error:?}", case.target));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment: {error:?}", case.target));
        emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission: {error:?}", case.target));
    }
}

#[test]
fn affine_cast_affine_sandwich_emits_on_every_native_target() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}
        data Token { value: u64; }
        machine Token::drop(&mut self) { Helper::touch(); }
        data Root {}
        machine Root::measure(
            token: Token,
            unsigned: u16,
            signed: i16,
            pre_zero: u16,
            post_zero: u16,
            enabled: bool
        ) -> bool
        requires unsigned <= 65532u16, unsigned <= 32764u16,
            unsigned <= 124u16, unsigned <= 61u16,
            signed <= 32764i16, -16387i16 <= signed, signed <= 16380i16,
            -67i16 <= signed, signed <= 60i16,
            -35i16 <= signed, signed <= 28i16,
            post_zero <= 65534u16, post_zero <= 254u16
        {
            ((((((unsigned + 3u16) * 2u16) as u8) - 1u8) * 2u8) < 255u8)
                && ((((((signed - -3i16) * 2i16) as i8) + 1i8) * 2i8) < 127i8)
                && ((((pre_zero * 0u16) as u8) + 255u8) <= 255u8)
                && (((((post_zero + 1u16) as u8) * 0u8) + 255u8) <= 255u8)
                && enabled
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize affine-cast-affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse affine-cast-affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve affine-cast-affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type affine-cast-affine source");
    let checked = lower_typed_trees(typed).expect("check affine-cast-affine source");
    let lowered =
        lower_machine(&checked, "Root::measure").expect("affine-cast-affine reaches Terminal Psi");
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::IntegerExactCast { .. }))
            .count(),
        4,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }))
            .count(),
        5,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerSubtract { .. }
            ))
            .count(),
        2,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerMultiply { .. }
            ))
            .count(),
        6,
    );
    verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("affine-cast-affine proofs verify independently");
    let semantics = encode_module(&lowered.semantic_module).expect("sandwich semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("sandwich proof");
    let abstract_plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("affine-cast-affine crosses the Omega boundary");
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::IntegerExactCast { .. }
            ))
            .count(),
        4,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerAdd { .. }
            ))
            .count(),
        5,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerSubtract { .. }
            ))
            .count(),
        2,
    );
    assert_eq!(
        abstract_plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerMultiply { .. }
            ))
            .count(),
        6,
    );
    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering: {error:?}", case.target));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment: {error:?}", case.target));
        emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission: {error:?}", case.target));
    }
}

#[test]
fn nominal_integer_comparison_convergence_has_one_physical_cleanup_tail_on_all_targets() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}
        data Token { retained: bool; }
        machine Token::drop(&mut self) { Helper::touch(); }
        data Root {}
        machine Root::measure(
            token: Token,
            input: u64 in Wrapping,
            small: u8,
            divisor: u8,
            count: u8,
            signed: i64,
            signed_arithmetic: i8,
            signed_divisor: i8,
            negative_divisor: i8,
            bounded_negative_divisor: i8,
            add_left: u8,
            add_right: u8,
            positive_addend: i8,
            negative_addend: i8,
            positive_subtrahend: i8,
            negative_subtrahend: i8,
            signed_count: i8,
            enabled: bool,
            wide: u16
        ) -> bool
        requires input <= 255u64, input <= 250u64, input <= 253u64, input <= 252u64,
            input <= 251u64, input <= 127u64, input <= 125u64, input <= 124u64,
            input <= 42u64, input <= 31u64,
            5u64 <= input, input <= 260u64,
            small <= 254u8, small <= 253u8, small <= 252u8,
            small <= 127u8, small <= 125u8, small <= 124u8, small <= 61u8,
            small <= 63u8, small <= 42u8, small <= 31u8,
            small <= 21u8, small <= 15u8,
            small <= 7u8, 1u8 <= small, 2u8 <= small, 3u8 <= small,
            1u8 <= divisor, divisor <= small,
            small <= 255u8 / divisor, count <= 2u8,
            -128i64 <= signed, signed <= 127i64,
            -125i64 <= signed, signed <= 130i64,
            -61i64 <= signed, signed <= 66i64,
            -64i64 <= signed, signed <= 63i64, -21i64 <= signed, signed <= 21i64,
            -16i64 <= signed, signed <= 15i64,
            -127i8 <= signed_arithmetic, signed_arithmetic <= 126i8,
            -126i8 <= signed_arithmetic, -125i8 <= signed_arithmetic,
            signed_arithmetic <= 124i8,
            -42i8 <= signed_arithmetic, signed_arithmetic <= 42i8,
            -61i8 <= signed_arithmetic, signed_arithmetic <= 66i8,
            -32i8 <= signed_arithmetic, signed_arithmetic <= 31i8,
            -3i8 <= signed_arithmetic, -1i8 <= signed_arithmetic, 0i8 <= signed_arithmetic,
            3i8 <= signed_arithmetic,
            1i8 <= signed_arithmetic, 0i8 <= signed_divisor,
            1i8 <= signed_divisor, signed_divisor <= 7i8,
            -128i8 / signed_divisor <= signed_arithmetic,
            signed_arithmetic <= 127i8 / signed_divisor,
            negative_divisor <= -2i8, bounded_negative_divisor <= -1i8,
            127i8 / negative_divisor <= signed_arithmetic,
            signed_arithmetic <= -128i8 / negative_divisor,
            add_left <= 255u8 - add_right,
            0i8 <= positive_addend, signed_arithmetic <= 127i8 - positive_addend,
            negative_addend <= 0i8, -128i8 - negative_addend <= signed_arithmetic,
            0i8 <= positive_subtrahend, -128i8 + positive_subtrahend <= signed_arithmetic,
            negative_subtrahend <= 0i8, signed_arithmetic <= 127i8 + negative_subtrahend,
            0i8 <= signed_count, signed_count <= 2i8
        {
            let staged: bool = (((~input) < 1u64) || ((input + 1u64) < 7u64))
                && (((input + 1u64) + 1u64) < 5u64)
                && ((small as u16) < 5u16)
                && ((input as u8) < 5u8)
                && (((input as u8) as u16) < 256u16)
                && (((small as u16) as u8) < 6u8)
                && (((((small as u16) as u32) as u64) as u8) < 7u8)
                && ((small + 1u8) < 6u8)
                && ((((small + 1u8) + 1u8) + 1u8) < 8u8)
                && ((~(small + 3u8)) < 255u8)
                && (((small - 3u8) as u16) < 255u16)
                && ((((small - 1u8) - 1u8) - 1u8) < 5u8)
                && ((15u8 & (small * 2u8)) < 16u8)
                && ((~((small + 3u8) as u16)) < 65535u16)
                && (((small + 1u8) & (small * 2u8)) < 255u8)
                && ((127u8 - small) < 125u8)
                && ((small - divisor) < 4u8)
                && ((small * 2u8) < 10u8)
                && ((((small * 2u8) * 3u8) * 1u8) < 255u8)
                && (((((small + 3u8) * 2u8) - 1u8) < 255u8))
                && (((((small + 3u8) * 0u8) + 255u8) < 255u8))
                && (((((signed_arithmetic + -3i8) * 2i8) - -1i8) < 127i8))
                && (((((((small + 3u8) * 2u8) - 1u8) as i8) < 127i8)
                    && (((((small + 3u8) * 0u8) + 127u8) as i8) < 127i8))
                    && (((((signed_arithmetic - 3i8) * 2i8) + 1i8) as u8) < 255u8))
                && (((((small * 2u8) * 3u8) as i8) < 127i8))
                && (((((small * 2u8) * 0u8) as i8) < 127i8))
                && ((small * divisor) < 50u8)
            && (((small / 2u8) < 3u8)
                && ((small % 2u8) <= 1u8)
                && (((((small / 2u8) % 3u8) / 2u8) < 2u8)
                && (((((input as u8) / 2u8) % 3u8) / 2u8) < 2u8)
                && ((((signed as i8) / 2i8) % -3i8) < 3i8)
                && ((((signed_arithmetic as u8) / 2u8) % 3u8) < 3u8)
                && ((((wide / 256u16) as u8) < 255u8)
                    && ((((wide / 2u16) % 3u16) as u8) < 3u8)
                    && (((signed % -3i64) as i8) < 3i8)
                    && (((wide % 3u16) as i8) < 3i8)
                    && ((((small / divisor) % 2u8) < 2u8)
                        && ((((input as u8) / divisor) % 2u8) < 2u8)
                        && (((signed_arithmetic / signed_divisor) % -3i8) < 3i8)
                        && (((signed_arithmetic / negative_divisor) % 3i8) < 3i8)
                        && ((((signed as i8) / signed_divisor) % -3i8) < 3i8)
                        && ((((signed as i8) / negative_divisor) % 3i8) < 3i8)))))
                && ((small / divisor) < 6u8)
                && ((small % divisor) <= small)
                && ((small >> small) < 1u8)
                && ((signed_arithmetic >> signed_divisor) < 4i8)
                && ((((small >> 1i8) >> 2u16) >> 0i32) < 2u8)
                && (((((small >> 1i8) >> 2u16) >> 0i32) as i8) < 127i8)
                && (((small >> 0i8) as i8) < 127i8)
                && ((((small << 1i8) << 2u16) << 0i32) < 255u8)
                && (((((small << 1i8) << 2u16) << 0i32) as i8) < 127i8)
                && (((small << 0i8) as i8) < 127i8)
                && ((small << 1u8) < 11u8)
                && ((small << count) < 29u8)
                && ((small << signed_count) < 255u8)
                && ((signed_arithmetic << 2u8) < 127i8)
                && ((signed_arithmetic << count) < 127i8)
                && ((signed_arithmetic << signed_count) < 127i8)
                && ((signed as i8) < 4i8)
                && ((small as i8) < 4i8)
                && ((signed_arithmetic as u8) < 4u8)
                && ((signed_arithmetic + 1i8) < 4i8)
                && ((signed_arithmetic + -1i8) < 4i8)
                && ((signed_arithmetic - 1i8) < 4i8)
                && ((signed_arithmetic - -1i8) < 4i8)
                && ((((small + 3u8) - 2u8) + 1u8) < 255u8)
                && ((((signed_arithmetic - -3i8) + -5i8) - -1i8) < 127i8)
                && (((((small + 3u8) - 2u8) + 1u8) as i8) < 127i8)
                && (((((signed_arithmetic - -3i8) + -5i8) - -1i8) as u8) < 127u8)
                && (((input as u8) + 5u8) < 255u8)
                && (((input as u8) - 5u8) < 255u8)
                && (((((input as u8) + 5u8) - 3u8) + 2u8) < 255u8)
                && ((((input as u8) + 5u8) - 5u8) < 255u8)
                && (((signed_arithmetic as u8) + 1u8) < 255u8)
                && ((((signed_arithmetic as u8) + 3u8) - 2u8) < 255u8)
                && ((((input as u8) * 2u8) * 3u8) < 255u8)
                && ((((input as u8) * 2u8) * 0u8) < 255u8)
                && ((((((input as u8) + 3u8) * 2u8) - 1u8) < 255u8)
                    && (((((input as u8) + 3u8) * 0u8) + 255u8) < 255u8)
                    && (((((signed as i8) - 3i8) * 2i8) + 1i8) < 127i8))
                && ((((signed as i8) * 2i8) * 3i8) < 127i8)
                && ((((signed_arithmetic as u8) * 2u8) * 3u8) < 255u8)
                && ((((small as i8) * 2i8) * 3i8) < 127i8)
                && (((((input as u8) << 1i8) << 2u16) << 0i32) < 255u8)
                && ((((signed as i8) << 1u16) << 2i32) < 127i8)
                && ((((signed_arithmetic as u8) << 1i8) << 2u16) < 255u8)
                && (((((small as i8) << 1u16) << 2i32) < 127i8)
                    && (((((input as u8) >> 1i8) >> 2u16) >> 0i32) < 255u8)
                    && ((((signed as i8) >> 1u16) >> 2i32) < 127i8)
                    && ((((signed_arithmetic as u8) >> 1i8) >> 2u16) < 255u8))
                && ((signed_arithmetic * 3i8) < 4i8)
                && ((signed_arithmetic * -3i8) < 4i8)
                && ((signed_arithmetic * signed_divisor) <= 127i8)
                && ((signed_arithmetic * negative_divisor) <= 127i8)
                && ((signed_arithmetic / 2i8) < 4i8)
                && ((signed_arithmetic % -2i8) <= 1i8)
                && ((signed_arithmetic / signed_divisor) < 4i8)
                && ((signed_arithmetic % signed_divisor) <= signed_arithmetic)
                && ((signed_arithmetic / negative_divisor) < 4i8)
                && ((signed_arithmetic % negative_divisor) <= signed_arithmetic)
                && ((signed_arithmetic / bounded_negative_divisor) < 4i8)
                && ((signed_arithmetic % bounded_negative_divisor) <= signed_arithmetic)
                && ((add_left + add_right) <= 255u8)
                && ((signed_arithmetic + positive_addend) <= 127i8)
                && ((signed_arithmetic + negative_addend) < 4i8)
                && ((signed_arithmetic - positive_subtrahend) < 4i8)
                && ((signed_arithmetic - negative_subtrahend) <= 127i8)
                && enabled;
            staged
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize integer-comparison convergence");
    let syntax = parse_syntax_trees(&tokens).expect("parse integer-comparison convergence");
    let resolved = lower_syntax_trees(&syntax).expect("resolve integer-comparison convergence");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type integer-comparison convergence");
    let checked = lower_typed_trees(typed).expect("check integer-comparison convergence");
    let lowered =
        lower_machine(&checked, "Root::measure").expect("lower integer-comparison convergence");
    let terminal_entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("terminal integer-comparison convergence entry");
    assert!(terminal_entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerLessThan { .. }))
    }));
    assert!(terminal_entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::WrappingIntegerAdd { .. }))
    }));
    assert!(terminal_entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerBitwiseNot { .. }))
    }));
    assert!(terminal_entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerWiden { .. }))
    }));
    let cast_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the guarded exact cast");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == cast_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let signed_parameter = terminal_entry.parameters[4].id;
    let signed_cast_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } if operand == signed_parameter => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the signed guarded exact cast");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == signed_cast_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let cross_sign_cast_obligations = [
        terminal_entry.parameters[1].id,
        terminal_entry.parameters[5].id,
    ]
    .into_iter()
    .map(|parameter| {
        terminal_entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::IntegerExactCast {
                    operand,
                    obligation,
                } if operand == parameter => Some(obligation),
                _ => None,
            })
            .expect("shared convergence retains each cross-sign guarded exact cast")
    });
    for obligation in cross_sign_cast_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_arithmetic_parameter = terminal_entry.parameters[5].id;
    let signed_add_sites = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } if left == signed_arithmetic_parameter => terminal_entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then(|| match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .map(|addend| (obligation, addend)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        signed_add_sites
            .iter()
            .any(|(_, addend)| *addend == IntegerValue::Signed(1))
    );
    assert!(
        signed_add_sites
            .iter()
            .any(|(_, addend)| *addend == IntegerValue::Signed(-1))
    );
    for (obligation, _) in signed_add_sites {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_subtract_sites = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } if left == signed_arithmetic_parameter => terminal_entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then(|| match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .map(|subtrahend| (obligation, subtrahend)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        signed_subtract_sites
            .iter()
            .any(|(_, subtrahend)| *subtrahend == IntegerValue::Signed(1))
    );
    assert!(
        signed_subtract_sites
            .iter()
            .any(|(_, subtrahend)| *subtrahend == IntegerValue::Signed(-1))
    );
    for (obligation, _) in signed_subtract_sites {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_multiply_sites = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            } if left == signed_arithmetic_parameter => terminal_entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then(|| match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .map(|factor| (obligation, factor)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        signed_multiply_sites
            .iter()
            .any(|(_, factor)| *factor == IntegerValue::Signed(3))
    );
    assert!(
        signed_multiply_sites
            .iter()
            .any(|(_, factor)| *factor == IntegerValue::Signed(-3))
    );
    for (obligation, _) in signed_multiply_sites {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_division_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                left, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                left, obligation, ..
            } if left == signed_arithmetic_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        terminal_entry
            .blocks
            .iter()
            .any(|block| block.operations.iter().any(
                |operation| matches!(operation.kind, OperationKind::ExactIntegerDivide { left, .. }
            if left == signed_arithmetic_parameter)
            ))
    );
    assert!(terminal_entry.blocks.iter().any(|block| {
        block.operations.iter().any(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerRemainder { left, .. }
            if left == signed_arithmetic_parameter)
        })
    }));
    assert!(signed_division_obligations.len() >= 2);
    for obligation in signed_division_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_divisor_parameter = terminal_entry.parameters[6].id;
    let runtime_signed_division_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                right, obligation, ..
            } if right == signed_divisor_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(runtime_signed_division_obligations.len() >= 2);
    for obligation in runtime_signed_division_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let negative_divisor_parameter = terminal_entry.parameters[7].id;
    let runtime_negative_signed_division_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                right, obligation, ..
            } if right == negative_divisor_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(runtime_negative_signed_division_obligations.len() >= 2);
    for obligation in runtime_negative_signed_division_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let bounded_negative_divisor_parameter = terminal_entry.parameters[8].id;
    let runtime_bounded_negative_signed_division_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                right, obligation, ..
            } if right == bounded_negative_divisor_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(runtime_bounded_negative_signed_division_obligations.len() >= 2);
    for obligation in runtime_bounded_negative_signed_division_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let runtime_exact_add_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } if left == terminal_entry.parameters[9].id
                && right == terminal_entry.parameters[10].id =>
            {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the computed-bound runtime addition");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_exact_add_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    for addend in [
        terminal_entry.parameters[11].id,
        terminal_entry.parameters[12].id,
    ] {
        let obligation = terminal_entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation,
                } if left == terminal_entry.parameters[5].id && right == addend => Some(obligation),
                _ => None,
            })
            .expect("shared convergence retains each signed computed-bound runtime addition");
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    for subtrahend in [
        terminal_entry.parameters[13].id,
        terminal_entry.parameters[14].id,
    ] {
        let obligation = terminal_entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation,
                } if left == terminal_entry.parameters[5].id && right == subtrahend => {
                    Some(obligation)
                }
                _ => None,
            })
            .expect("shared convergence retains each signed computed-bound runtime subtraction");
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let exact_divide_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains exact division by a nonzero constant");
    let exact_remainder_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerRemainder { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains exact remainder by a nonzero constant");
    for obligation in [exact_divide_obligation, exact_remainder_obligation] {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let divisor_parameter = terminal_entry.parameters[2].id;
    let runtime_divisor_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                right, obligation, ..
            } if right == divisor_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(runtime_divisor_obligations.len() >= 2);
    for obligation in runtime_divisor_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let exact_shift_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftRight { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact right shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_shift_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_shift_left_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact left shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_shift_left_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let signed_count_exact_shift_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            } if value == terminal_entry.parameters[5].id
                && count == terminal_entry.parameters[6].id =>
            {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the signed-count exact right shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == signed_count_exact_shift_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let count_parameter = terminal_entry.parameters[3].id;
    let runtime_shift_left_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft {
                count, obligation, ..
            } if count == count_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!runtime_shift_left_obligations.is_empty());
    for obligation in runtime_shift_left_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let runtime_signed_count_shift_left_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            } if value == terminal_entry.parameters[1].id
                && count == terminal_entry.parameters[15].id =>
            {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the signed-count runtime exact left shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_signed_count_shift_left_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let bitwise_not_exact_add_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } if left == terminal_entry.parameters[1].id => terminal_entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then(|| match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .filter(|value| *value == IntegerValue::Unsigned(3))
                .map(|_| obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!bitwise_not_exact_add_obligations.is_empty());
    for obligation in bitwise_not_exact_add_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let widen_exact_subtract_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } if left == terminal_entry.parameters[1].id => terminal_entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then(|| match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .filter(|value| *value == IntegerValue::Unsigned(3))
                .map(|_| obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!widen_exact_subtract_obligations.is_empty());
    for obligation in widen_exact_subtract_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_value_shift_left_obligations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft {
                value, obligation, ..
            } if value == terminal_entry.parameters[5].id => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(signed_value_shift_left_obligations.len() >= 3);
    for obligation in signed_value_shift_left_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let exact_multiply_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact multiplication");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_multiply_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let runtime_exact_multiply_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            } if left == terminal_entry.parameters[1].id
                && right == terminal_entry.parameters[2].id =>
            {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the computed-bound runtime multiplication");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_exact_multiply_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    for factor in [
        terminal_entry.parameters[6].id,
        terminal_entry.parameters[7].id,
    ] {
        let obligation = terminal_entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ExactIntegerMultiply {
                    left,
                    right,
                    obligation,
                } if left == terminal_entry.parameters[5].id && right == factor => Some(obligation),
                _ => None,
            })
            .expect("shared convergence retains each signed quotient-bound multiplication");
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let exact_subtract_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact subtraction");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_subtract_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let runtime_exact_subtract_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } if left == terminal_entry.parameters[1].id
                && right == terminal_entry.parameters[2].id =>
            {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the relationally proven runtime subtraction");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_exact_subtract_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_add_obligation = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the proven exact addition");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_add_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let operations = terminal_entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let has_u8_constant = |id, expected| {
        operations.iter().any(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(id)
                && matches!(
                    operation.kind,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(value)
                    } if value == expected
                )
        })
    };
    let has_integer_constant = |id, scalar_type, expected| {
        operations.iter().any(|operation| {
            operation
                .result
                .scalar_ref()
                .map(|result| (result.id, result.scalar_type))
                == Some((id, ScalarType::Integer(scalar_type)))
                && matches!(
                    operation.kind,
                    OperationKind::IntegerConstant { value } if value == expected
                )
        })
    };
    let mixed_add_subtract_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 1) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 2) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[1].id && has_u8_constant(inner_right, 3))
                .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("native path retains the finite mixed exact-add/subtract chain");
    for obligation in mixed_add_subtract_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let offset_chain_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            if cast.result.scalar_ref().map(|result| result.scalar_type)
                != Some(ScalarType::Integer(i8_type))
            {
                return None;
            }
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 1) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 2) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[1].id && has_u8_constant(inner_right, 3))
                .then_some([
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                    cast_obligation,
                ])
        })
        .expect("native path retains one exact narrowing after the complete offset chain");
    for obligation in offset_chain_cast_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let find_cast_then_offset = |subtract: bool| {
        operations.iter().find_map(|outer| {
            let (left, right, arithmetic_obligation) = match outer.kind {
                OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation,
                } if !subtract => (left, right, obligation),
                OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation,
                } if subtract => (left, right, obligation),
                _ => return None,
            };
            if !has_u8_constant(right, 5) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == terminal_entry.parameters[0].id)
                .then_some([cast_obligation, arithmetic_obligation])
        })
    };
    let cast_then_add_obligations = find_cast_then_offset(false)
        .expect("native path retains one direct exact cast feeding exact addition");
    let cast_then_subtract_obligations = find_cast_then_offset(true)
        .expect("native path retains one direct exact cast feeding exact subtraction");
    for obligations in [cast_then_add_obligations, cast_then_subtract_obligations] {
        assert_ne!(obligations[0], obligations[1]);
        for obligation in obligations {
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let finite_cast_then_offset_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 2) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 3) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !has_u8_constant(inner_right, 5) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == terminal_entry.parameters[0].id).then_some([
                cast_obligation,
                inner_obligation,
                middle_obligation,
                outer_obligation,
            ])
        })
        .expect("native path retains a finite exact-cast-then-offset chain");
    let cancelling_cast_then_offset_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 5) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !has_u8_constant(inner_right, 5) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == terminal_entry.parameters[0].id).then_some([
                cast_obligation,
                inner_obligation,
                outer_obligation,
            ])
        })
        .expect("native path retains every obligation through cancellation");
    for obligations in [
        finite_cast_then_offset_obligations.as_slice(),
        cancelling_cast_then_offset_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let find_cast_then_multiply_chain = |outer_factor| {
        operations.iter().find_map(|outer| {
            let OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, outer_factor) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !has_u8_constant(inner_right, 2) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == terminal_entry.parameters[0].id).then_some([
                cast_obligation,
                inner_obligation,
                outer_obligation,
            ])
        })
    };
    let cast_then_multiply_obligations = find_cast_then_multiply_chain(3)
        .expect("native path retains one complete post-cast exact-multiply chain");
    let zero_cast_then_multiply_obligations = find_cast_then_multiply_chain(0)
        .expect("native path retains every post-cast prefix through a zero factor");
    for obligations in [
        cast_then_multiply_obligations.as_slice(),
        zero_cast_then_multiply_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let find_multiply_chain_then_cast = |outer_factor| {
        operations.iter().find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, outer_factor) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !has_u8_constant(inner_right, 2) {
                return None;
            }
            (inner_left == terminal_entry.parameters[1].id).then_some([
                inner_obligation,
                outer_obligation,
                cast_obligation,
            ])
        })
    };
    let multiply_chain_then_cast_obligations = find_multiply_chain_then_cast(3)
        .expect("native path retains one complete pre-cast exact-multiply chain");
    let zero_multiply_chain_then_cast_obligations = find_multiply_chain_then_cast(0)
        .expect("native path retains every pre-cast prefix through a zero product and cast");
    for obligations in [
        multiply_chain_then_cast_obligations.as_slice(),
        zero_multiply_chain_then_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let nested_multiply_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 1) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 3) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[1].id && has_u8_constant(inner_right, 2))
                .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("native path retains the finite exact-multiply chain");
    for obligation in nested_multiply_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let affine_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 1) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 2) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[1].id && has_u8_constant(inner_right, 3))
                .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("native path retains one mixed exact-affine chain");
    let zero_affine_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 255) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 0) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[1].id && has_u8_constant(inner_right, 3))
                .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("native path retains every affine prefix through a later zero factor");
    let signed_affine_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(right, i8_type, IntegerValue::Signed(-1)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_integer_constant(middle_right, i8_type, IntegerValue::Signed(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[5].id
                && has_integer_constant(inner_right, i8_type, IntegerValue::Signed(-3)))
            .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("native path retains one signed mixed exact-affine chain");
    for obligations in [
        affine_obligations.as_slice(),
        zero_affine_obligations.as_slice(),
        signed_affine_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let affine_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            if cast.result.scalar_ref().map(|result| result.scalar_type)
                != Some(ScalarType::Integer(i8_type))
            {
                return None;
            }
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 1) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 2) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[1].id && has_u8_constant(inner_right, 3))
                .then_some([
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                    cast_obligation,
                ])
        })
        .expect("native path retains one affine chain before an exact cast");
    let zero_affine_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            if cast.result.scalar_ref().map(|result| result.scalar_type)
                != Some(ScalarType::Integer(i8_type))
            {
                return None;
            }
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 127) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 0) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[1].id && has_u8_constant(inner_right, 3))
                .then_some([
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                    cast_obligation,
                ])
        })
        .expect("native path retains every affine/cast proof through zero collapse");
    let signed_affine_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            if cast.result.scalar_ref().map(|result| result.scalar_type)
                != Some(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                ))
            {
                return None;
            }
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(right, i8_type, IntegerValue::Signed(1)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_integer_constant(middle_right, i8_type, IntegerValue::Signed(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[5].id
                && has_integer_constant(inner_right, i8_type, IntegerValue::Signed(3)))
            .then_some([
                inner_obligation,
                middle_obligation,
                outer_obligation,
                cast_obligation,
            ])
        })
        .expect("native path retains one signed affine chain before a cross-sign cast");
    for obligations in [
        affine_cast_obligations.as_slice(),
        zero_affine_cast_obligations.as_slice(),
        signed_affine_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let find_cast_then_affine = |signed: bool, zero: bool| {
        operations.iter().find_map(|outer| {
            let (left, right, outer_obligation) = match outer.kind {
                OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation,
                } if signed || zero => (left, right, obligation),
                OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation,
                } if !signed && !zero => (left, right, obligation),
                _ => return None,
            };
            let outer_matches = if signed {
                has_integer_constant(right, i8_type, IntegerValue::Signed(1))
            } else if zero {
                has_u8_constant(right, 255)
            } else {
                has_u8_constant(right, 1)
            };
            if !outer_matches {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            let factor_matches = if signed {
                has_integer_constant(middle_right, i8_type, IntegerValue::Signed(2))
            } else if zero {
                has_u8_constant(middle_right, 0)
            } else {
                has_u8_constant(middle_right, 2)
            };
            if !factor_matches {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let (inner_left, inner_right, inner_obligation) = match inner.kind {
                OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation,
                } if signed => (left, right, obligation),
                OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation,
                } if !signed => (left, right, obligation),
                _ => return None,
            };
            let inner_matches = if signed {
                has_integer_constant(inner_right, i8_type, IntegerValue::Signed(3))
            } else {
                has_u8_constant(inner_right, 3)
            };
            if !inner_matches {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let parameter = if signed {
                terminal_entry.parameters[4].id
            } else {
                terminal_entry.parameters[0].id
            };
            (operand == parameter).then_some([
                cast_obligation,
                inner_obligation,
                middle_obligation,
                outer_obligation,
            ])
        })
    };
    let cast_then_affine_obligations = find_cast_then_affine(false, false)
        .expect("native path retains one post-cast mixed affine chain");
    let zero_cast_then_affine_obligations = find_cast_then_affine(false, true)
        .expect("native path retains all post-cast affine proofs through zero collapse");
    let signed_cast_then_affine_obligations = find_cast_then_affine(true, false)
        .expect("native path retains one signed post-cast mixed affine chain");
    for obligations in [
        cast_then_affine_obligations.as_slice(),
        zero_cast_then_affine_obligations.as_slice(),
        signed_cast_then_affine_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let nested_divide_remainder_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 2) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerRemainder {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 3) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == terminal_entry.parameters[1].id && has_u8_constant(inner_right, 2))
                .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("native path retains the finite mixed exact-divide/remainder chain");
    for obligation in nested_divide_remainder_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let has_integer_constant = |id, expected| {
        operations.iter().any(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(id)
                && matches!(
                    operation.kind,
                    OperationKind::IntegerConstant { value } if value == expected
                )
        })
    };
    let cast_then_divide_remainder_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_u8_constant(right, 2) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerRemainder {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_u8_constant(middle_right, 3) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !has_u8_constant(inner_right, 2) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == terminal_entry.parameters[0].id).then_some([
                cast_obligation,
                inner_obligation,
                middle_obligation,
                outer_obligation,
            ])
        })
        .expect("native path retains one post-cast divide/remainder chain");
    let find_two_link_cast_then_divide_remainder = |parameter, signed| {
        operations.iter().find_map(|outer| {
            let OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            let outer_matches = if signed {
                has_integer_constant(right, IntegerValue::Signed(-3))
            } else {
                has_u8_constant(right, 3)
            };
            if !outer_matches {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            let inner_matches = if signed {
                has_integer_constant(inner_right, IntegerValue::Signed(2))
            } else {
                has_u8_constant(inner_right, 2)
            };
            if !inner_matches {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == parameter).then_some([cast_obligation, inner_obligation, outer_obligation])
        })
    };
    let signed_cast_then_divide_remainder_obligations =
        find_two_link_cast_then_divide_remainder(terminal_entry.parameters[4].id, true)
            .expect("native path retains one signed post-cast divide/remainder chain");
    let cross_cast_then_divide_remainder_obligations =
        find_two_link_cast_then_divide_remainder(terminal_entry.parameters[5].id, false)
            .expect("native path retains one cross-sign post-cast divide/remainder chain");
    for obligations in [
        cast_then_divide_remainder_obligations.as_slice(),
        signed_cast_then_divide_remainder_obligations.as_slice(),
        cross_cast_then_divide_remainder_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let wide_parameter = terminal_entry.parameters[17].id;
    let find_cast_after_divide_remainder = |parameter, divisor, remainder: bool| {
        operations.iter().find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let arithmetic = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let (left, right, arithmetic_obligation) = match arithmetic.kind {
                OperationKind::ExactIntegerDivide {
                    left,
                    right,
                    obligation,
                } if !remainder => (left, right, obligation),
                OperationKind::ExactIntegerRemainder {
                    left,
                    right,
                    obligation,
                } if remainder => (left, right, obligation),
                _ => return None,
            };
            (left == parameter && has_integer_constant(right, divisor))
                .then_some([arithmetic_obligation, cast_obligation])
        })
    };
    let divide_chain_cast_obligations =
        find_cast_after_divide_remainder(wide_parameter, IntegerValue::Unsigned(256), false)
            .expect("native path retains one carrier-total divide feeding an exact cast");
    let mixed_divide_remainder_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let remainder = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: remainder_obligation,
            } = remainder.kind
            else {
                return None;
            };
            if !has_integer_constant(right, IntegerValue::Unsigned(3)) {
                return None;
            }
            let divide = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: divide_left,
                right: divide_right,
                obligation: divide_obligation,
            } = divide.kind
            else {
                return None;
            };
            (divide_left == wide_parameter
                && has_integer_constant(divide_right, IntegerValue::Unsigned(2)))
            .then_some([divide_obligation, remainder_obligation, cast_obligation])
        })
        .expect("native path retains one carrier-total mixed chain feeding an exact cast");
    let signed_remainder_cast_obligations = find_cast_after_divide_remainder(
        terminal_entry.parameters[4].id,
        IntegerValue::Signed(-3),
        true,
    )
    .expect("native path retains one signed carrier-total remainder feeding an exact cast");
    let cross_remainder_cast_obligations =
        find_cast_after_divide_remainder(wide_parameter, IntegerValue::Unsigned(3), true).expect(
            "native path retains one cross-sign carrier-total remainder feeding an exact cast",
        );
    for obligations in [
        divide_chain_cast_obligations.as_slice(),
        mixed_divide_remainder_cast_obligations.as_slice(),
        signed_remainder_cast_obligations.as_slice(),
        cross_remainder_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let find_direct_runtime_divisor_chain = |root, runtime_divisor, outer_value| {
        operations.iter().find_map(|outer| {
            let OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(right, outer_value) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == root && inner_right == runtime_divisor)
                .then_some([inner_obligation, outer_obligation])
        })
    };
    let direct_unsigned_runtime_divisor_obligations = find_direct_runtime_divisor_chain(
        terminal_entry.parameters[1].id,
        terminal_entry.parameters[2].id,
        IntegerValue::Unsigned(2),
    )
    .expect("native path retains one direct unsigned runtime-divisor chain");
    let direct_signed_positive_runtime_divisor_obligations = find_direct_runtime_divisor_chain(
        terminal_entry.parameters[5].id,
        terminal_entry.parameters[6].id,
        IntegerValue::Signed(-3),
    )
    .expect("native path retains one direct signed-positive runtime-divisor chain");
    let direct_signed_negative_runtime_divisor_obligations = find_direct_runtime_divisor_chain(
        terminal_entry.parameters[5].id,
        terminal_entry.parameters[7].id,
        IntegerValue::Signed(3),
    )
    .expect("native path retains one direct signed-negative runtime-divisor chain");
    let find_post_cast_runtime_divisor_chain = |root, runtime_divisor, outer_value| {
        operations.iter().find_map(|outer| {
            let OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(right, outer_value) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if inner_right != runtime_divisor {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == root).then_some([cast_obligation, inner_obligation, outer_obligation])
        })
    };
    let post_cast_unsigned_runtime_divisor_obligations = find_post_cast_runtime_divisor_chain(
        terminal_entry.parameters[0].id,
        terminal_entry.parameters[2].id,
        IntegerValue::Unsigned(2),
    )
    .expect("native path retains one post-cast unsigned runtime-divisor chain");
    let post_cast_signed_positive_runtime_divisor_obligations =
        find_post_cast_runtime_divisor_chain(
            terminal_entry.parameters[4].id,
            terminal_entry.parameters[6].id,
            IntegerValue::Signed(-3),
        )
        .expect("native path retains one post-cast signed-positive runtime-divisor chain");
    let post_cast_signed_negative_runtime_divisor_obligations =
        find_post_cast_runtime_divisor_chain(
            terminal_entry.parameters[4].id,
            terminal_entry.parameters[7].id,
            IntegerValue::Signed(3),
        )
        .expect("native path retains one post-cast signed-negative runtime-divisor chain");
    for obligations in [
        direct_unsigned_runtime_divisor_obligations.as_slice(),
        direct_signed_positive_runtime_divisor_obligations.as_slice(),
        direct_signed_negative_runtime_divisor_obligations.as_slice(),
        post_cast_unsigned_runtime_divisor_obligations.as_slice(),
        post_cast_signed_positive_runtime_divisor_obligations.as_slice(),
        post_cast_signed_negative_runtime_divisor_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let nested_shift_right_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(count, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_integer_constant(middle_count, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_value == terminal_entry.parameters[1].id
                && has_integer_constant(inner_count, IntegerValue::Signed(1)))
            .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("native path retains the finite exact-shift-right chain");
    for obligation in nested_shift_right_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let shift_right_then_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(count, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_integer_constant(middle_count, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_value == terminal_entry.parameters[1].id
                && has_integer_constant(inner_count, IntegerValue::Signed(1)))
            .then_some([
                inner_obligation,
                middle_obligation,
                outer_obligation,
                cast_obligation,
            ])
        })
        .expect("native path retains one complete pre-cast exact-right-shift chain");
    let zero_shift_right_then_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let shift = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: shift_obligation,
            } = shift.kind
            else {
                return None;
            };
            (value == terminal_entry.parameters[1].id
                && has_integer_constant(count, IntegerValue::Signed(0)))
            .then_some([shift_obligation, cast_obligation])
        })
        .expect("native path retains one zero-count right shift and its following cast");
    for obligations in [
        shift_right_then_cast_obligations.as_slice(),
        zero_shift_right_then_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let cast_then_shift_right_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(count, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_integer_constant(middle_count, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !has_integer_constant(inner_count, IntegerValue::Signed(1)) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_value)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == terminal_entry.parameters[0].id).then_some([
                cast_obligation,
                inner_obligation,
                middle_obligation,
                outer_obligation,
            ])
        })
        .expect("native path retains one post-cast heterogeneous right-shift chain");
    let find_two_link_cast_then_shift_right = |parameter| {
        operations.iter().find_map(|outer| {
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(count, IntegerValue::Unsigned(2))
                && !has_integer_constant(count, IntegerValue::Signed(2))
            {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !has_integer_constant(inner_count, IntegerValue::Unsigned(1))
                && !has_integer_constant(inner_count, IntegerValue::Signed(1))
            {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_value)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == parameter).then_some([cast_obligation, inner_obligation, outer_obligation])
        })
    };
    let signed_cast_then_shift_right_obligations =
        find_two_link_cast_then_shift_right(terminal_entry.parameters[4].id)
            .expect("native path retains one signed post-cast right-shift chain");
    let cross_cast_then_shift_right_obligations =
        find_two_link_cast_then_shift_right(terminal_entry.parameters[5].id)
            .expect("native path retains one cross-sign post-cast right-shift chain");
    for obligations in [
        cast_then_shift_right_obligations.as_slice(),
        signed_cast_then_shift_right_obligations.as_slice(),
        cross_cast_then_shift_right_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let nested_shift_left_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(count, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_integer_constant(middle_count, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_value == terminal_entry.parameters[1].id
                && has_integer_constant(inner_count, IntegerValue::Signed(1)))
            .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("native path retains the finite exact-shift-left chain");
    for obligation in nested_shift_left_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let cast_then_shift_left_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(count, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_integer_constant(middle_count, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !has_integer_constant(inner_count, IntegerValue::Signed(1)) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_value)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == terminal_entry.parameters[0].id).then_some([
                cast_obligation,
                inner_obligation,
                middle_obligation,
                outer_obligation,
            ])
        })
        .expect("native path retains one complete post-cast exact-left-shift chain");
    for (index, obligation) in cast_then_shift_left_obligations.iter().enumerate() {
        for other in &cast_then_shift_left_obligations[index + 1..] {
            assert_ne!(obligation, other);
        }
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let shift_left_then_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !has_integer_constant(count, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !has_integer_constant(middle_count, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_value == terminal_entry.parameters[1].id
                && has_integer_constant(inner_count, IntegerValue::Signed(1)))
            .then_some([
                inner_obligation,
                middle_obligation,
                outer_obligation,
                cast_obligation,
            ])
        })
        .expect("native path retains one complete pre-cast exact-left-shift chain");
    let zero_shift_then_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let shift = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation: shift_obligation,
            } = shift.kind
            else {
                return None;
            };
            (value == terminal_entry.parameters[1].id
                && has_integer_constant(count, IntegerValue::Signed(0)))
            .then_some([shift_obligation, cast_obligation])
        })
        .expect("native path retains one zero-count shift and its following cast");
    for obligations in [
        shift_left_then_cast_obligations.as_slice(),
        zero_shift_then_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        psi_proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    assert_eq!(
        terminal_entry
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Return { .. }))
            .count(),
        1
    );
    let semantics = encode_module(&lowered.semantic_module).expect("shared integer semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("shared integer proof");
    let entry_machine = lowered.semantic_module.entry;
    let abstract_plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("integer-comparison convergence crosses Omega boundary");

    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering: {error:?}", case.target));
        let target_entry = target_plan
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("target integer-comparison convergence entry");
        assert!(matches!(
            &target_entry.operation,
            TerminalTargetOperation::ScalarReturnWithCleanup { scalar, .. }
                if matches!(scalar.as_ref(),
                    TerminalTargetOperation::ReturnBooleanSharedConvergence { .. })
        ));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment: {error:?}", case.target));
        assert!(matches!(
            assigned
                .functions
                .iter()
                .find(|function| function.machine == entry_machine)
                .map(|function| &function.operation),
            Some(TerminalAssignedOperation::ScalarReturnWithCleanup { scalar, .. })
                if matches!(scalar.as_ref(),
                    TerminalAssignedOperation::ReturnBooleanSharedConvergence { .. })
        ));
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission: {error:?}", case.target));
        let emitted = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("emitted integer-comparison convergence entry");
        assert!(emitted.scalar_affine_cleanup.is_some());
        assert!(emitted.scalar_control_affine_cleanups.is_empty());
        let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
            decisions,
            joins,
            structural_conditions,
            merge_offset,
        } = &emitted
            .scalar_stack
            .as_ref()
            .expect("shared integer convergence stack evidence")
            .control_flow
        else {
            panic!("native integer-comparison convergence must retain its exact join")
        };
        assert!(decisions.len() >= 2);
        assert_eq!(joins.len(), decisions.len());
        assert!(structural_conditions.is_empty());
        assert_eq!(
            *merge_offset,
            emitted
                .scalar_affine_cleanup
                .as_ref()
                .expect("shared integer cleanup")
                .code_offset
        );

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} object replay: {error:?}", case.target));
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} image: {error:?}", case.target));
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} installation: {error:?}", case.target));
        validate_terminal_installation_record(&installation, &image)
            .expect("installed integer-comparison convergence binds its exact image");
    }
}

#[test]
fn contextual_scalar_cleanup_is_verified_then_projected_on_all_targets() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}

        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        { Helper::touch(); }

        data Plain { observed: bool; }

        data Root {}
        machine Root::measure(
            first: Token,
            offset: u64 in Wrapping,
            plain: Plain,
            factor: u64 in Wrapping,
            second: Token
        ) -> u64 in Wrapping
        requires first.ready, plain.observed, second.ready
        {
            let seed: u64 in Wrapping = offset + 1u64;
            let result: u64 in Wrapping = seed * factor;
            result
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize contextual scalar cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual scalar cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual scalar cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type contextual scalar cleanup");
    let checked = lower_typed_trees(typed).expect("check contextual scalar cleanup");
    let lowered = lower_machine(&checked, "Root::measure")
        .expect("contextual scalar cleanup reaches terminal Psi");
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof artifact");
    let entry_machine = lowered.semantic_module.entry;

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == entry_machine)
        .expect("contextual scalar entry");
    assert_eq!(entry.parameters.len(), 2);
    assert_eq!(entry.structural_parameters.len(), 3);
    assert_eq!(entry.contract.requires.len(), 3);
    let Terminator::Return {
        cleanup_actions, ..
    } = &entry.blocks[0].terminator
    else {
        panic!("contextual scalar entry returns a scalar")
    };
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);
    assert!(matches!(
        cleanup_actions.as_slice(),
        [
            TerminalAffineCleanupAction::InvokeNominal(second),
            TerminalAffineCleanupAction::DiscardRoot(_),
            TerminalAffineCleanupAction::InvokeNominal(first),
        ] if second.cleanup_receiver.is_some()
            && first.cleanup_receiver == second.cleanup_receiver
            && second.requirement_obligations.len() == 1
            && first.requirement_obligations.len() == 1
    ));
    let structural_arguments = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xc0de_u64 + u64::try_from(index).expect("argument index fits u64"),
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 integer type"),
                value: IntegerValue::Unsigned(6),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 integer type"),
                value: IntegerValue::Unsigned(7),
            },
        ],
        &structural_arguments,
    )
    .expect("contextual scalar-local cleanup artifact starts");
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .expect("contextual scalar-local cleanup executes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 integer type"),
                value: IntegerValue::Unsigned(49),
            }
        ))
    );
    drop(checked);
    drop(lowered);

    let abstract_plan =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("verified contextual scalar cleanup crosses the Omega boundary");
    let abstract_entry = abstract_plan
        .functions
        .iter()
        .find(|function| function.machine == entry_machine)
        .expect("abstract contextual scalar entry");
    let [offset_parameter, factor_parameter] = abstract_entry.parameters.as_slice() else {
        panic!("abstract contextual scalar entry retains both scalar parameters")
    };
    let [
        TerminalAbstractOperation::IntegerConstant {
            result: increment_value,
            value: IntegerValue::Unsigned(1),
            ..
        },
        TerminalAbstractOperation::WrappingIntegerAdd {
            result: seed_value,
            left: add_left,
            right: add_right,
            ..
        },
        TerminalAbstractOperation::WrappingIntegerMultiply {
            result: computed_value,
            left: multiply_left,
            right: multiply_right,
            ..
        },
        TerminalAbstractOperation::Return {
            value: returned_value,
            cleanup_actions,
            ..
        },
    ] = abstract_entry.operations.as_slice()
    else {
        panic!("contextual scalar result precedes its cleanup")
    };
    assert_eq!(
        (*add_left, *add_right),
        (offset_parameter.value, *increment_value)
    );
    assert_eq!(
        (*multiply_left, *multiply_right),
        (*seed_value, factor_parameter.value)
    );
    assert_eq!(*returned_value, *computed_value);
    assert!(matches!(
        cleanup_actions.as_slice(),
        [
            TerminalAffineCleanupAction::InvokeNominal(second),
            TerminalAffineCleanupAction::DiscardRoot(_),
            TerminalAffineCleanupAction::InvokeNominal(first),
        ] if second.cleanup_receiver.is_none()
            && first.cleanup_receiver.is_none()
            && second.requirement_obligations.is_empty()
            && first.requirement_obligations.is_empty()
    ));

    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering failed: {error:?}", case.target));
        let target_entry = target_plan
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("target contextual scalar entry");
        let TerminalTargetOperation::ScalarReturnWithCleanup {
            scalar,
            call_plan,
            structural_parameters,
            ..
        } = &target_entry.operation
        else {
            panic!("runtime scalar expression remains wrapped by structural cleanup")
        };
        assert_eq!(call_plan.parameters.len(), 5);
        assert_direct_register_placement(&call_plan.parameters[0], case.parameter);
        assert_direct_register_placement(&call_plan.parameters[1], case.second_parameter);
        for (placement, parameter) in call_plan.parameters[2..].iter().zip(structural_parameters) {
            assert_eq!(placement, &parameter.placement);
        }
        let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } = scalar.as_ref()
        else {
            panic!("scalar cleanup retains a runtime input-derived integer expression")
        };
        let TerminalTargetIntegerExpression::WrappingMultiply { left, right, .. } = expression
        else {
            panic!("runtime result retains the final wrapping multiply")
        };
        assert!(matches!(
            left.as_ref(),
            TerminalTargetIntegerExpression::WrappingAdd {
                left: add_left,
                right: add_right,
                ..
            } if matches!(
                add_left.as_ref(),
                TerminalTargetIntegerExpression::Parameter {
                    parameter_index: 0,
                    location: TerminalScalarParameterLocation::Register(register),
                    ..
                } if *register == case.parameter
            ) && matches!(
                add_right.as_ref(),
                TerminalTargetIntegerExpression::Immediate {
                    value: IntegerValue::Unsigned(1),
                    ..
                }
            )
        ));
        assert!(matches!(
            right.as_ref(),
            TerminalTargetIntegerExpression::Parameter {
                parameter_index: 1,
                location: TerminalScalarParameterLocation::Register(register),
                ..
            } if *register == case.second_parameter
        ));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment failed: {error:?}", case.target));
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission failed: {error:?}", case.target));
        let emitted_entry = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("emitted contextual scalar entry");
        let cleanup = emitted_entry
            .scalar_affine_cleanup
            .as_ref()
            .expect("emitted contextual scalar cleanup");
        assert_eq!(cleanup.actions, *cleanup_actions);
        assert_eq!(emitted_entry.scalar_structural_parameter_homes.len(), 3);
        assert_eq!(emitted_entry.internal_unit_calls.len(), 2);
        assert!(cleanup.code_offset > 0, "result bytes precede cleanups");

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} object failed: {error:?}", case.target));
        let object_entry = object
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("object contextual scalar entry");
        assert_eq!(
            object_entry.scalar_structural_parameter_homes,
            emitted_entry.scalar_structural_parameter_homes
        );
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} image failed: {error:?}", case.target));
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} installation failed: {error:?}", case.target));
        let installed_entry = installation
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("installed contextual scalar entry");
        assert_eq!(
            installed_entry.scalar_affine_cleanup.as_ref(),
            Some(cleanup)
        );
        assert_eq!(
            installed_entry.scalar_structural_parameter_homes,
            emitted_entry.scalar_structural_parameter_homes
        );
        let installation_bytes = encode_terminal_installation_record(&installation)
            .expect("canonical contextual scalar installation");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation.clone())
        );
        validate_terminal_installation_record(&installation, &image)
            .expect("installed contextual scalar cleanup binds its exact image");
    }
}

#[test]
fn source_scalar_result_runs_distinct_nominal_roots_in_reverse_order_on_all_targets() {
    let source = r#"
        data FirstHelper {}
        machine FirstHelper::touch() {}
        data SecondHelper {}
        machine SecondHelper::touch() {}

        data First { value: u64; }
        machine First::drop(&mut self) { FirstHelper::touch(); }
        data Second { value: u64; }
        machine Second::drop(&mut self) { SecondHelper::touch(); }

        data Root {}
        machine Root::measure(first: First, second: Second) -> u64 { 7u64 }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize ordered scalar cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse ordered scalar cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve ordered scalar cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type ordered scalar cleanup");
    let checked = lower_typed_trees(typed).expect("check ordered scalar cleanup");
    let lowered = lower_machine(&checked, "Root::measure")
        .expect("ordered scalar nominal cleanup reaches terminal Psi");
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof artifact");
    let entry_machine = lowered.semantic_module.entry;
    drop(checked);
    drop(lowered);

    let module = decode_module(&semantic_bytes).expect("semantic artifact decodes");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == entry_machine)
        .expect("ordered scalar cleanup entry");
    let Terminator::Return {
        cleanup_actions, ..
    } = &entry.blocks[0].terminator
    else {
        panic!("ordered scalar cleanup entry returns a scalar")
    };
    let [
        TerminalAffineCleanupAction::InvokeNominal(second),
        TerminalAffineCleanupAction::InvokeNominal(first),
    ] = cleanup_actions.as_slice()
    else {
        panic!("two nominal roots form one ordered scalar cleanup stream")
    };
    assert_eq!(second.place, entry.structural_parameters[1].place);
    assert_eq!(first.place, entry.structural_parameters[0].place);
    assert_ne!(second.cleanup_machine, first.cleanup_machine);

    let abstract_plan =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("verified ordered scalar cleanup crosses the Omega boundary");
    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering failed: {error:?}", case.target));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment failed: {error:?}", case.target));
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission failed: {error:?}", case.target));
        let emitted_entry = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("emitted ordered scalar cleanup entry");
        let cleanup = emitted_entry
            .scalar_affine_cleanup
            .as_ref()
            .expect("emitted ordered scalar cleanup custody");
        assert_eq!(cleanup.actions, *cleanup_actions);
        assert_eq!(emitted_entry.scalar_structural_parameter_homes.len(), 2);
        assert_eq!(emitted_entry.internal_unit_calls.len(), 2);
        assert_eq!(
            emitted_entry
                .internal_unit_calls
                .iter()
                .map(|call| call.target)
                .collect::<Vec<_>>(),
            vec![second.cleanup_machine, first.cleanup_machine]
        );
        assert!(cleanup.code_offset > 0, "result bytes precede cleanups");

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} object failed: {error:?}", case.target));
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} image failed: {error:?}", case.target));
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} installation failed: {error:?}", case.target));
        let installed_entry = installation
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("installed ordered scalar cleanup entry");
        assert_eq!(
            installed_entry.scalar_affine_cleanup.as_ref(),
            Some(cleanup)
        );
        let installation_bytes = encode_terminal_installation_record(&installation)
            .expect("canonical ordered scalar cleanup installation");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation.clone())
        );
        validate_terminal_installation_record(&installation, &image)
            .expect("installed ordered scalar cleanup binds its exact image");
    }
}

#[test]
fn source_scalar_result_preserves_mixed_cleanup_order_on_all_targets() {
    let source = r#"
        data First { value: u64; }

        data Helper {}
        machine Helper::touch() {}
        data Token { value: u64; }
        machine Token::drop(&mut self) { Helper::touch(); }

        data Last { value: u64; }

        data Root {}
        machine Root::measure(first: First, token: Token, last: Last) -> u64 { 7u64 }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize mixed scalar cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed scalar cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed scalar cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type mixed scalar cleanup");
    let checked = lower_typed_trees(typed).expect("check mixed scalar cleanup");
    let lowered = lower_machine(&checked, "Root::measure")
        .expect("mixed scalar cleanup reaches terminal Psi");
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof artifact");
    let entry_machine = lowered.semantic_module.entry;
    drop(checked);
    drop(lowered);

    let module = decode_module(&semantic_bytes).expect("semantic artifact decodes");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == entry_machine)
        .expect("mixed scalar cleanup entry");
    let Terminator::Return {
        edge,
        cleanup_actions,
        ..
    } = &entry.blocks[0].terminator
    else {
        panic!("mixed scalar cleanup entry returns a scalar")
    };
    let [
        TerminalAffineCleanupAction::DiscardRoot(last),
        TerminalAffineCleanupAction::InvokeNominal(token),
        TerminalAffineCleanupAction::DiscardRoot(first),
    ] = cleanup_actions.as_slice()
    else {
        panic!("mixed roots form one ordered scalar cleanup stream")
    };
    assert_eq!(*last, entry.structural_parameters[2].place);
    assert_eq!(token.place, entry.structural_parameters[1].place);
    assert_eq!(*first, entry.structural_parameters[0].place);

    let abstract_plan =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("verified mixed scalar cleanup crosses the Omega boundary");
    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering failed: {error:?}", case.target));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment failed: {error:?}", case.target));
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission failed: {error:?}", case.target));
        let emitted_entry = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("emitted mixed scalar cleanup entry");
        let cleanup = emitted_entry
            .scalar_affine_cleanup
            .as_ref()
            .expect("emitted mixed scalar cleanup custody");
        assert_eq!(cleanup.actions, *cleanup_actions);
        assert_eq!(emitted_entry.scalar_structural_parameter_homes.len(), 3);
        assert_eq!(emitted_entry.internal_unit_calls.len(), 1);
        assert_eq!(
            emitted_entry.internal_unit_calls[0].target,
            token.cleanup_machine
        );
        assert_eq!(
            emitted_entry.internal_unit_calls[0].owner,
            TerminalCallSiteOwner::CleanupAction {
                edge: *edge,
                action_ordinal: 1,
            }
        );
        assert!(cleanup.code_offset > 0, "result bytes precede cleanups");

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} object failed: {error:?}", case.target));
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} image failed: {error:?}", case.target));
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} installation failed: {error:?}", case.target));
        let installed_entry = installation
            .functions()
            .iter()
            .find(|function| function.machine == entry_machine)
            .expect("installed mixed scalar cleanup entry");
        assert_eq!(
            installed_entry.scalar_affine_cleanup.as_ref(),
            Some(cleanup)
        );
        let installation_bytes = encode_terminal_installation_record(&installation)
            .expect("canonical mixed scalar cleanup installation");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation.clone())
        );
        validate_terminal_installation_record(&installation, &image)
            .expect("installed mixed scalar cleanup binds its exact image");
    }
}

fn assert_source_structural_return(
    machine_name: &str,
    parameter_cleanup_count: usize,
    local_cleanup_count: usize,
) {
    let checked = checked_source();
    let planned_structural_returns = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .map(|plan| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == plan.machine)
                .expect("planned machine remains present")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert!(planned_structural_returns.contains(&machine_name));
    let lowered = lower_machine(&checked, machine_name)
        .expect("exact whole-root passthrough should lower to terminal Psi");
    let original_identity =
        terminal_psi_identity(&lowered.semantic_module).expect("terminal identity");
    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("structural semantics encode");
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("structural proof bundle encodes");
    drop(checked);
    drop(lowered);

    let module = decode_module(&semantic_bytes).expect("structural semantics decode");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);
    let [machine] = module.machines.as_slice() else {
        panic!("fixture must produce one terminal machine")
    };
    let TerminalMachineResult::Structural(result) = &machine.result else {
        panic!("fixture result must remain structural")
    };
    let [entry_claim] = machine.entry_claims.as_slice() else {
        panic!("fixture must carry one whole-root entry claim")
    };
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("fixture must transfer its structural input")
    };
    let trivial_affine_locals = machine.blocks[0]
        .operations
        .iter()
        .map(|operation| {
            let OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind else {
                panic!("structural return may only establish trivial affine locals")
            };
            let place = machine
                .structural_places
                .iter()
                .find(|place| place.id == destination)
                .expect("local establishment has a typed place")
                .clone();
            let StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = place.kind
            else {
                panic!("local establishment must name a trivial affine local")
            };
            let local_type = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .expect("local type remains declared")
                .clone();
            (operation.id, place, local_type)
        })
        .collect::<Vec<_>>();
    assert_eq!(trivial_affine_locals.len(), local_cleanup_count);
    assert_eq!(
        trivial_affine_locals
            .iter()
            .map(|(_, local, _)| match local.kind {
                StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    ..
                } => declaration_ordinal,
                _ => unreachable!("local kind checked above"),
            })
            .collect::<Vec<_>>(),
        (0..u32::try_from(local_cleanup_count).expect("local count fits u32")).collect::<Vec<_>>()
    );
    let expected_cleanup = trivial_affine_locals
        .iter()
        .rev()
        .map(|(_, place, _)| place.id)
        .chain(
            machine
                .structural_parameters
                .iter()
                .skip(1)
                .rev()
                .map(|parameter| parameter.place),
        )
        .collect::<Vec<_>>();
    assert_eq!(
        machine.structural_parameters.len(),
        parameter_cleanup_count + 1
    );
    assert_eq!(*source, machine.structural_parameters[0].place);
    assert_eq!(returned_claims, &[entry_claim.claim]);
    assert_eq!(trivial_affine_discards, &expected_cleanup);
    assert_eq!(machine.content_entry_claims[0].claim, entry_claim.claim);
    assert_eq!(
        machine.content_identity_reshuffles[0].claim,
        entry_claim.claim
    );

    verify_module(
        &module,
        &psi_terminal_codec::decode_proof_bundle(&proof_bytes).expect("proof bundle decodes"),
        &AdmissionProfile::default(),
    )
    .expect("decoded structural artifact verifies independently");

    let argument = TerminalStructuralValue {
        opaque_identity: OPAQUE_REGION_IDENTITY,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
        path: Vec::new(),
    };
    let mut structural_arguments = vec![argument.clone()];
    for (index, cleanup_parameter) in machine.structural_parameters.iter().skip(1).enumerate() {
        structural_arguments.push(TerminalStructuralValue {
            opaque_identity: 0xd15c_a4d + u64::try_from(index).expect("cleanup index fits u64"),
            structural_type: cleanup_parameter.structural_type,
            qualifications: cleanup_parameter.qualifications.clone(),
            path: Vec::new(),
        });
    }
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        &structural_arguments,
    )
    .expect("decoded structural artifact starts without frontend state");
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .expect("structural artifact executes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![entry_claim.claim],
            }
        ))
    );

    let optimizer_input = lower_artifact_sections_for_optimization(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
    )
    .expect("source structural return verifies for optimizer admission");
    let verified_unit = build_verified_psi_optimization_unit(
        optimizer_input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("source structural return retains its optimizer unit");
    validate_verified_psi_optimization_unit(&verified_unit)
        .expect("source structural return satisfies optimizer local custody");
    let optimizer_function = verified_unit
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .expect("optimizer unit retains the structural-return machine");
    let optimizer_return = optimizer_function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            TerminalAbstractOperation::ReturnStructural {
                trivial_affine_locals,
                trivial_affine_discards,
                ..
            } => Some((trivial_affine_locals, trivial_affine_discards)),
            _ => None,
        })
        .expect("optimizer unit retains the compressed structural return");
    assert_eq!(optimizer_return.0, &trivial_affine_locals);
    assert_eq!(optimizer_return.1, &expected_cleanup);
    assert!(
        optimizer_return
            .0
            .iter()
            .all(|(_, local, _)| { !optimizer_function.declared_places.contains(&local.id) })
    );

    let structural_parameters = machine.structural_parameters.clone();
    let structural_parameter = structural_parameters[0].clone();
    let structural_result = result.clone();
    let source_place = *source;
    let return_edge = match &machine.blocks[0].terminator {
        Terminator::ReturnStructural { edge, .. } => *edge,
        _ => unreachable!("fixture return was checked above"),
    };
    let claim = entry_claim.claim;
    let abstract_plan =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("verified structural artifact crosses the Omega boundary");
    assert_eq!(abstract_plan.terminal_psi, original_identity);
    let [abstract_function] = abstract_plan.functions.as_slice() else {
        panic!("fixture must produce one abstract function")
    };
    assert_eq!(
        abstract_function.result,
        TerminalAbstractFunctionResult::Structural(structural_result.clone())
    );
    assert_eq!(
        abstract_function.structural_parameters,
        structural_parameters
    );
    assert_eq!(abstract_function.entry_claims, [entry_claim.clone()]);
    assert!(matches!(
        abstract_function.operations.as_slice(),
        [TerminalAbstractOperation::ReturnStructural {
            psi_edge,
            source,
            returned_claims,
            trivial_affine_locals: abstract_locals,
            trivial_affine_discards,
        }] if *psi_edge == return_edge
            && *source == source_place
            && returned_claims == &[claim]
            && abstract_locals == &trivial_affine_locals
            && trivial_affine_discards == &expected_cleanup
    ));
    if parameter_cleanup_count != 0 || local_cleanup_count != 0 {
        let mut missing_cleanup = abstract_plan.clone();
        let [
            TerminalAbstractOperation::ReturnStructural {
                trivial_affine_discards,
                ..
            },
        ] = missing_cleanup.functions[0].operations.as_mut_slice()
        else {
            unreachable!("structural return checked above")
        };
        trivial_affine_discards.clear();
        assert!(
            lower_to_target_operations(&missing_cleanup, NativeTarget::linux_x64()).is_err(),
            "target lowering must reject incomplete verified cleanup custody"
        );
    }

    for case in target_cases() {
        let target_plan =
            lower_to_target_operations(&abstract_plan, case.target).unwrap_or_else(|error| {
                panic!("{:?} structural selection failed: {error:?}", case.target)
            });
        assert_eq!(target_plan.terminal_psi, original_identity);
        let [target_function] = target_plan.functions.as_slice() else {
            panic!("fixture must produce one target function")
        };
        let TerminalTargetOperation::ReturnStructuralParameter {
            call_plan,
            parameters,
            source,
            result,
            shape,
            source_placement,
            result_placement,
            psi_edge,
            returned_claims,
            trivial_affine_locals: target_locals,
            trivial_affine_discards,
        } = &target_function.operation
        else {
            panic!("structural passthrough must remain distinct from scalar and Unit returns")
        };
        assert_eq!(call_plan.policy, case.policy);
        assert_eq!(parameters, &machine.structural_parameters);
        assert_eq!(source, &structural_parameter);
        assert_eq!(result, &structural_result);
        assert_eq!(*shape, ValueShape::integer(8, 8));
        assert_direct_register_placement(source_placement, case.parameter);
        assert_direct_register_placement(result_placement, case.result);
        assert_eq!(*psi_edge, return_edge);
        assert_eq!(returned_claims, &[claim]);
        assert_eq!(target_locals, &trivial_affine_locals);
        assert_eq!(trivial_affine_discards, &expected_cleanup);
        if parameter_cleanup_count == 8 {
            assert!(
                call_plan.parameters.iter().any(|placement| placement
                    .locations
                    .iter()
                    .any(|location| matches!(location, ValueLocation::Stack { .. }))),
                "eight affine tails must exercise incoming stack placement under {:?}",
                case.policy
            );
        }

        let assigned = assign_registers(&target_plan).unwrap_or_else(|error| {
            panic!("{:?} structural assignment failed: {error:?}", case.target)
        });
        let TerminalAssignedOperation::ReturnStructuralParameter {
            call_plan: assigned_call_plan,
            parameters: assigned_parameters,
            source: assigned_source,
            result: assigned_result,
            shape: assigned_shape,
            source_placement: assigned_source_placement,
            result_placement: assigned_result_placement,
            psi_edge: assigned_edge,
            returned_claims: assigned_claims,
            trivial_affine_locals: assigned_locals,
            trivial_affine_discards: assigned_cleanup,
        } = &assigned.functions[0].operation
        else {
            panic!("assignment must retain the typed structural return")
        };
        assert_eq!(assigned_call_plan, call_plan);
        assert_eq!(assigned_parameters, parameters);
        assert_eq!(assigned_source, source);
        assert_eq!(assigned_result, result);
        assert_eq!(assigned_shape, shape);
        assert_eq!(assigned_source_placement, source_placement);
        assert_eq!(assigned_result_placement, result_placement);
        assert_eq!(assigned_edge, psi_edge);
        assert_eq!(assigned_claims, returned_claims);
        assert_eq!(assigned_locals, target_locals);
        assert_eq!(assigned_cleanup, trivial_affine_discards);
        if parameter_cleanup_count != 0 || local_cleanup_count != 0 {
            let mut noncanonical_cleanup = target_plan.clone();
            let TerminalTargetOperation::ReturnStructuralParameter {
                trivial_affine_discards,
                ..
            } = &mut noncanonical_cleanup.functions[0].operation
            else {
                unreachable!("structural target operation checked above")
            };
            trivial_affine_discards[0] = source.place;
            assert!(
                assign_registers(&noncanonical_cleanup).is_err(),
                "assignment must reject cleanup outside reverse-declaration affine order"
            );
        }

        let machine_code = emit_machine_code(&assigned).unwrap_or_else(|error| {
            panic!("{:?} structural emission failed: {error:?}", case.target)
        });
        assert_eq!(machine_code.terminal_psi, original_identity);
        let [machine_function] = machine_code.functions.as_slice() else {
            panic!("fixture must emit one machine function")
        };
        assert_eq!(machine_function.bytes, case.bytes);
        assert_eq!(machine_function.provenance.edges, [return_edge]);
        assert_eq!(
            machine_function.provenance.operations,
            trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            machine_function.fuel_attribution.len(),
            trivial_affine_locals.len() + 1
        );
        for (ordinal, (operation, _, _)) in trivial_affine_locals.iter().enumerate() {
            let attribution = &machine_function.fuel_attribution[ordinal];
            assert_eq!(
                attribution.site,
                TerminalNativeFuelSite::Operation(*operation)
            );
            assert_eq!(attribution.operation_ordinal, ordinal);
            assert_eq!(attribution.code_offset, 0);
            assert_eq!(attribution.byte_count, 0);
        }
        let return_attribution = machine_function
            .fuel_attribution
            .last()
            .expect("structural return has edge fuel attribution");
        assert_eq!(
            return_attribution.site,
            TerminalNativeFuelSite::Edge(return_edge)
        );
        assert_eq!(
            return_attribution.operation_ordinal,
            trivial_affine_locals.len()
        );
        assert_eq!(return_attribution.code_offset, 0);
        assert_eq!(return_attribution.byte_count, case.bytes.len());
        let custody = machine_function
            .structural_return
            .as_ref()
            .expect("machine code must retain zero-runtime structural custody");
        assert_eq!(custody.psi_edge, return_edge);
        assert_eq!(custody.parameters, machine.structural_parameters);
        assert_eq!(custody.parameter_placements, call_plan.parameters);
        assert_eq!(custody.source, structural_parameter);
        assert_eq!(custody.result, structural_result);
        assert_eq!(custody.shape, ValueShape::integer(8, 8));
        assert_eq!(&custody.source_placement, source_placement);
        assert_eq!(&custody.result_placement, result_placement);
        assert_eq!(custody.returned_claims, [claim]);
        assert_eq!(custody.trivial_affine_locals, trivial_affine_locals);
        assert_eq!(custody.trivial_affine_discards, expected_cleanup);
        assert_eq!(custody.code_offset, 0);
        assert_eq!(custody.byte_count, case.bytes.len());

        let mut dropped_claim = machine_code.clone();
        dropped_claim.functions[0]
            .structural_return
            .as_mut()
            .expect("structural custody row")
            .returned_claims
            .clear();
        assert!(
            build_terminal_object_artifact(&dropped_claim).is_err(),
            "{:?} object validation must reject a silently dropped live claim",
            case.target
        );
        if parameter_cleanup_count != 0 || local_cleanup_count != 0 {
            let mut dropped_cleanup = machine_code.clone();
            dropped_cleanup.functions[0]
                .structural_return
                .as_mut()
                .expect("structural custody row")
                .trivial_affine_discards
                .clear();
            assert!(
                build_terminal_object_artifact(&dropped_cleanup).is_err(),
                "{:?} object validation must reject missing cleanup custody",
                case.target
            );
        }
        if local_cleanup_count != 0 {
            assert_eq!(
                custody.parameter_placements.len(),
                machine.structural_parameters.len(),
                "a no-code local must not receive an ABI placement"
            );

            let mut missing_local = machine_code.clone();
            missing_local.functions[0]
                .structural_return
                .as_mut()
                .expect("structural custody row")
                .trivial_affine_locals
                .clear();
            assert!(
                build_terminal_object_artifact(&missing_local).is_err(),
                "object validation must reject cleanup without its typed local declaration"
            );

            let mut aliased_local = machine_code.clone();
            let aliased = aliased_local.functions[0]
                .structural_return
                .as_mut()
                .expect("structural custody row");
            let (_, local, _) = &mut aliased.trivial_affine_locals[0];
            local.id = aliased.source.place;
            aliased.trivial_affine_discards[0] = aliased.source.place;
            assert!(
                build_terminal_object_artifact(&aliased_local).is_err(),
                "object validation must reject a local aliased to the returned source"
            );

            let mut mutated_local_type = machine_code.clone();
            let mutated = mutated_local_type.functions[0]
                .structural_return
                .as_mut()
                .expect("structural custody row");
            let (_, _, local_type) = &mut mutated.trivial_affine_locals[0];
            local_type.id = mutated.source.structural_type;
            assert!(
                build_terminal_object_artifact(&mutated_local_type).is_err(),
                "object validation must reject a mismatched local type declaration identity"
            );

            let mut missing_establishment_fuel = machine_code.clone();
            missing_establishment_fuel.functions[0]
                .fuel_attribution
                .remove(0);
            assert!(
                build_terminal_object_artifact(&missing_establishment_fuel).is_err(),
                "object validation must reject a local establishment without exact fuel attribution"
            );

            if local_cleanup_count == 2 {
                let mut duplicate_local = machine_code.clone();
                let duplicate = duplicate_local.functions[0]
                    .structural_return
                    .as_mut()
                    .expect("structural custody row");
                let first_place = duplicate.trivial_affine_locals[0].1.id;
                duplicate.trivial_affine_locals[1].1.id = first_place;
                duplicate.trivial_affine_discards[0] = first_place;
                assert!(
                    build_terminal_object_artifact(&duplicate_local).is_err(),
                    "object validation must reject two local declarations aliased to one place"
                );

                let mut gapped_ordinal = machine_code.clone();
                let (_, second, _) = &mut gapped_ordinal.functions[0]
                    .structural_return
                    .as_mut()
                    .expect("structural custody row")
                    .trivial_affine_locals[1];
                let StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    ..
                } = &mut second.kind
                else {
                    unreachable!("local kind checked above")
                };
                *declaration_ordinal = 2;
                assert!(
                    build_terminal_object_artifact(&gapped_ordinal).is_err(),
                    "object validation must reject a gap in local declaration ordinals"
                );

                let mut reordered_cleanup = machine_code.clone();
                reordered_cleanup.functions[0]
                    .structural_return
                    .as_mut()
                    .expect("structural custody row")
                    .trivial_affine_discards
                    .swap(0, 1);
                assert!(
                    build_terminal_object_artifact(&reordered_cleanup).is_err(),
                    "object validation must reject non-reverse local cleanup"
                );

                let mut missing_second_fuel = machine_code.clone();
                missing_second_fuel.functions[0].fuel_attribution.remove(1);
                assert!(
                    build_terminal_object_artifact(&missing_second_fuel).is_err(),
                    "object validation must reject missing second-local fuel evidence"
                );
            }
        }
        if parameter_cleanup_count != 0 {
            let mut aliased_parameter = machine_code.clone();
            let aliased_custody = aliased_parameter.functions[0]
                .structural_return
                .as_mut()
                .expect("structural custody row");
            aliased_custody.parameters[1].place = aliased_custody.source.place;
            aliased_custody.trivial_affine_discards[0] = aliased_custody.source.place;
            assert!(
                build_terminal_object_artifact(&aliased_parameter).is_err(),
                "{:?} object validation must reject an affine parameter aliased to the returned place",
                case.target
            );
            if parameter_cleanup_count > 1 {
                let mut duplicate_tail = machine_code.clone();
                let custody = duplicate_tail.functions[0]
                    .structural_return
                    .as_mut()
                    .expect("structural custody row");
                custody.parameters[2].place = custody.parameters[1].place;
                custody.trivial_affine_discards[parameter_cleanup_count - 2] =
                    custody.parameters[1].place;
                assert!(
                    build_terminal_object_artifact(&duplicate_tail).is_err(),
                    "{:?} object validation must reject pairwise-aliased affine tails",
                    case.target
                );
            }
        }

        let object = build_terminal_object_artifact(&machine_code).unwrap_or_else(|error| {
            panic!("{:?} structural object failed: {error:?}", case.target)
        });
        assert_eq!(object.terminal_psi(), original_identity);
        assert_eq!(object.entry_function().bytes(&object), case.bytes);
        assert_eq!(
            object.entry_function().structural_return.as_ref(),
            Some(custody)
        );
        let container = emit_terminal_object_container(&object);
        assert_eq!(container.terminal_psi, original_identity);
        assert_eq!(container.output.text_bytes, case.bytes.len());

        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} structural image failed: {error}", case.target));
        assert_eq!(image.terminal_psi(), original_identity);
        assert_eq!(image.output().final_text_bytes, case.bytes);
        assert_eq!(
            image.functions()[0].structural_return.as_ref(),
            Some(custody)
        );
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .expect("structural image produces an installation record");
        let [installed_return] = installation.structural_returns() else {
            panic!("installation must retain one typed structural return")
        };
        assert_eq!(installed_return.machine, machine.id);
        assert_eq!(&installed_return.returned, custody);
        validate_terminal_installation_record(&installation, &image)
            .expect("installation record binds the structural image");
        let installation_bytes = encode_terminal_installation_record(&installation)
            .expect("structural installation record encodes");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation)
        );

        let mut custody_suffix = Vec::new();
        custody_suffix.extend_from_slice(&claim.get().to_le_bytes());
        custody_suffix.extend_from_slice(
            &u32::try_from(trivial_affine_locals.len())
                .expect("local count fits u32")
                .to_le_bytes(),
        );
        let mut local_record_offsets = Vec::with_capacity(trivial_affine_locals.len());
        for (operation, local, local_type) in &trivial_affine_locals {
            local_record_offsets.push(custody_suffix.len());
            let StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
            } = local.kind
            else {
                unreachable!("typed local checked above")
            };
            custody_suffix.extend_from_slice(&operation.get().to_le_bytes());
            custody_suffix.extend_from_slice(&local.id.get().to_le_bytes());
            custody_suffix.extend_from_slice(&declaration_ordinal.to_le_bytes());
            custody_suffix.extend_from_slice(&0_u32.to_le_bytes());
            custody_suffix.extend_from_slice(&structural_type.get().to_le_bytes());
            custody_suffix.extend_from_slice(&local_type.id.get().to_le_bytes());
            custody_suffix.extend_from_slice(
                &u32::try_from(local_type.identity.len())
                    .expect("type identity length fits u32")
                    .to_le_bytes(),
            );
            custody_suffix.extend_from_slice(local_type.identity.as_bytes());
            custody_suffix.extend_from_slice(&0_u32.to_le_bytes());
        }
        let cleanup_count_in_suffix = custody_suffix.len();
        custody_suffix.extend_from_slice(
            &u32::try_from(expected_cleanup.len())
                .expect("cleanup count fits u32")
                .to_le_bytes(),
        );
        for place in &expected_cleanup {
            custody_suffix.extend_from_slice(&place.get().to_le_bytes());
        }
        custody_suffix.extend_from_slice(&0_u64.to_le_bytes());
        custody_suffix.extend_from_slice(
            &u64::try_from(case.bytes.len())
                .expect("machine byte count fits u64")
                .to_le_bytes(),
        );
        let custody_offset = installation_bytes
            .windows(custody_suffix.len())
            .enumerate()
            .filter_map(|(offset, window)| (window == custody_suffix).then_some(offset))
            .next()
            .expect("canonical installation must contain the structural-custody suffix");
        let mut changed_claim_bytes = installation_bytes.clone();
        changed_claim_bytes[custody_offset..custody_offset + 8]
            .copy_from_slice(&(claim.get() + 1).to_le_bytes());
        let changed_claim = decode_terminal_installation_record(&changed_claim_bytes)
            .expect("a different nonzero claim remains structurally decodable");
        assert_eq!(
            changed_claim.structural_returns()[0]
                .returned
                .returned_claims[0]
                .get(),
            claim.get() + 1,
            "the encoded mutation must target the structural custody claim"
        );
        assert!(
            validate_terminal_installation_record(&changed_claim, &image).is_err(),
            "installation validation must reject a changed structural custody claim"
        );
        if local_cleanup_count != 0 {
            let mut changed_local_type_bytes = installation_bytes.clone();
            let local_type_declaration_offset = custody_offset + 44;
            changed_local_type_bytes
                [local_type_declaration_offset..local_type_declaration_offset + 8]
                .copy_from_slice(&source.structural_type.get().to_le_bytes());
            assert_eq!(
                decode_terminal_installation_record(&changed_local_type_bytes),
                Err(TerminalInstallationError::InvalidStructuralReturn(
                    machine.id
                )),
                "canonical installation decoding must reject a mutated local type identity"
            );
        }
        if local_cleanup_count == 2 {
            let mut changed_second_ordinal_bytes = installation_bytes.clone();
            let second_ordinal_offset = custody_offset + local_record_offsets[1] + 16;
            changed_second_ordinal_bytes[second_ordinal_offset..second_ordinal_offset + 4]
                .copy_from_slice(&2_u32.to_le_bytes());
            assert_eq!(
                decode_terminal_installation_record(&changed_second_ordinal_bytes),
                Err(TerminalInstallationError::InvalidStructuralReturn(
                    machine.id
                )),
                "canonical installation decoding must reject a gapped second-local ordinal"
            );
        }

        if expected_cleanup.len() >= 2 {
            let mut reordered_cleanup_bytes = installation_bytes.clone();
            let cleanup_place_offset = custody_offset + cleanup_count_in_suffix + 4;
            let first_cleanup =
                reordered_cleanup_bytes[cleanup_place_offset..cleanup_place_offset + 8].to_vec();
            let second_cleanup = reordered_cleanup_bytes
                [cleanup_place_offset + 8..cleanup_place_offset + 16]
                .to_vec();
            reordered_cleanup_bytes[cleanup_place_offset..cleanup_place_offset + 8]
                .copy_from_slice(&second_cleanup);
            reordered_cleanup_bytes[cleanup_place_offset + 8..cleanup_place_offset + 16]
                .copy_from_slice(&first_cleanup);
            assert_eq!(
                decode_terminal_installation_record(&reordered_cleanup_bytes),
                Err(TerminalInstallationError::InvalidStructuralReturn(
                    machine.id
                )),
                "canonical installation decoding must reject reordered cleanup"
            );
        }
        if parameter_cleanup_count != 0 || local_cleanup_count != 0 {
            let mut changed_cleanup_bytes = installation_bytes.clone();
            let cleanup_place_offset = custody_offset + cleanup_count_in_suffix + 4;
            changed_cleanup_bytes[cleanup_place_offset..cleanup_place_offset + 8]
                .copy_from_slice(&source.place.get().to_le_bytes());
            assert_eq!(
                decode_terminal_installation_record(&changed_cleanup_bytes),
                Err(TerminalInstallationError::InvalidStructuralReturn(
                    machine.id
                )),
                "canonical installation decoding must reject changed cleanup custody"
            );
        }

        #[cfg(unix)]
        if case.target == NativeTarget::host() {
            assert!(host_structural_round_trip(
                case.bytes,
                OPAQUE_REGION_IDENTITY,
                parameter_cleanup_count
            ));
        }
    }
}

#[test]
fn source_structural_return_preserves_opaque_value_and_claim_after_frontend_drop() {
    assert_source_structural_return("Main::forward", 0, 0);
    assert_source_structural_return("Main::forward_and_drop", 1, 0);
    assert_source_structural_return("Main::forward_and_drop_eight", 8, 0);
    assert_source_structural_return("Main::forward_with_local", 0, 1);
    assert_source_structural_return("Main::forward_with_two_locals", 0, 2);
    assert_source_structural_return("Main::forward_with_local_and_drop", 1, 1);
}

#[test]
fn direct_structural_result_call_reaches_every_native_artifact() {
    let checked = checked_source();
    let lowered = lower_machine(&checked, "Main::through_call")
        .expect("bounded structural-result call reaches terminal Psi");
    let semantic = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof artifact");
    let entry = lowered.semantic_module.entry;
    let optimizer_input =
        lower_artifact_sections_for_optimization(&semantic, &proof, &AdmissionProfile::default())
            .expect("structural-result call verifies for optimizer admission");
    let verified_unit = build_verified_psi_optimization_unit(
        optimizer_input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("structural-result call retains its optimizer unit");
    validate_verified_psi_optimization_unit(&verified_unit)
        .expect("source-derived structural call result dominates its return");
    let optimizer_caller = verified_unit
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == entry)
        .expect("optimizer unit retains the structural caller");
    let optimizer_entry = optimizer_caller
        .blocks
        .iter()
        .find(|block| block.id == optimizer_caller.entry)
        .expect("optimizer caller retains its declared entry block");
    let [call_node, return_node] = optimizer_entry.nodes.as_slice() else {
        panic!("optimizer caller retains one structural call and return")
    };
    let (
        TerminalAbstractOperation::CallStructural { result, .. },
        TerminalAbstractOperation::ReturnStructural { source, .. },
    ) = (&call_node.operation, &return_node.operation)
    else {
        panic!("optimizer caller retains structural result production and return")
    };
    assert_eq!(result.place, *source);
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("structural-result call crosses the Omega boundary");
    let abstract_entry = abstract_plan
        .functions
        .iter()
        .find(|function| function.machine == entry)
        .expect("abstract structural caller");
    assert!(matches!(
        abstract_entry.operations.as_slice(),
        [
            TerminalAbstractOperation::CallStructural { .. },
            TerminalAbstractOperation::ReturnStructural { .. }
        ]
    ));

    for case in target_cases() {
        let target_plan = lower_to_target_operations(&abstract_plan, case.target)
            .unwrap_or_else(|error| panic!("{:?} target lowering failed: {error:?}", case.target));
        assert!(matches!(
            target_plan
                .functions
                .iter()
                .find(|function| function.machine == entry)
                .map(|function| &function.operation),
            Some(TerminalTargetOperation::ReturnStructuralCall { .. })
        ));
        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} assignment failed: {error:?}", case.target));
        assert!(matches!(
            assigned
                .functions
                .iter()
                .find(|function| function.machine == entry)
                .map(|function| &function.operation),
            Some(TerminalAssignedOperation::ReturnStructuralCall { .. })
        ));
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} emission failed: {error:?}", case.target));
        let caller = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry)
            .expect("emitted structural caller");
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one exact structural call custody row")
        };
        let result = call
            .structural_result
            .as_ref()
            .expect("call retains structural result custody");
        assert!(call.result.is_none());
        assert_eq!(call.arguments.len(), 1);
        assert!(call.arguments[0].path.is_empty());
        assert_eq!(call.claim_transfers.len(), 1);
        assert_eq!(result.returned_claim_transfers.len(), 1);
        assert_eq!(result.returned_claims.len(), 1);
        assert_eq!(
            result.returned_claim_transfers[0].caller_claim,
            call.claim_transfers[0].claim
        );
        assert_eq!(result.returned_claims, vec![call.claim_transfers[0].claim]);

        let mut changed_machine_code = machine_code.clone();
        changed_machine_code
            .functions
            .iter_mut()
            .find(|function| function.machine == entry)
            .and_then(|function| function.internal_unit_calls[0].structural_result.as_mut())
            .expect("mutable emitted structural result")
            .returned_claims[0] = psi_core::ClaimId::new(99).unwrap();
        assert!(
            build_terminal_object_artifact(&changed_machine_code).is_err(),
            "object validation rejects returned-claim substitution"
        );

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} object failed: {error:?}", case.target));
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} image failed: {error:?}", case.target));
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} installation failed: {error:?}", case.target));
        let installed = installation
            .internal_unit_calls()
            .iter()
            .find(|call| call.machine == entry)
            .expect("installed structural call custody");
        assert_eq!(installed.custody.structural_result.as_ref(), Some(result));
        let encoded = encode_terminal_installation_record(&installation)
            .expect("canonical structural-call installation");
        assert_eq!(
            decode_terminal_installation_record(&encoded),
            Ok(installation.clone())
        );
        validate_terminal_installation_record(&installation, &image)
            .expect("installed structural call binds its exact image");
    }
}

#[test]
fn two_fragment_structural_result_call_is_exact_on_direct_aggregate_abis() {
    let checked = checked_source();
    let lowered = lower_machine(&checked, "Main::through_call_wide")
        .expect("two-fragment structural-result call reaches terminal Psi");
    let semantic = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof artifact");
    let entry = lowered.semantic_module.entry;
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("wide structural-result call crosses the Omega boundary");
    let TerminalAbstractOperation::CallStructural { callee, .. } = &abstract_plan
        .functions
        .iter()
        .find(|function| function.machine == entry)
        .expect("abstract wide caller")
        .operations[0]
    else {
        panic!("wide caller retains one structural call")
    };
    let callee = *callee;

    for case in target_cases() {
        let target_plan = match case.policy {
            CallingPolicy::MicrosoftX64 => {
                assert!(
                    lower_to_target_operations(&abstract_plan, case.target).is_err(),
                    "{:?} keeps its indirect 16-byte result fenced",
                    case.target
                );
                continue;
            }
            CallingPolicy::SystemVAMD64 | CallingPolicy::Aapcs64 => {
                lower_to_target_operations(&abstract_plan, case.target).unwrap_or_else(|error| {
                    panic!("{:?} wide target lowering failed: {error:?}", case.target)
                })
            }
            _ => unreachable!("target canary uses only native user calling policies"),
        };
        let caller_target = target_plan
            .functions
            .iter()
            .find(|function| function.machine == entry)
            .expect("target wide caller");
        let TerminalTargetOperation::ReturnStructuralCall {
            call_plan,
            callee_call_plan,
            ..
        } = &caller_target.operation
        else {
            panic!("wide target caller retains structural-result operation")
        };
        for placement in [
            &call_plan.parameters[0],
            call_plan.result.as_ref().expect("caller wide result"),
            &callee_call_plan.parameters[0],
            callee_call_plan
                .result
                .as_ref()
                .expect("callee wide result"),
        ] {
            assert_eq!(placement.shape, ValueShape::integer(16, 8));
            assert_eq!(placement.locations.len(), 2);
            assert!(
                placement
                    .locations
                    .iter()
                    .enumerate()
                    .all(|(index, location)| {
                        matches!(
                            location,
                            ValueLocation::Register {
                                value_byte_offset,
                                byte_size: 8,
                                ..
                            } if usize::from(*value_byte_offset) == index * 8
                        )
                    })
            );
        }

        let assigned = assign_registers(&target_plan)
            .unwrap_or_else(|error| panic!("{:?} wide assignment failed: {error:?}", case.target));
        assert!(matches!(
            assigned
                .functions
                .iter()
                .find(|function| function.machine == entry)
                .map(|function| &function.operation),
            Some(TerminalAssignedOperation::ReturnStructuralCall { .. })
        ));
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{:?} wide emission failed: {error:?}", case.target));
        let caller = machine_code
            .functions
            .iter()
            .find(|function| function.machine == entry)
            .expect("emitted wide caller");
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one exact wide structural call custody row")
        };
        let result = call
            .structural_result
            .as_ref()
            .expect("wide call retains structural result custody");
        assert_eq!(result.caller_result_placement.locations.len(), 2);
        assert_eq!(result.callee_result_placement.locations.len(), 2);
        let callee_code = machine_code
            .functions
            .iter()
            .find(|function| function.machine == callee)
            .expect("emitted wide callee");
        let returned = callee_code
            .structural_return
            .as_ref()
            .expect("wide callee retains structural return custody");
        assert_eq!(returned.source_placement.locations.len(), 2);
        assert_eq!(returned.result_placement.locations.len(), 2);
        match case.policy {
            CallingPolicy::SystemVAMD64 => assert_eq!(
                callee_code.bytes,
                [0x48, 0x89, 0xf8, 0x48, 0x89, 0xf2, 0xc3]
            ),
            CallingPolicy::Aapcs64 => assert_eq!(callee_code.bytes, AAPCS64_RETURN),
            _ => unreachable!(),
        }

        let mut reordered_callee = machine_code.clone();
        reordered_callee
            .functions
            .iter_mut()
            .find(|function| function.machine == callee)
            .and_then(|function| function.structural_return.as_mut())
            .expect("mutable wide callee return")
            .result_placement
            .locations
            .swap(0, 1);
        assert!(
            build_terminal_object_artifact(&reordered_callee).is_err(),
            "{:?} object replay rejects reordered wide result fragments",
            case.target
        );

        let mut reordered_call = machine_code.clone();
        reordered_call
            .functions
            .iter_mut()
            .find(|function| function.machine == entry)
            .and_then(|function| function.internal_unit_calls[0].structural_result.as_mut())
            .expect("mutable wide call result")
            .caller_result_placement
            .locations
            .swap(0, 1);
        assert!(
            build_terminal_object_artifact(&reordered_call).is_err(),
            "{:?} object replay rejects reordered caller result fragments",
            case.target
        );

        let object = build_terminal_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{:?} wide object failed: {error:?}", case.target));
        let image = emit_terminal_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{:?} wide image failed: {error:?}", case.target));
        assert_eq!(
            image
                .functions()
                .iter()
                .find(|function| function.machine == callee)
                .and_then(|function| function.structural_return.as_ref())
                .map(|record| record.result_placement.locations.len()),
            Some(2),
            "image retains both independently validated result fragments"
        );
        let installation = build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{:?} wide installation failed: {error:?}", case.target));
        let encoded = encode_terminal_installation_record(&installation)
            .expect("canonical wide structural installation");
        assert_eq!(
            decode_terminal_installation_record(&encoded),
            Ok(installation.clone())
        );

        let mut reordered_installation = encoded.clone();
        let location_count = 2_u32.to_le_bytes();
        let fragment_start = reordered_installation
            .windows(16)
            .position(|window| {
                window[0..4] == location_count
                    && window[4] == 1
                    && window[6..8] == 0_u16.to_le_bytes()
                    && window[8..10] == 8_u16.to_le_bytes()
                    && window[10] == 1
                    && window[12..14] == 8_u16.to_le_bytes()
                    && window[14..16] == 8_u16.to_le_bytes()
            })
            .expect("wide installation contains a two-fragment placement")
            + 4;
        let first = reordered_installation[fragment_start..fragment_start + 6].to_vec();
        let second = reordered_installation[fragment_start + 6..fragment_start + 12].to_vec();
        reordered_installation[fragment_start..fragment_start + 6].copy_from_slice(&second);
        reordered_installation[fragment_start + 6..fragment_start + 12].copy_from_slice(&first);
        let decoded = decode_terminal_installation_record(&reordered_installation)
            .expect("reordered fragments remain syntactically decodable");
        assert!(
            validate_terminal_installation_record(&decoded, &image).is_err(),
            "{:?} installation replay rejects reordered wide fragments",
            case.target
        );
    }
}

#[cfg(unix)]
fn host_structural_round_trip(bytes: &[u8], value: u64, cleanup_count: usize) -> bool {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let sequence = NEXT_SCRATCH_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-terminal-structural-return-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create structural-return test directory");
    let cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let encoded_bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(
            ".text\n.globl _terminal_entry\n.p2align 2\n_terminal_entry:\n.byte {encoded_bytes}\n"
        )
    } else {
        format!(
            ".text\n.globl terminal_entry\n.type terminal_entry,@function\nterminal_entry:\n.byte {encoded_bytes}\n.size terminal_entry, .-terminal_entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = if cleanup_count == 8 {
        format!(
            "#include <stdint.h>\n\
extern uint64_t terminal_entry(uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t);\n\
int main(void) {{ return terminal_entry(UINT64_C({value}), 1, 2, 3, 4, 5, 6, 7, 8) == UINT64_C({value}) ? 0 : 1; }}\n"
        )
    } else if cleanup_count == 1 {
        format!(
            "#include <stdint.h>\n\
extern uint64_t terminal_entry(uint64_t, uint64_t);\n\
int main(void) {{ return terminal_entry(UINT64_C({value}), UINT64_C(0xcafe)) == UINT64_C({value}) ? 0 : 1; }}\n"
        )
    } else {
        format!(
            "#include <stdint.h>\n\
extern uint64_t terminal_entry(uint64_t);\n\
int main(void) {{ return terminal_entry(UINT64_C({value})) == UINT64_C({value}) ? 0 : 1; }}\n"
        )
    };
    std::fs::write(&assembly_path, assembly).expect("write structural-return assembly harness");
    std::fs::write(&driver_path, driver).expect("write structural-return C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected structural-return machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    let success = Command::new(&executable_path)
        .status()
        .expect("execute structural-return native canary")
        .success();
    drop(cleanup);
    success
}

#[cfg(unix)]
struct ScratchDirectory(PathBuf);

#[cfg(unix)]
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
