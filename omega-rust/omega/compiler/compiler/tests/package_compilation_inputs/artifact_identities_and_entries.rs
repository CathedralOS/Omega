use super::{TempTree, identity};
use crate::fixtures;
use checked_interpreter::InterpretOptions;
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct,
    RetainedNativeRealizationRequest, TargetCompileConfiguration, compile, compile_to_checked,
    realize_retained_native_artifact, retained_terminal_report_from_checked_package,
};
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind,
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::fs;
use std::path::Path;

#[test]
fn failing_sibling_does_not_change_successful_package_artifact_identity() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root")
        .join("tests/omega/pass")
        .join(fixtures::NO_SELECTION_EMPTY_ENTRY);
    let package = identity(74);
    let output = TempTree::new();
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "multi-target-artifact-fixture",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("single-package artifact graph");
    let (sources, target_inputs) = inputs.into_parts();
    let request = CompileRequest::new(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: None,
        target_name: None,
    })
    .with_package_sources(sources)
    .with_requested_product(RequestedCompileProduct::NativeArtifact);
    let configuration_for_target = |profile: target::TargetProfile| {
        TargetCompileConfiguration::new(profile)
            .with_build_dir(output.0.join(profile.target_name()))
            .with_package_target_inputs(target_inputs.clone())
    };

    let linux_only = request
        .clone()
        .with_target_configurations(vec![configuration_for_target(
            target::TargetProfile::LinuxX64,
        )]);
    let linux_only = compile(linux_only).expect("one-target batch should admit");
    let linux_only = linux_only.outcomes()[0]
        .report()
        .expect("Linux artifact should compile without siblings");
    let linux_only_manifest = linux_only
        .production_manifest()
        .expect("package artifact should retain its exact manifest");

    let with_failing_sibling = request.with_target_configurations(vec![
        configuration_for_target(target::TargetProfile::UefiX64),
        configuration_for_target(target::TargetProfile::LinuxX64),
    ]);
    let with_failing_sibling =
        compile(with_failing_sibling).expect("two-target batch should admit");
    assert_eq!(with_failing_sibling.outcomes().len(), 2);
    let linux_with_sibling = with_failing_sibling.outcomes()[0]
        .report()
        .expect("Linux artifact should survive the failing sibling");
    assert_eq!(
        linux_with_sibling
            .production_manifest()
            .expect("batched Linux package manifest")
            .identity(),
        linux_only_manifest.identity(),
    );
    assert_eq!(
        linux_with_sibling
            .retained_native_artifact()
            .expect("batched Linux retained artifact")
            .identity(),
        linux_only
            .retained_native_artifact()
            .expect("one-target Linux retained artifact")
            .identity(),
    );
    assert_eq!(
        with_failing_sibling.outcomes()[1].target_profile(),
        Some(target::TargetProfile::UefiX64),
    );
    assert!(
        with_failing_sibling.outcomes()[1]
            .diagnostics()
            .expect("undeclared UEFI target should reject only its child")
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uefi_x86_64")),
    );
}

#[test]
fn reviewed_checked_package_continues_after_generated_source_staging_is_removed() {
    let tree = TempTree::new();
    let root = tree.package("reviewed-generated-root");
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("reviewed-generated-root");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    let generated: BuildPath = builder.output.resolve("marker.generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(descriptor, "pub data GeneratedMarker { value: u8; }\n");
    let close: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
}
"#,
    );
    TempTree::write(
        root.join("main.omg"),
        "data Main { }\nmachine Main::main(&mut self) { }\n",
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.join("build.omg"), fs::Permissions::from_mode(0o444))
            .expect("seal application build source");
        fs::set_permissions(root.join("main.omg"), fs::Permissions::from_mode(0o444))
            .expect("seal application source");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555))
            .expect("seal application source root");
    }
    let inputs = PackageCompilationInputs::new(
        identity(63),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(identity(63), "reviewed-generated-root", root.clone())
                .with_canonical_source_metadata()
                .expect("canonical application Source metadata"),
        ],
        Vec::new(),
    )
    .expect("single application package graph");
    let session_root = tree.0.join("review-session");
    fs::create_dir(&session_root).expect("create sponsored review session");
    let session_root = fs::canonicalize(session_root).expect("canonical sponsored review session");
    let build_dir = session_root.join("reviewed-generated-root");
    let filesystem_sponsor =
        checked_interpreter::FilesystemSponsor::new(&session_root).expect("sponsor review session");
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.to_owned()),
        package_inputs: Some(inputs),
        filesystem_sponsor: Some(filesystem_sponsor),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("package review retains its generated source in checked custody");
    assert_eq!(
        checked
            .package_generated_source_bundle()
            .expect("checked package generated-source bundle")
            .sources()
            .len(),
        1
    );
    fs::remove_dir_all(&session_root).expect("remove package review staging before production");

    let report = retained_terminal_report_from_checked_package(
        root.join("main.omg"),
        checked,
        proof_admission::AdmissionProfile::default(),
    )
    .expect("retained checked package continues without reopening build staging");
    assert_eq!(
        report.output_kind(),
        compiler::CompileOutputKind::TerminalArtifact
    );
    assert!(report.production_manifest().is_some());

    let standalone_root = tree.package("standalone-checked");
    TempTree::write(
        standalone_root.join("main.omg"),
        "pub data Standalone { value: u8; }\n",
    );
    let standalone = compile_to_checked(CheckedCompileRequest::new(
        &standalone_root.join("main.omg"),
        None,
    ))
    .expect("standalone source checks without package custody");
    let diagnostics = retained_terminal_report_from_checked_package(
        standalone_root.join("main.omg"),
        standalone,
        proof_admission::AdmissionProfile::default(),
    )
    .expect_err("package-only continuation rejects standalone checked custody");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires package-aware checked custody")
    }));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755))
            .expect("restore temporary source root for cleanup");
    }
}

