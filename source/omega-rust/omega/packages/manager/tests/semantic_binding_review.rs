use omega_effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use omega_package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind,
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCompilerIntrinsicExecution,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
};
use omega_package_manager::admission::{
    accept_ordinary_closure_evidence, accepted_terminal_authority_permission_policy,
    realize_accepted_reviewed_package_candidate_with_source_evaluated_imports_and_policy,
    realize_accepted_terminal_artifact_with_source_evaluated_imports_and_policy,
};
use omega_package_manager::declarations::{PackageKey, PackageName};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure_with_storage,
};
use omega_package_manager::resolution::package_compilation_inputs;
use omega_package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    ConsumerScopedSemanticBindingReviewInput, FreshPackageRootPolicyError,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDisposition,
    bind_fresh_package_root_policy, compare_review_only_initial_capabilities,
    compile_resolved_package_candidate_for_production_with_semantic_bindings,
    compile_resolved_package_candidate_reviews, compile_resolved_package_reviews,
    compile_resolved_package_reviews_with_semantic_bindings,
    resolve_review_only_root_policy_decisions,
};
use omega_package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceLineage, SourceResolverStorage,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY_TREE: AtomicU64 = AtomicU64::new(0);

struct TemporaryTree(PathBuf);

impl TemporaryTree {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-semantic-binding-review-{}-{}",
            std::process::id(),
            NEXT_TEMPORARY_TREE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create semantic-binding test tree");
        Self(root)
    }

    fn package(&self, name: &str) -> PathBuf {
        let package = self.0.join(name);
        fs::create_dir(&package).expect("create semantic-binding test package");
        package
    }
}

impl Drop for TemporaryTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_file(path: impl AsRef<Path>, source: &str) {
    fs::write(path, source).expect("write semantic-binding fixture source");
}

