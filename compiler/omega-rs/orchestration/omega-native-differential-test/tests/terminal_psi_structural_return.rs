//! Focused source-to-native canary for the first whole-root structural return.

use omega_calling_conventions::{
    CallingPolicy, MachineRegister, ValueLocation, ValuePlacement, ValueShape,
};
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
use omega_terminal_psi_to_abstract_operations::lower_artifact_sections;
use omega_terminal_target_operations::{
    TerminalCallSiteOwner, TerminalScalarParameterLocation, TerminalTargetIntegerExpression,
    TerminalTargetOperation,
};
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::{IntegerSign, IntegerType, IntegerValue, ProfileDecisionId, StructuralPlaceKind};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{OperationKind, TerminalAffineCleanupAction, TerminalMachineResult, Terminator};
use psi_terminal_codec::{
    decode_module, encode_module, encode_proof_bundle, terminal_psi_identity,
};
use psi_terminal_fuel::TerminalFuelMeter;
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
        .nth(4)
        .expect("omega-native-differential-test lives under compiler/omega-rs/orchestration")
        .join("canaries/pass/terminal_psi/structural_content_passthrough/main.omg")
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
        data Token { value: u64; }
        machine Token::drop(&mut self) { Helper::touch(); }
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(token: Token, input: bool, plain: Plain) -> bool {
            let staged: bool = (input && true) || false;
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
            .expect("emitted shared convergence entry");
        let cleanup = emitted
            .scalar_affine_cleanup
            .as_ref()
            .expect("one shared scalar cleanup record");
        assert!(emitted.scalar_control_affine_cleanups.is_empty());
        assert_eq!(emitted.internal_unit_calls.len(), 1);
        let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
            join_offset,
            merge_offset,
            ..
        } = &emitted
            .scalar_stack
            .as_ref()
            .expect("shared convergence stack evidence")
            .control_flow
        else {
            panic!("native shared convergence must retain its exact join")
        };
        assert!(join_offset < merge_offset);
        assert_eq!(*merge_offset, cleanup.code_offset);

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

        let mut forged = machine_code.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.machine == entry_machine)
            .expect("forged shared convergence entry");
        function.bytes[*join_offset] ^= 1;
        assert!(build_terminal_object_artifact(&forged).is_err());
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
