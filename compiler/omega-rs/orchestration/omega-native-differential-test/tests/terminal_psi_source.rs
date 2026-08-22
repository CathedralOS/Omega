//! Real-source proof that Psi emits a self-contained terminal module: frontend
//! trees are dropped before canonical decoding, verification, interpretation,
//! and Omega lowering.

use omega_artifacts::external_root_manifest_json;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineStateSet, RegisterSet, StateFootprintEvidence,
    evaluate_ordinary_boundary_entry_plan,
};
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
    AdmittedOpaqueFuelSuspensionFree, DynamicFuelMeterValidationReceiptId,
    DynamicNativeFuelMeterPlan, ExternalRootCandidate, ExternalRootId, FixedFuelProviderSummary,
    FuelExhaustionTransferPlanId, FuelProvisionId, FuelSuspensionValidationReceiptId,
    FuelValidationReceiptId, InstalledRootLedger, LogicalFuelResourceColumn,
    MachineStateResourceColumn, NativeFuelMeterPlanId, NestingRelationId,
    OpaqueProviderExitAssurance, ProviderExecution, ProviderExecutionId, ProviderFuelSummaryId,
    ProviderFuelValidationReceiptId, ProviderPlanId, ProviderStackSummary, RootAdmission,
    RootAdmissionId, RootProviderId, RootSlotAuthority, RootSlotId, RootSlotOwnerId,
    SponsorContextTransportId, StackNestingRelation, StackResourceColumn, StackValidationReceiptId,
    StateValidationReceiptId, TrustReceiptId, admit_fixed_native_fuel,
    bind_installed_dynamic_fuel_attributions, bind_installed_terminal_entry_fuel,
    bind_installed_terminal_entry_stack, bind_suspension_free_fixed_fuel, compose_artifact_stacks,
    compose_fixed_fuel, derive_fuel_suspension_free, validate_external_root,
    validate_installed_terminal_entry_fuel, validate_installed_terminal_entry_stack,
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
    decode_terminal_installation_record, derive_terminal_installation_stack_demand,
    derive_terminal_stack_demand, emit_terminal_executable_image, emit_terminal_object_container,
    encode_terminal_installation_record, validate_terminal_installation_record,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::{ArtifactLoweringError, lower_artifact_sections};
use omega_terminal_target_operations::{
    TerminalTargetBooleanControl, TerminalTargetBooleanExpression, TerminalTargetIntegerControl,
    TerminalTargetIntegerExpression, TerminalTargetOperation,
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
use psi_terminal::{
    CrashCause, CrashRouteGuard, OperationKind, TerminalModule, Terminator, VocabularyMarker,
};
use psi_terminal_codec::{
    DebugSubject, build_artifact_manifest, decode_debug_map, decode_module, decode_proof_bundle,
    encode_debug_map, encode_module, encode_proof_bundle, terminal_psi_identity,
    validate_artifact_manifest,
};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_interpreter::{
    MeasuredTerminalExecution, TerminalArtifactInterpretError, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    interpret_terminal_artifact_measured,
};
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

fn terminal_source_canary(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-native-differential-test lives under compiler/omega-rs/orchestration")
        .join("canaries/pass/terminal_psi")
        .join(name)
        .join("main.omg")
}

fn source_canary() -> PathBuf {
    terminal_source_canary("integer_control_contract")
}

fn artifact_sections(verified: &VerifiedTerminalModule<'_>) -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(verified.module()).expect("verified terminal semantics encode"),
        encode_proof_bundle(verified.proof_bundle()).expect("verified proof bundle encodes"),
    )
}

fn interpret_verified_artifact(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let (semantic_bytes, proof_bytes) = artifact_sections(verified);
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
    let (semantic_bytes, proof_bytes) = artifact_sections(verified);
    lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
}

fn start_verified_artifact(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecution, TerminalArtifactInterpretError> {
    let (semantic_bytes, proof_bytes) = artifact_sections(verified);
    TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        arguments,
    )
}

fn expected_crash(module: &TerminalModule) -> TerminalExecutionStatus {
    let crash = module.machines[0]
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => Some(psi_terminal_interpreter::TerminalCrash {
                edge: *edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            }),
            _ => None,
        })
        .expect("test module should contain a crash terminator");
    TerminalExecutionStatus::Crashed(crash)
}

fn collect_integer_crash_leaves(
    control: &TerminalTargetIntegerControl,
    output: &mut Vec<(EdgeId, CrashCause)>,
) {
    match control {
        TerminalTargetIntegerControl::Crash {
            psi_crash_edge,
            cause,
            ..
        } => output.push((*psi_crash_edge, *cause)),
        TerminalTargetIntegerControl::Conditional {
            when_true,
            when_false,
            ..
        }
        | TerminalTargetIntegerControl::ConditionalExpression {
            when_true,
            when_false,
            ..
        } => {
            collect_integer_crash_leaves(&when_true.control, output);
            collect_integer_crash_leaves(&when_false.control, output);
        }
        TerminalTargetIntegerControl::Return { .. } => {}
    }
}

fn target_integer_crash_leaves(operation: &TerminalTargetOperation) -> Vec<(EdgeId, CrashCause)> {
    let mut output = Vec::new();
    match operation {
        TerminalTargetOperation::Crash {
            psi_edge, cause, ..
        } => output.push((*psi_edge, *cause)),
        TerminalTargetOperation::ReturnIntegerConditionalControl {
            when_true,
            when_false,
            ..
        }
        | TerminalTargetOperation::ReturnIntegerExpressionConditionalControl {
            when_true,
            when_false,
            ..
        } => {
            collect_integer_crash_leaves(&when_true.control, &mut output);
            collect_integer_crash_leaves(&when_false.control, &mut output);
        }
        _ => {}
    }
    output.sort_unstable();
    output
}

fn assert_guarded_crash_emits(verified: &VerifiedTerminalModule<'_>) {
    let abstract_operations = lower_verified_artifact(verified)
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

#[path = "terminal_psi_source/admission_crashes_and_native.rs"]
mod admission_crashes_and_native;
#[path = "terminal_psi_source/comparisons_bitwise_and_casts.rs"]
mod comparisons_bitwise_and_casts;
#[path = "terminal_psi_source/contracts_and_frontend_drop.rs"]
mod contracts_and_frontend_drop;
#[path = "terminal_psi_source/control_graphs.rs"]
mod control_graphs;
#[path = "terminal_psi_source/divide_remainder_policies.rs"]
mod divide_remainder_policies;
#[path = "terminal_psi_source/exact_add_subtract_bounds.rs"]
mod exact_add_subtract_bounds;
#[path = "terminal_psi_source/exact_multiply_bounds.rs"]
mod exact_multiply_bounds;
#[path = "terminal_psi_source/exact_shift_bounds.rs"]
mod exact_shift_bounds;
#[path = "terminal_psi_source/locals_calls_and_short_circuit.rs"]
mod locals_calls_and_short_circuit;
#[path = "terminal_psi_source/runtime_policy_and_narrowing.rs"]
mod runtime_policy_and_narrowing;
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

    fn extent_provider_issuance(seed: u64) -> psi_extents::ExtentProviderIssuance {
        let base = seed * 16;
        psi_extents::ExtentProviderIssuance::from_normalized_identities([
            base + 1,
            base + 2,
            base + 3,
            base + 4,
            base + 5,
            base + 6,
            base + 7,
            base + 8,
            base + 9,
            base + 10,
            base + 11,
            base + 12,
            base + 13,
        ])
        .expect("normalized provider issuance")
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
        extent_provider_issuance(0x5331),
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