#[test]
fn accepted_package_console_binding_closes_linux_intrinsic_without_toolchain_origin() {
    let tree = TempTree::new();
    let root = tree.package("accepted-console-application");
    let console = tree.package("accepted-console-package");
    let root_package = identity(48);
    let console_package = identity(49);
    let exact_console_source = r#"pub boundary trait Console {
    machine exit_process(return_code: i32)
    reaches Console;
}

pub data ConsoleNativeProvider { }
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
"#;
    TempTree::write(console.join("console.omg"), exact_console_source);
    TempTree::write(
        root.join("main.omg"),
        r#"use accepted_console::console;
data Main { console: Console; }
machine Main::main(&mut self) reaches Console {
    self.console.exit_process(70);
}
"#,
    );
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("accepted-console-application");
    builder.depend_as("accepted_console", Source::Path { location: "../accepted-console-package" });
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    );
    let base_inputs = || {
        PackageCompilationInputs::new(
            root_package,
            BuildDeclarationKind::Application,
            vec![
                PackageSourceBinding::new(
                    root_package,
                    "accepted-console-application",
                    root.clone(),
                ),
                PackageSourceBinding::new(
                    console_package,
                    "accepted-console-package",
                    console.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                root_package,
                "accepted_console",
                console_package,
            )],
        )
        .expect("ordinary application and Console dependency graph")
    };

    let candidate = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(base_inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("candidate Console package should check before consumer acceptance");
    let (plan, retained) = candidate
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(candidate.selected_provider_provenance())
        .find(|(plan, _)| plan.schema.trait_name == "Console")
        .expect("candidate retains its exact Console provider plan");
    let exit = plan
        .rows
        .iter()
        .position(|row| row.method == "exit_process")
        .expect("candidate Console plan retains exit_process");
    assert_eq!(
        retained.row_compiler_intrinsic_executions[exit], None,
        "ordinary package provenance alone cannot select a compiler intrinsic",
    );
    let declaration_path = candidate
        .typed
        .symbols
        .display_path(retained.provider.schema.symbol(), "::");
    let schema_digest = plan.schema.identity_digest();
    let provider_plan_digest = plan.identity_digest();
    let exit_requirement = plan.rows[exit].requirement_identity.clone();
    for (label, stale) in [
        (
            "nominal declaration",
            AcceptedSemanticBinding::new(
                AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                console_package,
                "OtherConsole",
                schema_digest,
                provider_plan_digest,
            )
            .unwrap(),
        ),
        (
            "service schema",
            AcceptedSemanticBinding::new(
                AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                console_package,
                declaration_path.clone(),
                effects::provider_plan::ServiceSchemaDigest::from_digest([91; 32]),
                provider_plan_digest,
            )
            .unwrap(),
        ),
        (
            "provider plan",
            AcceptedSemanticBinding::new(
                AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                console_package,
                declaration_path.clone(),
                schema_digest,
                effects::provider_plan::ProviderPlanDigest::from_digest([92; 32]),
            )
            .unwrap(),
        ),
    ] {
        let stale_inputs = base_inputs()
            .with_accepted_semantic_bindings(vec![stale])
            .expect("stale binding remains graph-scoped input authority");
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(stale_inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
        })
        .expect_err("stale accepted semantic authority must not survive settlement");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("was not consumed by one exact selected provider plan")),
            "unexpected {label} diagnostics: {diagnostics:#?}",
        );
    }
    let accepted = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        console_package,
        declaration_path,
        schema_digest,
        provider_plan_digest,
    )
    .expect("candidate-derived accepted Console binding")
    .with_terminal_authority_permissions(vec![effects::ServiceTerminalAuthorityPermission::new(
        schema_digest,
        exit_requirement.clone(),
        effects::TerminalAuthorityDisposition::from_classes([
            effects::TerminalAuthorityClass::ProcessTermination,
        ]),
    )])
    .expect("accepted Console binding carries its exact terminal permission");

    for (label, declaration) in [
        (
            "unscoped boundary realization",
            "boundary machine ConsoleNativeProvider::exit_process(return_code: i32)\n    satisfies Console::exit_process;",
        ),
        (
            "wrong-target boundary realization",
            "windows_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)\n    satisfies Console::exit_process;",
        ),
    ] {
        TempTree::write(
            console.join("console.omg"),
            &format!(
                "pub boundary trait Console {{\n    machine exit_process(return_code: i32)\n    reaches Console;\n}}\n\npub data ConsoleNativeProvider {{ }}\n{declaration}\n"
            ),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(
                base_inputs()
                    .with_accepted_semantic_bindings(vec![accepted.clone()])
                    .expect("negative binding remains graph-scoped authority"),
            ),
            ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
        })
        .expect_err("non-selected catalog origin must not consume accepted authority");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("was not consumed by one exact selected provider plan")
                    || diagnostic
                        .message
                        .contains("unknown boundary slot `Console`")
            }),
            "unexpected {label} diagnostics: {diagnostics:#?}",
        );
    }

    TempTree::write(
        console.join("console.omg"),
        r#"pub boundary trait Console {
    machine exit_process(return_code: i32)
    reaches Console;
}

pub data ConsoleNativeProvider { }
linux_x86_64 machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process
    via Binding::CompilerIntrinsic;
