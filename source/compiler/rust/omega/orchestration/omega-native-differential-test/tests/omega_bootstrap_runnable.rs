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
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::TerminalModule;
use psi_terminal_codec::{decode_module, encode_proof_bundle, terminal_psi_identity};
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};
use psi_terminal_verifier::ProofBundle;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const EXPECTED_STDOUT: &[u8] = b"Hello, Omega.\n";
const EXPECTED_STATUS: i32 = 0;

fn straight_line_console_source(write_count: usize, exit_status: i32) -> String {
    let writes = (0..write_count)
        .map(|index| format!("        self.console.write_line(\"line-{index:02}\");\n"))
        .collect::<String>();
    format!(
        r#"
    boundary trait Console {{
        machine write_line(text: &[u8])
        reaches Console;
        machine exit_process(return_code: i32)
        reaches Console;
    }}

    data Main {{ console: Console; }}
    machine Main::main(&mut self)
    reaches Console
    {{
{writes}        self.console.exit_process({exit_status});
    }}
"#
    )
}

fn project_source(source: &str) -> (Vec<u8>, Vec<u8>) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize O1 source");
    let syntax = parse_syntax_trees(&tokens).expect("parse O1 source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve O1 source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type O1 source");
    let checked = lower_typed_trees(typed).expect("check O1 source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::main")
        .expect("lower O1 source to terminal Psi");
    (
        psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("encode O1 terminal Psi"),
        encode_proof_bundle(&lowered.proof_bundle).expect("encode O1 proof bundle"),
    )
}

fn numbered_stdout(count: usize) -> Vec<u8> {
    (0..count)
        .flat_map(|index| format!("line-{index:02}\n").into_bytes())
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    output
}

