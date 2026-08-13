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
    build_terminal_installation_record, build_terminal_object_artifact,
    decode_terminal_installation_record, emit_terminal_executable_image,
    emit_terminal_object_container, encode_terminal_installation_record,
    validate_terminal_installation_record,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::lower_artifact_sections;
use omega_terminal_target_operations::TerminalTargetOperation;
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::ProfileDecisionId;
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{TerminalMachineResult, Terminator};
use psi_terminal_codec::{
    decode_module, encode_module, encode_proof_bundle, terminal_psi_identity,
};
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralResult,
    TerminalStructuralValue,
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
            result: MachineRegister::X86Rax,
            bytes: SYSV_RETURN,
        },
        TargetCase {
            target: NativeTarget::windows_x64(),
            policy: CallingPolicy::MicrosoftX64,
            parameter: MachineRegister::X86Rcx,
            result: MachineRegister::X86Rax,
            bytes: MICROSOFT_RETURN,
        },
        TargetCase {
            target: NativeTarget::uefi_x64(),
            policy: CallingPolicy::MicrosoftX64,
            parameter: MachineRegister::X86Rcx,
            result: MachineRegister::X86Rax,
            bytes: MICROSOFT_RETURN,
        },
        TargetCase {
            target: NativeTarget::linux_arm64(),
            policy: CallingPolicy::Aapcs64,
            parameter: MachineRegister::Aarch64X(0),
            result: MachineRegister::Aarch64X(0),
            bytes: AAPCS64_RETURN,
        },
        TargetCase {
            target: NativeTarget::macos_arm64(),
            policy: CallingPolicy::Aapcs64,
            parameter: MachineRegister::Aarch64X(0),
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
fn source_structural_return_preserves_opaque_value_and_claim_after_frontend_drop() {
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
    assert_eq!(planned_structural_returns, ["Main::forward"]);
    let lowered = lower_machine(&checked, "Main::forward")
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
    assert_eq!(*source, machine.structural_parameters[0].place);
    assert_eq!(returned_claims, &[entry_claim.claim]);
    assert!(trivial_affine_discards.is_empty());
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
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
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

    let structural_parameter = machine.structural_parameters[0].clone();
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
        [structural_parameter.clone()]
    );
    assert_eq!(abstract_function.entry_claims, [entry_claim.clone()]);
    assert!(matches!(
        abstract_function.operations.as_slice(),
        [TerminalAbstractOperation::ReturnStructural {
            psi_edge,
            source,
            returned_claims,
            trivial_affine_discards,
        }] if *psi_edge == return_edge
            && *source == source_place
            && returned_claims == &[claim]
            && trivial_affine_discards.is_empty()
    ));

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
            source,
            result,
            shape,
            source_placement,
            result_placement,
            psi_edge,
            returned_claims,
        } = &target_function.operation
        else {
            panic!("structural passthrough must remain distinct from scalar and Unit returns")
        };
        assert_eq!(call_plan.policy, case.policy);
        assert_eq!(source, &structural_parameter);
        assert_eq!(result, &structural_result);
        assert_eq!(*shape, ValueShape::integer(8, 8));
        assert_direct_register_placement(source_placement, case.parameter);
        assert_direct_register_placement(result_placement, case.result);
        assert_eq!(*psi_edge, return_edge);
        assert_eq!(returned_claims, &[claim]);

        let assigned = assign_registers(&target_plan).unwrap_or_else(|error| {
            panic!("{:?} structural assignment failed: {error:?}", case.target)
        });
        let TerminalAssignedOperation::ReturnStructuralParameter {
            call_plan: assigned_call_plan,
            source: assigned_source,
            result: assigned_result,
            shape: assigned_shape,
            source_placement: assigned_source_placement,
            result_placement: assigned_result_placement,
            psi_edge: assigned_edge,
            returned_claims: assigned_claims,
        } = &assigned.functions[0].operation
        else {
            panic!("assignment must retain the typed structural return")
        };
        assert_eq!(assigned_call_plan, call_plan);
        assert_eq!(assigned_source, source);
        assert_eq!(assigned_result, result);
        assert_eq!(assigned_shape, shape);
        assert_eq!(assigned_source_placement, source_placement);
        assert_eq!(assigned_result_placement, result_placement);
        assert_eq!(assigned_edge, psi_edge);
        assert_eq!(assigned_claims, returned_claims);

        let machine_code = emit_machine_code(&assigned).unwrap_or_else(|error| {
            panic!("{:?} structural emission failed: {error:?}", case.target)
        });
        assert_eq!(machine_code.terminal_psi, original_identity);
        let [machine_function] = machine_code.functions.as_slice() else {
            panic!("fixture must emit one machine function")
        };
        assert_eq!(machine_function.bytes, case.bytes);
        assert_eq!(machine_function.provenance.edges, [return_edge]);
        assert!(machine_function.provenance.operations.is_empty());
        let custody = machine_function
            .structural_return
            .as_ref()
            .expect("machine code must retain zero-runtime structural custody");
        assert_eq!(custody.psi_edge, return_edge);
        assert_eq!(custody.source, structural_parameter);
        assert_eq!(custody.result, structural_result);
        assert_eq!(custody.shape, ValueShape::integer(8, 8));
        assert_eq!(&custody.source_placement, source_placement);
        assert_eq!(&custody.result_placement, result_placement);
        assert_eq!(custody.returned_claims, [claim]);
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
            .last()
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

        #[cfg(unix)]
        if case.target == NativeTarget::host() {
            assert!(host_structural_round_trip(
                case.bytes,
                OPAQUE_REGION_IDENTITY
            ));
        }
    }
}

#[cfg(unix)]
fn host_structural_round_trip(bytes: &[u8], value: u64) -> bool {
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
    let driver = format!(
        "#include <stdint.h>\n\
extern uint64_t terminal_entry(uint64_t);\n\
int main(void) {{ return terminal_entry(UINT64_C({value})) == UINT64_C({value}) ? 0 : 1; }}\n"
    );
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