"#,
    );
    let legacy = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![accepted.clone()])
                .expect("legacy-via binding remains graph-scoped authority"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("legacy `via` remains semantically selectable during migration");
    let (legacy_plan, legacy_retained) = legacy
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(legacy.selected_provider_provenance())
        .find(|(plan, _)| plan.schema.trait_name == "Console")
        .expect("legacy Console plan");
    let legacy_exit = legacy_plan
        .rows
        .iter()
        .position(|row| row.method == "exit_process")
        .expect("legacy Console exit row");
    assert_eq!(
        legacy_retained.row_compiler_intrinsic_executions[legacy_exit], None,
        "authored legacy `via` must remain outside physical compiler-intrinsic closure",
    );

    TempTree::write(console.join("console.omg"), exact_console_source);
    let accepted_inputs = base_inputs()
        .with_accepted_semantic_bindings(vec![accepted])
        .expect("accepted binding names the exact package closure");
    let output = tree.0.join("accepted-console-output");
    let compile_retained = |label: &str| {
        compile(
            CompileRequest::new(CompileOptions {
                root_path: root.join("main.omg"),
                build_dir: Some(output.join(label)),
                target_name: Some("linux_x86_64".to_owned()),
            })
            .with_package_inputs(accepted_inputs.clone())
            .with_requested_product(RequestedCompileProduct::TerminalArtifact),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!(
                "exact accepted Console binding should reach retained Terminal: {diagnostics:#?}"
            )
        })
        .into_retained_terminal_artifact()
        .expect("Terminal request retains its exact product")
    };
    let checked_for_subject = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(accepted_inputs.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("accepted Console package checks for production-subject substitution test");
    let mismatched_profile = target::TargetProfile::WindowsX64;
    let mismatched_subject = compilation_report::ProductionCompilationSubject::from_checked(
        checked_for_subject
            .package_compilation_subject()
            .expect("package-aware check retains its package subject")
            .clone(),
        checked_for_subject
            .selected_build_machine_identity()
            .expect("package-aware check retains its build-machine identity")
            .to_owned(),
        checked_for_subject
            .build_evaluation_usage()
            .expect("package-aware check retains build-evaluation usage"),
        checked_for_subject
            .build_observation_summary()
            .expect("package-aware check retains build-observation custody"),
        mismatched_profile,
        mismatched_profile.native_target(),
    )
    .expect("internally coherent but substituted production subject");
    assert_eq!(
        compilation_report::CompileReport::from_retained_terminal_artifact(
            root.join("main.omg"),
            checked_for_subject.source_file_count(),
            compile_retained("retained-target-substitution"),
            Some(mismatched_subject),
        )
        .expect_err("production subject target cannot differ from retained proposal target"),
        "compiler report retained inconsistent Terminal-artifact custody",
    );
    let retained = compile_retained("retained-exact");
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    let [retained_permission] = proposal.package_terminal_authority_permissions() else {
        panic!("retained Terminal proposal preserves one exact package permission")
    };
    assert_eq!(retained_permission.service_schema(), schema_digest);
    assert_eq!(retained_permission.requirement_identity(), exit_requirement);
    assert_eq!(
        retained_permission.permitted().classes(),
        &[effects::TerminalAuthorityClass::ProcessTermination]
    );
    assert!(
        compilation_report::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            compilation_report::TerminalNativeRealizationInputs {
                target_profile: proposal.target_profile(),
                native_target: proposal.native_target(),
                subsystem: proposal.subsystem(),
                application_intent: proposal.application_intent(),
                application_identifier: proposal.application_identifier().cloned(),
                application_name: proposal.application_name().map(str::to_owned),
                post_terminal_optimizations: proposal.post_terminal_optimizations().clone(),
                program_entry: proposal.program_entry().clone(),
                checked_program_entry: proposal.checked_program_entry().clone(),
                selected_provider_plans: proposal.selected_provider_plans().clone(),
                external_binding_rows: proposal.external_binding_rows().to_vec(),
                package_terminal_authority_permissions: vec![
                    retained_permission.clone(),
                    retained_permission.clone()
                ],
                compiler_builtins: proposal.compiler_builtins().to_vec(),
                callback_occurrences: proposal.callback_occurrences().to_vec(),
                ieee_float_fma_occurrences: proposal.ieee_float_fma_occurrences().to_vec(),
                ieee_float_comparison_occurrences: proposal
                    .ieee_float_comparison_occurrences()
                    .to_vec(),
                integer_comparison_occurrences: proposal.integer_comparison_occurrences().to_vec(),
                boundary_application_demands: proposal.boundary_application_demands().clone(),
                boundary_application_realizations: proposal
                    .boundary_application_realizations()
                    .clone(),
                checked_boundary_operator_scope: proposal.checked_boundary_operator_scope().clone(),
                behavior_exclusions: proposal.behavior_exclusions().clone()
            }
        )
        .is_err(),
        "retained Terminal proposal must reject duplicate package permission coordinates",
    );

    let permission_policy = |classes: &[effects::TerminalAuthorityClass], include_unrelated| {
        let mut rows = vec![
            native_realization::TerminalAuthorityPermissionPolicyRow::new(
                schema_digest,
                exit_requirement.clone(),
                effects::TerminalAuthorityDisposition::from_classes(classes.iter().copied()),
            ),
        ];
        if include_unrelated {
            rows.push(
                native_realization::TerminalAuthorityPermissionPolicyRow::new(
                    effects::provider_plan::ServiceSchemaDigest::from_digest([93; 32]),
                    "Unrelated::operation#exact",
                    effects::TerminalAuthorityDisposition::from_classes([]),
                ),
            );
        }
        native_realization::terminal_authority_permission_policy_with_rows(rows)
            .expect("exact receiving permission policy")
    };
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let accepted_permission_policy = || {
        permission_policy(
            &[effects::TerminalAuthorityClass::ProcessTermination],
            false,
        )
    };
    let reconstruct_with_permissions = |label: &str,
                                        permissions: Vec<
        effects::ServiceTerminalAuthorityPermission,
    >| {
        let retained = compile_retained(label);
        let (artifact, callback_placements, proposal) = retained.into_parts();
        let proposal = proposal.expect("retained Terminal product has a native proposal");
        let reconstructed = compilation_report::TerminalNativeRealizationProposal::new(
            &artifact,
            compilation_report::TerminalNativeRealizationInputs {
                target_profile: proposal.target_profile(),
                native_target: proposal.native_target(),
                subsystem: proposal.subsystem(),
                application_intent: proposal.application_intent(),
                application_identifier: proposal.application_identifier().cloned(),
                application_name: proposal.application_name().map(str::to_owned),
                post_terminal_optimizations: proposal.post_terminal_optimizations().clone(),
                program_entry: proposal.program_entry().clone(),
                checked_program_entry: proposal.checked_program_entry().clone(),
                selected_provider_plans: proposal.selected_provider_plans().clone(),
                external_binding_rows: proposal.external_binding_rows().to_vec(),
                package_terminal_authority_permissions: permissions,
                compiler_builtins: proposal.compiler_builtins().to_vec(),
                callback_occurrences: proposal.callback_occurrences().to_vec(),
                ieee_float_fma_occurrences: proposal.ieee_float_fma_occurrences().to_vec(),
                ieee_float_comparison_occurrences: proposal
                    .ieee_float_comparison_occurrences()
                    .to_vec(),
                integer_comparison_occurrences: proposal.integer_comparison_occurrences().to_vec(),
                boundary_application_demands: proposal.boundary_application_demands().clone(),
                boundary_application_realizations: proposal
                    .boundary_application_realizations()
                    .clone(),
                checked_boundary_operator_scope: proposal.checked_boundary_operator_scope().clone(),
                behavior_exclusions: proposal.behavior_exclusions().clone(),
            },
        )
        .expect("syntactically valid reconstructed proposal");
        compilation_report::RetainedTerminalArtifact::new_with_native_realization_proposal(
            artifact,
            callback_placements,
            reconstructed,
        )
        .expect("syntactically valid reconstructed retained product")
    };

    let omitted = {
        let retained = reconstruct_with_permissions("retained-omitted-proposal", vec![]);
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &profile,
                optimization_selections: &optimizations,
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy: accepted_permission_policy(),
                terminal_authority_permission_policy: Some(accepted_permission_policy()),
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
    .expect_err("reconstructed proposal cannot omit an accepted package permission");
    assert!(
        omitted.iter().any(|diagnostic| diagnostic
            .message
            .contains("differ from the independently accepted package policy")),
        "unexpected omitted-proposal diagnostics: {omitted:#?}",
    );

    // Retained production without a receiving permission policy emits an
    // artifact that carries exact package custody but no receiver-admission
    // claim: the accepted package row is never projected into a receiver row.
    let unclaimed = {
        let retained = compile_retained("retained-no-receiving-policy");
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &profile,
                optimization_selections: &optimizations,
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy: accepted_permission_policy(),
                terminal_authority_permission_policy: None,
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
    .expect("retained production without a receiving policy emits an artifact");
    assert_eq!(
        unclaimed.terminal_authority_permission_policy_identity(),
        None,
        "absent receiving policy is not an empty policy and mints no identity",
    );
    assert!(
        unclaimed
            .terminal_authority_closure_review()
            .leaves()
            .iter()
            .all(|leaf| leaf.permitted().is_none()),
        "no leaf may carry a receiver-admission verdict without a receiving policy",
    );
    let accepted_policy_identity =
        native_realization::current_terminal_authority_permission_policy().identity();
    assert!(
        unclaimed
            .validate_for_terminal_authority_policies(
                unclaimed.terminal_authority_policy_identity(),
                accepted_policy_identity,
                unclaimed.terminal_authority_closure_review().identity(),
            )
            .is_err(),
        "an artifact with no receiver-admission claim cannot satisfy explicit admission replay",
    );

    let widened_permission = effects::ServiceTerminalAuthorityPermission::new(
        schema_digest,
        exit_requirement.clone(),
        effects::TerminalAuthorityDisposition::from_classes([
            effects::TerminalAuthorityClass::ProcessOutput,
            effects::TerminalAuthorityClass::ProcessTermination,
        ]),
    );
    let widened = {
        let retained =
            reconstruct_with_permissions("retained-widened-proposal", vec![widened_permission]);
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &profile,
                optimization_selections: &optimizations,
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy: accepted_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy(
                    &[
                        effects::TerminalAuthorityClass::ProcessOutput,
                        effects::TerminalAuthorityClass::ProcessTermination,
                    ],
                    false,
                )),
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
    .expect_err("coordinated proposal/policy widening cannot replace accepted evidence");
    assert!(
        widened.iter().any(|diagnostic| diagnostic
            .message
            .contains("differ from the independently accepted package policy")),
        "unexpected widened-proposal diagnostics: {widened:#?}",
    );

    let missing = {
        let retained = compile_retained("retained-missing");
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &profile,
                optimization_selections: &optimizations,
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy: accepted_permission_policy(),
                terminal_authority_permission_policy: Some(
                    native_realization::current_terminal_authority_permission_policy(),
                ),
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
    .expect_err("retained re-entry must reject a missing accepted package permission");
    assert!(
        missing
            .iter()
            .any(|diagnostic| diagnostic.message.contains("omits the accepted permission")),
        "unexpected missing-permission diagnostics: {missing:#?}",
    );

    let substituted = {
        let retained = compile_retained("retained-substituted");
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &profile,
                optimization_selections: &optimizations,
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy: accepted_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy(
                    &[effects::TerminalAuthorityClass::ProcessOutput],
                    false,
                )),
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
    .expect_err("retained re-entry must reject changed accepted package classes");
    assert!(
        substituted.iter().any(|diagnostic| diagnostic
            .message
            .contains("substitutes the accepted permission")),
        "unexpected substituted-permission diagnostics: {substituted:#?}",
    );

    let retained_policy =
        permission_policy(&[effects::TerminalAuthorityClass::ProcessTermination], true);
    let accepted_policy_identity = retained_policy.identity();
    let retained_native = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &profile,
                optimization_selections: &optimizations,
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy: accepted_permission_policy(),
                terminal_authority_permission_policy: Some(retained_policy),
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
    .expect("exact retained permission plus an unrelated row should realize");
    assert_eq!(
        retained_native.terminal_authority_permission_policy_identity(),
        Some(accepted_policy_identity),
    );

    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(output),
            target_name: Some("linux_x86_64".to_owned()),
        })
        .with_package_inputs(accepted_inputs)
        .with_terminal_authority_permission_policy(permission_policy(
            &[effects::TerminalAuthorityClass::ProcessTermination],
            true,
        ))
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("exact accepted Console binding should close Linux native realization");
    assert_eq!(
        report
            .require_package_native_physical_evidence()
            .expect("accepted package Console carries exact physical evidence")
            .children()
            .len(),
        1,
    );
    assert_eq!(
        report
            .retained_native_artifact()
            .expect("direct native request retains its artifact")
            .terminal_authority_permission_policy_identity(),
        Some(accepted_policy_identity),
        "direct and retained native routes must consume the same exact policy identity",
    );
}

