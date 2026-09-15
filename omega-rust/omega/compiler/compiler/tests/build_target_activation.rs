use compiler::CheckedCompileRequest;
use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct,
    RetainedNativeRealizationRequest, compile, compile_to_checked,
    realize_retained_native_artifact,
};
use package_compilation::{
    BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new(build: &str) -> Self {
        Self::with_main("const ANSWER: u32 = 42;\n", build)
    }

    fn with_main(main: &str, build: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../target/omega-test-projects")
            .join(format!(
                "omega-build-target-activation-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).expect("create temporary in-repository Omega project");
        fs::write(path.join("main.omg"), main).expect("write temporary Omega source");
        fs::write(path.join("build.omg"), build).expect("write temporary Omega build source");
        Self(path)
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn exact_target_build(body: &str) -> String {
    application_build(body)
}

fn application_build(body: &str) -> String {
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"target-activation\");\n{body}\n}}\n"
    )
}

#[path = "fixture_rosters/build_target_activation.rs"]
mod fixtures;

#[path = "support/console_acceptance.rs"]
mod console_acceptance;

fn pass_canary_main(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass")
        .join(name)
        .join("main.omg")
}

fn package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero fixture package identity")
}

fn package_inputs_with_standard_library(
    main: &std::path::Path,
    canonical_name: &str,
) -> PackageCompilationInputs {
    let root = main.parent().expect("canary project root");
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root")
        .to_path_buf();
    let root_identity = package_identity(1);
    let standard_library_identity = package_identity(2);
    PackageCompilationInputs::new(
        root_identity,
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(root_identity, canonical_name, root.to_path_buf()),
            PackageSourceBinding::new(
                standard_library_identity,
                "omega-language-std",
                repository.join("source/library/std"),
            ),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "omega_language_std",
            standard_library_identity,
        )],
    )
    .expect("ordinary std dependency graph")
}

fn diagnostic_text(project: &TempProject) -> String {
    compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("immutable target violation must reject")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn exact_target_is_source_visible_and_drives_build_evaluation() {
    let project = TempProject::new(&exact_target_build(
        r#"    transition builder.target {
        TargetProfile::WindowsX86_64 -> windows(builder)
        _ -> other(builder)
    }
    state windows(builder: &mut Build) {
        builder.subsystem = Subsystem::Gui;
    }
    state other(builder: &mut Build) {
        builder.subsystem = Subsystem::Console;
    }"#,
    ));

    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("the selected target must be an ordinary readable Omega value");
    assert_eq!(
        checked.selected_target_profile(),
        Some(target::TargetProfile::WindowsX64)
    );
    assert_eq!(checked.subsystem(), 2, "Windows branch must select Gui");
    assert_eq!(
        checked.application_intent(),
        Some(build_evaluation::HostedApplicationIntent::Gui)
    );
    let other = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("macos_arm64"),
    ))
    .expect("the macOS selection must execute its own authored branch");
    assert_eq!(
        other.application_intent(),
        Some(build_evaluation::HostedApplicationIntent::Console)
    );
    assert_eq!(other.subsystem(), 3);
}

#[test]
fn checked_build_preserves_hosted_intent_without_interpreting_raw_pe_words() {
    use build_evaluation::HostedApplicationIntent::{Console, Gui};
    for (subsystem, intent, pe_word) in [
        ("Gui", Some(Gui), 2),
        ("Console", Some(Console), 3),
        ("Unspecified { value: 2 }", None, 2),
        ("Unspecified { value: 3 }", None, 3),
    ] {
        let project = TempProject::new(&application_build(&format!(
            "    builder.subsystem = Subsystem::{subsystem};"
        )));
        for target in ["macos_arm64", "windows_x86_64", "linux_x86_64"] {
            let checked =
                compile_to_checked(CheckedCompileRequest::new(&project.main(), Some(target)))
                    .expect("authored subsystem must survive the ordinary checked route");
            assert_eq!(
                checked.application_intent(),
                intent,
                "{target}: {subsystem}"
            );
            assert_eq!(checked.subsystem(), pe_word, "{target}: {subsystem}");
        }
    }
}

#[test]
fn authored_identifier_reaches_the_checked_and_retained_carriers() {
    // `builder.identifier` is ordinary Build vocabulary: it lands on the
    // checked compilation and survives on the retained native proposal so a
    // later invocation can sign without the frontend.
    let project = TempProject::with_main(
        "data Main { }\nmachine Main::main(&mut self) { }\n",
        &application_build(
            "    builder.subsystem = Subsystem::Gui;\n    builder.identifier = \"com.omega.window-app\";\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);",
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("macos_arm64"),
    ))
    .expect("an authored identifier must survive the ordinary checked route");
    assert_eq!(
        checked.application_intent(),
        Some(build_evaluation::HostedApplicationIntent::Gui)
    );
    assert_eq!(
        checked
            .application_identifier()
            .map(build_evaluation::ApplicationIdentifier::as_str),
        Some("com.omega.window-app")
    );

    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!("authored identifier reaches retained Terminal production: {diagnostics:#?}")
    });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    let proposal = retained
        .native_realization_proposal()
        .expect("Terminal report retains the native proposal");
    assert_eq!(
        proposal.application_intent(),
        Some(build_evaluation::HostedApplicationIntent::Gui),
        "retained intent stays separate from the PE loader word"
    );
    assert_eq!(
        proposal
            .application_identifier()
            .map(build_evaluation::ApplicationIdentifier::as_str),
        Some("com.omega.window-app"),
        "the retained proposal carries the authored signing identity"
    );
    assert_eq!(
        proposal.application_name(),
        Some("target-activation"),
        "the retained proposal carries the authored application name for a later packaging invocation"
    );
}

#[test]
fn identifier_omission_and_malformed_bytes_follow_the_build_contract() {
    let project = TempProject::new(&application_build(
        "    builder.subsystem = Subsystem::Gui;",
    ));
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("macos_arm64"),
    ))
    .expect("a GUI build without an authored identifier still checks");
    assert_eq!(checked.application_identifier(), None);

    let invalid = TempProject::new(&application_build(
        "    builder.identifier = \"has_underscore\";",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &invalid.main(),
        Some("macos_arm64"),
    ))
    .expect_err("a malformed identifier must reject")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("Build.identifier"),
        "unexpected diagnostic: {diagnostics}"
    );
}

