//! Checked hosted program entry must provision the authored receiver under
//! exact custody all the way from source to the published process image.
//!
//! This is the differential suite's leg of the canary `entry_and_abi::
//! hosted_receiver*` contract: an authored `Service<Console>`-carrying
//! receiver compiles through checked trees, retains its binding row on the
//! admitted native object, replays independently against corruption, and —
//! on the one host whose produced ELF can execute here — is run natively and
//! witnessed at exit status and stdout. A bare interface spelling of the
//! same field must refuse before any binding is admitted.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct,
    compile_to_checked,
};
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
    PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;

static PROJECT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("native-differential manifest dir sits under the repository root")
}

fn project_directory(name: &str) -> PathBuf {
    let sequence = PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-native-diff-hosted-receiver-{name}-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create exclusively owned hosted-entry project");
    directory
}

fn package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

fn root_identity() -> PackageKeyIdentity {
    package_identity(101)
}

fn standard_library_identity() -> PackageKeyIdentity {
    package_identity(102)
}

/// Author the hosted receiver program and its build declaration into a
/// fresh project directory. `bound_service` selects the intrinsic
/// `Service<Console>` carrier or the bare `Console` interface that must
/// refuse; `explicit_exit` completes through `exit_process(37)` so the
/// provider's own status survives the hosted bridge.
fn author_hosted_receiver_project(
    directory: &Path,
    explicit_exit: bool,
    bound_service: bool,
) -> PathBuf {
    let standard_library = repo_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    std::fs::write(
        directory.join("build.omg"),
        format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("native-diff-hosted-receiver");
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
        "Service<Console>"
    } else {
        "Console"
    };
    let root = directory.join("main.omg");
    std::fs::write(
        &root,
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
    .expect("write receiver storage and fused Console customer");
    root
}

/// Derive the checked Linux x86-64 entry acceptance for the retained
/// application package — the same contract the canary suite binds through
/// `linux_entry_acceptance`, spelled against the public checked API.
fn linux_x86_64_entry_binding() -> AcceptedSemanticBinding {
    let standard_library_root = repo_root().join("source/library/std");
    let inputs = PackageCompilationInputs::new_package(
        standard_library_identity(),
        vec![PackageSourceBinding::new(
            standard_library_identity(),
            "omega-language-std",
            standard_library_root.clone(),
        )],
        Vec::new(),
    )
    .expect("entry schema package inputs");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(
            &standard_library_root.join("targets/linux_x86_64/entry.omg"),
            Some("linux_x86_64"),
        )
    })
    .expect("the Linux x86-64 entry schema checks");
    checked
        .candidate_service_binding(
            AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry,
            standard_library_identity(),
            "LinuxX86_64Application",
        )
        .expect("exact Linux x86-64 program entry binding")
}

/// Derive the Console service acceptance for this exact application from
/// its own preliminary checked compile: the bound provider plan must be
/// std's `Console` schema realized by `ConsoleNativeProvider`, and only the
/// output and termination authorities the fixture calls may be admitted.
fn console_binding(preliminary: &compiler::CheckedCompilation) -> AcceptedSemanticBinding {
    let candidates = preliminary
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(preliminary.selected_provider_provenance())
        .filter(|(plan, provenance)| {
            plan.schema.trait_name == "Console"
                && plan.rows.iter().any(|row| row.method == "write_byte")
                && preliminary
                    .typed
                    .symbols
                    .symbol_package_identity(provenance.provider.schema.symbol())
                    == Some(standard_library_identity())
        })
        .collect::<Vec<_>>();
    let [(plan, provenance)] = candidates.as_slice() else {
        panic!(
            "hosted receiver resolved {} exact std Console provider plans instead of one",
            candidates.len()
        )
    };
    let declaration_path = preliminary
        .typed
        .symbols
        .display_path(provenance.provider.schema.symbol(), "::");
    let binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        standard_library_identity(),
        declaration_path,
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .expect("construct Console service binding");
    let mut permissions = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            matches!(
                method.name.as_str(),
                "exit_process" | "write" | "write_byte" | "write_line"
            )
        })
        .map(|method| {
            effects::ServiceTerminalAuthorityPermission::new(
                plan.schema.identity_digest(),
                method.requirement_identity.clone(),
                effects::TerminalAuthorityDisposition::from_classes(match method.name.as_str() {
                    "exit_process" => {
                        vec![effects::TerminalAuthorityClass::ProcessTermination]
                    }
                    _ => vec![effects::TerminalAuthorityClass::ProcessOutput],
                }),
            )
        })
        .collect::<Vec<_>>();
    permissions.sort_by(|left, right| {
        left.requirement_identity()
            .cmp(right.requirement_identity())
    });
    binding
        .with_terminal_authority_permissions(permissions)
        .expect("attach Console terminal authority permissions")
}