#[test]
fn accepted_package_filesystem_binding_requires_exact_owner_path_and_schema() {
    let tree = TempTree::new();
    let root = tree.package("filesystem-application");
    let filesystem = tree.package("filesystem-package");
    let root_package = identity(50);
    let filesystem_package = identity(51);
    TempTree::write(
        filesystem.join("filesystem_host.omg"),
        r#"pub boundary trait FilesystemHost {
    machine create(path: &[u8], mode: i32) -> i32
    reaches FilesystemHost;
    machine write(descriptor: i32, bytes: &[u8]) -> i64
    reaches FilesystemHost;
}
"#,
    );
    TempTree::write(
        root.join("main.omg"),
        r#"use host_services::filesystem_host;
data Main { filesystem: FilesystemHost; descriptor: i32; }
machine Main::main(&mut self) reaches FilesystemHost {
    self.descriptor = self.filesystem.create("probe.txt", 438);
}

pub machine append(filesystem: FilesystemHost, descriptor: i32, bytes: &[u8]) -> i64
reaches FilesystemHost
invokes filesystem;
{
    filesystem.write(descriptor, bytes)
}
"#,
    );
    let base_inputs = || {
        PackageCompilationInputs::new_package(
            root_package,
            vec![
                PackageSourceBinding::new(root_package, "filesystem-application", root.clone()),
                PackageSourceBinding::new(
                    filesystem_package,
                    "filesystem-package",
                    filesystem.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                root_package,
                "host_services",
                filesystem_package,
            )],
        )
        .expect("ordinary filesystem dependency graph")
    };

    let candidate = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(base_inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("ordinary filesystem requirement should check without semantic authority");
    assert!(
        candidate
            .resolved_semantic_binding(AcceptedSemanticBindingRole::FilesystemHostService)
            .is_none(),
        "a readable service name alone cannot grant filesystem authority",
    );
    let accepted = candidate
        .candidate_service_binding(
            AcceptedSemanticBindingRole::FilesystemHostService,
            filesystem_package,
            "FilesystemHost",
        )
        .expect("compiler should derive exact ordinary-package filesystem coordinates");

    for (label, stale) in [
        (
            "package owner",
            AcceptedSemanticBinding::new_service(
                AcceptedSemanticBindingRole::FilesystemHostService,
                root_package,
                accepted.declaration_path(),
                accepted.normalized_schema_digest(),
            )
            .unwrap(),
        ),
        (
            "nominal declaration",
            AcceptedSemanticBinding::new_service(
                AcceptedSemanticBindingRole::FilesystemHostService,
                filesystem_package,
                "OtherFilesystemHost",
                accepted.normalized_schema_digest(),
            )
            .unwrap(),
        ),
        (
            "service schema",
            AcceptedSemanticBinding::new_service(
                AcceptedSemanticBindingRole::FilesystemHostService,
                filesystem_package,
                accepted.declaration_path(),
                effects::provider_plan::ServiceSchemaDigest::from_digest([93; 32]),
            )
            .unwrap(),
        ),
    ] {
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(
                base_inputs()
                    .with_accepted_semantic_bindings(vec![stale])
                    .expect("stale binding still names a package in the closure"),
            ),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("stale filesystem authority must not survive exact settlement");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("resolved to 0 exact package-owned boundary declarations")),
            "unexpected {label} diagnostics: {diagnostics:#?}",
        );
    }

    let accepted_compilation = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![accepted.clone()])
                .expect("exact accepted binding names the ordinary dependency"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("exact ordinary-package filesystem authority should settle");
    assert_eq!(
        accepted_compilation
            .resolved_semantic_binding(AcceptedSemanticBindingRole::FilesystemHostService)
            .expect("exact filesystem binding was consumed")
            .accepted(),
        &accepted,
    );
    let unbound = checked_interpreter::interpret_entry(
        &accepted_compilation,
        "Main::main",
        &[],
        InterpretOptions::default(),
    );
    assert!(
        unbound.is_error(),
        "an ordinary package boundary cannot route from its readable operation names",
    );
    let declaration_symbol = accepted_compilation
        .resolved_semantic_binding(AcceptedSemanticBindingRole::FilesystemHostService)
        .expect("exact filesystem binding was consumed")
        .declaration_symbol();
    let interpreter_binding =
        checked_interpreter::FilesystemServiceBinding::from_compiler_resolved_declaration(
            &accepted_compilation,
            declaration_symbol,
        )
        .expect("compiler-resolved declaration is one exact checked boundary");
    let bound = checked_interpreter::interpret_entry(
        &accepted_compilation,
        "Main::main",
        &[],
        checked_interpreter::InterpretOptions::default()
            .with_filesystem_service_binding(interpreter_binding),
    );
    assert_eq!(
        bound.error, None,
        "the exact accepted declaration symbol should route the filesystem provider",
    );
    // A second compilation of the same source is a distinct checked program:
    // `CheckedCompilation` derefs into a shared `Arc<CheckedTrees>`, so cloning
    // the compilation would reuse the exact inner program address and defeat
    // the binding's program-identity check. Recompile to obtain a genuinely
    // different checked program whose raw symbol coordinate must not rejoin.
    let substituted_program = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![accepted.clone()])
                .expect("exact accepted binding names the ordinary dependency"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("the identical source compiles into a second checked program");
    let substituted = checked_interpreter::interpret_entry(
        &substituted_program,
        "Main::main",
        &[],
        checked_interpreter::InterpretOptions::default()
            .with_filesystem_service_binding(interpreter_binding),
    );
    assert!(
        substituted
            .error
            .as_deref()
            .is_some_and(|error| error.contains("different checked program")),
        "a raw symbol coordinate cannot be substituted into another checked program",
    );
}

