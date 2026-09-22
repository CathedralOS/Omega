//! Hosted receiver harness: compile one attached-entry program through the
//! ordinary Psi front-end and the production fragment-emission ladder, bind the
//! target's exact hosted receiver bridge onto the emitted object, and replay —
//! or, where the host can execute the format, physically run — the sealed image.
//!
//! This replaces per-crate copies of the same recipe (image-emission's
//! `fragment_container`/`hosted_receiver` fixtures and the compiler crate's
//! per-target source-evaluated entry tests) for corpus programs shaped like a
//! provisioned `data Main` + `machine Main::main(&mut self)` entry. Entries that
//! take caller-supplied arguments cannot route through the bridge: the hosted
//! entry has a fixed physical signature per target, so those corpus legs stay
//! on `native_function`'s C caller.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use std::sync::Arc;

/// One compiled attached-entry program: the retained container custody, the
/// emitted object artifact built from it, and the selected source signature
/// the checked trees prove for the entry machine.
pub(crate) struct HostedEntry {
    pub container: Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    pub artifact: image_emission::ObjectArtifact,
    pub signature: program_entry_plan::SelectedProgramEntrySourceSignature,
}

/// Re-derive the source signature the entry route admits: the terminal
/// selection's own machine/state symbols, its normalized callable spelling,
/// and the receiver's normalized type identity when the entry carries
/// `&mut self`.
fn entry_signature(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
    profile: target::TargetProfile,
) -> program_entry_plan::SelectedProgramEntrySourceSignature {
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == machine_name)
        .expect("source-selected entry");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == selection.machine)
        .expect("checked source entry");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("entry state");
    let receiver = checked
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .map_or(
            program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            |parameter| {
                program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                    normalized_type_identity: checked
                        .normalized_type_identity(parameter.type_reference)
                        .into_string(),
                }
            },
        );
    let provisioned = matches!(
        receiver,
        program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable { .. }
    );
    program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        profile.program_entry_slot(),
        selection.machine,
        selection.machine,
        selection.name.clone(),
        "entry".into(),
        if provisioned {
            format!("test::{machine_name}(&mut self) -> Unit")
        } else {
            format!("test::{machine_name}() -> Unit")
        },
        receiver,
        Vec::new(),
    )
    .expect("selected source signature")
}

/// The exact admitted physical contract for each hosted bridge target,
/// mirroring the sealed package plans the production settlement carries.
pub(crate) fn physical_contract(
    profile: target::TargetProfile,
) -> program_entry_plan::ProgramEntryPhysicalContractPlan {
    let (slot, package, digest, requirement, plan, parameters, result) = match profile {
        target::TargetProfile::MacosArm64 => (
            target::TargetProfile::MacosArm64.program_entry_slot(),
            target::ProgramEntryPhysicalContractPackage::MacosArm64,
            program_entry_plan::exact_macos_arm64_physical_contract_package_source_digest(),
            program_entry_plan::MACOS_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
            program_entry_plan::exact_macos_arm64_physical_boundary_entry_plan(),
            vec![
                program_entry_plan::MACOS_ARM64_I32_TYPE_IDENTITY,
                program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            ],
            program_entry_plan::MACOS_ARM64_I32_TYPE_IDENTITY,
        ),
        target::TargetProfile::LinuxX64 => (
            target::TargetProfile::LinuxX64.program_entry_slot(),
            target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
            program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest(),
            program_entry_plan::LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
            program_entry_plan::exact_linux_x86_64_physical_boundary_entry_plan(),
            vec![program_entry_plan::LINUX_X86_64_ADDRESS_TYPE_IDENTITY],
            program_entry_plan::LINUX_X86_64_I32_TYPE_IDENTITY,
        ),
        target::TargetProfile::LinuxArm64 => (
            target::TargetProfile::LinuxArm64.program_entry_slot(),
            target::ProgramEntryPhysicalContractPackage::LinuxArm64,
            program_entry_plan::exact_linux_arm64_physical_contract_package_source_digest(),
            program_entry_plan::LINUX_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
            program_entry_plan::exact_linux_arm64_physical_boundary_entry_plan(),
            vec![program_entry_plan::LINUX_ARM64_U64_TYPE_IDENTITY],
            program_entry_plan::LINUX_ARM64_I32_TYPE_IDENTITY,
        ),
        target::TargetProfile::WindowsX64 => (
            target::TargetProfile::WindowsX64.program_entry_slot(),
            target::ProgramEntryPhysicalContractPackage::WindowsX64,
            program_entry_plan::exact_windows_x86_64_physical_contract_package_source_digest(),
            program_entry_plan::WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
            program_entry_plan::exact_windows_x86_64_physical_boundary_entry_plan(),
            Vec::new(),
            program_entry_plan::WINDOWS_X86_64_U32_TYPE_IDENTITY,
        ),
        _ => panic!("no hosted receiver bridge admits {profile:?}"),
    };
    program_entry_plan::ProgramEntryPhysicalContractPlan::new(
        slot,
        requirement.into(),
        package,
        digest,
        // The package-source report fingerprint is retained for diagnostics
        // only; contract custody replays the strong source digest instead.
        0,
        parameters.into_iter().map(str::to_owned).collect(),
        result.into(),
        plan.contract_report_fingerprint(),
        plan.plan().clone(),
    )
    .expect("exact admitted physical contract")
}

