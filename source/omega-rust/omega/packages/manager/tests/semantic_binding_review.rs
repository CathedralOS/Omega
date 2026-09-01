use omega_effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use omega_package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCompilerIntrinsicExecution,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
};
use omega_package_manager::admission::{
    accept_ordinary_closure_evidence, accepted_terminal_authority_permission_policy,
};
use omega_package_manager::declarations::{PackageKey, PackageName};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_package_closure_with_storage,
};
use omega_package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    ConsumerScopedSemanticBindingReviewInput, FreshPackageRootPolicyError,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDisposition,
    bind_fresh_package_root_policy, compare_review_only_initial_capabilities,
    compile_resolved_package_candidate_reviews, compile_resolved_package_reviews,
    compile_resolved_package_reviews_with_semantic_bindings,
    resolve_review_only_root_policy_decisions,
};
use omega_package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceLineage, SourceResolverStorage,
};
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
    builder.package("console-consumer");
    builder.depend_as("ordinary_console", Source::Path {
        location: "../console"
    });
    builder.select_provider<Console, ConsoleNativeProvider>();
}
"#,
    );
    write_file(
        application.join("main.omg"),
        r#"use ordinary_console::main;

pub machine terminate(console: Console, return_code: i32)
reaches Console
invokes console;
{
    console.exit_process(return_code);
}
"#,
    );

    let storage = SourceResolverStorage::for_hardened_base(temporary.0.join("resolved"))
        .expect("create semantic-binding resolver storage");
    let closure = resolve_external_local_package_closure_with_storage(
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

    let reviews = compile_resolved_package_reviews_with_semantic_bindings(
        &closure,
        "linux_x86_64",
        &temporary.0.join("accepted-build"),
        std::slice::from_ref(&binding_input),
    )
    .expect("compile exact consumer-bound Console review with explicit terminal permission");
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

    let windows_storage =
        SourceResolverStorage::for_hardened_base(temporary.0.join("windows-resolved"))
            .expect("create Windows semantic-binding resolver storage");
    let windows_closure = resolve_external_local_package_closure_with_storage(
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