#[test]
fn accepted_package_uefi_binding_selects_exact_ordinary_schema() {
    let tree = TempTree::new();
    let root = tree.package("uefi-application");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root");
    let standard_library = repository.join("source/library/std");
    let root_package = identity(53);
    let standard_library_package = identity(54);
    TempTree::write(
        root.join("main.omg"),
        r#"use omega::language::core::extent;
use ordinary_std::targets::uefi_x86_64;

data Boot { launch_count: u64; }
machine Boot::launch(
    &mut self,
    image: Extent in Granted,
    initial_storage: Extent in Granted
) {
    transition { _ -> retain(image as Extent, initial_storage as Extent) }
    state retain(&mut self, image: Extent, initial_storage: Extent) {
        transition { _ -> retain(image, initial_storage) }
    }
}
"#,
    );
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("uefi-application");
    builder.subsystem = Subsystem::EfiApplication;
    builder.freestanding = true;
    builder.roots.bind(uefi_x86_64::ProgramEntry, Boot::launch);
}
"#,
    );
    let base_inputs = || {
        PackageCompilationInputs::new(
            root_package,
            BuildDeclarationKind::Application,
            vec![
                PackageSourceBinding::new(root_package, "uefi-application", root.clone()),
                PackageSourceBinding::new(
                    standard_library_package,
                    "ordinary-std",
                    standard_library.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                root_package,
                "ordinary_std",
                standard_library_package,
            )],
        )
        .expect("ordinary application and std dependency graph")
    };

    let candidate = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(base_inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("semantic-only compilation can derive the exact UEFI schema candidate");
    let binding = candidate
        .candidate_service_binding(
            AcceptedSemanticBindingRole::UefiX64ProgramEntry,
            standard_library_package,
            "UefiApplication",
        )
        .expect("compiler should derive exact package-owned UEFI coordinates");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(base_inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("uefi_x86_64"))
    })
    .expect_err("ordinary UEFI source requires exact consumer acceptance");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("accepted package-owned UEFI binding")),
        "unexpected missing-binding diagnostics: {diagnostics:#?}",
    );

    let stale = AcceptedSemanticBinding::new_service(
        AcceptedSemanticBindingRole::UefiX64ProgramEntry,
        standard_library_package,
        binding.declaration_path(),
        effects::provider_plan::ServiceSchemaDigest::from_digest([94; 32]),
    )
    .expect("stale row remains structurally valid input");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![stale])
                .expect("stale binding still names a package in the closure"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("uefi_x86_64"))
    })
    .expect_err("stale UEFI schema identity must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not match the exact accepted")),
        "unexpected stale-binding diagnostics: {diagnostics:#?}",
    );

    let accepted = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![binding.clone()])
                .expect("exact UEFI binding names the ordinary dependency"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("uefi_x86_64"))
    })
    .expect("exact ordinary-package UEFI schema should settle");
    assert_eq!(
        accepted
            .resolved_semantic_binding(AcceptedSemanticBindingRole::UefiX64ProgramEntry)
            .expect("exact UEFI binding was consumed")
            .accepted(),
        &binding,
    );
    assert!(
        accepted
            .selected_program_entry()
            .and_then(|entry| entry.calling_plans())
            .and_then(|plans| plans.storage_entry.physical_contract())
            .is_some(),
        "accepted UEFI binding must retain the target-fixed physical contract",
    );
    let uefi_source = accepted
        .typed
        .symbols
        .source_files()
        .find(|source| source.path.ends_with("targets/uefi_x86_64/entry.omg"))
        .expect("ordinary UEFI package source remains loaded");
    assert_eq!(uefi_source.origin, source::SourceOrigin::User);
    assert_eq!(uefi_source.package_identity, Some(standard_library_package),);
}