/// Compile `machine_name` in `source` for `target` and stage the retained
/// fragment custody the emitted object replays against.
fn fragment_container(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
    target: target::NativeTarget,
) -> Arc<object_file::StagedOptimizedRelocationFreeObjectContainer> {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name(machine_name),
    )
    .expect("entry machine must reach Terminal");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("semantic bytes");
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("proof bytes");
    let selections = optimization_core::OptimizationSelections::new([]).expect("empty selections");
    let optimized = native_realization::optimize_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("canonical artifact independently verifies and optimizes");
    let post_terminal = optimized.selections().project_post_terminal();
    let optimized_target =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
        )
        .expect("verified plan lowers to target operations");
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        optimized_target,
        post_terminal.selections(),
    )
    .expect("entry machine reaches physical realization");
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("function fragments");
    let framed = machine_emission::stage_function_fragment_frame_application(fragments)
        .expect("frame application");
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed)
        .expect("placed text section");
    Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed)
            .expect("relocation-free object container"),
    )
}

/// Compile one attached-entry source program for `profile`, emit its object
/// artifact through the fragment route, and replay the retained container
/// against the emitted object once so callers start from verified custody.
pub(crate) fn compile_attached_entry(
    source: &str,
    machine_name: &str,
    profile: target::TargetProfile,
) -> HostedEntry {
    let checked = crate::front_end::checked_program(source);
    let signature = entry_signature(&checked, machine_name, profile);
    let container = fragment_container(&checked, machine_name, profile.native_target());
    let artifact = image_emission::build_function_fragment_object_artifact(Arc::clone(&container))
        .expect("fragment object artifact");
    image_emission::validate_function_fragment_object_artifact(&container, &artifact)
        .expect("the retained container replays the emitted object");
    HostedEntry {
        container,
        artifact,
        signature,
    }
}

/// Bind the exact bridge the admitted settlement selects — same source
/// signature, same target-slot physical contract, empty Fused roster, and the
/// derived entry stack demand — onto `artifact` for `profile`.
pub(crate) fn bind_hosted_receiver(
    artifact: &mut image_emission::ObjectArtifact,
    signature: &program_entry_plan::SelectedProgramEntrySourceSignature,
    profile: target::TargetProfile,
) {
    let demand = image_emission::derive_stack_demand(artifact, artifact.entry())
        .expect("entry stack demand");
    // The differential fixtures attach receivers whose data needs no nominal
    // cleanup, so no hosted extent stays occupied through completion.
    image_emission::bind_hosted_receiver(
        artifact,
        signature,
        &physical_contract(profile),
        &[],
        &demand,
        false,
    )
    .expect("the exact admitted bridge binds the emitted object");
}

/// The subsystem argument for a profile's writer: ELF ignores it, Mach-O has
/// no subsystem field, and the PE writer reads the console-subsystem value.
fn writer_subsystem(profile: target::TargetProfile) -> u16 {
    match profile {
        target::TargetProfile::WindowsX64 => 3,
        _ => 0,
    }
}

/// Emit the bound image for `profile` without replaying it — for tests that
/// must observe emission itself failing closed on mutated custody.
pub(crate) fn emit_receiver_image_unchecked(
    artifact: &image_emission::ObjectArtifact,
    profile: target::TargetProfile,
) -> Result<image_emission::ExecutableImage, diagnostics::Diagnostic> {
    image_emission::emit_direct_executable_image(artifact, writer_subsystem(profile))
}

/// Emit and independently replay the bound image for `profile`.
pub(crate) fn emit_receiver_image(
    artifact: &image_emission::ObjectArtifact,
    profile: target::TargetProfile,
) -> image_emission::ExecutableImage {
    let image = emit_receiver_image_unchecked(artifact, profile).expect("receiver image emits");
    image_emission::validate_direct_executable_image(artifact, &image)
        .expect("the emitted bridge replays its exact custody");
    image
}

/// Write `bytes` to a private executable file and run it as a hosted process;
/// returns the exit status code and captured standard streams. Hosts without a
/// matching object format are expected to skip the calling leg instead.
#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
pub(crate) fn run_emitted_image(bytes: &[u8]) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "omega-hosted-receiver-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("create hosted receiver executable");
    let cleanup = ExecutableFile(path);
    file.write_all(bytes).expect("write complete emitted image");
    file.set_permissions(std::fs::Permissions::from_mode(0o700))
        .expect("make image executable");
    drop(file);
    let output = std::process::Command::new(&cleanup.0)
        .output()
        .expect("launch emitted hosted receiver image");
    (output.status.code(), output.stdout, output.stderr)
}

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
struct ExecutableFile(std::path::PathBuf);

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
impl Drop for ExecutableFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
