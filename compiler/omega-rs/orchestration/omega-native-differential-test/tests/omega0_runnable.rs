use omega_calling_conventions::{CallSignature, ValueShape};
use omega_native_differential_test::admit_native_provider;
use omega_target::NativeTarget;
use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, LoweringError,
    lower_to_target_operations_with_provider_executions,
};
use omega_terminal_image_emission::{
    build_terminal_installation_record_with_provider_executions, build_terminal_object_artifact,
    decode_terminal_installation_record, emit_terminal_executable_image,
    encode_terminal_installation_record, validate_terminal_installation_record,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::lower_artifact_sections;
use omega_terminal_target_operations::{
    TerminalLinuxExitGroupI32Realization, TerminalLinuxWriteLineRealization,
};
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{BoundaryMachineId, IntegerValue, ProfileDecisionId};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::TerminalModule;
use psi_terminal_codec::{decode_module, encode_proof_bundle, terminal_psi_identity};
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};
use psi_terminal_verifier::ProofBundle;

const EXPECTED_STDOUT: &[u8] = b"Hello, Omega.\n";
const EXPECTED_STATUS: i32 = 0;

#[test]
fn canonical_omega0_agrees_from_terminal_meaning_through_runnable_linux_image() {
    let semantic = fixture_bytes();
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("canonical empty proof");
    let profile = AdmissionProfile::default();
    let decoded = decode_module(&semantic).expect("decode canonical vocabulary-25 O0 fixture");
    let (write_boundary, exit_boundary) = o0_boundaries(&decoded);

    let mut meaning = O0Meaning::new(write_boundary, exit_boundary);
    let mut execution = TerminalExecution::start_artifact(&semantic, &proof, &profile, &[])
        .expect("shared decode and verification admit canonical O0");
    let status = execution
        .resume_with_effect_handler(&mut TerminalFuelMeter::default(), &mut meaning)
        .expect("canonical O0 meaning executes");
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meaning.stdout, EXPECTED_STDOUT);
    assert_eq!(meaning.exit_status, Some(EXPECTED_STATUS));
    assert_eq!(execution.effects().len(), 2);

    let abstract_plan = lower_artifact_sections(&semantic, &proof, &profile)
        .expect("Omega consumes only decoded and verified O0 sections");
    assert_eq!(
        abstract_plan.terminal_psi,
        terminal_psi_identity(&decoded).expect("canonical O0 identity")
    );
    assert!(
        abstract_plan
            .functions
            .iter()
            .find(|function| function.machine == abstract_plan.entry)
            .expect("O0 entry function")
            .attachment
            .is_some(),
        "the authored Main attachment must survive the artifact boundary"
    );

    let mut target_images = Vec::new();
    for (target, seed) in [
        (NativeTarget::linux_x64(), 10_000_u64),
        (NativeTarget::linux_arm64(), 20_000_u64),
    ] {
        let write_identity = boundary_identity(&decoded, write_boundary);
        let exit_identity = boundary_identity(&decoded, exit_boundary);
        let write_provider = admit_native_provider(
            target,
            write_identity,
            seed,
            CallSignature {
                parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
                result: None,
            },
        );
        let exit_provider = admit_native_provider(
            target,
            exit_identity,
            seed + 1_000,
            CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result: None,
            },
        );
        let settlements = [
            AdmittedTerminalBoundarySettlement {
                boundary: write_boundary,
                provider_execution: &write_provider,
                realization: TerminalLinuxWriteLineRealization.into(),
            },
            AdmittedTerminalBoundarySettlement {
                boundary: exit_boundary,
                provider_execution: &exit_provider,
                realization: TerminalLinuxExitGroupI32Realization.into(),
            },
        ];

        let image = realize_image(&abstract_plan, target, &settlements);
        let repeat = realize_image(&abstract_plan, target, &settlements);
        assert_eq!(image.output().bytes, repeat.output().bytes);
        assert_eq!(
            image.output().final_text_bytes,
            repeat.output().final_text_bytes
        );
        assert_eq!(image.terminal_psi(), abstract_plan.terminal_psi);
        assert_eq!(image.boundary_settlements().len(), 2);
        assert!(image.functions()[0].attachment.is_some());

        let installation = build_terminal_installation_record_with_provider_executions(
            &image,
            ProfileDecisionId::new(seed).expect("profile decision"),
            [&write_provider, &exit_provider],
        )
        .expect("exact two-provider O0 installation closes");
        validate_terminal_installation_record(&installation, &image)
            .expect("O0 installation independently replays the image");
        let installation_bytes =
            encode_terminal_installation_record(&installation).expect("installation encoding");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation)
        );

        target_images.push((target, image.output().bytes.clone()));
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    assert_native_observable(
        &target_images
            .iter()
            .find(|(target, _)| target.architecture == omega_target::Architecture::X86_64)
            .expect("x86-64 image")
            .1,
        &meaning.stdout,
        meaning.exit_status.expect("meaning exit status"),
    );

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    assert_native_observable(
        &target_images
            .iter()
            .find(|(target, _)| target.architecture == omega_target::Architecture::Aarch64)
            .expect("AArch64 image")
            .1,
        &meaning.stdout,
        meaning.exit_status.expect("meaning exit status"),
    );
}

