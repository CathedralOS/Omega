//! Source-evaluated Linux ARM64 hosted receiver: a retained `&mut self`
//! entry bound to `linux_arm64::ProgramEntry` must provision its receiver
//! storage through the emitted bridge, reach the exact semantic continuation,
//! and complete through exit_group — with no test-supplied self.

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use std::os::unix::fs::PermissionsExt;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use std::process::Command;

use super::{Fixture, console_acceptance, linux_entry_acceptance};
use build_declarations::{BuildDeclaration, extract_build_declaration};
use compiler::{
    CheckedCompileRequest, RetainedNativeRealizationRequest, compile_to_checked,
    realize_retained_native_artifact,
};
use native_realization as native;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate lives under omega-rust/omega/compiler/compiler")
        .to_path_buf()
}

fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("repository fixture package identity is nonzero")
}

/// Package-level custody for the fixture project: the application root depends
/// on the exact repository standard library, accepts the checked
/// `LinuxArm64Application` entry schema, and accepts the exact Console
/// provider plan (with output — and exit when authored — authority).
fn linux_arm64_package_inputs(fixture: &Fixture) -> PackageCompilationInputs {
    let standard_library_root = repo_root().join("source/library/std");
    let declaration = extract_build_declaration(&fixture.root)
        .unwrap_or_else(|error| panic!("fixture {}: {error}", fixture.root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        other => panic!("fixture must declare an application, not {other:?}"),
    };
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    let inputs = PackageCompilationInputs::new(
        root_identity,
        root_role,
        vec![
            PackageSourceBinding::new(root_identity, root_name.into_string(), fixture.root.clone()),
            PackageSourceBinding::new(
                standard_library_identity,
                "omega-language-std",
                standard_library_root.clone(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "omega_language_std",
            standard_library_identity,
        )],
    )
    .unwrap_or_else(|errors| panic!("fixture package inputs: {errors:#?}"));
    let mut bindings = vec![
        linux_entry_acceptance::candidate_linux_arm64_entry_binding(
            &standard_library_root,
            standard_library_identity,
        )
        .unwrap_or_else(|diagnostics| {
            panic!("Linux ARM64 entry fixture acceptance: {diagnostics:#?}")
        }),
    ];
    // Admission requires the accepted entry binding already at checked time:
    // attach it before the preliminary compile that resolves the Console plan.
    let inputs = inputs
        .with_accepted_semantic_bindings(bindings.clone())
        .unwrap_or_else(|errors| panic!("fixture entry acceptance: {errors:#?}"));
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&fixture.main, Some("linux_arm64"))
    })
    .unwrap_or_else(|diagnostics| {
        panic!("preliminary checked fixture compilation: {diagnostics:#?}")
    });
    // The fixture writes one byte in every successful path; explicit-exit
    // variants additionally reach `exit_process` through the same Console plan
    // and share its terminal authority permission row.
    bindings.push(
        console_acceptance::candidate_console_exit_binding(
            &preliminary,
            standard_library_identity,
            true,
            false,
        )
        .unwrap_or_else(|diagnostics| panic!("Console fixture acceptance: {diagnostics:#?}")),
    );
    inputs
        .with_accepted_semantic_bindings(bindings)
        .unwrap_or_else(|errors| panic!("fixture acceptance: {errors:#?}"))
}

fn linux_arm64_hosted_receiver_fixture(name: &str, explicit_exit: bool) -> Fixture {
    let standard_library = repo_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    let completion = if explicit_exit {
        "self.console.exit_process(37);"
    } else {
        ""
    };
    let source = format!(
        r#"use omega_language_std::console;
use omega::language::core::service;

data Main {{
    value: i32;
    bytes: [u8; 256];
    console: Service<Console> in Bound;
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
"#,
    );
    let build = format!(
        r#"machine build(builder: &mut Build) {{
    builder.application("source-evaluated-linux-arm64-hosted-receiver");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
}}
"#,
    );
    let fixture = Fixture::with_source(name, "linux_arm64", &source, &build);
    let inputs = linux_arm64_package_inputs(&fixture);
    fixture.with_package_inputs(inputs)
}

/// Realize a direct (static) ELF image for the retained artifact. The object
/// carries no normalized imports, so the request stays in the direct lane.
fn realize_linux_arm64_direct(
    retained: compilation_report::RetainedTerminalArtifact,
) -> native::RequestedNativeArtifact {
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let subsystem = proposal.subsystem();
    // The fixture carries no normalized imports. The accepted-package policy
    // must rejoin the exact retained permission rows; the receiving policy may
    // be the current superset, matching the compile-time request.
    let accepted_package_policy =
        native_realization::terminal_authority_permission_policy_with_rows(
            proposal.package_terminal_authority_permissions().to_vec(),
        )
        .expect("retained package permissions form the accepted policy");
    realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy: accepted_package_policy.clone(),
            terminal_authority_permission_policy: accepted_package_policy,
            image_request: native::ExecutableImageEmissionRequest::direct(subsystem),
            imports: &[],
        },
    )
    .unwrap_or_else(|(_, diagnostics)| {
        panic!("Linux ARM64 hosted receiver should realize: {diagnostics:#?}")
    })
}

fn run_linux_arm64_hosted_receiver(explicit_exit: bool) {
    let fixture = linux_arm64_hosted_receiver_fixture(
        if explicit_exit {
            "linux-arm64-hosted-explicit-exit"
        } else {
            "linux-arm64-hosted-normal-return"
        },
        explicit_exit,
    );
    let candidate = realize_linux_arm64_direct(fixture.compile_terminal());
    let artifact = candidate
        .as_direct()
        .expect("an import-free Linux ARM64 object must select direct ELF custody");
    artifact
        .validate()
        .expect("direct native artifact independently replays");
    assert_eq!(artifact.target(), target::NativeTarget::linux_arm64());
    let binding = artifact
        .object()
        .hosted_receiver_binding()
        .expect("retained receiver requires the admitted hosted bridge binding");
    assert_eq!(binding.receiver_byte_count(), 260);
    let image_bytes = artifact.image().output().bytes.clone();
    assert!(image_bytes.starts_with(b"\x7fELF"));

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        // Write the emitted image and execute it unwrapped: the kernel arrives
        // at e_entry through sp alone, so process behavior proves the bridge.
        let path = fixture.root.join("omega-program");
        std::fs::write(&path, &image_bytes).expect("write emitted ELF image");
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        let output = Command::new(&path)
            .output()
            .expect("execute authored Linux ARM64 hosted receiver process");
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
    #[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
    eprintln!("SKIP: hosted receiver runtime requires Linux ARM64; emission checked");
}

#[test]
fn linux_arm64_hosted_receiver_normal_return_provisions_zii_storage_and_console() {
    run_linux_arm64_hosted_receiver(false);
}

#[test]
fn linux_arm64_hosted_receiver_explicit_exit_process_preserves_its_distinct_outcome() {
    run_linux_arm64_hosted_receiver(true);
}