#[test]
fn canonical_o0_agrees_from_terminal_meaning_through_runnable_linux_image() {
    let semantic = fixture_bytes();
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("canonical empty proof");
    let profile = AdmissionProfile::default();
    let decoded = decode_module(&semantic).expect("decode canonical vocabulary-28 O0 fixture");
    let (write_boundary, exit_boundary) = o0_boundaries(&decoded);

    let mut meaning = O0Meaning::new(Some(write_boundary), exit_boundary);
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

    if let Ok(path) = std::env::var("OMEGA_BOOTSTRAP_WRITE_X64_IMAGE")
        .or_else(|_| std::env::var("OMEGA0_WRITE_X64_IMAGE"))
    {
        let bytes = &target_images
            .iter()
            .find(|(target, _)| target.architecture == omega_target::Architecture::X86_64)
            .expect("x86-64 image")
            .1;
        std::fs::write(path, bytes).expect("write requested x86-64 O0 image");
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
fn straight_line_console_o1_agrees_for_zero_one_two_and_sixteen_writes() {
    let profile = AdmissionProfile::default();
    let targets = [
        (NativeTarget::linux_x64(), 40_000_u64),
        (NativeTarget::linux_arm64(), 50_000_u64),
    ];
    let mut exports = Vec::new();

    for write_count in [0, 1, 2, 16] {
        let exit_status = write_count as i32;
        let expected_stdout = numbered_stdout(write_count);
        let (semantic, proof) =
            project_source(&straight_line_console_source(write_count, exit_status));
        let decoded = decode_module(&semantic).expect("decode projected O1 terminal Psi");
        let exit_boundary = find_boundary(&decoded, "Console::exit_process");
        let write_boundary = decoded
            .boundary_machines
            .iter()
            .find(|boundary| boundary.identity.contains("Console::write_line"))
            .map(|boundary| boundary.id);
        assert_eq!(write_boundary.is_some(), write_count > 0);

        let mut meaning = O0Meaning::new(write_boundary, exit_boundary);
        let mut execution = TerminalExecution::start_artifact(&semantic, &proof, &profile, &[])
            .expect("shared decode and verification admit projected O1");
        let status = execution
            .resume_with_effect_handler(&mut TerminalFuelMeter::default(), &mut meaning)
            .expect("projected O1 meaning executes");
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(meaning.stdout, expected_stdout);
        assert_eq!(meaning.exit_status, Some(exit_status));
        assert_eq!(execution.effects().len(), write_count + 1);

        let abstract_plan = lower_artifact_sections(&semantic, &proof, &profile)
            .expect("Omega consumes projected and verified O1 sections");
        assert_eq!(
            abstract_plan.terminal_psi,
            terminal_psi_identity(&decoded).expect("projected O1 identity")
        );

        let mut target_images = Vec::new();
        for (target, seed) in targets {
            let exit_provider = admit_native_provider(
                target,
                boundary_identity(&decoded, exit_boundary),
                seed + write_count as u64 * 100,
                CallSignature {
                    parameters: vec![ValueShape::integer(4, 4)],
                    result: None,
                },
            );
            let write_provider = write_boundary.map(|boundary| {
                admit_native_provider(
                    target,
                    boundary_identity(&decoded, boundary),
                    seed + write_count as u64 * 100 + 50,
                    CallSignature {
                        parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
                        result: None,
                    },
                )
            });
            let mut settlements = Vec::new();
            if let (Some(boundary), Some(provider)) = (write_boundary, write_provider.as_ref()) {
                settlements.push(AdmittedTerminalBoundarySettlement {
                    boundary,
                    provider_execution: provider,
                    realization: TerminalLinuxWriteLineRealization.into(),
                });
            }
            settlements.push(AdmittedTerminalBoundarySettlement {
                boundary: exit_boundary,
                provider_execution: &exit_provider,
                realization: TerminalLinuxExitGroupI32Realization.into(),
            });

            let image = realize_image(&abstract_plan, target, &settlements);
            let repeat = realize_image(&abstract_plan, target, &settlements);
            assert_eq!(image.output().bytes, repeat.output().bytes);
            assert_eq!(
                image.output().final_text_bytes,
                repeat.output().final_text_bytes
            );
            assert_eq!(image.boundary_settlements().len(), write_count + 1);

            let mut provider_executions = Vec::new();
            if let Some(provider) = write_provider.as_ref() {
                provider_executions.push(provider);
            }
            provider_executions.push(&exit_provider);
            let installation = build_terminal_installation_record_with_provider_executions(
                &image,
                ProfileDecisionId::new(seed + write_count as u64 + 1).expect("O1 profile decision"),
                provider_executions,
            )
            .expect("exact O1 installation closes");
            validate_terminal_installation_record(&installation, &image)
                .expect("O1 installation independently replays the image");

            target_images.push((target, image.output().bytes.clone()));
        }

        let x64 = target_images
            .iter()
            .find(|(target, _)| target.architecture == omega_target::Architecture::X86_64)
            .expect("x86-64 O1 image")
            .1
            .clone();

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert_native_observable(&x64, &expected_stdout, exit_status);

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        assert_native_observable(
            &target_images
                .iter()
                .find(|(target, _)| target.architecture == omega_target::Architecture::Aarch64)
                .expect("AArch64 O1 image")
                .1,
            &expected_stdout,
            exit_status,
        );

        exports.push((write_count, exit_status, expected_stdout, semantic, x64));
    }

    if let Some(directory) = std::env::var_os("OMEGA1_WRITE_REFERENCE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("create O1 reference directory");
        let mut manifest = String::from("case\twrites\texit\tstdout_hex\tterminal\tx86_64_image\n");
        for (write_count, exit_status, stdout, terminal, x64) in exports {
            let terminal_file = format!("writes-{write_count}.terminal");
            let image_file = format!("writes-{write_count}.x86_64.elf");
            std::fs::write(directory.join(&terminal_file), terminal)
                .expect("write requested O1 terminal reference");
            std::fs::write(directory.join(&image_file), x64)
                .expect("write requested O1 x86-64 image reference");
            manifest.push_str(&format!(
                "writes-{write_count}\t{write_count}\t{exit_status}\t{}\t{terminal_file}\t{image_file}\n",
                hex_bytes(&stdout)
            ));
        }
        std::fs::write(directory.join("manifest.tsv"), manifest)
            .expect("write requested O1 reference manifest");
    }
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
    let hex = include_str!(
        "../../../../../../../bootstrap/omega-bootstrap/gates/fixtures/omega-bootstrap-terminal-v28.hex"
    );
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
    (
        find_boundary(module, "Console::write_line"),
        find_boundary(module, "Console::exit_process"),
    )
}

fn find_boundary(module: &TerminalModule, name: &str) -> BoundaryMachineId {
    module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity.contains(name))
        .unwrap_or_else(|| panic!("canonical console boundary {name}"))
        .id
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
    write_boundary: Option<BoundaryMachineId>,
    exit_boundary: BoundaryMachineId,
    stdout: Vec<u8>,
    exit_status: Option<i32>,
}

impl O0Meaning {
    fn new(write_boundary: Option<BoundaryMachineId>, exit_boundary: BoundaryMachineId) -> Self {
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
        if Some(*boundary) == self.write_boundary {
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
        "omega-bootstrap-runnable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir(&directory).expect("create O0 execution directory");
    let executable = directory.join("omega-bootstrap");
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
