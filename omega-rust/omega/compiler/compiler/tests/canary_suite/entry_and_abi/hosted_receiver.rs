//! Real process entry must provision the receiver; the test supplies no pointer,
//! storage grant, interpreter arguments, or replacement native entry stub.

use super::*;

struct HostedProject(PathBuf);

impl Drop for HostedProject {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "retained failed hosted receiver observations: {}",
                self.0.display()
            );
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

enum ReceiverObservation {
    ScalarMutation,
    BorrowedRecordCopy,
    BorrowedResultSnapshot,
    LocalBorrowedResultSnapshot,
}

fn compile_and_run_hosted_receiver(
    explicit_exit: bool,
    bound_service: bool,
    observation: ReceiverObservation,
) {
    let directory = unique_no_output_build_dir();
    fs::create_dir(&directory).expect("create exclusively owned hosted-entry project");
    let project = HostedProject(directory);
    let standard_library = repo_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        project.0.join("build.omg"),
        format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("hosted-receiver");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}}
"#
        ),
    )
    .expect("write authored target, entry, and provider selection");
    let completion = if explicit_exit {
        "self.console.exit_process(37);"
    } else {
        ""
    };
    let console_type = if bound_service {
        "Service<Console> in Bound"
    } else {
        "Console"
    };
    let (extra_fields, initialization, observation_condition, receiver_bytes) = match observation {
        ReceiverObservation::ScalarMutation => ("", "self.value = 65;", "self.value == 65", 260),
        ReceiverObservation::BorrowedRecordCopy => (
            "source: Counter; destination: Counter;",
            "self.source.value = 65; copy_counter(&self.source, &mut self.destination); self.value = self.destination.value;",
            "self.value == 65",
            268,
        ),
        ReceiverObservation::BorrowedResultSnapshot => (
            "source: Counter;",
            "self.source.value = 65; let saved: i32 = self.source.read(); self.source.write(66); let current: i32 = self.source.read(); self.value = saved;",
            "saved == 65 && current == 66",
            264,
        ),
        ReceiverObservation::LocalBorrowedResultSnapshot => (
            "",
            "let mut counter: Counter = Counter::new(65); let saved: i32 = counter.read(); counter.write(66); let current: i32 = counter.read(); self.value = saved;",
            "saved == 65 && current == 66",
            260,
        ),
    };
    fs::write(
        project.0.join("main.omg"),
        format!(
            r#"use omega_language_std::console;
use omega::language::core::service;

data Counter {{ value: i32; }}
machine copy_counter(source: &Counter, destination: &mut Counter) {{
    destination.value = source.value;
}}
machine Counter::read(&self) -> i32 {{ self.value }}
machine Counter::write(&mut self, value: i32) {{ self.value = value; }}
machine Counter::new(value: i32) -> Counter {{ Counter {{ value: value }} }}

data Main {{
    value: i32;
    bytes: [u8; 256];
    {extra_fields}
    console: {console_type};
}}

machine Main::main(&mut self) reaches Console {{
    transition self.value == 0 {{
        true -> initialized()
        false -> failed()
    }}
    state initialized(&mut self) {{
        {initialization}
        transition {observation_condition} {{
            true -> observed()
            false -> failed()
        }}
    }}
    state observed(&mut self) {{
        self.console.write_byte(self.value);
        {completion}
    }}
    state failed(&mut self) {{
        self.console.write_byte(70);
    }}
}}
"#
        ),
    )
    .expect("write receiver storage and Fused Console customer");
    let result = compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: project.0.join("main.omg"),
        build_dir: Some(project.0.join("build")),
        target_name: Some("macos_arm64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    });
    if !bound_service {
        let diagnostics = result.expect_err("a bare interface field supplies no Bound occurrence");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "macOS hosted receiver bridge lost exact contract, storage, or entry custody"
                )),
            "unexpected missing-establishment rejection: {diagnostics:#?}"
        );
        return;
    }
    let report = result.unwrap_or_else(|diagnostics| {
        panic!("authored hosted receiver must produce its executable: {diagnostics:#?}")
    });
    let receiver = report
        .retained_native_artifact()
        .unwrap()
        .object()
        .hosted_receiver_binding()
        .expect("retain exact provisioned receiver");
    assert_eq!(
        receiver.receiver_byte_count(),
        receiver_bytes,
        "scalar and fixed array both occupy the image-backed receiver"
    );
    if !explicit_exit {
        assert_hosted_binding_replay_rejects_corruption(&report);
    }
    let report = report
        .publish_retained_native_artifact(&project.0.join("build"))
        .expect("publish exact admitted native artifact after corruption controls");
    let executable = report
        .checked_native_executable_path()
        .expect("exact executable publication receipt");
    let bytes = fs::read(executable).expect("read published signed Mach-O");
    assert_eq!(bytes.get(..4), Some([0xcf, 0xfa, 0xed, 0xfe].as_slice()));

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        // Execute the published bytes without a host wrapper or re-signing.
        let output = Command::new(executable)
            .output()
            .expect("execute authored hosted receiver process");
        assert_eq!(
            output.status.code(),
            Some(if explicit_exit { 37 } else { 0 }),
            "unexpected process completion: {output:?}"
        );
        assert_eq!(
            output.stdout, b"A",
            "receiver must begin at zero and retain its write"
        );
        assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    eprintln!(
        "SKIP: hosted receiver runtime requires macOS AArch64; source cross-emission checked"
    );
}

