use super::{
    MACOS_HOSTED_MAIN, TempProject, application_build, diagnostic_text, exact_target_build,
};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct,
    RetainedNativeRealizationRequest, compile, compile_to_checked,
    realize_retained_native_artifact,
};
use std::fs;

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
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
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
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
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
            terminal_authority_permission_policy: Some(
                native_realization::current_terminal_authority_permission_policy(),
            ),
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
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
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
            terminal_authority_permission_policy: Some(
                native_realization::current_terminal_authority_permission_policy(),
            ),
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
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
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
            terminal_authority_permission_policy: Some(
                native_realization::current_terminal_authority_permission_policy(),
            ),
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
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
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
            terminal_authority_permission_policy: Some(
                native_realization::current_terminal_authority_permission_policy(),
            ),
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
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
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
            terminal_authority_permission_policy: Some(
                native_realization::current_terminal_authority_permission_policy(),
            ),
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
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
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
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
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
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
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
fn canonical_spelling_selects_the_profile_and_retired_aliases_reject() {
    let project = TempProject::new(&exact_target_build(""));
    let canonical = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("canonical CLI spelling should compile");
    assert_eq!(
        canonical
            .selected_target_profile()
            .expect("selected profile")
            .target_name(),
        "windows_x86_64"
    );
    assert!(canonical.selected_native_target().is_some());

    for retired in ["windows_x64", "linux_x64", "uefi_x64"] {
        let diagnostics =
            compile_to_checked(CheckedCompileRequest::new(&project.main(), Some(retired)))
                .expect_err("retired CLI alias must reject before source selection");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains(&format!("unknown target profile `{retired}`"))),
            "{retired} should reject as an unknown target profile: {diagnostics:?}"
        );
    }
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
