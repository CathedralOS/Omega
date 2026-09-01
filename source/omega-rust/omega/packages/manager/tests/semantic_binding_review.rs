use omega_package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use omega_package_evidence::record::{
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
};
use omega_package_manager::admission::accept_ordinary_closure_evidence;
use omega_package_manager::declarations::{PackageKey, PackageName};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_package_closure_with_storage,
};
use omega_package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    ConsumerScopedSemanticBindingReviewInput, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyRootPolicyDisposition, compare_review_only_initial_capabilities,
    compile_resolved_package_reviews, compile_resolved_package_reviews_with_semantic_bindings,
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
linux_x86_64 machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process
    via Binding::CompilerIntrinsic;
"#,
    );
    write_file(
        application.join("build.omg"),
        r#"target linux_x86_64 { }

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
    let binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::LinuxConsoleExitGroupI32,
        console_package,
        provider.schema_declaration().path(),
        provider.schema().identity_digest(),
        provider.selected_plan_digest(),
    )
    .expect("derive exact candidate Console binding");
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
            role: AcceptedSemanticBindingRole::LinuxConsoleExitGroupI32,
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
                role: AcceptedSemanticBindingRole::LinuxConsoleExitGroupI32,
            }
        ) if consumer == root_key
    ));

    let reviews = compile_resolved_package_reviews_with_semantic_bindings(
        &closure,
        "linux_x86_64",
        &temporary.0.join("accepted-build"),
        &[binding_input],
    )
    .expect("compile exact consumer-bound Console review");
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
    let root_evidence = evidence
        .packages()
        .iter()
        .find(|package| package.package() == &root_key)
        .expect("accepted evidence retains selected root");
    assert_eq!(root_evidence.semantic_bindings(), &[binding]);
}
