//! Source-evaluated Windows x86-64 hosted receiver: a retained `&mut self`
//! entry bound to `windows_x86_64::ProgramEntry` must provision its receiver
//! storage through the emitted bridge, reach the exact semantic continuation,
//! and return a zero completion status the loader maps to the process exit
//! code — with no test-supplied self.
//!
//! Windows Console and ProcessExit intrinsics are not lowered by
//! selected-dispatch yet, so this fixture stays storage-only; the admitted
//! bridge, receiver layout, and PE entry custody are the checked behavior.

use super::{Fixture, windows_entry_acceptance};
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
/// on the exact repository standard library and accepts the checked
/// `WindowsX86_64Application` entry schema. The storage-only receiver requires
/// no provider plan.
fn windows_package_inputs(fixture: &Fixture) -> PackageCompilationInputs {
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
    let bindings = vec![
        windows_entry_acceptance::candidate_windows_x86_64_entry_binding(
            &standard_library_root,
            standard_library_identity,
        )
        .unwrap_or_else(|diagnostics| panic!("Windows entry fixture acceptance: {diagnostics:#?}")),
    ];
    // Admission requires the accepted entry binding already at checked time:
    // attach it before the compile that selects the exact receiver entry.
    let inputs = inputs
        .with_accepted_semantic_bindings(bindings.clone())
        .unwrap_or_else(|errors| panic!("fixture entry acceptance: {errors:#?}"));
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&fixture.main, Some("windows_x86_64"))
    })
    .unwrap_or_else(|diagnostics| {
        panic!("preliminary checked fixture compilation: {diagnostics:#?}")
    });
    inputs
}

fn windows_hosted_receiver_fixture(name: &str) -> Fixture {
    let standard_library = repo_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    let source = r#"data Main {
    value: i32;
    bytes: [u8; 256];
}

machine Main::main(&mut self) {
    transition self.value == 0 {
        true -> initialized()
        false -> failed()
    }
    state initialized(&mut self) {
        self.value = 65;
        self.bytes[255] = 64;
        transition self.value == 65 && self.bytes[255] == 64 && self.bytes[254] == 0 {
            true -> observed()
            false -> failed()
        }
    }
    state observed(&mut self) {
        self.value = 1;
    }
    state failed(&mut self) {
        self.value = 70;
    }
}
"#;
    let build = format!(
        r#"machine build(builder: &mut Build) {{
    builder.application("source-evaluated-windows-hosted-receiver");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}}
"#,
    );
    let fixture = Fixture::with_source(name, "windows_x86_64", source, &build);
    let inputs = windows_package_inputs(&fixture);
    fixture.with_package_inputs(inputs)
}

/// Realize a direct PE image for the retained artifact. The object carries no
/// normalized imports, so the request stays in the direct lane.
fn realize_windows_direct(
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
        panic!("Windows hosted receiver should realize: {diagnostics:#?}")
    })
}

#[test]
fn windows_hosted_receiver_normal_return_provisions_zii_storage() {
    let fixture = windows_hosted_receiver_fixture("windows-hosted-normal-return");
    let candidate = realize_windows_direct(fixture.compile_terminal());
    let artifact = candidate
        .as_direct()
        .expect("an import-free Windows object must select direct PE custody");
    artifact
        .validate()
        .expect("direct native artifact independently replays");
    assert_eq!(artifact.target(), target::NativeTarget::windows_x64());
    let binding = artifact
        .object()
        .hosted_receiver_binding()
        .expect("retained receiver requires the admitted hosted bridge binding");
    assert_eq!(binding.receiver_byte_count(), 260);
    let image_bytes = artifact.image().output().bytes.clone();
    assert_eq!(
        image_bytes.get(..2),
        Some(b"MZ".as_slice()),
        "the emitted hosted receiver must be a PE image"
    );

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        // Write the emitted image and execute it unwrapped: the loader
        // arrives at AddressOfEntryPoint, so a working bridge is the only way
        // the receiver reaches the semantic continuation.
        let executable = fixture.root.join("omega-program.exe");
        std::fs::write(&executable, &image_bytes).expect("write emitted PE image");
        let output = std::process::Command::new(&executable)
            .output()
            .expect("execute authored Windows hosted receiver process");
        assert_eq!(
            output.status.code(),
            Some(0),
            "unexpected process completion: {output:?}"
        );
    }
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    eprintln!("SKIP: hosted receiver runtime requires Windows x86-64; emission checked");
}