#[test]
fn standalone_macos_entry_loads_exact_authored_contract_without_an_import() {
    let tree = TempTree::new();
    let root = tree.package("standalone-macos-entry");
    TempTree::write(
        root.join("main.omg"),
        "data Main {}\nmachine Main::main() {}\n",
    );
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) { builder.application(\"standalone-macos-entry\"); builder.roots.bind(macos_arm64::ProgramEntry, Main::main); }\n",
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("macos_arm64"),
    ))
    .expect("standalone target selection loads the exact bundled contract");
    let plans = checked
        .selected_program_entry()
        .and_then(|entry| entry.calling_plans())
        .expect("standalone entry retains both calling applications");
    plans
        .semantic_calling_application
        .replayed_validated_application()
        .expect("standalone semantic application replays");
    plans
        .physical_calling_application
        .replayed_validated_application()
        .expect("standalone physical application replays");
    let physical = plans
        .storage_entry
        .physical_contract()
        .expect("physical contract");
    assert!(physical.matches_exact_macos_arm64_physical_contract());
    assert!(physical.guaranteed_entry_stack().is_none());
    let contract_source = checked
        .typed
        .symbols
        .source_files()
        .find(|source| source.path.ends_with("targets/macos_arm64/entry.omg"))
        .expect("bundled entry contract source is retained");
    assert_eq!(contract_source.origin, source::SourceOrigin::Toolchain);
    assert_eq!(contract_source.package_identity, None);
}

