//! Linux x86-64 process entry must provision the receiver; the test supplies
//! no pointer, storage grant, interpreter arguments, or replacement native
//! entry stub. The kernel arrives at the emitted ELF entry point through the
//! exact hosted bridge, which switches to a private stack, passes the
//! zero-initialized receiver in rdi under System V, reaches the semantic
//! continuation, and completes through exit_group.

use crate::{
    CanaryCompileProduct, CanaryCompileSpec, CompileReport, PathBuf, compile, fs, repo_root,
    unique_no_output_build_dir,
};
// Launching the emitted ELF is the Linux x86-64 leg alone; every arrival check
// in this file is gated to it and skips elsewhere.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use crate::{Command, Stdio};

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
/// selects between `Binding<Console>` and a bare interface field that must
/// fail closed; `explicit_exit` routes normal completion through
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
    builder.application("linux_hosted_receiver");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.select_provider<omega_language_std::Console, omega_language_std::ConsoleNativeProvider>();
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
        "Binding<Console>"
    } else {
        "Console"
    };
    fs::write(
        project.0.join("main.omg"),
        format!(
            r#"use omega_language_std::console;
use omega::language::core::binding;

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
    .expect("write receiver storage and fused Console customer");
    let result = compile(CanaryCompileSpec {
        root_path: project.0.join("main.omg"),
        build_dir: Some(project.0.join("build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    });
    if !bound_service {
        let diagnostics = result.expect_err("a bare interface field is not a service carrier");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("the intrinsic `Binding<R>` carrier is the only service value spelling")),
            "unexpected bare-carrier rejection: {diagnostics:#?}"
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
    builder.application("linux_hosted_receiver_sum_array");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.select_provider<omega_language_std::Console, omega_language_std::ConsoleNativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}}
"#
        ),
    )
    .expect("write authored target, entry, and provider selection");
    fs::write(
        project.0.join("main.omg"),
        r#"use omega_language_std::console;
use omega::language::core::binding;

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
    console: Binding<Console>;
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
    .expect("write widened receiver storage and fused Console customer");
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

/// A free Unit entry binds no service receiver, but the kernel still supplies
/// no return continuation: the product-owned process adapter calls the semantic
/// entry and completes through `exit_group` with status zero.
#[test]
fn linux_free_unit_entry_runs_and_completes_with_status_zero() {
    let directory = unique_no_output_build_dir();
    fs::create_dir(&directory).expect("create exclusively owned free-entry project");
    let project = HostedProject(directory);
    fs::write(
        project.0.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.application(\"linux_free_unit_entry\");\n    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);\n}\n",
    )
    .expect("write free-entry binding");
    fs::write(
        project.0.join("main.omg"),
        "data Main {\n    value: i32;\n}\n\nmachine Main::main(&mut self) {\n    self.value = 41;\n}\n",
    )
    .expect("write free Unit entry source");
    let arm64_report = compile(CanaryCompileSpec {
        root_path: project.0.join("main.omg"),
        build_dir: Some(project.0.join("build-arm64")),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    })
    .unwrap_or_else(|diagnostics| {
        panic!("free Unit entry must cross-produce its Linux ARM64 executable: {diagnostics:#?}")
    });
    let arm64_report = arm64_report
        .publish_retained_native_artifact(&project.0.join("build-arm64"))
        .expect("publish exact admitted ARM64 native artifact");
    let arm64_executable = arm64_report
        .checked_native_executable_path()
        .expect("exact ARM64 executable publication receipt");
    let arm64_bytes = fs::read(arm64_executable).expect("read published ARM64 ELF");
    assert_eq!(
        arm64_bytes.get(..4),
        Some([0x7f, 0x45, 0x4c, 0x46].as_slice()),
        "the published free-entry ARM64 artifact must be an ELF image"
    );
    assert_eq!(
        u16::from_le_bytes([arm64_bytes[18], arm64_bytes[19]]),
        183,
        "the published free-entry ARM64 artifact must declare the AArch64 machine"
    );
    let report = compile(CanaryCompileSpec {
        root_path: project.0.join("main.omg"),
        build_dir: Some(project.0.join("build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    })
    .unwrap_or_else(|diagnostics| {
        panic!("free Unit entry must produce its executable: {diagnostics:#?}")
    });
    let report = report
        .publish_retained_native_artifact(&project.0.join("build"))
        .expect("publish exact admitted native artifact");
    let executable = report
        .checked_native_executable_path()
        .expect("exact executable publication receipt");
    let bytes = fs::read(executable).expect("read published ELF");
    assert_eq!(
        bytes.get(..4),
        Some([0x7f, 0x45, 0x4c, 0x46].as_slice()),
        "the published free-entry artifact must be an ELF image"
    );

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // Execute the published bytes: without the adapter the ELF entry would
        // return into no continuation and fault.
        let output = Command::new(executable)
            .output()
            .expect("execute free Unit entry process");
        assert_eq!(
            output.status.code(),
            Some(0),
            "unexpected process completion: {output:?}"
        );
        assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    eprintln!("SKIP: free Unit entry runtime requires Linux x86-64; source cross-emission checked");
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
fn linux_hosted_receiver_rejects_bare_interface_field() {
    compile_and_run_linux_hosted_receiver(false, false);
}

/// The committed `samples/cli/basics/number_guess` application — a published
/// `Binding<Console>` receiver process carrying a `[copy]` search record and a
/// 256-byte pause buffer — compiles to a Linux x86-64 native artifact and runs
/// to its documented exit 70 through the same kernel entry bridge. The pause
/// read completes on EOF, so stdin is closed rather than scripted.
#[test]
fn linux_hosted_receiver_number_guess_runs_to_documented_exit_70() {
    let directory = unique_no_output_build_dir();
    fs::create_dir(&directory).expect("create exclusively owned number-guess project");
    let project = HostedProject(directory);
    // Publication consumes the retained artifact, so the image-backed
    // receiver binding is witnessed on a retained compile and the executable
    // on a second publish compile.
    compile(CanaryCompileSpec {
        root_path: repo_root().join("samples/cli/basics/number_guess/main.omg"),
        build_dir: Some(project.0.join("retain")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    })
    .unwrap_or_else(|diagnostics| {
        panic!("number_guess hosted receiver must produce its artifact: {diagnostics:#?}")
    })
    .retained_native_artifact()
    .expect("retain admitted native object")
    .object()
    .hosted_receiver_binding()
    .expect("number_guess provisions an image-backed receiver");
    let report = compile(CanaryCompileSpec {
        root_path: repo_root().join("samples/cli/basics/number_guess/main.omg"),
        build_dir: Some(project.0.join("build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .unwrap_or_else(|diagnostics| {
        panic!("number_guess hosted receiver must produce its executable: {diagnostics:#?}")
    });
    let executable = report
        .checked_native_executable_path()
        .expect("exact executable publication receipt");
    let bytes = fs::read(executable).expect("read published ELF");
    assert_eq!(
        bytes.get(..4),
        Some([0x7f, 0x45, 0x4c, 0x46].as_slice()),
        "the published number_guess artifact must be an ELF image"
    );

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let output = Command::new(executable)
            .stdin(Stdio::null())
            .output()
            .expect("execute published number_guess process");
        assert_eq!(
            output.status.code(),
            Some(70),
            "unexpected process completion: {output:?}"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("PASS: found 42 in exactly 7 guesses (exit 70)"),
            "expected the documented PASS line, got {stdout:?}"
        );
        assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    eprintln!("SKIP: number_guess runtime requires Linux x86-64; source cross-emission checked");
}