fn assert_hosted_binding_replay_rejects_corruption(report: &CompileReport) {
    let native = report
        .retained_native_artifact()
        .expect("retain admitted native object");
    let object = native.object();
    let fragments = object
        .fragment_source_for_test()
        .expect("retain physical fragment source");
    image_emission::validate_function_fragment_object_artifact(fragments, object)
        .expect("complete bound object must independently replay");

    let mut receiver_size = object.clone();
    *receiver_size
        .hosted_receiver_byte_count_mut_for_test()
        .unwrap() += 1;
    let mut stack_demand = object.clone();
    *stack_demand
        .hosted_receiver_stack_ceiling_mut_for_test()
        .unwrap() += 16;
    let mut receiver_source = object.clone();
    let source = receiver_source
        .hosted_receiver_source_mut_for_test()
        .unwrap();
    *source = program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        source.target_slot(),
        source.machine_symbol(),
        source.state_symbol(),
        source.machine_name().into(),
        source.state_name().into(),
        source.normalized_callable_identity().into(),
        program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
        source.visible_parameters().to_vec(),
    )
    .expect("free declaration is valid alone, but cannot replace this receiver occurrence");
    for (name, corrupted) in [
        ("receiver size", receiver_size),
        ("stack demand", stack_demand),
        ("source receiver", receiver_source),
    ] {
        assert!(
            image_emission::validate_function_fragment_object_artifact(fragments, &corrupted)
                .is_err(),
            "independent fragment replay must reject changed {name}"
        );
    }
}

#[test]
fn hosted_receiver_normal_return_provisions_zii_storage_and_fused_console() {
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::ScalarMutation);
}

#[test]
fn hosted_receiver_explicit_process_exit_preserves_its_distinct_outcome() {
    compile_and_run_hosted_receiver(true, true, ReceiverObservation::ScalarMutation);
}

#[test]
fn hosted_receiver_rejects_bare_interface_without_bound_establishment() {
    compile_and_run_hosted_receiver(false, false, ReceiverObservation::ScalarMutation);
}

#[test]
fn hosted_receiver_observes_copy_between_disjoint_borrowed_records() {
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::BorrowedRecordCopy);
}

#[test]
fn hosted_receiver_keeps_scalar_call_results_across_later_mutation() {
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::BorrowedResultSnapshot);
}

#[test]
fn hosted_receiver_keeps_local_scalar_call_results_across_later_mutation() {
    compile_and_run_hosted_receiver(
        false,
        true,
        ReceiverObservation::LocalBorrowedResultSnapshot,
    );
}