#[test]
fn retained_native_realization_binds_the_image_request_to_the_proposal_subsystem() {
    let project = TempProject::with_main(
        "data Main { }\nmachine Main::main(&mut self) { }\n",
        &application_build("    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);"),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("retained Terminal custody: {diagnostics:#?}"));
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    let retained_subsystem = retained
        .native_realization_proposal()
        .expect("Terminal report retains the native proposal")
        .subsystem();
    let (_, diagnostics) = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            image_request: native_realization::ExecutableImageEmissionRequest::direct(
                retained_subsystem + 1,
            ),
            imports: &[],
        },
    )
    .expect_err("an image request naming another subsystem must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("image request subsystem differs from the retained build subsystem")
    }));
}

// A receiver-less `main` avoids both hosted-receiver custody provisioning and
// boundary-service reach on macOS ARM64 `ProgramEntry`; the signing-identity
// contract under test lives entirely in the image-emission request.
const MACOS_HOSTED_MAIN: &str = "data Main { }\nmachine Main::main() { }\n";

#[test]
fn retained_native_realization_binds_the_retained_identifier_into_signed_bytes() {
    // An image request that omits the signing identity inherits the retained
    // authored identifier (wiki/spec/build/macos_application.md): the
    // CodeDirectory spelled inside the emitted Mach-O carries the checked
    // `builder.identifier`, not the executable-leaf fallback.
    let project = TempProject::with_main(
        MACOS_HOSTED_MAIN,
        &application_build(
            "    builder.subsystem = Subsystem::Gui;\n    builder.identifier = \"com.omega.window-app\";\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);",
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("retained Terminal custody: {diagnostics:#?}"));
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    let retained_subsystem = retained
        .native_realization_proposal()
        .expect("Terminal report retains the native proposal")
        .subsystem();
    let realized = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            image_request: native_realization::ExecutableImageEmissionRequest::direct(
                retained_subsystem,
            ),
            imports: &[],
        },
    )
    .unwrap_or_else(|(_, diagnostics)| {
        panic!("retained identifier must realize signed macOS output: {diagnostics:#?}")
    });
    let artifact = realized.into_direct().expect("direct image custody");
    let bytes = &artifact.image().output().bytes;
    assert!(
        bytes
            .windows(b"com.omega.window-app".len())
            .any(|window| window == b"com.omega.window-app"),
        "the emitted Mach-O must carry the retained CodeDirectory identifier"
    );
    assert!(
        !bytes
            .windows(b"com.evil.other".len())
            .any(|window| window == b"com.evil.other"),
        "no substitute identity may appear in the signed bytes"
    );
}

#[test]
fn retained_native_realization_rejects_a_conflicting_request_identifier() {
    let project = TempProject::with_main(
        "data Main { }\nmachine Main::main(&mut self) { }\n",
        &application_build(
            "    builder.subsystem = Subsystem::Gui;\n    builder.identifier = \"com.omega.window-app\";\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);",
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("retained Terminal custody: {diagnostics:#?}"));
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    let retained_subsystem = retained
        .native_realization_proposal()
        .expect("Terminal report retains the native proposal")
        .subsystem();
    let (_, diagnostics) = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            image_request: native_realization::ExecutableImageEmissionRequest::direct(
                retained_subsystem,
            )
            .with_code_signature_identifier(Some("com.evil.other".to_owned())),
            imports: &[],
        },
    )
    .expect_err("an image request naming another signing identity must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "image request code signature identifier differs from the retained build identifier",
        )
    }));
}

#[test]
fn macos_gui_image_emission_requires_the_authored_identifier() {
    // `builder.identifier` may be absent at Terminal production, but signed
    // macOS GUI image emission requires it on both the direct and retained
    // routes; a later consumer may still supply it under its own authority.
    let retained_project = TempProject::with_main(
        MACOS_HOSTED_MAIN,
        &application_build(
            "    builder.subsystem = Subsystem::Gui;\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);",
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: retained_project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!("Terminal production needs no identifier: {diagnostics:#?}")
    });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    let retained_subsystem = retained
        .native_realization_proposal()
        .expect("Terminal report retains the native proposal")
        .subsystem();
    let (_, diagnostics) = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            image_request: native_realization::ExecutableImageEmissionRequest::direct(
                retained_subsystem,
            ),
            imports: &[],
        },
    )
    .expect_err("macOS GUI emission without any identifier must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("signed macOS GUI image emission requires the retained build identifier")
    }));

    // A receiving invocation may bind an identifier under its own authority:
    // the retained proposal authored none, so the request value is adopted
    // rather than conflicting.
    let supplied_project = TempProject::with_main(
        MACOS_HOSTED_MAIN,
        &application_build(
            "    builder.subsystem = Subsystem::Gui;\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);",
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: supplied_project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("retained Terminal custody: {diagnostics:#?}"));
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    let retained_subsystem = retained
        .native_realization_proposal()
        .expect("Terminal report retains the native proposal")
        .subsystem();
    let realized = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            image_request: native_realization::ExecutableImageEmissionRequest::direct(
                retained_subsystem,
            )
            .with_code_signature_identifier(Some("com.omega.consumer-app".to_owned())),
            imports: &[],
        },
    )
    .unwrap_or_else(|(_, diagnostics)| {
        panic!("a consumer-supplied identifier must satisfy GUI emission: {diagnostics:#?}")
    });
    let artifact = realized.into_direct().expect("direct image custody");
    assert!(
        artifact
            .image()
            .output()
            .bytes
            .windows(b"com.omega.consumer-app".len())
            .any(|window| window == b"com.omega.consumer-app"),
        "the consumer-supplied identity is bound into the signed bytes"
    );

    // The direct compile route enforces the same requiredness before
    // realization.
    let direct_project = TempProject::with_main(
        MACOS_HOSTED_MAIN,
        &application_build(
            "    builder.subsystem = Subsystem::Gui;\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);",
        ),
    );
    let diagnostics = compile(
        CompileRequest::new(CompileOptions {
            root_path: direct_project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect_err("direct macOS GUI realization without an identifier must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("signed macOS GUI image emission requires the authored Build identifier")
    }));
}

