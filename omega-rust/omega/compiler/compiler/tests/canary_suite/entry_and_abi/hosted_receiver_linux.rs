//! Linux x86-64 process entry must provision the receiver; the test supplies
//! no pointer, storage grant, interpreter arguments, or replacement native
//! entry stub. The kernel arrives at the emitted ELF entry point through the
//! exact hosted bridge, which switches to a private stack, passes the
//! zero-initialized receiver in rdi under System V, reaches the semantic
//! continuation, and completes through exit_group.

use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, CompileReport, PathBuf, compile, fs,
    repo_root, unique_no_output_build_dir,
};

struct HostedProject(PathBuf);

impl Drop for HostedProject {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "retained failed Linux hosted receiver observations: {}",
                self.0.display()
            );
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Compile one authored Linux x86-64 receiver application. `bound_service`
/// selects between `Service<Console> in Bound` and a bare interface field that
/// must fail closed; `explicit_exit` routes normal completion through
/// `exit_process(37)` so the provider's own status survives the bridge.
fn compile_and_run_linux_hosted_receiver(explicit_exit: bool, bound_service: bool) {
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
    builder.application("linux-hosted-receiver");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
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
    fs::write(
        project.0.join("main.omg"),
        format!(
            r#"use omega_language_std::console;
use omega::language::core::service;

data Main {{
    value: i32;
    bytes: [u8; 256];
    console: {console_type};
}}

machine Main::main(&mut self) reaches Console {{
    transition self.value == 0 {{
        true -> initialized()
        false -> failed()
    }}
    state initialized(&mut self) {{
        self.value = 65;
        self.bytes[255] = 64;
        transition self.value == 65 && self.bytes[255] == 64 && self.bytes[254] == 0 {{
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
    .expect("write receiver storage and Bound Console customer");
    let result = compile(CanaryCompileSpec {
        root_path: project.0.join("main.omg"),
        build_dir: Some(project.0.join("build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    });
    if !bound_service {
        let diagnostics = result.expect_err("a bare interface field supplies no Bound occurrence");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "Linux x86-64 hosted receiver bridge lost exact contract, storage, or entry custody"
                )),
            "unexpected missing-establishment rejection: {diagnostics:#?}"
        );
        return;
    }
    let report = result.unwrap_or_else(|diagnostics| {
        panic!("authored Linux hosted receiver must produce its executable: {diagnostics:#?}")
    });
    let receiver = report
        .retained_native_artifact()
        .unwrap()
        .object()
        .hosted_receiver_binding()
        .expect("retain exact provisioned receiver");
    assert_eq!(
        receiver.receiver_byte_count(),
        260,
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
    let bytes = fs::read(executable).expect("read published ELF");
    assert_eq!(
        bytes.get(..4),
        Some([0x7f, 0x45, 0x4c, 0x46].as_slice()),
        "the published hosted receiver must be an ELF image"
    );

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // Execute the published bytes without a host wrapper: the kernel
        // arrives through rsp only, so a working bridge is the only way the
        // receiver reaches the semantic continuation.
        let output = Command::new(executable)
            .output()
            .expect("execute authored Linux hosted receiver process");
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
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    eprintln!("SKIP: hosted receiver runtime requires Linux x86-64; source cross-emission checked");
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

/// A receiver carrying nested records, structural arrays, a closed sum, and
/// a mixed declaration provisions all of them under the same ZII contract:
/// record fields, element payloads, common fields, and the first declared
/// case's payload begin established at zero. The entry transition observes
/// `self.event in Event::Loud` — `Loud` is declared first, so the zero tag
/// selects its zeroed payload case — and nested/indexed scalar stores
/// survive the bridge round trip.
#[test]
fn linux_hosted_receiver_provisions_record_arrays_and_the_zero_tag_sum_case() {
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
    builder.application("linux-hosted-receiver-sum-array");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}}
"#
        ),
    )
    .expect("write authored target, entry, and provider selection");
    fs::write(
        project.0.join("main.omg"),
        r#"use omega_language_std::console;
use omega::language::core::service;

data Pair {
    first: i32;
    second: i32;
}

data Event {
    case Loud(gain: i32);
    case Quiet;
}

data Mixed {
    flag: i32;
    case Flat;
    case Raised(level: i32);
}

data Main {
    value: i32;
    pair: Pair;
    grid: [Pair; 2];
    tags: [Event; 2];
    event: Event;
    mixed: Mixed;
    console: Service<Console> in Bound;
}

machine Main::main(&mut self) reaches Console {
    transition self.value == 0 && self.pair.first == 0 && self.pair.second == 0
        && self.event in Event::Loud
        && self.tags[0] in Event::Loud && self.tags[1] in Event::Loud {
        true -> initialized()
        false -> failed()
    }
    state initialized(&mut self) {
        self.pair.second = 63;
        self.grid[1].second = 63;
        self.value = 65;
        transition self.value == 65 && self.pair.second == 63 {
            true -> observed()
            false -> failed()
        }
    }
    state observed(&mut self) {
        self.console.write_byte(self.value);
    }
    state failed(&mut self) {
        self.console.write_byte(70);
    }
}
"#,
    )
    .expect("write widened receiver storage and Bound Console customer");
    let report = compile(CanaryCompileSpec {
        root_path: project.0.join("main.omg"),
        build_dir: Some(project.0.join("build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    })
    .unwrap_or_else(|diagnostics| {
        panic!("widened Linux hosted receiver must produce its executable: {diagnostics:#?}")
    });
    let receiver = report
        .retained_native_artifact()
        .unwrap()
        .object()
        .hosted_receiver_binding()
        .expect("retain exact provisioned receiver");
    assert_eq!(
        receiver.receiver_byte_count(),
        64,
        "nested record, record array, sum array, sum, and mixed storage all occupy the image-backed receiver"
    );
    assert_hosted_binding_replay_rejects_corruption(&report);
    let report = report
        .publish_retained_native_artifact(&project.0.join("build"))
        .expect("publish exact admitted native artifact after corruption controls");
    let executable = report
        .checked_native_executable_path()
        .expect("exact executable publication receipt");
    let bytes = fs::read(executable).expect("read published ELF");
    assert_eq!(
        bytes.get(..4),
        Some([0x7f, 0x45, 0x4c, 0x46].as_slice()),
        "the published hosted receiver must be an ELF image"
    );

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // Execute the published bytes without a host wrapper: the kernel
        // arrives through rsp only, so a working bridge is the only way the
        // receiver reaches the semantic continuation.
        let output = Command::new(executable)
            .output()
            .expect("execute authored Linux hosted receiver process");
        assert_eq!(
            output.status.code(),
            Some(0),
            "unexpected process completion: {output:?}"
        );
        assert_eq!(
            output.stdout, b"A",
            "receiver must begin at zero and retain its writes"
        );
        assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    eprintln!("SKIP: hosted receiver runtime requires Linux x86-64; source cross-emission checked");
}

#[test]
fn linux_hosted_receiver_normal_return_provisions_zii_storage_and_fused_console() {
    compile_and_run_linux_hosted_receiver(false, true);
}

#[test]
fn linux_hosted_receiver_explicit_process_exit_preserves_its_distinct_outcome() {
    compile_and_run_linux_hosted_receiver(true, true);
}

#[test]
fn linux_hosted_receiver_rejects_bare_interface_without_bound_establishment() {
    compile_and_run_linux_hosted_receiver(false, false);
}