#[test]
fn accepted_package_macos_binding_selects_exact_ordinary_schema() {
    let tree = TempTree::new();
    let root = tree.package("macos-application");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root");
    let standard_library = repository.join("source/library/std");
    let root_package = identity(55);
    let standard_library_package = identity(56);
    TempTree::write(
        root.join("main.omg"),
        // Import only the contract submodule: the `targets::macos_arm64`
        // definition also loads `filesystem_impl`, which declares target
        // `FilesystemHost` implementations outside this entry-only custody.
        r#"use ordinary_std::targets::macos_arm64::entry;

data Boot { launch_count: u64; }
machine Boot::launch(&mut self) {
    transition { _ -> retain() }
    state retain(&mut self) {
        transition { _ -> retain() }
    }
}
"#,
    );
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("macos-application");
    builder.roots.bind(macos_arm64::ProgramEntry, Boot::launch);
}
"#,
    );
    let base_inputs = || {
        PackageCompilationInputs::new(
            root_package,
            BuildDeclarationKind::Application,
            vec![
                PackageSourceBinding::new(root_package, "macos-application", root.clone()),
                PackageSourceBinding::new(
                    standard_library_package,
                    "ordinary-std",
                    standard_library.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                root_package,
                "ordinary_std",
                standard_library_package,
            )],
        )
        .expect("ordinary application and std dependency graph")
    };

    let candidate = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(base_inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("semantic-only compilation can derive the exact macOS schema candidate");
    let binding = candidate
        .candidate_service_binding(
            AcceptedSemanticBindingRole::MacosArm64ProgramEntry,
            standard_library_package,
            "MacosApplication",
        )
        .expect("compiler should derive exact package-owned macOS coordinates");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(base_inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("macos_arm64"))
    })
    .expect_err("ordinary macOS source requires exact consumer acceptance");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("accepted package-owned macOS ARM64 binding")),
        "unexpected missing-binding diagnostics: {diagnostics:#?}",
    );

    let stale = AcceptedSemanticBinding::new_service(
        AcceptedSemanticBindingRole::MacosArm64ProgramEntry,
        standard_library_package,
        binding.declaration_path(),
        effects::provider_plan::ServiceSchemaDigest::from_digest([95; 32]),
    )
    .expect("stale row remains structurally valid input");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![stale.clone()])
                .expect("stale binding still names a package in the closure"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("macos_arm64"))
    })
    .expect_err("stale macOS schema identity must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not match the exact accepted")),
        "unexpected stale-binding diagnostics: {diagnostics:#?}",
    );

    let accepted = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![binding.clone()])
                .expect("exact macOS binding names the ordinary dependency"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("macos_arm64"))
    })
    .expect("exact ordinary-package macOS schema should settle");
    assert_eq!(
        accepted
            .resolved_semantic_binding(AcceptedSemanticBindingRole::MacosArm64ProgramEntry)
            .expect("exact macOS binding was consumed")
            .accepted(),
        &binding,
    );
    let physical_contract = accepted
        .selected_program_entry()
        .and_then(|entry| entry.calling_plans())
        .and_then(|plans| plans.storage_entry.physical_contract())
        .expect("accepted macOS binding must retain the target-fixed physical contract");
    assert!(
        physical_contract.matches_exact_macos_arm64_physical_contract(),
        "package custody must replay the exact dyld arrival contract",
    );
    let macos_source = accepted
        .typed
        .symbols
        .source_files()
        .find(|source| source.path.ends_with("targets/macos_arm64/entry.omg"))
        .expect("ordinary macOS package source remains loaded");
    assert_eq!(macos_source.origin, source::SourceOrigin::User);
    assert_eq!(
        macos_source.package_identity,
        Some(standard_library_package),
    );

    // Selecting the entry without an authored import must retain the same
    // ordinary supplier custody, not relabel that file as toolchain source.
    let source =
        std::fs::read_to_string(root.join("main.omg")).expect("read explicit-import fixture");
    let source = source
        .strip_prefix("use ordinary_std::targets::macos_arm64::entry;\n")
        .expect("fixture begins with the exact contract import being removed");
    TempTree::write(root.join("main.omg"), source);
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(base_inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("macos_arm64"))
    })
    .expect_err("auto-seeded ordinary contract still requires exact consumer acceptance");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("accepted package-owned macOS ARM64 binding")),
        "unexpected auto-seeded missing-binding diagnostics: {diagnostics:#?}",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![stale])
                .expect("stale binding still names the exact supplier"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("macos_arm64"))
    })
    .expect_err("auto-seeding cannot repair a stale accepted schema");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not match the exact accepted")),
        "unexpected auto-seeded stale-binding diagnostics: {diagnostics:#?}",
    );
    let seeded = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            base_inputs()
                .with_accepted_semantic_bindings(vec![binding.clone()])
                .expect("exact accepted ordinary supplier"),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("macos_arm64"))
    })
    .expect("auto-seeded ordinary contract settles with exact consumer acceptance");
    assert_eq!(
        seeded
            .resolved_semantic_binding(AcceptedSemanticBindingRole::MacosArm64ProgramEntry)
            .expect("auto-seeded entry consumes the accepted binding")
            .accepted(),
        &binding,
    );
    let plans = seeded
        .selected_program_entry()
        .and_then(|entry| entry.calling_plans())
        .expect("auto-seeded entry retains both calling applications");
    plans
        .semantic_calling_application
        .replayed_validated_application()
        .expect("semantic application independently replays");
    plans
        .physical_calling_application
        .replayed_validated_application()
        .expect("physical application independently replays");
    let seeded_contract = plans
        .storage_entry
        .physical_contract()
        .expect("auto-seeded entry retains exact physical contract");
    assert!(seeded_contract.matches_exact_macos_arm64_physical_contract());
    assert_eq!(seeded_contract, physical_contract);
    let seeded_source = seeded
        .typed
        .symbols
        .source_files()
        .find(|source| source.path.ends_with("targets/macos_arm64/entry.omg"))
        .expect("auto-seeded ordinary contract source is retained");
    assert_eq!(seeded_source.origin, source::SourceOrigin::User);
    assert_eq!(
        seeded_source.package_identity,
        Some(standard_library_package)
    );
}

#[test]
fn free_process_exit_helper_lowers_without_a_synthetic_attachment() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root");
    let process_exit_root = repository.join("tests/fixtures/packages/process-exit");
    let host_services_root = repository.join("tests/fixtures/packages/host-services");
    let process_exit_package = identity(45);
    let host_services_package = identity(52);
    let inputs = PackageCompilationInputs::new_package(
        process_exit_package,
        vec![
            PackageSourceBinding::new(
                process_exit_package,
                "process-exit",
                process_exit_root.clone(),
            ),
            PackageSourceBinding::new(host_services_package, "host-services", host_services_root),
        ],
        vec![PackageDependencyBinding::new(
            process_exit_package,
            "host_services",
            host_services_package,
        )],
    )
    .expect("ordinary process-exit dependency graph");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&process_exit_root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("ordinary process-exit candidate should check");

    selected_dispatch::validate_fused_service_terminal_custody(
        &checked,
        checked.selected_provider_provenance(),
    )
    .expect("free process-exit helper should rejoin exact Fused Service custody");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "terminate")
        .expect("free process-exit helper should lower to canonical Terminal Psi");
    let terminal = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("free process-exit helper should remain the Terminal entry");
    assert_eq!(terminal.attachment, None);
    assert_eq!(terminal.structural_parameters.len(), 1);
    assert_eq!(terminal.parameters.len(), 1);
    let calls = terminal
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::BoundaryCall { arguments, .. } => Some(arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("free process-exit helper should retain one Terminal boundary call")
    };
    assert_eq!(call.as_slice(), [terminal.parameters[0].id]);
}