#[test]
fn macos_gui_publication_installs_one_app_package() {
    // Complete selected macOS GUI output is the `.app` tree, not a flat
    // executable beside it (wiki/spec/build/macos_application.md).
    let project = TempProject::with_main(
        MACOS_HOSTED_MAIN,
        &application_build(
            "    builder.subsystem = Subsystem::Gui;\n    builder.identifier = \"com.omega.window-app\";\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);",
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("macOS GUI compilation: {diagnostics:#?}"));
    let build_dir = project.0.join("build");
    let published = report
        .publish_retained_native_artifact(&build_dir)
        .expect("macOS GUI publication installs one .app package");
    let package_root = published
        .checked_native_package_path()
        .expect("checked package root")
        .to_path_buf();
    assert_eq!(package_root, build_dir.join("target-activation.app"));
    let executable = published
        .checked_native_executable_path()
        .expect("checked inner executable")
        .to_path_buf();
    assert_eq!(
        executable,
        package_root.join("Contents/MacOS/target-activation"),
        "the inner executable path is checked separately from the package root"
    );
    assert!(package_root.join("Contents/Info.plist").is_file());
    assert!(executable.is_file());
    assert!(
        !build_dir.join("omega-program").exists(),
        "no redundant flat executable beside the bundle"
    );
    let plist =
        fs::read_to_string(package_root.join("Contents/Info.plist")).expect("read installed plist");
    assert!(
        plist.contains("<key>CFBundleIdentifier</key>\n\t<string>com.omega.window-app</string>"),
        "the retained authored identifier is the plist identity"
    );
    assert!(
        plist.contains("<key>CFBundleExecutable</key>\n\t<string>target-activation</string>"),
        "the authored application name is the plist executable leaf"
    );
    assert!(plist.contains("<key>CFBundlePackageType</key>\n\t<string>APPL</string>"),);
    // The installed executable carries the authored CodeDirectory identity.
    let bytes = fs::read(&executable).expect("read installed executable");
    assert!(
        bytes
            .windows(b"com.omega.window-app".len())
            .any(|window| window == b"com.omega.window-app"),
        "the installed inner executable is the signed artifact"
    );
}

#[test]
fn macos_console_publication_stays_flat() {
    // macOS console output remains a flat Mach-O: only selected GUI intent
    // selects the `.app` package.
    let project = TempProject::with_main(
        MACOS_HOSTED_MAIN,
        &application_build("    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);"),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("macOS console compilation: {diagnostics:#?}"));
    let build_dir = project.0.join("build");
    let published = report
        .publish_retained_native_artifact(&build_dir)
        .expect("macOS console publication installs a flat executable");
    assert!(published.checked_native_package_path().is_none());
    let executable = published
        .checked_native_executable_path()
        .expect("checked flat executable")
        .to_path_buf();
    assert_eq!(executable, build_dir.join("omega-program"));
    assert!(executable.is_file());
    assert!(
        !build_dir.join("target-activation.app").exists(),
        "console output never becomes a bundle"
    );
}

#[test]
fn legacy_and_canonical_cli_spellings_select_the_same_canonical_profile() {
    let project = TempProject::new(&exact_target_build(""));
    let legacy = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x64"),
    ))
    .expect("legacy CLI alias should normalize before source selection");
    let canonical = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("canonical CLI spelling should compile");

    assert_eq!(
        legacy.selected_target_profile(),
        canonical.selected_target_profile()
    );
    assert_eq!(
        legacy.selected_native_target(),
        canonical.selected_native_target()
    );
    assert_eq!(
        legacy
            .selected_target_profile()
            .expect("selected profile")
            .target_name(),
        "windows_x86_64"
    );
}

#[test]
fn targetless_checking_retains_no_synthetic_target() {
    let project = TempProject::new(
        "machine build(builder: &mut Build) { builder.application(\"targetless\"); }\n",
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("targetless check should pass");
    assert_eq!(checked.selected_target_profile(), None);
}

#[test]
fn direct_target_assignment_is_rejected() {
    let project = TempProject::new(&exact_target_build(
        "    builder.target = TargetProfile::MacosArm64;",
    ));
    let diagnostics = diagnostic_text(&project);
    assert!(
        diagnostics.contains("Build.target is compiler-owned and cannot be assigned"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn transient_target_overwrite_then_restore_is_rejected() {
    let project = TempProject::new(&exact_target_build(
        r#"    builder.target = TargetProfile::MacosArm64;
    builder.target = TargetProfile::WindowsX86_64;"#,
    ));
    let diagnostics = diagnostic_text(&project);
    assert!(
        diagnostics.contains("Build.target is compiler-owned and cannot be assigned"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn exclusive_target_borrow_is_rejected() {
    let project = TempProject::new(&exact_target_build(
        "    let target: &mut TargetProfile = &mut builder.target;",
    ));
    let diagnostics = diagnostic_text(&project);
    assert!(
        diagnostics.contains(
            "Build.target is compiler-owned and cannot enter a mutable or write-only borrow"
        ),
        "unexpected diagnostics: {diagnostics}"
    );
}

fn foreign_helper_inputs(project: &TempProject, helper: &TempProject) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        package_identity(1),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(package_identity(1), "root-binding-owner", project.0.clone()),
            PackageSourceBinding::new(package_identity(2), "root-binding-helper", helper.0.clone()),
        ],
        vec![PackageDependencyBinding::new(
            package_identity(1),
            "support",
            package_identity(2),
        )],
    )
    .expect("explicit package graph")
}

#[test]
fn borrowing_build_does_not_lend_private_product_names_to_a_foreign_helper() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, launch); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics =
        compile_to_checked(request).expect_err("a helper cannot select its caller's private entry");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not a product declaration visible from this build source's package")),
        "{diagnostics:?}"
    );
}

#[test]
fn foreign_helper_binds_its_own_package_entry_through_borrowed_root_build() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine launch() { let marker: u8 = 0; } pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, launch); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "const ANSWER: u32 = 42;\n",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request)
        .expect("a helper may bind its own package's entry through the borrowed root Build");
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
}