#[test]
fn consumer_scoped_console_binding_survives_review_and_fresh_admission() {
    let temporary = TemporaryTree::new();
    let application = temporary.package("application");
    let console = temporary.package("console");

    write_file(
        console.join("build.omg"),
        r#"target linux_x86_64 { }
target windows_x86_64 { }

machine build(builder: &mut Build) {
    builder.package("ordinary-console");
}
"#,
    );
    write_file(
        console.join("main.omg"),
        r#"pub boundary trait Console {
    machine exit_process(return_code: i32)
    reaches Console;
}

pub data ConsoleNativeProvider { }
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
windows_x86_64 machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process
    via Binding::CompilerIntrinsic;
"#,
    );
    write_file(
        application.join("build.omg"),
        r#"target linux_x86_64 { }
target windows_x86_64 { }

machine build(builder: &mut Build) {
    builder.application("console-consumer");
    builder.depend_as("ordinary_console", Source::Path {
        location: "../console"
    });
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
    );
    write_file(
        application.join("main.omg"),
        r#"use ordinary_console::main;
use omega::language::core::service;

data Main { console: Service<Console> in Bound; }
machine Main::main(&mut self) {
    self.console.exit_process(70);
}

pub machine terminate(console: Service<Console> in Bound, return_code: i32)
reaches Console
invokes console;
{
    console.exit_process(return_code);
}
"#,
    );

    let storage = SourceResolverStorage::for_hardened_base(temporary.0.join("resolved"))
        .expect("create semantic-binding resolver storage");
    let closure = resolve_external_local_project_closure_with_storage(
        &application,
        ExternalSourceContext::derive(b"consumer-scoped-console-binding"),
        omega_target::TargetProfile::LinuxX64,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve ordinary Console closure");
    let preliminary = compile_resolved_package_reviews(
        &closure,
        "linux_x86_64",
        &temporary.0.join("preliminary-build"),
    )
    .expect("compile Console candidate without consumer authority");
    let root_key = closure.graph().root().clone();
    let root_candidate = preliminary
        .review(&root_key)
        .expect("preliminary root review");
    assert!(root_candidate.semantic_bindings().is_empty());
    assert!(
        root_candidate
            .projection()
            .dangerous_authorities()
            .is_empty()
    );

    let provider = root_candidate
        .projection()
        .selected_providers()
        .iter()
        .find(|provider| provider.service_schema() == "Console")
        .expect("candidate retains the selected ordinary Console plan");
    let PackageReviewNominalOwner::Package(console_package) = provider.schema_declaration().owner()
    else {
        panic!("ordinary Console declaration must retain its exact package owner")
    };
    let exit_requirement = provider
        .schema()
        .methods
        .iter()
        .find(|method| method.name == "exit_process")
        .expect("selected Console schema retains exact exit requirement")
        .requirement_identity
        .clone();
    let binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        console_package,
        provider.schema_declaration().path(),
        provider.schema().identity_digest(),
        provider.selected_plan_digest(),
    )
    .expect("derive exact candidate Console binding")
    .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
        provider.schema().identity_digest(),
        exit_requirement.clone(),
        TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::ProcessTermination]),
    )])
    .expect("attach exact Console exit terminal permission");
    let binding_input =
        ConsumerScopedSemanticBindingReviewInput::new(root_key.clone(), binding.clone());

    let absent_consumer = PackageKey::new(
        PackageName::parse("absent-consumer").expect("absent package name"),
        SourceLineage::git("https://github.com/CathedralOS/absent-consumer.git")
            .expect("absent package lineage"),
    );
    assert!(matches!(
        compile_resolved_package_reviews_with_semantic_bindings(
            &closure,
            "linux_x86_64",
            &temporary.0.join("absent-consumer-build"),
            &[ConsumerScopedSemanticBindingReviewInput::new(
                absent_consumer.clone(),
                binding.clone(),
            )],
        ),
        Err(CompileResolvedPackageReviewsError::SemanticBindingConsumerAbsent {
            consumer,
            role: AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        }) if consumer == absent_consumer
    ));
    assert!(matches!(
        compile_resolved_package_reviews_with_semantic_bindings(
            &closure,
            "linux_x86_64",
            &temporary.0.join("duplicate-binding-build"),
            &[binding_input.clone(), binding_input.clone()],
        ),
        Err(
            CompileResolvedPackageReviewsError::DuplicateConsumerSemanticBindingRole {
                consumer,
                role: AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            }
        ) if consumer == root_key
    ));

    let production_candidate =
        compile_resolved_package_candidate_for_production_with_semantic_bindings(
            &closure,
            "linux_x86_64",
            &temporary.0.join("accepted-build"),
            std::slice::from_ref(&binding_input),
        )
        .expect("compile exact consumer-bound Console review with explicit terminal permission");
    assert_eq!(production_candidate.root(), &root_key);
    assert_eq!(
        production_candidate.root_role(),
        BuildDeclarationKind::Application
    );
    assert_eq!(
        production_candidate.target_profile(),
        omega_target::TargetProfile::LinuxX64
    );
    let reviews = production_candidate.reviews();
    let root_review = reviews.review(&root_key).expect("bound root review");
    assert_eq!(root_review.semantic_bindings(), &[binding.clone()]);
    let [authority] = root_review.projection().dangerous_authorities() else {
        panic!("resolved Console binding must expose one process authority")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Process
    );
    assert_eq!(
        authority.service().owner(),
        PackageReviewNominalOwner::Package(console_package)
    );

    let conflict_limits = ReviewOnlyCapabilityConflictLimits::default();
    let conflicts = compare_review_only_initial_capabilities(&reviews, &closure, conflict_limits)
        .expect("derive complete fresh conflicts");
    let permission_conflicts = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .filter(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::TerminalAuthorityPermission
        })
        .collect::<Vec<_>>();
    let [permission_conflict] = permission_conflicts.as_slice() else {
        panic!("exact Console exit permission must produce one fresh blocking conflict")
    };
    assert!(permission_conflict.is_blocking());
    assert!(
        conflicts
            .render_bounded(1024 * 1024)
            .expect("render terminal permission conflict")
            .contains("kind terminal_authority_permission\n")
    );
    assert!(matches!(
        bind_fresh_package_root_policy(
            &closure,
            &reviews,
            CanonicalPackageReconstructionQuestionLimits::default(),
            conflict_limits,
            None,
        ),
        Err(FreshPackageRootPolicyError::MissingRootPolicy)
    ));
    let decisions = conflicts
        .packages()
        .iter()
        .flat_map(|package| {
            package
                .conflicts()
                .iter()
                .filter(|conflict| conflict.is_blocking())
                .map(|conflict| {
                    package
                        .root_policy_decision(
                            conflict,
                            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
                        )
                        .expect("bind exact fresh conflict decision")
                })
        })
        .collect::<Vec<_>>();
    let root_policy = resolve_review_only_root_policy_decisions(&conflicts, &decisions)
        .expect("accept every exact blocking row");
    let evidence = accept_ordinary_closure_evidence(
        &closure,
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
        conflict_limits,
        Some(&root_policy),
    )
    .expect("fresh policy admits consumer-bound Console evidence");
    assert_eq!(evidence.schema().version(), 5);
    let root_evidence = evidence
        .packages()
        .iter()
        .find(|package| package.package() == &root_key)
        .expect("accepted evidence retains selected root");
    assert_eq!(
        root_evidence.selected_build_machine_identity(),
        root_review.selected_build_machine_identity(),
    );
    assert_eq!(
        root_evidence.build_evaluation_usage(),
        root_review.build_evaluation_usage(),
    );
    assert_eq!(
        root_evidence.build_observation(),
        root_review.build_observation_summary(),
    );
    assert_eq!(root_evidence.semantic_bindings(), &[binding]);
    let propagated_permissions = evidence
        .acceptance()
        .obligations()
        .root_open_terminal_authority_permissions()
        .collect::<Vec<_>>();
    let [(permission_owner, propagated_permission)] = propagated_permissions.as_slice() else {
        panic!("root reconstruction propagates one owner-retaining terminal permission")
    };
    assert_eq!(*permission_owner, &root_key);
    assert_eq!(
        propagated_permission.permission().requirement_identity(),
        exit_requirement
    );
    let [permission] = root_evidence
        .results()
        .open_terminal_authority_permissions()
    else {
        panic!("accepted evidence retains one open exact terminal permission")
    };
    assert_eq!(
        permission.permission().requirement_identity(),
        exit_requirement
    );
    assert_eq!(
        permission.permission().permitted().classes(),
        &[TerminalAuthorityClass::ProcessTermination]
    );
    let accepted_permission_policy = accepted_terminal_authority_permission_policy(&evidence)
        .expect("accepted root-policy evidence projects one exact receiving policy");
    let [accepted_permission] = accepted_permission_policy.rows() else {
        panic!("accepted root-policy projection retains one exact permission row")
    };
    assert_eq!(
        accepted_permission.service_schema(),
        provider.schema().identity_digest()
    );
    assert_eq!(accepted_permission.requirement_identity(), exit_requirement);
    assert_eq!(
        accepted_permission.permitted().classes(),
        &[TerminalAuthorityClass::ProcessTermination]
    );

    let root_path = closure
        .custody(&root_key)
        .expect("resolved closure retains application root custody")
        .snapshot_root()
        .join("main.omg");
    let production_inputs = |semantic_bindings: Vec<AcceptedSemanticBinding>| {
        let dependency_generated_sources = evidence
            .packages()
            .iter()
            .filter(|package| package.package() != &root_key)
            .map(|package| package.generated_sources().clone())
            .collect();
        package_compilation_inputs(&closure)
            .expect("reconstruct exact accepted application compilation inputs")
            .with_complete_dependency_generated_sources(dependency_generated_sources)
            .expect("attach every accepted dependency generated-source bundle")
            .with_accepted_semantic_bindings(semantic_bindings)
            .expect("attach exact root semantic bindings")
    };
    let compile_terminal_report = |label: &str, semantic_bindings: Vec<AcceptedSemanticBinding>| {
        omega_compiler::compile(
                omega_compiler::CompileRequest::new(omega_compiler::CompileOptions {
                    root_path: root_path.clone(),
                    build_dir: Some(temporary.0.join(label)),
                    target_name: Some("linux_x86_64".to_owned()),
                })
                .with_package_inputs(production_inputs(semantic_bindings))
                .with_requested_product(omega_compiler::RequestedCompileProduct::TerminalArtifact),
            )
            .unwrap_or_else(|diagnostics| {
                panic!("accepted package application must produce one retained Terminal report: {diagnostics:#?}")
            })
    };
    let exact_checked = omega_compiler::compile_to_checked_with_packages_in_build_dir(
        &root_path,
        &temporary.0.join("exact-checked-build"),
        Some("linux_x86_64"),
        production_inputs(root_evidence.semantic_bindings().to_vec()),
    )
    .expect("accepted package application checks for subject mutation coverage");

    let receiving_policy_identity = accepted_permission_policy.identity();
    let native =
        realize_accepted_reviewed_package_candidate_with_source_evaluated_imports_and_policy(
            production_candidate,
            &evidence,
            &psi_proof_admission::AdmissionProfile::default(),
            &omega_optimization_core::OptimizationSelections::default(),
            omega_terminal_psi_to_native_artifact::current_terminal_authority_policy(),
            accepted_permission_policy.clone(),
            &[],
        )
        .expect("manager joins accepted evidence to the exact retained Terminal report");
    native
        .validate()
        .expect("manager-realized native artifact remains internally valid");
    assert_eq!(
        native.terminal_authority_permission_policy_identity(),
        receiving_policy_identity,
    );

    let observation_probe = temporary.package("observation-probe");
    write_file(
        observation_probe.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("observation-probe");
}
"#,
    );
    write_file(
        observation_probe.join("main.omg"),
        "pub data ObservationProbe { value: u8; }\n",
    );
    let observation_probe_identity =
        PackageKeyIdentity::from_digest([0x7a; 32]).expect("nonzero observation-probe identity");
    let observation_probe_checked = omega_compiler::compile_to_checked_with_packages(
        &observation_probe.join("main.omg"),
        None,
        PackageCompilationInputs::new_package(
            observation_probe_identity,
            vec![PackageSourceBinding::new(
                observation_probe_identity,
                "observation-probe",
                observation_probe.clone(),
            )],
            Vec::new(),
        )
        .expect("single-package observation probe"),
    )
    .expect("compile distinct build observation probe");
    let substituted_observation_subject =
        omega_compiler::ProductionCompilationSubject::from_checked(
            exact_checked
                .package_compilation_subject()
                .expect("exact checked package subject")
                .clone(),
            exact_checked
                .selected_build_machine_identity()
                .expect("exact checked build-machine identity")
                .to_owned(),
            exact_checked
                .build_evaluation_usage()
                .expect("exact checked invocation usage"),
            observation_probe_checked
                .build_observation_summary()
                .expect("probe retains a distinct observation"),
            omega_target::TargetProfile::LinuxX64,
            omega_target::TargetProfile::LinuxX64.native_target(),
        )
        .expect("construct structurally valid observation-substituted subject");
    let exact_report = compile_terminal_report(
        "observation-substitution-build",
        root_evidence.semantic_bindings().to_vec(),
    );
    let observation_substituted_report =
        omega_compiler::CompileReport::from_retained_terminal_artifact(
            root_path.clone(),
            exact_checked.source_file_count(),
            exact_report
                .into_retained_terminal_artifact()
                .expect("exact report retains Terminal product"),
            Some(substituted_observation_subject),
        )
        .expect("observation substitution remains structurally valid report custody");
    let observation_diagnostics =
        realize_accepted_terminal_artifact_with_source_evaluated_imports_and_policy(
            observation_substituted_report,
            &evidence,
            &psi_proof_admission::AdmissionProfile::default(),
            &omega_optimization_core::OptimizationSelections::default(),
            omega_terminal_psi_to_native_artifact::current_terminal_authority_policy(),
            accepted_permission_policy.clone(),
            &[],
        )
        .expect_err("accepted evidence must reject a substituted build observation");
    assert!(observation_diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("build observation differs from accepted package evidence")
    }));

    let substituted_root = temporary.package("source-substituted-application");
    fs::copy(
        root_path
            .parent()
            .expect("resolved root source has a parent")
            .join("build.omg"),
        substituted_root.join("build.omg"),
    )
    .expect("copy exact build source for source-substitution fixture");
    let mut substituted_main = fs::read_to_string(&root_path)
        .expect("read exact accepted application source for substitution fixture");
    substituted_main.push_str("\n// source-consumption substitution\n");
    write_file(substituted_root.join("main.omg"), &substituted_main);
    let console_root = closure
        .custodies()
        .iter()
        .find(|custody| custody.key().identity() == console_package)
        .expect("resolved closure retains ordinary Console custody")
        .snapshot_root()
        .to_path_buf();
    let substituted_inputs = PackageCompilationInputs::new(
        root_key.identity(),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                root_key.identity(),
                "console-consumer",
                substituted_root.clone(),
            ),
            PackageSourceBinding::new(console_package, "ordinary-console", console_root),
        ],
        vec![PackageDependencyBinding::new(
            root_key.identity(),
            "ordinary_console",
            console_package,
        )],
    )
    .expect("construct same-identity source-substituted package graph")
    .with_complete_dependency_generated_sources(
        evidence
            .packages()
            .iter()
            .filter(|package| package.package() != &root_key)
            .map(|package| package.generated_sources().clone())
            .collect(),
    )
    .expect("attach accepted dependency bundle to substituted source graph")
    .with_accepted_semantic_bindings(root_evidence.semantic_bindings().to_vec())
    .expect("attach exact accepted semantic binding to substituted source graph");
    let substituted_checked = omega_compiler::compile_to_checked_with_packages_in_build_dir(
        &substituted_root.join("main.omg"),
        &temporary.0.join("source-substitution-checked-build"),
        Some("linux_x86_64"),
        substituted_inputs,
    )
    .expect("compile same-identity source-substituted package subject");
    assert_ne!(
        substituted_checked.source_consumption_commitment(),
        exact_checked.source_consumption_commitment(),
    );
    let substituted_source_subject = omega_compiler::ProductionCompilationSubject::from_checked(
        substituted_checked
            .package_compilation_subject()
            .expect("substituted checked package subject")
            .clone(),
        exact_checked
            .selected_build_machine_identity()
            .expect("exact checked build-machine identity")
            .to_owned(),
        exact_checked
            .build_evaluation_usage()
            .expect("exact checked invocation usage"),
        exact_checked
            .build_observation_summary()
            .expect("exact checked build observation"),
        omega_target::TargetProfile::LinuxX64,
        omega_target::TargetProfile::LinuxX64.native_target(),
    )
    .expect("construct structurally valid source-substituted subject");
    let exact_report = compile_terminal_report(
        "source-substitution-build",
        root_evidence.semantic_bindings().to_vec(),
    );
    let source_substituted_report = omega_compiler::CompileReport::from_retained_terminal_artifact(
        root_path.clone(),
        exact_checked.source_file_count(),
        exact_report
            .into_retained_terminal_artifact()
            .expect("exact report retains Terminal product"),
        Some(substituted_source_subject),
    )
    .expect("source substitution remains structurally valid report custody");
    let source_diagnostics =
        realize_accepted_terminal_artifact_with_source_evaluated_imports_and_policy(
            source_substituted_report,
            &evidence,
            &psi_proof_admission::AdmissionProfile::default(),
            &omega_optimization_core::OptimizationSelections::default(),
            omega_terminal_psi_to_native_artifact::current_terminal_authority_policy(),
            accepted_permission_policy.clone(),
            &[],
        )
        .expect_err("accepted evidence must reject substituted source consumption");
    assert!(source_diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("source consumption differs from accepted package evidence")
    }));

    let widened_binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        console_package,
        provider.schema_declaration().path(),
        provider.schema().identity_digest(),
        provider.selected_plan_digest(),
    )
    .expect("reconstruct exact candidate Console binding for proposal mutation")
    .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
        provider.schema().identity_digest(),
        exit_requirement.clone(),
        TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessOutput,
            TerminalAuthorityClass::ProcessTermination,
        ]),
    )])
    .expect("attach widened retained proposal permission");
    let widened_report =
        compile_terminal_report("proposal-substitution-build", vec![widened_binding]);
    let widened_receiving_policy =
        omega_terminal_psi_to_native_artifact::terminal_authority_permission_policy_with_rows(
            vec![
                omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicyRow::new(
                    provider.schema().identity_digest(),
                    exit_requirement.clone(),
                    TerminalAuthorityDisposition::from_classes([
                        TerminalAuthorityClass::ProcessOutput,
                        TerminalAuthorityClass::ProcessTermination,
                    ]),
                ),
            ],
        )
        .expect("construct coordinated widened receiving permission policy");
    let proposal_diagnostics =
        realize_accepted_terminal_artifact_with_source_evaluated_imports_and_policy(
            widened_report,
            &evidence,
            &psi_proof_admission::AdmissionProfile::default(),
            &omega_optimization_core::OptimizationSelections::default(),
            omega_terminal_psi_to_native_artifact::current_terminal_authority_policy(),
            widened_receiving_policy,
            &[],
        )
        .expect_err("coordinated retained proposal and receiving-policy widening must reject");
    assert!(proposal_diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("differ from the independently accepted package policy")
    }));

    let windows_storage =
        SourceResolverStorage::for_hardened_base(temporary.0.join("windows-resolved"))
            .expect("create Windows semantic-binding resolver storage");
    let windows_closure = resolve_external_local_project_closure_with_storage(
        &application,
        ExternalSourceContext::derive(b"target-independent-console-binding"),
        omega_target::TargetProfile::WindowsX64,
        &windows_storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve ordinary Windows Console closure");
    let windows_reviews = compile_resolved_package_candidate_reviews(
        &windows_closure,
        "windows_x86_64",
        &temporary.0.join("windows-build"),
    )
    .expect("bind target-independent Console semantics on Windows");
    let windows_root = windows_reviews
        .review(windows_closure.graph().root())
        .expect("Windows root review");
    assert_eq!(windows_root.semantic_bindings().len(), 1);
    assert_eq!(
        windows_root.semantic_bindings()[0].role(),
        AcceptedSemanticBindingRole::ConsoleExitProcessI32
    );
    let [windows_authority] = windows_root.projection().dangerous_authorities() else {
        panic!("Windows Console binding must expose Process authority")
    };
    assert_eq!(
        windows_authority.class(),
        PackageReviewDangerousAuthorityClass::Process
    );
    let windows_console = windows_root
        .projection()
        .selected_providers()
        .iter()
        .find(|provider| provider.service_schema() == "Console")
        .expect("Windows review retains selected Console provider");
    assert!(
        windows_console
            .row_declarations()
            .iter()
            .all(|row| row.compiler_intrinsic_execution()
                != Some(PackageReviewCompilerIntrinsicExecution::LinuxExitGroupI32)),
        "semantic recognition must not mint Linux physical execution on Windows",
    );
}