#[test]
fn package_native_physical_evidence_gate_borrows_exact_supported_evidence() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root");
    let output = TempTree::new();
    let exit_root = output.package("physical-exit-source");
    TempTree::write(
        exit_root.join("main.omg"),
        r#"use host_services::console;
data Main { console: Console; }
machine Main::main(&mut self) reaches Console {
    self.console.exit_process(70);
}
"#,
    );
    TempTree::write(
        exit_root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("physical-exit");
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    );
    let host_services_root = repository.join("tests/fixtures/packages/host-services");
    let empty_root = repository
        .join("tests/omega/pass")
        .join(fixtures::NO_SELECTION_EMPTY_ENTRY);
    let port_root = repository
        .join("tests/omega/pass")
        .join(fixtures::ASM_PORT_OUT_FINAL_VALIDATION);

    let compile_package = |root: &Path, marker: u8, label: &str| {
        let package = identity(marker);
        let inputs = PackageCompilationInputs::new_package(
            package,
            vec![PackageSourceBinding::new(
                package,
                label,
                root.to_path_buf(),
            )],
            Vec::new(),
        )
        .expect("single-package native graph");
        compile(
            CompileRequest::new(CompileOptions {
                root_path: root.join("main.omg"),
                build_dir: Some(output.0.join(label)),
                target_name: Some("linux_x86_64".to_owned()),
            })
            .with_package_inputs(inputs)
            .with_requested_product(RequestedCompileProduct::NativeArtifact),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
    };

    let exit_package = identity(45);
    let host_services_package = identity(52);
    let exit_inputs = || {
        PackageCompilationInputs::new_package(
            exit_package,
            vec![
                PackageSourceBinding::new(exit_package, "physical-exit", exit_root.clone()),
                PackageSourceBinding::new(
                    host_services_package,
                    "host-services",
                    host_services_root.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                exit_package,
                "host_services",
                host_services_package,
            )],
        )
        .expect("ordinary process-exit dependency graph")
    };
    let exit_candidate = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(exit_inputs()),
        ..CheckedCompileRequest::new(&exit_root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("ordinary process-exit candidate should check");
    let (console_plan, console_provenance) = exit_candidate
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(exit_candidate.selected_provider_provenance())
        .find(|(plan, _)| plan.schema.trait_name == "Console")
        .expect("process-exit candidate retains selected Console plan");
    let console_path = exit_candidate
        .typed
        .symbols
        .display_path(console_provenance.provider.schema.symbol(), "::");
    let console_schema = console_plan.schema.identity_digest();
    let console_exit_requirement = console_plan
        .rows
        .iter()
        .find(|row| row.method == "exit_process")
        .expect("selected Console plan retains exit_process")
        .requirement_identity
        .clone();
    let console_permission = effects::ServiceTerminalAuthorityPermission::new(
        console_schema,
        console_exit_requirement.clone(),
        effects::TerminalAuthorityDisposition::from_classes([
            effects::TerminalAuthorityClass::ProcessTermination,
        ]),
    );
    let console_binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        host_services_package,
        console_path,
        console_schema,
        console_plan.identity_digest(),
    )
    .expect("exact process-exit Console candidate")
    .with_terminal_authority_permissions(vec![console_permission.clone()])
    .expect("accepted Console binding carries its exact terminal permission");
    let console_permission_policy =
        native_realization::terminal_authority_permission_policy_with_rows(vec![
            native_realization::TerminalAuthorityPermissionPolicyRow::new(
                console_schema,
                console_exit_requirement,
                console_permission.permitted().clone(),
            ),
        ])
        .expect("receiving policy accepts the exact Console terminal permission");
    let exit = compile(
        CompileRequest::new(CompileOptions {
            root_path: exit_root.join("main.omg"),
            build_dir: Some(output.0.join("physical-exit")),
            target_name: Some("linux_x86_64".to_owned()),
        })
        .with_package_inputs(
            exit_inputs()
                .with_accepted_semantic_bindings(vec![console_binding])
                .expect("accept exact ordinary Console binding"),
        )
        .with_terminal_authority_permission_policy(console_permission_policy)
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("package-aware ordinary process-exit fixture should compile");
    let evidence = exit
        .require_package_native_physical_evidence()
        .expect("the supported Linux exit lane carries exact physical evidence");
    assert_eq!(evidence.children().len(), 1);
    assert!(std::ptr::eq(
        evidence,
        exit.retained_native_artifact()
            .expect("retained native artifact")
            .physical_evidence()
            .expect("artifact-owned physical evidence"),
    ));

    let empty = compile_package(&empty_root, 46, "physical-empty")
        .expect("package-aware empty Linux native fixture should compile");
    assert_eq!(
        exit.production_manifest()
            .expect("exit production manifest")
            .require_native_physical_evidence(
                empty
                    .retained_native_artifact()
                    .expect("empty retained native artifact"),
            )
            .expect_err("a manifest cannot consume a substituted native artifact"),
        compiler::FinalRealizationEvidenceError::NativeArtifactMismatch,
    );

    let port_diagnostics = compile_package(&port_root, 47, "physical-port").expect_err(
        "root PortIo authority without provider custody must reject before D32 evidence derivation",
    );
    assert!(
        port_diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("no selected provider requirement custody")),
        "unexpected PortIo diagnostics: {port_diagnostics:#?}",
    );
}
