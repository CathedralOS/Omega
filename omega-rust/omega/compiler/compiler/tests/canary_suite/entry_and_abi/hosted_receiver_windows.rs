//! Windows x86-64 process entry must provision the receiver; the test
//! supplies no pointer, storage grant, interpreter arguments, or replacement
//! native entry stub. The loader arrives at the emitted PE
//! `AddressOfEntryPoint` under Microsoft x64; the exact hosted bridge
//! preserves the incoming stack continuation in its own partition, switches
//! to a private stack with callee shadow space, passes the zero-initialized
//! receiver in rcx, reaches the semantic continuation, and returns a zero
//! completion status the loader maps to the process exit code.
//!
//! Windows Console and ProcessExit intrinsics are not lowered by
//! selected-dispatch yet, so this fixture stays storage-only: the admitted
//! bridge, receiver layout, and PE entry custody are the checked behavior.

use crate::{
    CanaryCompileProduct, CanaryCompileSpec, CompileReport, PathBuf, compile, fs, repo_root,
    unique_no_output_build_dir,
};
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
use std::process::Command;

struct HostedProject(PathBuf);

impl Drop for HostedProject {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "retained failed Windows hosted receiver observations: {}",
                self.0.display()
            );
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Compile one authored Windows x86-64 receiver application. `bare_interface`
/// swaps the storage-only receiver for one carrying a `Service` customer field
/// that no provider selection ever establishes, which must fail closed.
fn compile_and_run_windows_hosted_receiver(bare_interface: bool) {
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
    builder.application("windows_hosted_receiver");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}}
"#
        ),
    )
    .expect("write authored target and entry selection");
    let (helper_header, helper_field) = if bare_interface {
        (
            "use omega::language::core::binding;\n\npub boundary trait Helper {\n    machine help();\n}\n\n",
            "    helper: Binding<Helper>;\n",
        )
    } else {
        ("", "")
    };
    fs::write(
        project.0.join("main.omg"),
        format!(
            r#"{helper_header}
data Main {{
    value: i32;
    bytes: [u8; 256];
{helper_field}}}

machine Main::main(&mut self) {{
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
        self.value = 1;
    }}
    state failed(&mut self) {{
        self.value = 70;
    }}
}}
"#
        ),
    )
    .expect("write receiver storage program");
    let result = compile(CanaryCompileSpec {
        root_path: project.0.join("main.omg"),
        build_dir: Some(project.0.join("build")),
        target_name: Some("windows_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    });
    if bare_interface {
        let diagnostics = result.expect_err(
            "a receiver service field without a selected provider is never established",
        );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("requires a selected Fused provider for boundary `Helper`")),
            "unexpected missing-establishment rejection: {diagnostics:#?}"
        );
        return;
    }
    let report = result.unwrap_or_else(|diagnostics| {
        panic!("authored Windows hosted receiver must produce its executable: {diagnostics:#?}")
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
    assert_hosted_binding_replay_rejects_corruption(&report);
    let report = report
        .publish_retained_native_artifact(&project.0.join("build"))
        .expect("publish exact admitted native artifact after corruption controls");
    let executable = report
        .checked_native_executable_path()
        .expect("exact executable publication receipt");
    let bytes = fs::read(executable).expect("read published PE image");
    assert_eq!(
        bytes.get(..2),
        Some(b"MZ".as_slice()),
        "the published hosted receiver must be a PE image"
    );

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        // Execute the published bytes without a host wrapper: the loader
        // arrives at AddressOfEntryPoint, so a working bridge is the only way
        // the receiver reaches the semantic continuation. The value-free Unit
        // completion publishes process exit status zero.
        let output = Command::new(executable)
            .output()
            .expect("execute authored Windows hosted receiver process");
        assert_eq!(
            output.status.code(),
            Some(0),
            "unexpected process completion: {output:?}"
        );
    }
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    eprintln!(
        "SKIP: hosted receiver runtime requires Windows x86-64; source cross-emission checked"
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
fn windows_hosted_receiver_normal_return_provisions_zii_storage() {
    compile_and_run_windows_hosted_receiver(false);
}

#[test]
fn windows_hosted_receiver_rejects_bare_interface_without_bound_establishment() {
    compile_and_run_windows_hosted_receiver(true);
}