fn compile_hosted_receiver(root: &Path, project: &Path) -> compiler::CompileReport {
    let inputs = PackageCompilationInputs::new(
        root_identity(),
        package_compilation::BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                root_identity(),
                "native-diff-hosted-receiver",
                project.to_path_buf(),
            ),
            PackageSourceBinding::new(
                standard_library_identity(),
                "omega-language-std",
                repo_root().join("source/library/std"),
            ),
        ],
        vec![PackageDependencyBinding::new(
            root_identity(),
            "omega_language_std",
            standard_library_identity(),
        )],
    )
    .expect("hosted receiver package inputs");
    // The program-entry root must be accepted before the build machine's
    // `roots.bind` evaluates during checked compilation.
    let entry = linux_x86_64_entry_binding();
    let inputs = inputs
        .with_accepted_semantic_bindings(vec![entry.clone()])
        .expect("hosted receiver entry binding admits");
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(root, Some("linux_x86_64"))
    })
    .expect("hosted receiver preliminary checked compilation");
    let inputs = inputs
        .with_accepted_semantic_bindings(vec![entry, console_binding(&preliminary)])
        .expect("hosted receiver semantic bindings admit");
    compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.to_path_buf(),
            build_dir: Some(project.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("authored hosted receiver must produce its executable")
}

#[test]
fn hosted_receiver_checked_entry_provisions_and_executes_on_linux_x86_64() {
    let project = project_directory("linux");
    let root = author_hosted_receiver_project(&project, false, true);
    let report = compile_hosted_receiver(&root, &project);
    let receiver = report
        .retained_native_artifact()
        .expect("retain admitted native object")
        .object()
        .hosted_receiver_binding()
        .expect("retain exact provisioned receiver");
    assert_eq!(
        receiver.receiver_byte_count(),
        260,
        "scalar and fixed array both occupy the image-backed receiver"
    );

    // Independent replay from the published parts rejects custody drift in
    // every receiver row the checked entry established.
    let object = report
        .retained_native_artifact()
        .expect("retain admitted native object")
        .object();
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

    let report = report
        .publish_retained_native_artifact(&project.join("build"))
        .expect("publish exact admitted native artifact after corruption controls");
    let executable = report
        .checked_native_executable_path()
        .expect("exact executable publication receipt");
    let bytes = std::fs::read(executable).expect("read published ELF");
    assert_eq!(
        bytes.get(..4),
        Some([0x7f, 0x45, 0x4c, 0x46].as_slice()),
        "the published hosted receiver must be an ELF image"
    );

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // The kernel arrives at the emitted ELF entry point through the
        // exact hosted bridge: a working bridge is the only way the receiver
        // reaches the semantic continuation.
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
            "receiver must begin at zero and retain its write"
        );
        assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    eprintln!(
        "SKIP: hosted receiver runtime requires Linux x86-64; source compile, custody replay, and image publication still asserted"
    );
}

#[test]
fn hosted_receiver_checked_entry_rejects_bare_interface_field() {
    let project = project_directory("bare");
    let root = author_hosted_receiver_project(&project, false, false);
    let inputs = PackageCompilationInputs::new(
        root_identity(),
        package_compilation::BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                root_identity(),
                "native-diff-hosted-receiver",
                project.clone(),
            ),
            PackageSourceBinding::new(
                standard_library_identity(),
                "omega-language-std",
                repo_root().join("source/library/std"),
            ),
        ],
        vec![PackageDependencyBinding::new(
            root_identity(),
            "omega_language_std",
            standard_library_identity(),
        )],
    )
    .expect("bare-interface package inputs");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root, Some("linux_x86_64"))
    })
    .expect_err("a bare interface field is not a service carrier");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("the intrinsic `Service<R>` carrier is the only service value spelling")),
        "unexpected bare-carrier rejection: {diagnostics:#?}"
    );
}