#[test]
fn same_named_entry_in_another_package_rejoins_production_and_settlement_by_symbol() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine launch() { let marker: u8 = 0; } pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, launch); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request)
        .expect("a same-named foreign entry binds by exact symbol, not by name");
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
    let entry = checked
        .selected_program_entry()
        .cloned()
        .expect("one exact selected ProgramEntry");
    let native_target = checked
        .selected_native_target()
        .expect("one selected native target");
    let psi_optimizations = checked
        .optimization_selections()
        .project_psi()
        .selections()
        .clone();
    let program = checked.into_program();
    // The lexical binding chose the helper's `setup::launch`, not the owner's
    // free `launch`: the exact symbol resolves to the helper package machine.
    let helper_launch = program
        .typed
        .machines()
        .iter()
        .find(|machine| program.typed.symbols.display_path(machine.symbol, "::") == "setup::launch")
        .expect("the helper package declares setup::launch");
    assert_eq!(
        entry.source_signature().machine_symbol(),
        helper_launch.symbol
    );
    let produced = terminal_production::TerminalProductionRequest {
        checked: &program,
        machine: terminal_production::TerminalMachineSelection::Symbol(
            entry.source_signature().machine_symbol(),
        ),
        optimization_selections: psi_optimizations,
    }
    .produce_program_entry(entry.source_signature().identity().bytes())
    .expect("Terminal production rejoins the selected machine by exact symbol");
    assert_eq!(
        produced.receipt().source_machine_name(),
        "setup::launch",
        "the receipt keeps the qualified display name for diagnostics"
    );
    assert_eq!(
        produced.receipt().source_machine_symbol(),
        entry.source_signature().machine_symbol(),
        "the receipt carries the exact selected machine symbol"
    );
    let calling_plans = entry.calling_plans().map(|plans| {
        (
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )
    });
    native_realization::validate_native_program_entry_settlement(
        produced.artifact(),
        produced.receipt(),
        native_realization::NativeProgramEntrySettlement::new(
            entry.source_signature(),
            calling_plans,
            entry.fused_service_establishments(),
        ),
        native_target,
    )
    .expect("native entry settlement rejoins the exact selected machine symbol");

    // The retained product path selects the same exact symbol: the compiler's
    // own `TerminalProductionRequest` must not fall back to the name spelling.
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
        .with_package_inputs(foreign_helper_inputs(&project, &helper)),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!("same-named foreign entry reaches retained Terminal production: {diagnostics:#?}")
    });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    retained
        .validate()
        .expect("same-named retained Terminal artifact verifies");
    assert_eq!(
        retained
            .native_realization_proposal()
            .expect("Terminal report retains the native proposal")
            .program_entry()
            .source_signature()
            .machine_symbol(),
        helper_launch.symbol,
        "the retained proposal keeps the helper package's exact entry symbol"
    );
}

#[test]
fn owner_selected_product_description_binds_through_foreign_helper() {
    // The owner selects its own private entry through the compiler-owned
    // `product.entry` query and hands the restricted description to a helper
    // in another package. The helper binds it without holding any product
    // namespace of its own; the target machine traps if it were ever executed
    // during selection, so a clean compile also witnesses non-execution.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build, entry: &ProductEntryRef) { builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "machine launch() { crash Trap; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let entry: ProductEntryRef = builder.product.entry(\"launch\", \"windows_x86_64::ProgramEntry\"); setup::configure(builder, &entry); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "an owner-selected product description binds through a helper without executing the target",
    );
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
}