#[test]
fn native_o0_lowering_rejects_a_provider_admitted_for_another_requirement() {
    let semantic = fixture_bytes();
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("canonical empty proof");
    let decoded = decode_module(&semantic).expect("decode fixture");
    let (write_boundary, exit_boundary) = o0_boundaries(&decoded);
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified O0 plan");
    let target = NativeTarget::linux_x64();
    let wrong_write_provider = admit_native_provider(
        target,
        boundary_identity(&decoded, exit_boundary),
        30_000,
        CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
            result: None,
        },
    );
    let exit_provider = admit_native_provider(
        target,
        boundary_identity(&decoded, exit_boundary),
        31_000,
        CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result: None,
        },
    );
    assert!(matches!(
        lower_to_target_operations_with_provider_executions(
            &plan,
            target,
            &[
                AdmittedTerminalBoundarySettlement {
                    boundary: write_boundary,
                    provider_execution: &wrong_write_provider,
                    realization: TerminalLinuxWriteLineRealization.into(),
                },
                AdmittedTerminalBoundarySettlement {
                    boundary: exit_boundary,
                    provider_execution: &exit_provider,
                    realization: TerminalLinuxExitGroupI32Realization.into(),
                },
            ],
        ),
        Err(LoweringError::ProviderExecutionRequirementMismatch { boundary, .. })
            if boundary == write_boundary
    ));
}

fn fixture_bytes() -> Vec<u8> {
    let hex = include_str!("../../../../omega/fixtures/omega0-terminal-v25.hex");
    let digits = hex
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    digits
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => panic!("fixture contains non-hex input"),
            };
            digit(pair[0]) << 4 | digit(pair[1])
        })
        .collect()
}

fn o0_boundaries(module: &TerminalModule) -> (BoundaryMachineId, BoundaryMachineId) {
    let find = |name: &str| {
        module
            .boundary_machines
            .iter()
            .find(|boundary| boundary.identity.contains(name))
            .unwrap_or_else(|| panic!("canonical O0 boundary {name}"))
            .id
    };
    (find("Console::write_line"), find("Console::exit_process"))
}

fn boundary_identity(module: &TerminalModule, id: BoundaryMachineId) -> &str {
    &module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.id == id)
        .expect("O0 boundary declaration")
        .identity
}

#[derive(Debug)]
struct O0Meaning {
    write_boundary: BoundaryMachineId,
    exit_boundary: BoundaryMachineId,
    stdout: Vec<u8>,
    exit_status: Option<i32>,
}

impl O0Meaning {
    fn new(write_boundary: BoundaryMachineId, exit_boundary: BoundaryMachineId) -> Self {
        Self {
            write_boundary,
            exit_boundary,
            stdout: Vec::new(),
            exit_status: None,
        }
    }

    fn reject(reason: &str) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection {
            reason: reason.into(),
        })
    }
}

impl TerminalEffectHandler for O0Meaning {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            result,
            ..
        } = effect
        else {
            return Self::reject("O0 admits only Console boundary effects");
        };
        if result.is_some() || self.exit_status.is_some() {
            return Self::reject("O0 effect after exit or with a result");
        }
        if *boundary == self.write_boundary {
            let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
                return Self::reject("write_line requires one exact byte-sequence literal");
            };
            if !arguments.is_empty() || structural_arguments.len() != 1 {
                return Self::reject("write_line operand shape drifted");
            }
            self.stdout.extend_from_slice(bytes);
            self.stdout.push(b'\n');
            return Ok(());
        }
        if *boundary == self.exit_boundary {
            let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice() else {
                return Self::reject("exit_process requires one integer");
            };
            if !structural_arguments.is_empty() || !byte_sequence_arguments.is_empty() {
                return Self::reject("exit_process operand shape drifted");
            }
            let value = match value {
                IntegerValue::Signed(value) => *value,
                IntegerValue::Unsigned(value) => *value as i128,
            };
            self.exit_status = Some((value as u8) as i32);
            return Ok(());
        }
        Self::reject("unknown O0 boundary")
    }
}

fn realize_image(
    plan: &omega_terminal_abstract_operations::TerminalAbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedTerminalBoundarySettlement<'_>],
) -> omega_terminal_image_emission::TerminalExecutableImage {
    let target_plan =
        lower_to_target_operations_with_provider_executions(plan, target, settlements)
            .expect("exact admitted O0 providers lower");
    let assigned = assign_registers(&target_plan).expect("O0 register assignment");
    let machine = emit_machine_code(&assigned).expect("O0 machine-code emission");
    let object = build_terminal_object_artifact(&machine)
        .unwrap_or_else(|error| panic!("O0 object replay for {target:?}: {error:?}\n{machine:#?}"));
    emit_terminal_executable_image(&object, 3).expect("O0 Linux executable image")
}

#[cfg(target_os = "linux")]
fn assert_native_observable(bytes: &[u8], stdout: &[u8], exit_status: i32) {
    use std::os::unix::fs::PermissionsExt;

    let directory = std::env::temp_dir().join(format!(
        "omega0-runnable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir(&directory).expect("create O0 execution directory");
    let executable = directory.join("omega0");
    std::fs::write(&executable, bytes).expect("write O0 image");
    let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).expect("make O0 image executable");
    let output = std::process::Command::new(&executable)
        .output()
        .expect("execute O0 image");
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(output.stdout, stdout);
    assert!(output.stderr.is_empty());
    assert_eq!(output.status.code(), Some(exit_status));
}