#[test]
fn product_entry_query_is_scoped_to_the_query_occurrences_package() {
    // The helper performs the query itself: `launch` exists only in the
    // owner's package, so the borrowed Build cannot reach it. Selection
    // authority belongs to the query occurrence's package, not the caller's.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build) { let entry: ProductEntryRef = builder.product.entry(\"launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics = compile_to_checked(request)
        .expect_err("a foreign helper cannot select the caller's private product entry")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        diagnostics
            .contains("not a product declaration visible from this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn forged_product_entry_description_is_rejected() {
    // An authored `ProductEntryRef {}` has the static type but carries no
    // compiler-issued description payload, so the bind refuses it at
    // evaluation rather than trusting the shape.
    let project = TempProject::new(&application_build(
        "    let forged: ProductEntryRef = ProductEntryRef {};\n    builder.roots.bind(windows_x86_64::ProgramEntry, forged);",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored ProductEntryRef value is not a product description")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("not a compiler-issued product entry description"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn delegated_root_binding_requires_a_product_entry_ref_operand() {
    // A typed-but-undescribed local cannot mint selection authority: it is not
    // a compiler-issued description, so the bind refuses it. Build evaluation
    // runs before checked authored selections finalize, so the refusal is the
    // evaluation-time marker check.
    let project = TempProject::new(&application_build(
        "    let marker: u8 = 1;\n    builder.roots.bind(windows_x86_64::ProgramEntry, marker);",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a non-description local cannot stand in for a product entry")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("not a compiler-issued product entry description")
            || diagnostics.contains("ProductEntryRef"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_description_binds_only_the_slot_it_was_selected_for() {
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "machine build(builder: &mut Build) { builder.application(\"slot-mismatch\"); let entry: ProductEntryRef = builder.product.entry(\"launch\", \"linux_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a description selected for another slot must not bind here")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("does not match the product description's slot"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn owner_selected_product_schema_inspects_through_foreign_helper() {
    // The owner selects its own private product data declaration through the
    // compiler-owned `product.schema` query and hands the restricted
    // description to a helper in another package. The helper inspects it
    // through `path()` without holding any product namespace of its own. A
    // clean compile plus the emitted log line also witnesses non-execution:
    // had the declared `BuildProduct::schema` body run, the returned authored
    // `ProductTypeSchema {}` would carry no description and `path()` would
    // trap.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build, schema: &ProductTypeSchema) { builder.log.write_line(schema.path()); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "data LaunchConfig { value: u8; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let schema: ProductTypeSchema = builder.product.schema(\"LaunchConfig\"); setup::configure(builder, &schema); builder.product.schema(\"LaunchConfig\"); schema.path(); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "an owner-selected product schema inspects through a helper without executing the declaration",
    );
    let observation = checked
        .build_observation_summary()
        .expect("schema inspection retains a build observation");
    assert_eq!(observation.build_log(), b"LaunchConfig\n");
}

#[test]
fn product_schema_query_is_scoped_to_the_query_occurrences_package() {
    // The helper performs the query itself: `LaunchConfig` exists only in the
    // owner's package, so the borrowed Build cannot reach it. Selection
    // authority belongs to the query occurrence's package, not the caller's.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build) { let schema: ProductTypeSchema = builder.product.schema(\"LaunchConfig\"); builder.log.write_line(schema.path()); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "data LaunchConfig { value: u8; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics = compile_to_checked(request)
        .expect_err("a foreign helper cannot select the caller's private product schema")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        diagnostics
            .contains("not a product declaration visible from this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn forged_product_schema_description_is_rejected() {
    // An authored `ProductTypeSchema {}` has the static type but carries no
    // compiler-issued description payload, so inspection refuses it at
    // evaluation rather than trusting the shape.
    let project = TempProject::new(&application_build(
        "    let forged: ProductTypeSchema = ProductTypeSchema {};\n    builder.log.write_line(forged.path());",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored ProductTypeSchema value is not a product schema description")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("requires a compiler-issued ProductTypeSchema"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_schema_query_rejects_a_non_facet_receiver() {
    // An authored `BuildProduct` lookalike resolves the toolchain machine by
    // member lookup but carries no facet marker: selection authority lives on
    // the compiler-issued `Build.product` value only.
    let project = TempProject::with_main(
        "data LaunchConfig { value: u8; }",
        "machine build(builder: &mut Build) { builder.application(\"lookalike-facet\"); let product: BuildProduct = BuildProduct {}; let schema: ProductTypeSchema = product.schema(\"LaunchConfig\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored BuildProduct value cannot perform the schema query")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("requires the compiler-issued Build.product facet"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_schema_query_requires_an_exact_data_declaration() {
    // `launch` names a machine, not a data declaration; a schema operand
    // selects only authored product data in the occurrence's package.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "machine build(builder: &mut Build) { builder.application(\"schema-miss\"); let schema: ProductTypeSchema = builder.product.schema(\"launch\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a machine name cannot stand in for a product schema")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("names no data declaration in this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_schema_cannot_stand_in_for_an_entry_description() {
    // The description kinds are distinct: `roots.bind` consumes only the
    // entry marker, so a schema description is refused wherever an entry
    // description is required.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }\ndata LaunchConfig { value: u8; }",
        "machine build(builder: &mut Build) { builder.application(\"schema-not-entry\"); let schema: ProductTypeSchema = builder.product.schema(\"LaunchConfig\"); builder.roots.bind(windows_x86_64::ProgramEntry, schema); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a product schema description cannot stand in as a root binding operand")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("not a compiler-issued product entry description")
            || diagnostics.contains("ProductEntryRef"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_schema_query_rejects_an_ambiguous_declaration() {
    // Two same-named data declarations in the query occurrence's own package
    // cannot mint one description: the query admits exactly one candidate.
    let project = TempProject::with_main(
        "use other;\ndata LaunchConfig { value: u8; }",
        "machine build(builder: &mut Build) { builder.application(\"schema-ambiguous\"); let schema: ProductTypeSchema = builder.product.schema(\"LaunchConfig\"); }",
    );
    fs::write(
        project.0.join("other.omg"),
        "module other; pub data LaunchConfig { flag: u8; }",
    )
    .expect("ambiguous sibling source");
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an ambiguous product schema name is refused")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("is ambiguous within its package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn owner_selected_product_provider_inspects_through_foreign_helper() {
    // The owner selects its own private provider declaration through the
    // compiler-owned `product.provider` query and hands the restricted
    // description to a helper in another package. The helper inspects it
    // through `path()` without holding any product namespace of its own. A
    // clean compile plus the emitted log line also witnesses non-execution:
    // had the declared `BuildProduct::provider` body run, the returned
    // authored `ProductProviderRef {}` would carry no description and
    // `path()` would trap.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build, provider: &ProductProviderRef) { builder.log.write_line(provider.path()); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "boundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); setup::configure(builder, &provider); builder.product.provider(\"AudioProvider\"); provider.path(); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "an owner-selected product provider inspects through a helper without executing the declaration",
    );
    let observation = checked
        .build_observation_summary()
        .expect("provider inspection retains a build observation");
    assert_eq!(observation.build_log(), b"AudioProvider\n");
}

#[test]
fn product_provider_query_is_scoped_to_the_query_occurrences_package() {
    // The helper performs the query itself: `AudioProvider` exists only in
    // the owner's package, so the borrowed Build cannot reach it. Selection
    // authority belongs to the query occurrence's package, not the caller's.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build) { let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); builder.log.write_line(provider.path()); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "boundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics = compile_to_checked(request)
        .expect_err("a foreign helper cannot select the caller's private product provider")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        diagnostics
            .contains("not a provider declaration visible from this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn forged_product_provider_description_is_rejected() {
    // An authored `ProductProviderRef {}` has the static type but carries no
    // compiler-issued description payload, so inspection refuses it at
    // evaluation rather than trusting the shape.
    let project = TempProject::new(&application_build(
        "    let forged: ProductProviderRef = ProductProviderRef {};\n    builder.log.write_line(forged.path());",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored ProductProviderRef value is not a product provider description")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("requires a compiler-issued ProductProviderRef"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_query_rejects_a_non_facet_receiver() {
    // An authored `BuildProduct` lookalike resolves the toolchain machine by
    // member lookup but carries no facet marker: selection authority lives on
    // the compiler-issued `Build.product` value only.
    let project = TempProject::with_main(
        "boundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"lookalike-facet\"); let product: BuildProduct = BuildProduct {}; let provider: ProductProviderRef = product.provider(\"AudioProvider\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored BuildProduct value cannot perform the provider query")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("requires the compiler-issued Build.product facet"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_query_requires_a_provider_declaration() {
    // `PlainConfig` names data with no `satisfies` machines and `launch`
    // names a machine: a provider operand selects only authored provider
    // declarations in the occurrence's package, never plain data or entries.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }\ndata PlainConfig { value: u8; }",
        "machine build(builder: &mut Build) { builder.application(\"provider-miss\"); let provider: ProductProviderRef = builder.product.provider(\"PlainConfig\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a plain data declaration cannot stand in for a product provider")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("names no provider declaration in this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );

    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "machine build(builder: &mut Build) { builder.application(\"provider-miss\"); let provider: ProductProviderRef = builder.product.provider(\"launch\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a machine name cannot stand in for a product provider")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("names no provider declaration in this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_cannot_stand_in_for_an_entry_description() {
    // The description kinds are distinct: `roots.bind` consumes only the
    // entry marker, so a provider description is refused wherever an entry
    // description is required.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }\nboundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"provider-not-entry\"); let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); builder.roots.bind(windows_x86_64::ProgramEntry, provider); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a product provider description cannot stand in as a root binding operand")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("not a compiler-issued product entry description")
            || diagnostics.contains("ProductEntryRef"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_query_rejects_an_ambiguous_declaration() {
    // Two same-named provider declarations in the query occurrence's own
    // package cannot mint one description: the query admits exactly one
    // candidate.
    let project = TempProject::with_main(
        "use other;\nboundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"provider-ambiguous\"); let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); }",
    );
    fs::write(
        project.0.join("other.omg"),
        "module other;\nboundary trait OtherPick {\n    machine choose() -> i32;\n}\npub data AudioProvider { }\npub machine AudioProvider::choose() -> i32 satisfies OtherPick::choose {\n    transition { _ -> (1) }\n}",
    )
    .expect("ambiguous sibling source");
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an ambiguous product provider name is refused")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("is ambiguous within its package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_description_is_not_a_static_provider_argument() {
    // `select_provider` still takes its exact static slot and provider type
    // paths; a retained description place cannot substitute for the provider
    // type argument, and the one-type-argument spelling never reaches
    // evaluation.
    let project = TempProject::with_main(
        "boundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"provider-operand\"); let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); builder.select_provider<Pick>(provider); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a product provider description is not a select_provider type argument")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("`select_provider` requires exactly two plain type paths"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn delegated_root_binding_rejects_a_computed_description_result() {
    // The delegated `roots.bind` operand admits only a retained
    // `ProductEntryRef` place: a call result spelling stays fenced until
    // ordinary call-result authority can carry it.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }\nmachine choose_entry() -> ProductEntryRef {\n    transition { _ -> (ProductEntryRef {}) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"computed-operand\"); builder.roots.bind(windows_x86_64::ProgramEntry, choose_entry()); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a computed call result cannot serve as the root binding operand")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains(
            "root-slot binding requires exactly one slot path and one implementation path"
        ) || diagnostics.contains("computed description results are not implemented"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn root_build_aliases_cannot_mutate_target_or_replace_the_activation() {
    for (operation, expected) in [
        (
            "alias.target = TargetProfile::MacosArm64;",
            "Build.target is compiler-owned and cannot be assigned",
        ),
        (
            "let target: &mut TargetProfile = &mut alias.target;",
            "Build.target is compiler-owned and cannot enter a mutable or write-only borrow",
        ),
        (
            "alias = alias;",
            "Build activation cannot be replaced as a whole value",
        ),
    ] {
        let project = TempProject::new(&exact_target_build(&format!(
            "let alias: &mut Build = &mut builder; {operation}"
        )));
        let diagnostics = diagnostic_text(&project);
        assert!(diagnostics.contains(expected), "{operation}: {diagnostics}");
    }
}

#[test]
fn authored_legacy_build_is_rejected_instead_of_receiving_a_hidden_target() {
    let project = TempProject::new(
        r#"data Build {
    subsystem: Subsystem;
    freestanding: bool;
}
data Subsystem {
    case Console;
}
machine build(builder: &mut Build) { }
"#,
    );

    let diagnostics = diagnostic_text(&project);
    assert!(
        diagnostics.contains("must not declare toolchain package vocabulary `Build`"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn exact_x86_build_must_opt_in_before_fma_admission_exists() {
    let baseline = TempProject::new(&exact_target_build(""));
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &baseline.main(),
        Some("windows_x86_64"),
    ))
    .expect("generic x86 baseline must remain available");
    assert_eq!(checked.x86_scalar_fma_provider(), None);

    let opted_in = TempProject::new(&exact_target_build(
        "    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;",
    ));
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &opted_in.main(),
        Some("windows_x86_64"),
    ))
    .expect("exact x86 build may select the canonical AVX+FMA3 deployment pair");
    let provider = checked
        .x86_scalar_fma_provider()
        .expect("explicit feature selection must retain one admitted provider");
    assert_eq!(provider.profile(), target::TargetProfile::WindowsX64);
    assert!(provider.has_canonical_identity());
    assert_eq!(
        provider.deployment().features(),
        &target::X86_SCALAR_FMA_REQUIRED_FEATURES
    );
    assert!(
        checked.x86_scalar_fma_plan_associations().is_empty(),
        "feature admission without source demand must not fabricate an association"
    );
}

#[test]
fn exact_x86_fma_demand_fails_closed_without_feature_admission() {
    let main = pass_canary_main(fixtures::NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT);
    for target in ["linux_x86_64", "windows_x86_64"] {
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(package_inputs_with_standard_library(
                &main,
                "named-provider-fused-multiply-add-exit",
            )),
            ..CheckedCompileRequest::new(&main, Some(target))
        })
        .expect_err("an exact-profile x86 FMA demand requires explicit deployment admission")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
        assert!(
            diagnostics.contains("requires explicit AVX+FMA3 admission"),
            "unexpected {target} diagnostics: {diagnostics}"
        );
        assert!(
            diagnostics.contains("F32::fused_multiply_add")
                && diagnostics.contains("F64::fused_multiply_add"),
            "each exact semantic slot must fail closed: {diagnostics}"
        );
    }
}

#[test]
fn admitted_x86_fma_demand_retains_exact_plan_associations() {
    for (target, profile, other_profile) in [
        (
            "linux_x86_64",
            target::TargetProfile::LinuxX64,
            target::TargetProfile::WindowsX64,
        ),
        (
            "windows_x86_64",
            target::TargetProfile::WindowsX64,
            target::TargetProfile::LinuxX64,
        ),
    ] {
        let main = pass_canary_main(fixtures::X86_FMA_PLAN_ASSOCIATION);
        let checked = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(package_inputs_with_standard_library(
                &main,
                "x86-fma-plan-association",
            )),
            ..CheckedCompileRequest::new(&main, Some(target))
        })
        .unwrap_or_else(|diagnostics| panic!("{target} FMA admission failed: {diagnostics:?}"));
        let provider = checked
            .x86_scalar_fma_provider()
            .expect("explicit feature selection must retain one provider");
        let associations = checked.x86_scalar_fma_plan_associations();
        assert_eq!(
            associations.len(),
            2,
            "repeated calls deduplicate while F32 and F64 remain distinct"
        );
        assert_eq!(
            associations
                .iter()
                .map(|association| association.slot())
                .collect::<Vec<_>>(),
            vec![
                target::X86ScalarFmaSlot::Binary32,
                target::X86ScalarFmaSlot::Binary64,
            ]
        );
        assert_eq!(
            associations
                .iter()
                .map(|association| association.selected_builtin())
                .collect::<Vec<_>>(),
            vec![
                symbols::BuiltinFunction::FloatFusedMultiplyAddF32,
                symbols::BuiltinFunction::FloatFusedMultiplyAddF64,
            ]
        );
        for association in associations {
            assert_eq!(association.selected_plan().target, target);
            assert_eq!(association.admitted_provider(), provider);
            assert!(
                association.matches_checked_inputs(checked.selected_provider_plans(), provider)
            );
        }
        assert_ne!(
            associations[0].selected_plan_digest(),
            associations[1].selected_plan_digest(),
            "F32 and F64 require distinct exact plans"
        );
        assert!(
            checked
                .selected_provider_plans()
                .plans()
                .iter()
                .any(|plan| plan.schema.trait_name.contains("F32::multiply_then_add")),
            "the fixture must select multiply-then-add while the FMA association excludes it"
        );

        let wrong_profile_provider = target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
            other_profile,
            &target::X86_SCALAR_FMA_REQUIRED_FEATURES,
        )
        .expect("canonical cross-profile provider fixture");
        assert!(
            associations.iter().all(|association| !association
                .matches_checked_inputs(checked.selected_provider_plans(), wrong_profile_provider)),
            "a provider for another policy profile must not substitute"
        );

        let mut substituted_plans = checked.selected_provider_plans().plans().to_vec();
        let associated_digest = associations[0].selected_plan_digest();
        substituted_plans
            .iter_mut()
            .find(|plan| plan.identity_digest() == associated_digest)
            .expect("associated plan must remain in the selected closure")
            .name
            .push_str(".substituted");
        let substituted =
            effects::SelectedProviderPlanFacts::from_selected_plans(substituted_plans)
                .expect("structurally valid substituted closure");
        assert!(
            !associations[0].matches_checked_inputs(&substituted, provider),
            "compact coordinates cannot authorize an exact-plan substitution"
        );
        assert_eq!(provider.profile(), profile);
    }
}

#[test]
fn boundary_operator_and_float_adapters_retain_terminal_execution() {
    let project = TempProject::with_main(
        r#"use omega::language::core::float_operations;
use omega_language_std::console;

data Arithmetic {}
boundary operator Arithmetic::identity(value: i32) -> i32;
data ArithmeticProvider {}
machine ArithmeticProvider::identity(value: i32) -> i32
    satisfies Arithmetic::identity { value }

data Main { console: Console; }
machine Main::main(&mut self) {
    let fused32: f32 = F32::fused_multiply_add(2.0f32, 3.0f32, 4.0f32);
    let fused64: f64 = F64::fused_multiply_add(2.0f64, 3.0f64, 4.0f64);
    self.emit();
}
machine Main::emit(&mut self) reaches Console {
    let value: i32 = Arithmetic::identity(7);
    self.console.write_line("mixed execution");
    self.console.exit_process(value);
}
"#,
        &application_build(
            r#"    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
    builder.select_provider<Arithmetic::identity, ArithmeticProvider>();"#,
        ),
    );
    let package_inputs = package_inputs_with_standard_library(&project.main(), "target-activation");
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs.clone()),
        ..CheckedCompileRequest::new(&project.main(), Some("linux_x86_64"))
    })
    .expect("derive the exact mixed fixture provider plans");
    let console_binding = console_acceptance::candidate_console_exit_binding(
        &preliminary,
        package_identity(2),
        true,
        false,
    )
    .expect("the fixture explicitly accepts Console output and exit");
    let package_inputs = package_inputs
        .with_accepted_semantic_bindings(vec![console_binding])
        .unwrap();
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
        .with_package_inputs(package_inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("mixed selected execution failed: {diagnostics:#?}"));
    let retained = report.into_retained_terminal_artifact().unwrap();
    retained
        .validate()
        .expect("mixed Terminal artifact verifies");
    let proposal = retained.native_realization_proposal().unwrap();
    assert_eq!(
        proposal
            .ieee_float_fma_occurrences()
            .iter()
            .map(|occurrence| occurrence.format())
            .collect::<Vec<_>>(),
        [
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64
        ],
        "boundary settlement preserves both ordered FMA applications"
    );
    assert_eq!(
        proposal
            .checked_boundary_operator_scope()
            .occurrences()
            .len(),
        3,
        "both float applications and the checked operator retain occurrence custody"
    );
    assert_eq!(proposal.boundary_application_realizations().rows().iter().filter(|realization| {
        realization.role() == boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody
    }).count(), 1, "boundary settlement preserves the checked operator realization");
}

#[test]
fn terminal_product_retains_exact_fma_operation_plan_and_x86_admission() {
    let project = TempProject::with_main(
        r#"use omega::language::core::float_operations;

data Main { }

machine Main::main(&mut self) {
    let fused32: f32 = F32::fused_multiply_add(
        1.00000011920928955078125f32,
        0.99999988079071044921875f32,
        -1.0f32,
    );
    let fused64: f64 = F64::fused_multiply_add(
        1.0000000000000002220446049250313080847263336181640625f64,
        0.9999999999999997779553950749686919152736663818359375f64,
        -1.0f64,
    );
}
"#,
        &application_build(
            r#"    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;"#,
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("FMA Terminal custody failed: {diagnostics:#?}"));
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    let proposal = retained
        .native_realization_proposal()
        .expect("Terminal report retains native proposal");
    let occurrences = proposal.ieee_float_fma_occurrences();
    assert_eq!(
        occurrences.len(),
        2,
        "both source-ordered selected FMA occurrences must survive"
    );
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence.format())
            .collect::<Vec<_>>(),
        vec![
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        ],
        "mixed formats must retain source order and distinct semantic slots"
    );
    assert_ne!(
        occurrences[0].terminal_operation(),
        occurrences[1].terminal_operation(),
        "plural source occurrences require distinct Terminal coordinates"
    );
    assert_ne!(
        occurrences[0].provider_plan_index(),
        occurrences[1].provider_plan_index(),
        "F32 and F64 must rejoin distinct selected plans"
    );
    let terminal_operations = occurrences
        .iter()
        .map(|occurrence| occurrence.terminal_operation())
        .collect::<Vec<_>>();
    let application_realizations = proposal.boundary_application_realizations().rows();
    assert_eq!(application_realizations.len(), 2);
    assert!(application_realizations.iter().all(|realization| {
        realization.role()
            == boundary_applications::BoundaryApplicationRealizationRole::ExactCompilerIntrinsic
            && realization.selected_plan_digest() != &[0; 32]
            && terminal_operations.contains(&realization.terminal_operation())
    }));
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("canonical Terminal semantics decode");

    for occurrence in occurrences {
        let plan = &proposal.selected_provider_plans().plans()[occurrence.provider_plan_index()];
        assert_eq!(plan.target, "linux_x86_64");
        let admission = occurrence
            .x86_admission()
            .expect("x86 occurrence retains admitted deployment provider");
        assert_eq!(
            admission.provider().profile(),
            target::TargetProfile::LinuxX64
        );
        assert_eq!(
            admission.slot(),
            match occurrence.format() {
                semantic_vocabulary::IeeeFloatFormat::Binary32 =>
                    target::X86ScalarFmaSlot::Binary32,
                semantic_vocabulary::IeeeFloatFormat::Binary64 =>
                    target::X86ScalarFmaSlot::Binary64,
            }
        );
        let matching = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == occurrence.terminal_operation())
            .collect::<Vec<_>>();
        let [operation] = matching.as_slice() else {
            panic!("occurrence must name one exact Terminal operation")
        };
        assert!(matches!(
            operation.kind,
            terminal_psi::OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        ));
    }
    retained
        .validate()
        .expect("FMA occurrence proposal replays against canonical artifact");
    let native = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                image_request,
                imports: &[],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| panic!("FMA native custody failed: {diagnostics:#?}"));
    native
        .validate()
        .expect("FMA native artifact independently replays");
    let physical = native
        .physical_evidence()
        .expect("FMA native artifact retains complete D32 evidence");
    assert_eq!(physical.projection().operator_occurrences().len(), 2);
    assert_eq!(physical.children().len(), 2);
    assert!(physical.children().iter().all(|child| {
        matches!(
            child.parent(),
            native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
        ) && matches!(
            child.occurrence(),
            native_realization::NativePhysicalOccurrence::Operator(_)
        ) && child.relocation()
            == native_realization::PhysicalRelocationDisposition::DirectInstructionBytes
    }));
    let fma_functions = native
        .object()
        .functions()
        .iter()
        .filter(|function| !function.x86_scalar_fma_occurrences.is_empty())
        .collect::<Vec<_>>();
    let [function] = fma_functions.as_slice() else {
        panic!("one bounded FMA function must survive ordinary object construction")
    };
    assert_eq!(function.x86_scalar_fma.len(), 2);
    assert_eq!(function.x86_scalar_fma_occurrences.len(), 2);
    assert_eq!(
        function
            .x86_scalar_fma_occurrences
            .iter()
            .map(|occurrence| occurrence.terminal_operation)
            .collect::<Vec<_>>(),
        terminal_operations,
        "machine custody must retain the exact source-ordered Terminal roster"
    );
    assert_eq!(
        function
            .x86_scalar_fma_occurrences
            .iter()
            .map(|occurrence| occurrence.slot)
            .collect::<Vec<_>>(),
        vec![
            target::X86ScalarFmaSlot::Binary32,
            target::X86ScalarFmaSlot::Binary64,
        ]
    );
    let control = function
        .x86_floating_control
        .expect("native FMA function retains canonical MXCSR custody");
    assert_eq!(control.canonical_mxcsr, 0x1f80);
    assert_eq!(
        native.object().x86_scalar_fma_provider(),
        Some(function.x86_scalar_fma_occurrences[0].admitted_provider)
    );
    assert!(
        function
            .x86_scalar_fma_occurrences
            .iter()
            .all(|occurrence| {
                occurrence.admitted_provider
                    == function.x86_scalar_fma_occurrences[0].admitted_provider
            })
    );
    let occurrence = function.x86_scalar_fma_occurrences[0];
    let mut parts = native.into_parts();
    let selected = parts
        .selected_provider_plans
        .iter_mut()
        .find(|plan| plan.report_identity() == occurrence.provider_plan_report_identity)
        .expect("FMA occurrence rejoins one selected native plan");
    let mut substituted_digest = *selected.plan_digest().as_bytes();
    substituted_digest[0] ^= 1;
    *selected = native_realization::NativeSelectedProviderPlan::new(
        selected.report_identity(),
        native_realization::NativeSelectedProviderPlanDigest::from_digest(substituted_digest),
        selected.requirement_identities().to_vec(),
    );
    let error = compilation_report::RetainedNativeArtifact::from_replayed_parts(parts)
        .expect_err("a substituted exact selected plan must reject native FMA replay");
    assert_eq!(
        error,
        "native artifact nearest-FMA does not rejoin one exact selected provider plan"
    );
}

#[test]
fn source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope() {
    let project = TempProject::with_main(
        r#"use omega::language::core::float_operations;

data Main { }

machine Main::after_fma(&mut self) { }

machine Main::main(&mut self) {
    let fused: f32 = F32::fused_multiply_add(
        1.00000011920928955078125f32,
        0.99999988079071044921875f32,
        -1.0f32,
    );
    self.after_fma();
}
"#,
        &application_build(
            r#"    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;"#,
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!("FMA followed by an internal Unit call should lower: {diagnostics:#?}")
    });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains native proposal");
    assert_eq!(
        retained
            .native_realization_proposal()
            .unwrap()
            .ieee_float_fma_occurrences()
            .len(),
        1
    );
    let native = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                image_request,
                imports: &[],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| {
        panic!("FMA plus internal Unit call should realize natively: {diagnostics:#?}")
    });
    native.validate().expect("native artifact replays");
    let function = native
        .object()
        .functions()
        .iter()
        .find(|function| !function.x86_scalar_fma_occurrences.is_empty())
        .expect("source FMA function survives object construction");
    let control = function
        .x86_floating_control
        .expect("FMA function has one canonical MXCSR envelope");
    let [call] = function.internal_unit_calls.as_slice() else {
        panic!("source function retains one internal Unit call")
    };
    assert!(control.install_offset + control.install_byte_count <= call.code_offset);
    assert!(call.code_offset + call.byte_count <= control.restore_offset);
}

#[test]
fn aarch64_fma_demand_is_not_an_x86_feature_association() {
    let main = pass_canary_main(fixtures::NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT);
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_with_standard_library(
            &main,
            "named-provider-fused-multiply-add-exit",
        )),
        ..CheckedCompileRequest::new(&main, Some("linux_arm64"))
    })
    .expect("AArch64 FMA remains admitted by its own target realization");
    assert_eq!(checked.x86_scalar_fma_provider(), None);
    assert!(checked.x86_scalar_fma_plan_associations().is_empty());
}

#[test]
fn x86_fma_build_admission_binds_the_exact_selected_profile() {
    let project = TempProject::new(
        r#"machine build(builder: &mut Build) {
    builder.application("profile-bound-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let linux = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("linux_x86_64"),
    ))
    .expect("Linux x86 deployment selection");
    let windows = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("Windows x86 deployment selection");
    let linux = linux.x86_scalar_fma_provider().expect("Linux admission");
    let windows = windows
        .x86_scalar_fma_provider()
        .expect("Windows admission");

    assert_eq!(linux.profile(), target::TargetProfile::LinuxX64);
    assert_eq!(windows.profile(), target::TargetProfile::WindowsX64);
    assert_ne!(linux.identity(), windows.identity());
}

#[test]
fn non_x86_profile_rejects_x86_deployment_feature_selection() {
    let project = TempProject::new(
        r#"machine build(builder: &mut Build) {
    builder.application("invalid-arm-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("linux_arm64"),
    ))
    .expect_err("an AArch64 profile cannot admit x86 deployment features")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");

    assert!(
        diagnostics.contains(
            "Build.x86_deployment_features cannot admit AVX+FMA3 for exact profile `linux_arm64`"
        ),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn targetless_build_cannot_mint_x86_deployment_feature_admission() {
    let project = TempProject::new(
        r#"machine build(builder: &mut Build) {
    builder.application("targetless-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect_err("targetless checking has no deployment feature field")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        diagnostics.contains("x86_deployment_features"),
        "unexpected diagnostics: {diagnostics}"
    );
}
