mod restricted_build_grants;

use super::super::PackageSourceVerificationPhase;
use super::{
    CandidateCompilation, CandidateSourcePreparation, CompileResolvedPackageReviewsError,
    SemanticBindingReview, TargetEntryDiscovery, candidate_semantic_binding_inputs,
    compile_candidate_for_check_reusing, compile_pass,
    compile_resolved_package_candidate_for_production, compile_resolved_package_reviews,
    compile_resolved_package_reviews_reusing,
};
use crate::resolution::graph::GitResolutionOptions;
use crate::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure,
};
use artifacts::compile_timings::counters;
use package_source::PrimaryGitChoices;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolver};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

#[test]
fn multi_target_build_reuse_preserves_each_snapshot_output_obligation() {
    let fixture = SourcePreparationFixture::new();
    let closure = fixture.closure();
    let source_root = closure.source_root(closure.graph().root()).unwrap();
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([73; 32]).unwrap();
    let inputs = package_compilation::PackageCompilationInputs::new_package(
        package,
        vec![
            package_compilation::PackageSourceBinding::new(
                package,
                "prepared_package",
                source_root.to_owned(),
            )
            .with_canonical_source_metadata()
            .unwrap(),
        ],
        Vec::new(),
    )
    .unwrap();
    let assemblies_before = counters::source_assembly_invocation_count();
    let frontier_reuses_before = counters::typed_frontier_reuse_count();
    let outcomes = compiler::compile(
        compiler::CompileRequest::new(compiler::CompileOptions {
            root_path: source_root.join("main.omg"),
            target_name: None,
            build_dir: None,
        })
        .with_target_configurations(vec![
            compiler::TargetCompileConfiguration::new(target::TargetProfile::LinuxX64)
                .with_build_dir(fixture.0.join("linux"))
                .with_build_snapshot(build_evaluation::BuildSnapshotRequest::new(Vec::new())),
            compiler::TargetCompileConfiguration::new(target::TargetProfile::MacosArm64)
                .with_build_dir(fixture.0.join("macos"))
                .with_build_snapshot(build_evaluation::BuildSnapshotRequest::new(Vec::new())),
            compiler::TargetCompileConfiguration::new(target::TargetProfile::WindowsX64)
                .with_build_dir(fixture.0.join("windows"))
                .with_build_snapshot(build_evaluation::BuildSnapshotRequest::new([
                    b"required-but-never-produced".to_vec(),
                ])),
        ])
        .with_package_inputs(inputs),
    )
    .expect("one outcome per target");
    let outcomes = outcomes.outcomes();
    assert_eq!(outcomes.len(), 3);
    assert!(outcomes[0].succeeded(), "{:?}", outcomes[0].diagnostics());
    assert!(outcomes[1].succeeded(), "{:?}", outcomes[1].diagnostics());
    // Equal captured inputs bypass the frontend entirely for the second target.
    // The different output obligation misses Build reuse, but shares the typed
    // frontend before its own Build rejects. Disabling Build reuse would give
    // two frontier reuses; wrongly sharing the third Build would give none.
    assert_eq!(
        counters::source_assembly_invocation_count() - assemblies_before,
        1
    );
    assert_eq!(
        counters::typed_frontier_reuse_count() - frontier_reuses_before,
        1
    );
    let diagnostics = outcomes[2]
        .diagnostics()
        .expect("a sibling's successful build cannot discharge this output obligation");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("required-but-never-produced")
                || diagnostic
                    .message
                    .contains("cannot complete its required outputs")
        }),
        "{diagnostics:?}",
    );
}

#[test]
fn policy_comparison_rejects_a_review_issued_for_another_purpose() {
    use crate::lock::{
        HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockTarget,
    };
    use crate::resolution::graph::{
        CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    };
    use crate::review::{
        LockedPolicyComparisonError, PackagePolicyChangeError, PackagePolicyChangeLimits,
        compare_locked_package_policies, compare_package_policy_changes,
        resolve_package_policy_decisions,
    };

    let fixture = SourcePreparationFixture::new();
    let closure = fixture.closure();
    let exact = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let reviews = compile_resolved_package_reviews(
        &exact,
        &fixture.0.join("review"),
        SemanticBindingReview::Discover,
    )
    .unwrap();
    let source = CanonicalSourceClosureSubject::from_resolved(
        &exact,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    let changes = compare_package_policy_changes(
        None,
        &reviews,
        &exact,
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert!(!changes.requires_decision());
    let decisions =
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &[]).unwrap();
    let history = HistoricalPackagePolicyDecisions::capture_policy(
        &source,
        &changes,
        &decisions,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    let accepted = PackageLockTarget::from_policies(
        source,
        &[(
            reviews.reviews[0].checked_context(),
            reviews.reviews[0].policy(),
        )],
        history,
    )
    .unwrap();
    assert!(
        compare_locked_package_policies(&accepted, &reviews)
            .unwrap()
            .is_empty()
    );

    // Fault injection: keep package, resolution, target, policy and source
    // commitment unchanged, but substitute the checked activation purpose.
    let mut substituted = reviews.clone();
    let review = &mut substituted.reviews[0];
    let bundle = review.generated_source_bundle();
    review.generated_source_bundle =
        package_compilation::PackageGeneratedSourceBundle::from_checked(
            bundle.package(),
            crate::declarations::DependencyPurpose::Build,
            bundle.target(),
            bundle.build_execution_profile(),
            bundle.dependency_closure().clone(),
            bundle.source_consumption_commitment(),
            bundle.sources().to_vec(),
        );
    assert!(matches!(
        compare_package_policy_changes(
            Some(&accepted),
            &substituted,
            &exact,
            PackagePolicyChangeLimits::default()
        ),
        Err(PackagePolicyChangeError::CandidateReview {
            reason: "checked review purpose differs from its source occurrence",
            ..
        })
    ));
    assert!(matches!(
        compare_locked_package_policies(&accepted, &substituted),
        Err(LockedPolicyComparisonError::PurposeMismatch { .. })
    ));
}

#[test]
fn production_rejects_package_role_before_either_binding_mode_creates_a_session() {
    let fixture = SourcePreparationFixture::new();
    let closure = fixture.closure();
    let exact = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let build = fixture.0.join("production");
    for bindings in [
        SemanticBindingReview::Discover,
        SemanticBindingReview::Explicit(&[]),
    ] {
        assert!(matches!(
            compile_resolved_package_candidate_for_production(&exact, &build, bindings, None),
            Err(CompileResolvedPackageReviewsError::InvalidProductionRootRole { .. })
        ));
        assert!(!build.exists());
    }
}

struct SourcePreparationFixture(PathBuf);

impl SourcePreparationFixture {
    fn new() -> Self {
        Self::with_main("pub const VALUE: u32 = 7;\n")
    }

    fn with_main(main_source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-candidate-source-preparation-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("package")).unwrap();
        fs::write(
            root.join("package/build.omg"),
            "machine build(builder: &mut Build) { Build::package(builder, \"prepared_package\"); }\n",
        )
        .unwrap();
        fs::write(root.join("package/main.omg"), main_source).unwrap();
        Self(root)
    }

    fn closure(&self) -> crate::resolution::graph::ResolvedPackageSourceClosure {
        let storage = SourceResolver::for_hardened_base(
            self.0.join("resolved"),
            PrimaryGitChoices::default(),
        )
        .unwrap();
        resolve_external_local_project_closure(
            self.0.join("package"),
            ExternalSourceContext::derive(b"candidate-source-preparation"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            GitResolutionOptions::default(),
        )
        .unwrap()
    }
}

impl Drop for SourcePreparationFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn retained_source_review_matches_independent_and_no_binding_candidates() {
    let fixture = SourcePreparationFixture::new();
    let closure = fixture.closure();
    let exact = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let discovery = compile_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Dependencies,
        None,
        &mut preparation,
    )
    .expect("discovery retains immutable preparation");
    assert!(preparation.slots.iter().all(Option::is_some));
    assert_eq!(
        preparation.fresh_preparation_count(),
        closure.graph().packages().len()
    );
    assert!(
        candidate_semantic_binding_inputs(&discovery.reviews, closure.graph().root())
            .unwrap()
            .is_empty()
    );
    // Identical bindings leave every occurrence's inputs untouched, so the
    // final pass carries the discovery compiles forward instead of paying a
    // second front end, build evaluation, and check for each package.
    let consumed = compile_pass(
        &exact,
        &fixture.0.join("consumed"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Disabled,
        Some(&discovery),
        &mut preparation,
    )
    .expect("final pass reuses preparation under fresh sponsors");
    let consumed = consumed.reviews;
    assert!(preparation.slots.iter().all(Option::is_some));
    assert_eq!(
        preparation.fresh_preparation_count(),
        closure.graph().packages().len(),
        "the final pass prepared nothing fresh"
    );
    let independent = compile_resolved_package_reviews(
        &exact,
        &fixture.0.join("independent"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("independent reference pass");
    let candidate = compile_resolved_package_reviews_reusing(
        CandidateCompilation {
            target_closure: &exact,
            build_root: &fixture.0.join("candidate"),
            root_build_snapshot: None,
            restricted_build_checkpoint: None,
        },
        SemanticBindingReview::Discover,
        &mut preparation,
    )
    .expect("a repeated candidate reuses the same preparation");
    assert_eq!(
        preparation.fresh_preparation_count(),
        closure.graph().packages().len(),
        "an unchanged repeated candidate prepared no source again"
    );
    let reference = independent.review(closure.graph().root()).unwrap();
    for reviews in [&discovery.reviews, &consumed, &candidate] {
        let review = reviews.review(closure.graph().root()).unwrap();
        assert_eq!(
            review.source_consumption_commitment(),
            reference.source_consumption_commitment()
        );
        assert_eq!(
            review.canonical_review_bytes,
            reference.canonical_review_bytes
        );
        assert_eq!(review.semantic_bindings(), reference.semantic_bindings());
    }
    for directory in ["discovery", "consumed", "independent", "candidate"] {
        assert!(
            fs::read_dir(fixture.0.join(directory))
                .unwrap()
                .next()
                .is_none()
        );
    }
    // A reusable source is input, not authority for a different root. Its
    // rejected outcome cannot publish that frontier or stale Build discovery.
    let prepared = preparation.slots[0].as_ref().unwrap().prepared.clone();
    let mut request = compiler::CheckedCompileRequest::new(
        &fixture.0.join("different-root.omg"),
        Some("windows_x86_64"),
    );
    request.prepared_source = Some(prepared);
    request.retain_prepared_source = true;
    let rejected = compiler::compile_checked(request);
    assert!(rejected.result.unwrap_err().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not match its prepared source checkpoint")
    }));
    assert!(rejected.prepared_source.is_none());
    assert!(rejected.independent_components.selections.is_empty());
    assert!(rejected.independent_components.evaluation_usage.is_none());
}

#[test]
#[cfg_attr(not(unix), allow(clippy::permissions_set_readonly_false))]
fn retained_source_review_rejects_source_drift_before_consuming_checkpoint() {
    let fixture = SourcePreparationFixture::new();
    let closure = fixture.closure();
    let exact = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    compile_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Dependencies,
        None,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("discovery completes before source drift");
    let main = closure
        .source_root(closure.graph().root())
        .unwrap()
        .join("main.omg");
    let mut permissions = fs::metadata(&main).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    fs::set_permissions(&main, permissions).unwrap();
    fs::write(&main, "pub const VALUE: u32 = 9;\n").unwrap();
    let result = compile_pass(
        &exact,
        &fixture.0.join("final"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Disabled,
        None,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews);
    assert!(matches!(
        result,
        Err(CompileResolvedPackageReviewsError::SourceCustody {
            phase: PackageSourceVerificationPhase::BeforeCompilation,
            ..
        })
    ));
    assert!(preparation.slots.iter().all(Option::is_some));
    assert!(
        fs::read_dir(fixture.0.join("final"))
            .unwrap()
            .next()
            .is_none()
    );
}

/// The effects canary source rides the review route unchanged: one store
/// serves repeated and cross-target candidates while custody still rejects a
/// changed source before its retained checkpoint can be consumed.
#[test]
#[cfg_attr(not(unix), allow(clippy::permissions_set_readonly_false))]
fn shared_preparation_serves_cross_target_reviews_and_still_rejects_drift() {
    let fixture = SourcePreparationFixture::with_main(include_str!(
        "../../../../../../../../tests/omega/pass/effects/nominal_callback_const_reach/main.omg"
    ));
    let closure = fixture.closure();
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let package_count = closure.graph().packages().len();

    let mut references = Vec::new();
    let mut reused = Vec::new();
    for target in [
        target::TargetProfile::WindowsX64,
        target::TargetProfile::LinuxX64,
    ] {
        let exact = closure.for_exact_target(target);
        references.push(
            compile_resolved_package_reviews(
                &exact,
                &fixture.0.join(format!(
                    "reference-{}",
                    exact.target_profile().target_name()
                )),
                SemanticBindingReview::Discover,
            )
            .expect("independent reference review"),
        );
        reused.push(
            compile_resolved_package_reviews_reusing(
                CandidateCompilation {
                    target_closure: &exact,
                    build_root: &fixture
                        .0
                        .join(format!("reused-{}", exact.target_profile().target_name())),
                    root_build_snapshot: None,
                    restricted_build_checkpoint: None,
                },
                SemanticBindingReview::Discover,
                &mut preparation,
            )
            .expect("cross-target review reuses prepared sources"),
        );
    }
    assert_eq!(
        preparation.fresh_preparation_count(),
        package_count,
        "every package prepared once across both target reviews"
    );
    for (reused, reference) in reused.iter().zip(&references) {
        let review = reused.review(closure.graph().root()).unwrap();
        let reference = reference.review(closure.graph().root()).unwrap();
        assert_eq!(
            review.source_consumption_commitment(),
            reference.source_consumption_commitment()
        );
        assert_eq!(
            review.canonical_review_bytes,
            reference.canonical_review_bytes
        );
        assert_eq!(review.semantic_bindings(), reference.semantic_bindings());
    }

    // A changed source invalidates: custody rejects before the retained
    // checkpoint can supply its stale parse frontier.
    let main = closure
        .source_root(closure.graph().root())
        .unwrap()
        .join("main.omg");
    let mut permissions = fs::metadata(&main).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    fs::set_permissions(&main, permissions).unwrap();
    fs::write(&main, "pub const VALUE: u32 = 9;\n").unwrap();
    let result = compile_resolved_package_reviews_reusing(
        CandidateCompilation {
            target_closure: &closure.for_exact_target(target::TargetProfile::WindowsX64),
            build_root: &fixture.0.join("drifted"),
            root_build_snapshot: None,
            restricted_build_checkpoint: None,
        },
        SemanticBindingReview::Discover,
        &mut preparation,
    );
    assert!(matches!(
        result,
        Err(CompileResolvedPackageReviewsError::SourceCustody {
            phase: PackageSourceVerificationPhase::BeforeCompilation,
            ..
        })
    ));
}

/// An ordinary Console package whose only compiler-intrinsic leaf is the
/// exit requirement.
const EXIT_ONLY_CONSOLE: &str = r#"pub boundary trait Console {
    machine exit_process(return_code: i32)
    reaches Console;
}

pub data ConsoleNativeProvider { }
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
linux_x86_64 machine ConsoleNativeProvider::provider_defaults(defaults: &mut ConsoleNativeProvider) {
    defaults.select_provider<Console, ConsoleNativeProvider>();
}
"#;

/// The same package with the hosted byte output and byte input leaves
/// declared as compiler-intrinsic rows beside the exit leaf.
const BYTE_CONSOLE: &str = r#"pub data ByteRead {
    case Eof;
    case Byte(value: i32 [0..=255]);
}

pub boundary trait Console {
    machine read_byte() -> ByteRead
    reaches Console
    blocks;
    crashes Trap;
    machine write_byte(byte: i32)
    reaches Console;
    machine exit_process(return_code: i32)
    reaches Console;
}

pub data ConsoleNativeProvider { }
linux_x86_64 machine ConsoleNativeProvider::read_byte() -> ByteRead
    satisfies Console::read_byte
    via ForeignBinding::CompilerIntrinsic
    crashes Trap
    blocks;
linux_x86_64 boundary machine ConsoleNativeProvider::write_byte(byte: i32)
    satisfies Console::write_byte;
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
linux_x86_64 machine ConsoleNativeProvider::provider_defaults(defaults: &mut ConsoleNativeProvider) {
    defaults.select_provider<Console, ConsoleNativeProvider>();
}
"#;

/// An application over an ordinary provider package. Both the application and
/// dependency select the provider, but only the application proposes permission.
struct HostedExitApplicationFixture(PathBuf);

impl HostedExitApplicationFixture {
    fn new(provider_source: &str, service: &str, requirement: &str) -> Self {
        let core_import = if requirement == "ProcessExit" {
            "\nuse omega::language::core::process_exit;"
        } else {
            ""
        };
        let root = std::env::temp_dir().join(format!(
            "omega-candidate-hosted-exit-permission-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("console")).unwrap();
        fs::create_dir_all(root.join("application")).unwrap();
        fs::write(
            root.join("console/build.omg"),
            "machine build(builder: &mut Build) { Build::package(builder, \"ordinary_console\"); }\n",
        )
        .unwrap();
        fs::write(root.join("console/main.omg"), provider_source).unwrap();
        fs::write(
            root.join("application/build.omg"),
            format!(
                r#"machine build(builder: &mut Build) {{
    Build::application(builder, "console_consumer");
    Build::depend_as(builder, "ordinary_console", Source::Path {{ location: "../console" }});
    builder.select_provider<{requirement}, ordinary_console::{service}NativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}}
"#
            ),
        )
        .unwrap();
        fs::write(
            root.join("application/main.omg"),
            format!(
                r#"use ordinary_console::main;
use omega::language::core::binding;{core_import}

data Main {{ console: Binding<{service}>; }}
machine Main::main(main: &mut Main)
reaches {service}
{{
    main.console.exit_process(70);
}}
"#
            ),
        )
        .unwrap();
        Self(root)
    }

    fn closure(&self) -> crate::resolution::graph::ResolvedPackageSourceClosure {
        let storage = SourceResolver::for_hardened_base(
            self.0.join("resolved"),
            PrimaryGitChoices::default(),
        )
        .unwrap();
        resolve_external_local_project_closure(
            self.0.join("application"),
            ExternalSourceContext::derive(b"candidate-console-permission"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            GitResolutionOptions::default(),
        )
        .unwrap()
    }
}

impl Drop for HostedExitApplicationFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Discovery proposes the exit permission only for the closure root, and the
/// final pass turns that proposal into one blocking review obligation rather
/// than a silently granted permission. A provider that declares no hosted
/// byte leaves proposes no output or input row.
#[test]
fn discovery_proposes_the_root_console_exit_permission_as_a_blocking_row() {
    use effects::TerminalAuthorityClass;

    assert_root_console_permissions(
        EXIT_ONLY_CONSOLE,
        &[("exit_process", TerminalAuthorityClass::ProcessTermination)],
    );
}

/// The hosted byte output and input leaves of the nominated provider are
/// proposed beside the exit leaf, one blocking row each, still root-only.
#[test]
fn discovery_proposes_the_root_console_output_and_input_permissions_per_declared_leaf() {
    use effects::TerminalAuthorityClass;

    assert_root_console_permissions(
        BYTE_CONSOLE,
        &[
            ("exit_process", TerminalAuthorityClass::ProcessTermination),
            ("read_byte", TerminalAuthorityClass::ProcessInput),
            ("write_byte", TerminalAuthorityClass::ProcessOutput),
        ],
    );
}

/// Run discovery and the final pass over an application consuming
/// `console_source`, asserting that the root proposal carries exactly the
/// expected permission rows, the dependency proposal carries none, the final
/// root policy retains them, and each is one blocking obligation.
fn assert_root_console_permissions(
    console_source: &str,
    expected: &[(&str, effects::TerminalAuthorityClass)],
) {
    assert_root_exit_permissions(
        HostedExitApplicationFixture::new(console_source, "Console", "ordinary_console::Console"),
        "Console",
        package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        expected,
    );
}

#[test]
fn discovery_proposes_core_process_exit_permission_as_a_blocking_row() {
    let source = "use omega::language::core::process_exit;
        pub data ProcessExitNativeProvider {}
        linux_x86_64 boundary machine ProcessExitNativeProvider::exit_process(return_code: i32)
            satisfies ProcessExit::exit_process;
        linux_x86_64 machine ProcessExitNativeProvider::provider_defaults(defaults: &mut ProcessExitNativeProvider) {
            defaults.select_provider<ProcessExit, ProcessExitNativeProvider>();
        }";
    assert_root_exit_permissions(
        HostedExitApplicationFixture::new(source, "ProcessExit", "ProcessExit"),
        "ProcessExit",
        package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
        &[(
            "exit_process",
            effects::TerminalAuthorityClass::ProcessTermination,
        )],
    );
}

#[test]
fn discovery_does_not_nominate_package_owned_process_exit_lookalikes() {
    let source = EXIT_ONLY_CONSOLE.replace("Console", "ProcessExit");
    let fixture =
        HostedExitApplicationFixture::new(&source, "ProcessExit", "ordinary_console::ProcessExit");
    let closure = fixture.closure();
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::LinuxX64),
        &fixture.0.join("review"),
        SemanticBindingReview::Discover,
    )
    .expect("ordinary same-named boundary is not core's process-exit contract");
    for review in reviews.reviews() {
        assert!(review.semantic_bindings().iter().all(|binding| {
            binding.role()
                != package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32
        }));
        assert!(review.policy().terminal_permissions().services().is_empty());
    }
}

fn assert_root_exit_permissions(
    fixture: HostedExitApplicationFixture,
    service_name: &str,
    role: package_compilation::AcceptedSemanticBindingRole,
    expected: &[(&str, effects::TerminalAuthorityClass)],
) {
    use package_evidence::record::PackageReviewCanonicalRowKind;
    // A source read in either discovery or final checking must observe the
    // caller's inventory. Full package custody still includes this sibling.
    let build_path = fixture.0.join("application/build.omg");
    let build = fs::read_to_string(&build_path).unwrap().replace(
        "\n}\n",
        r#"
    let path: BuildPath = BuildSource::resolve(&builder.source, "not-an-input.txt");
    let descriptor: i32 = BuildSource::open(&builder.source, path, 0);
    transition descriptor < 0 {
        true -> admitted()
        _ -> rejected(builder)
    }
    state admitted() {}
    state rejected(builder: &mut Build) {
        let output: RequiredOutput = BuildOutput::require(&mut builder.output, "inventory-verdict");
        BuildOutput::fail(&mut builder.output, output, "review pass widened the root input inventory");
    }
}
"#,
    );
    fs::write(build_path, build).unwrap();
    fs::write(fixture.0.join("application/not-an-input.txt"), "excluded").unwrap();
    let snapshot = build_evaluation::BuildSnapshotRequest::scoped(
        Vec::new(),
        package_compilation::BuildSourceCaptureRequest::new([
            (
                b"build.omg".to_vec(),
                package_compilation::BuildSourceCaptureObligation::Required,
            ),
            (
                b"main.omg".to_vec(),
                package_compilation::BuildSourceCaptureObligation::Required,
            ),
        ])
        .unwrap(),
    );
    let closure = fixture.closure();
    let root = closure.graph().root().clone();
    let exact = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let discovery = compile_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        None,
        Some(&snapshot),
        None,
        TargetEntryDiscovery::Dependencies,
        None,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("preliminary pass");
    let proposals = candidate_semantic_binding_inputs(&discovery, &root).unwrap();
    let exit_proposals = proposals
        .iter()
        .filter(|input| input.binding().role() == role)
        .collect::<Vec<_>>();
    assert_eq!(exit_proposals.len(), 2, "{proposals:#?}");
    let schema = discovery
        .review(&root)
        .unwrap()
        .projection()
        .selected_providers()
        .iter()
        .find(|provider| provider.service_schema() == service_name)
        .expect("selected exit provider")
        .schema()
        .clone();
    let requirement = |name: &str| {
        schema
            .methods
            .iter()
            .find(|method| method.name == name)
            .unwrap_or_else(|| panic!("{name} requirement"))
            .requirement_identity
            .clone()
    };
    for input in exit_proposals {
        let permissions = input.binding().terminal_authority_permissions();
        if input.consumer() == &root {
            assert_eq!(
                permissions.len(),
                expected.len(),
                "root proposal carries one permission per declared leaf: {permissions:?}"
            );
            for (name, class) in expected {
                let identity = requirement(name);
                let matching = permissions
                    .iter()
                    .filter(|permission| permission.requirement_identity() == identity)
                    .collect::<Vec<_>>();
                let [permission] = matching.as_slice() else {
                    panic!("one {name} permission: {permissions:?}");
                };
                assert_eq!(
                    permission.service_schema(),
                    input.binding().normalized_schema_digest()
                );
                assert_eq!(permission.permitted().classes(), &[*class]);
            }
        } else {
            assert!(permissions.is_empty(), "{permissions:?}");
        }
    }

    let reviews = compile_resolved_package_reviews_reusing(
        CandidateCompilation {
            target_closure: &exact,
            build_root: &fixture.0.join("final"),
            root_build_snapshot: Some(&snapshot),
            restricted_build_checkpoint: None,
        },
        SemanticBindingReview::Discover,
        &mut preparation,
    )
    .expect("final pass consumes the proposed permissions");
    let root_review = reviews.review(&root).unwrap();
    let [service] = root_review.policy().terminal_permissions().services() else {
        panic!("root policy retains one permitted service");
    };
    assert_eq!(service.service().path(), service_name);
    assert_eq!(service.permissions().len(), expected.len());
    for review in reviews.reviews() {
        if review.key() != &root {
            assert!(
                review.policy().terminal_permissions().services().is_empty(),
                "dependency reviews propose no permission"
            );
        }
    }
    let conflicts = crate::review::compare_review_only_initial_capabilities(
        &reviews,
        &exact,
        crate::review::ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("fresh conflicts");
    let permission_conflicts = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .filter(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::TerminalAuthorityPermission
        })
        .collect::<Vec<_>>();
    assert_eq!(
        permission_conflicts.len(),
        expected.len(),
        "one blocking permission obligation per leaf"
    );
    assert!(
        permission_conflicts
            .iter()
            .all(|conflict| conflict.is_blocking())
    );
}

/// The bound pass compiles only the occurrences whose inputs its discovered
/// bindings changed: a dependency whose consumer-scoped inputs stay identical
/// carries its discovery compile — whole-program check, build evaluation, and
/// review — forward under its exact package and inputs identity rather than
/// checking the same library again. The Console package is itself a binding
/// consumer, so the binding-free `plain_marks` package is the carried
/// occurrence.
#[test]
fn bound_pass_rechecks_discovery_inputs_and_reuses_typed_frontiers() {
    use crate::declarations::DependencyPurpose;

    let fixture = HostedExitApplicationFixture::new(
        EXIT_ONLY_CONSOLE,
        "Console",
        "ordinary_console::Console",
    );
    fs::create_dir_all(fixture.0.join("plain")).unwrap();
    fs::write(
        fixture.0.join("plain/build.omg"),
        "machine build(builder: &mut Build) { Build::package(builder, \"plain_marks\"); }\n",
    )
    .unwrap();
    fs::write(
        fixture.0.join("plain/main.omg"),
        "pub const MARKER: i32 = 41;\n",
    )
    .unwrap();
    let build_path = fixture.0.join("application/build.omg");
    let build = fs::read_to_string(&build_path).unwrap().replace(
        "\n}\n",
        "    Build::depend_as(builder, \"plain_marks\", Source::Path { location: \"../plain\" });\n}\n",
    );
    fs::write(build_path, build).unwrap();
    let closure = fixture.closure();
    let root = closure.graph().root().clone();
    let exact = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let discovery = compile_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Dependencies,
        None,
        &mut preparation,
    )
    .expect("discovery pass");
    let occurrences = discovery.reviews.reviews().len();
    assert_eq!(occurrences, 3, "console, plain_marks, and the root");
    let discovered = candidate_semantic_binding_inputs(&discovery.reviews, &root).unwrap();
    assert!(
        !discovered.is_empty(),
        "the fixture's discovery proposes root bindings"
    );
    let compilations_before = counters::checked_compilation_call_count();
    let frontiers_before = counters::typed_frontier_reuse_count();
    let preliminary_before = counters::preliminary_check_totals();
    let settled_before = counters::settled_check_totals();
    let bound = compile_pass(
        &exact,
        &fixture.0.join("bound"),
        &discovered,
        None,
        None,
        None,
        TargetEntryDiscovery::Disabled,
        Some(&discovery),
        &mut preparation,
    )
    .expect("bound pass");
    let bound_compilations = counters::checked_compilation_call_count() - compilations_before;
    assert_eq!(
        bound_compilations, 3,
        "every occurrence leaves discovery mode; accepted bindings also change two inputs"
    );
    assert_eq!(
        bound.carried, 0,
        "a discovery-mode review cannot stand in for the final exact inputs, \
         even when its eventual review bytes are unchanged"
    );
    let bound_frontier_reuses = counters::typed_frontier_reuse_count() - frontiers_before;
    assert_eq!(
        bound_frontier_reuses, 3,
        "discovery and service roles are post-frontier inputs, so every \
         typed frontier carries over without re-lowering source"
    );
    let (preliminary_calls, _) = counters::preliminary_check_totals();
    let (settled_calls, _) = counters::settled_check_totals();
    assert_eq!(
        preliminary_calls - preliminary_before.0,
        3,
        "each recompiled occurrence still asks the pre-build admission gate for \
         its own checked graph; the exact-input memo, not a skipped gate, is \
         what answers the identical frozen program"
    );
    assert_eq!(
        settled_calls - settled_before.0,
        3,
        "each recompiled occurrence settles its own checked program: the \
         preliminary result cannot feed settlement because the program it \
         checked predates provider settlement and target selection"
    );
    let plain = closure
        .graph()
        .packages()
        .iter()
        .map(|node| node.source().key())
        .find(|key| key.name().as_str() == "plain_marks")
        .expect("the closure holds the plain dependency");
    let discovered_review = discovery
        .reviews
        .review_occurrence(plain, DependencyPurpose::Product)
        .expect("the dependency's discovery review");
    let bound_review = bound
        .reviews
        .review_occurrence(plain, DependencyPurpose::Product)
        .expect("the dependency's bound review");
    assert_eq!(
        bound_review.canonical_review_bytes(),
        discovered_review.canonical_review_bytes(),
        "the carried review is the discovery pass's exact review"
    );
    assert_eq!(
        bound_review.source_consumption_commitment(),
        discovered_review.source_consumption_commitment()
    );
    assert_eq!(bound.reviews.reviews().len(), occurrences);
}

/// A minimal application importing `omega_language_std::console` over the
/// repository's real `source/library/std` package, resolved as an
/// external-local closure under a fresh temporary root. Returns the fixture
/// root (its tree is the caller's cleanup), the resolved closure, and the
/// checked entry inside the resolver's immutable snapshot. `entry_contract`
/// is the Build-side `roots.bind` spelling for the requested target
/// (`linux_x86_64::ProgramEntry`, `windows_x86_64::ProgramEntry`, ...).
fn std_importing_application_closure(
    entry_contract: &str,
) -> (
    PathBuf,
    crate::resolution::graph::ResolvedPackageSourceClosure,
    PathBuf,
) {
    let root = std::env::temp_dir().join(format!(
        "omega-candidate-std-check-once-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    ));
    let standard_library = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|ancestor| ancestor.join("source/library/std"))
        .find(|candidate| candidate.join("build.omg").is_file())
        .expect("repository source/library/std package")
        .canonicalize()
        .expect("canonical standard-library package root");
    fs::create_dir_all(root.join("application")).unwrap();
    fs::write(
        root.join("application/build.omg"),
        format!(
            r#"machine build(builder: &mut Build) {{
    Build::application(builder, "std_check_consumer");
    Build::depend(builder, Source::Path {{ location: "{}" }});
    builder.roots.bind({entry_contract}, Main::main);
}}
"#,
            standard_library.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();
    fs::write(
        root.join("application/main.omg"),
        r#"use omega_language_std::console;
use omega::language::core::binding;

data Main { console: Binding<Console>; }
machine Main::main(main: &mut Main)
reaches Console
{
    main.console.write_line("std check once");
}
"#,
    )
    .unwrap();
    let storage =
        SourceResolver::for_hardened_base(root.join("resolved"), PrimaryGitChoices::default())
            .unwrap();
    let closure = resolve_external_local_project_closure(
        root.join("application"),
        ExternalSourceContext::derive(b"candidate-std-check-once"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .unwrap();
    let entry = closure
        .source_root(closure.graph().root())
        .unwrap()
        .join("main.omg");
    (root, closure, entry)
}

/// The CLI check route over a real std-importing application runs a discovery
/// pass and a bound pass, and each pass requests the pre-build
/// declaration-authority check and the settled whole-program check for every
/// occurrence while the candidate compilation holds check retention. The
/// exact-input check memo serves the bound pass's identical frozen programs,
/// so on a host-profile target each occurrence — the standard library
/// included — executes each checkpoint once per invocation even though both
/// passes request it.
///
/// linux_x86_64 is the one reviewed toolchain-settled host: the standard
/// library's discovered FilesystemHost and TimeHost bindings let provider
/// settlement mint the canonical per-target realization plans, and those
/// plans stamp fused-service erasure authorizations into its typed program.
/// `lower_typed_trees` reads the authorizations to erase `Binding<R>`
/// carriers, so the bound program is a different check input and the library
/// executes its settled check once per distinct input — the same reason the
/// task forbids erasing changed bindings from reuse keys to manufacture a
/// hit. Every preliminary program is stamped before provider settlement, so
/// both targets' bound preliminary requests memo-hit exactly.
///
/// The preliminary/settled split inside one compile is BUILD-EVALUATES-ONCE's;
/// this pins repeated-check closure across passes: request deltas count every
/// request, executed deltas only the checks that actually ran.
#[test]
fn std_importing_check_executes_identical_checks_once_across_passes() {
    for (profile, entry_contract, bound_settled_executed) in [
        (
            target::TargetProfile::MacosArm64,
            "macos_arm64::ProgramEntry",
            0_u64,
        ),
        (
            target::TargetProfile::LinuxX64,
            "linux_x86_64::ProgramEntry",
            1_u64,
        ),
    ] {
        let (root, closure, entry) = std_importing_application_closure(entry_contract);
        let exact = closure.for_exact_target(profile);
        // The candidate route holds retention across both passes; the test
        // drives them explicitly so each pass's deltas are visible.
        let _retention = compiler::retain_repeated_checks();
        let mut preparation = CandidateSourcePreparation::for_closure(&closure);
        let preliminary_before = counters::preliminary_check_totals();
        let settled_before = counters::settled_check_totals();
        let preliminary_executed_before =
            compiler::RepeatedCheckRetention::executed_preliminary_check_count();
        let settled_executed_before =
            compiler::RepeatedCheckRetention::executed_settled_check_count();
        let discovery = compile_pass(
            &exact,
            &root.join("discovery"),
            &[],
            Some(&entry),
            None,
            None,
            TargetEntryDiscovery::Dependencies,
            None,
            &mut preparation,
        )
        .expect("discovery pass");
        let occurrences = discovery.reviews.reviews().len() as u64;
        assert_eq!(occurrences, 2, "the consumer and the standard library");
        let discovered =
            candidate_semantic_binding_inputs(&discovery.reviews, closure.graph().root()).unwrap();
        assert!(
            !discovered.is_empty(),
            "the fixture's discovery proposes bindings"
        );
        assert_eq!(
            counters::preliminary_check_totals().0 - preliminary_before.0,
            occurrences,
            "discovery requests each occurrence's preliminary check"
        );
        assert_eq!(
            counters::settled_check_totals().0 - settled_before.0,
            occurrences,
            "discovery requests each occurrence's settled check"
        );
        assert_eq!(
            compiler::RepeatedCheckRetention::executed_preliminary_check_count()
                - preliminary_executed_before,
            occurrences,
            "first sight: discovery executes each preliminary check"
        );
        assert_eq!(
            compiler::RepeatedCheckRetention::executed_settled_check_count()
                - settled_executed_before,
            occurrences,
            "first sight: discovery executes each settled check"
        );
        let preliminary_before = counters::preliminary_check_totals();
        let settled_before = counters::settled_check_totals();
        let preliminary_executed_before =
            compiler::RepeatedCheckRetention::executed_preliminary_check_count();
        let settled_executed_before =
            compiler::RepeatedCheckRetention::executed_settled_check_count();
        let bound = compile_pass(
            &exact,
            &root.join("bound"),
            &discovered,
            Some(&entry),
            None,
            None,
            TargetEntryDiscovery::Disabled,
            Some(&discovery),
            &mut preparation,
        )
        .expect("bound pass");
        assert_eq!(bound.reviews.reviews().len(), occurrences as usize);
        assert_eq!(
            counters::preliminary_check_totals().0 - preliminary_before.0,
            occurrences,
            "the bound pass still requests every occurrence's preliminary check"
        );
        assert_eq!(
            counters::settled_check_totals().0 - settled_before.0,
            occurrences,
            "the bound pass still requests every occurrence's settled check"
        );
        assert_eq!(
            compiler::RepeatedCheckRetention::executed_preliminary_check_count()
                - preliminary_executed_before,
            0,
            "every bound preliminary request repeats an identical frozen \
             program, so the memo serves all of them"
        );
        assert_eq!(
            compiler::RepeatedCheckRetention::executed_settled_check_count()
                - settled_executed_before,
            bound_settled_executed,
            "the memo serves every bound settled request whose program is \
             unchanged; on linux_x86_64 the standard library's accepted host \
             bindings mint toolchain plans that stamp fused-service erasures \
             the check reads, so its changed program re-checks once"
        );
        let _ = fs::remove_dir_all(&root);
    }
}

/// `compile_candidate_for_check_reusing` is the exact per-target call the
/// CLI's `--check` makes (`check_one_prepared_target`), and the retention that
/// keeps a repeated whole-program check from executing twice is
/// `compile_candidate`'s own guard across its discovery and bound passes —
/// not anything the caller supplies. The per-pass regression above holds that
/// guard itself; this one holds none of it, so a route that dropped or
/// narrowed its retention would re-execute the bound pass's requests against
/// unretained first-sight results and only these whole-invocation deltas
/// would move.
///
/// windows_x86_64 is the acceptance host's shape: discovered bindings leave
/// every checked program byte-identical, so the bound pass's requests are
/// memo hits. linux_x86_64's toolchain-settled host bindings stamp
/// fused-service erasure authorizations into the library's typed program — a
/// genuinely changed input checked once on first sight — which is the
/// authority correctness the item forbids erasing for a manufactured hit. The
/// fixture issues no restricted build requests, so the audit checkpoint the
/// verify-mode check route also passes is immaterial.
///
/// A second call is the same route repeated: the persisted review cache
/// replays each carried dependency's retained facts (entry candidates
/// included) while the memo serves every identical program, so the repeated
/// invocation executes no check at all.
#[test]
fn std_importing_check_invocation_executes_each_distinct_program_once() {
    for (profile, entry_contract, bound_settled_executed) in [
        (
            target::TargetProfile::WindowsX64,
            "windows_x86_64::ProgramEntry",
            0_u64,
        ),
        (
            target::TargetProfile::LinuxX64,
            "linux_x86_64::ProgramEntry",
            1_u64,
        ),
    ] {
        let (root, closure, entry) = std_importing_application_closure(entry_contract);
        let exact = closure.for_exact_target(profile);
        let mut preparation = CandidateSourcePreparation::for_closure(&closure);
        let preliminary_requests_before = counters::preliminary_check_totals();
        let settled_requests_before = counters::settled_check_totals();
        let preliminary_executed_before =
            compiler::RepeatedCheckRetention::executed_preliminary_check_count();
        let settled_executed_before =
            compiler::RepeatedCheckRetention::executed_settled_check_count();
        let (checked, reviews) = compile_candidate_for_check_reusing(
            CandidateCompilation {
                target_closure: &exact,
                build_root: &root.join("check"),
                root_build_snapshot: None,
                restricted_build_checkpoint: None,
            },
            &entry,
            &mut preparation,
        )
        .expect("the consuming check route completes");
        let occurrences = reviews.reviews().len() as u64;
        assert_eq!(occurrences, 2, "the consumer and the standard library");
        assert!(checked.sources().source_file_count() > 0);
        assert_eq!(
            preparation.fresh_preparation_count(),
            closure.graph().packages().len(),
            "each package's parse frontier assembled once across both passes"
        );
        let preliminary_requests =
            counters::preliminary_check_totals().0 - preliminary_requests_before.0;
        let settled_requests = counters::settled_check_totals().0 - settled_requests_before.0;
        let preliminary_executed =
            compiler::RepeatedCheckRetention::executed_preliminary_check_count()
                - preliminary_executed_before;
        let settled_executed = compiler::RepeatedCheckRetention::executed_settled_check_count()
            - settled_executed_before;
        assert_eq!(
            preliminary_executed, occurrences,
            "each occurrence's pre-build declaration-authority check ran once \
             per invocation; the bound pass's identical programs were served"
        );
        assert_eq!(
            settled_executed,
            occurrences + bound_settled_executed,
            "each occurrence's whole-program check ran once per invocation; on \
             linux_x86_64 the library's changed bound program re-checks once"
        );
        assert!(
            preliminary_requests > preliminary_executed,
            "the bound pass still requested preliminary checks the route's own \
             retention served without an execution — requests are not checks"
        );
        assert!(
            settled_requests > settled_executed,
            "the bound pass still requested settled checks the route's own \
             retention served without an execution — requests are not checks"
        );

        // A repeated identical candidate — the shape of a second `--check`
        // invocation on the same resolved closure — executes no check at
        // all: unchanged dependencies arrive through the persisted review
        // cache and the memo serves every identical program past the route's
        // own guard. The two products stage under different session roots,
        // so custody paths may legitimately differ; the memo's service is
        // proven by the unchanged executed counters, not product equality.
        let (checked_again, reviews_again) = compile_candidate_for_check_reusing(
            CandidateCompilation {
                target_closure: &exact,
                build_root: &root.join("check-again"),
                root_build_snapshot: None,
                restricted_build_checkpoint: None,
            },
            &entry,
            &mut preparation,
        )
        .expect("a repeated identical check completes");
        assert_eq!(reviews_again.reviews().len() as u64, occurrences);
        assert_eq!(
            checked_again.sources().source_file_count(),
            checked.sources().source_file_count()
        );
        assert_eq!(
            preparation.fresh_preparation_count(),
            closure.graph().packages().len(),
            "the repeated candidate consumed the retained parse frontiers"
        );
        assert_eq!(
            compiler::RepeatedCheckRetention::executed_preliminary_check_count()
                - preliminary_executed_before,
            preliminary_executed,
            "the repeated candidate executed no preliminary check"
        );
        assert_eq!(
            compiler::RepeatedCheckRetention::executed_settled_check_count()
                - settled_executed_before,
            settled_executed,
            "the repeated candidate executed no settled check"
        );
        let _ = fs::remove_dir_all(&root);
    }
}

/// A dependency package compiled as its own component: its build seals the
/// `Pick` requirement with the fused vtable provider while the checked
/// `PickProvider` adapter beside it is the realization a consumer may deploy
/// independently. The component entry's own call puts that adapter in the
/// module's realization roster, which is what the published description
/// exports — nothing here asserts a roster.
const INDEPENDENT_COMPONENT_SOURCE: &str = r#"use omega::language::core::binding;

pub boundary trait Pick {
    machine mark(value: i32);
}

pub data VtablePick { mark: addr; }
pub machine VtablePick::mark(value: i32)
satisfies Pick::mark
via ForeignBinding::VtableField(mark);

pub data PickProvider { }
pub machine PickProvider::mark_adapter(value: i32) satisfies Pick::mark { }

pub data ComponentEntry { pick: Binding<Pick>; }
pub machine ComponentEntry::main(&mut self) reaches Pick invokes Pick; {
    self.pick.mark(7);
}
"#;

/// A two-package closure whose application root selects the dependency's
/// provider with `CompositionMode::Independent`. Without the compile loop
/// publishing the named component's description itself, the root's checked
/// compile rejects at the component-closure fence.
struct IndependentComponentFixture(PathBuf);

impl IndependentComponentFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-candidate-independent-component-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("pick-component")).unwrap();
        fs::create_dir_all(root.join("consumer")).unwrap();
        fs::write(
            root.join("pick-component/build.omg"),
            r#"machine build(builder: &mut Build) {
    Build::package(builder, "pick_component");
    builder.select_provider<Pick, VtablePick>(CompositionMode::Fused);
    builder.roots.bind(linux_x86_64::ProgramEntry, ComponentEntry::main);
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("pick-component/main.omg"),
            INDEPENDENT_COMPONENT_SOURCE,
        )
        .unwrap();
        fs::write(
            root.join("consumer/build.omg"),
            r#"machine build(builder: &mut Build) {
    Build::application(builder, "independent_consumer");
    Build::depend_as(builder, "pick_component", Source::Path { location: "../pick-component" });
    builder.select_provider<pick_component::Pick, pick_component::PickProvider>(CompositionMode::Independent);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("consumer/main.omg"),
            "use pick_component::main;\n\ndata Main { }\nmachine Main::main(&mut self) { }\n",
        )
        .unwrap();
        Self(root)
    }

    fn closure(&self) -> crate::resolution::graph::ResolvedPackageSourceClosure {
        let storage = SourceResolver::for_hardened_base(
            self.0.join("resolved"),
            PrimaryGitChoices::default(),
        )
        .unwrap();
        resolve_external_local_project_closure(
            self.0.join("consumer"),
            ExternalSourceContext::derive(b"candidate-independent-component"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            GitResolutionOptions::default(),
        )
        .unwrap()
    }
}

impl Drop for IndependentComponentFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The component publication contract's acceptance shape: an ordinary
/// two-package closure whose root selects `Independent` publishes the
/// dependency's description from the dependency's own checked compilation and
/// consumes it, with no description attached from outside the compile loop.
#[test]
fn review_publishes_the_named_component_description_for_an_independent_selection() {
    let fixture = IndependentComponentFixture::new();
    let closure = fixture.closure();
    let root = closure.graph().root().clone();
    let exact = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let reviews = compile_resolved_package_reviews(
        &exact,
        &fixture.0.join("review"),
        SemanticBindingReview::Discover,
    )
    .expect("an Independent selection settles on the component description the loop published");
    assert_eq!(reviews.reviews().len(), 2);
    reviews
        .review(&root)
        .expect("the consuming root is reviewed");
}

/// An ordinary package whose boundary trait spells one toolchain-settled
/// filesystem cohort leaf: `set_len` classifies as content write.
const SET_LEN_FILESYSTEM: &str = r#"pub boundary trait FilesystemHost {
    machine set_len(descriptor: i32, length: i32) -> i32
    reaches FilesystemHost;
}

pub data FilesystemProvider { }
machine FilesystemProvider::set_len(descriptor: i32, length: i32) -> i32
    satisfies FilesystemHost::set_len { length }
"#;

/// An application root over the filesystem package; the root consumer's
/// callable reaches the boundary, which is what discovery proposes the
/// `FilesystemHostService` binding and its cohort permission rows for.
struct FilesystemApplicationFixture(PathBuf);

impl FilesystemApplicationFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-candidate-filesystem-permission-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("filesystem")).unwrap();
        fs::create_dir_all(root.join("application")).unwrap();
        fs::write(
            root.join("filesystem/build.omg"),
            "machine build(builder: &mut Build) { Build::package(builder, \"ordinary_filesystem\"); }\n",
        )
        .unwrap();
        fs::write(root.join("filesystem/main.omg"), SET_LEN_FILESYSTEM).unwrap();
        fs::write(
            root.join("application/build.omg"),
            r#"machine build(builder: &mut Build) {
    Build::application(builder, "filesystem_consumer");
    Build::depend_as(builder, "ordinary_filesystem", Source::Path { location: "../filesystem" });
    builder.select_provider<ordinary_filesystem::FilesystemHost, ordinary_filesystem::FilesystemProvider>(CompositionMode::Fused);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("application/main.omg"),
            r#"use ordinary_filesystem::main;
use omega::language::core::binding;

pub data Main { files: Binding<FilesystemHost>; rc: i32; }
pub machine Main::main(&mut self)
reaches FilesystemHost
invokes FilesystemHost;
{
    let n: i32 = self.files.set_len(3, 0);
    self.rc = n;
}
"#,
        )
        .unwrap();
        Self(root)
    }

    fn closure(&self) -> crate::resolution::graph::ResolvedPackageSourceClosure {
        let storage = SourceResolver::for_hardened_base(
            self.0.join("resolved"),
            PrimaryGitChoices::default(),
        )
        .unwrap();
        resolve_external_local_project_closure(
            self.0.join("application"),
            ExternalSourceContext::derive(b"candidate-filesystem-permission"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            GitResolutionOptions::default(),
        )
        .unwrap()
    }
}

impl Drop for FilesystemApplicationFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Discovery proposes the settled filesystem cohort permission table only for
/// the closure root: each declared boundary method resolves to exactly its own
/// requirement row, and the final pass retains it as a blocking review
/// obligation rather than a silently granted permission.
#[test]
fn discovery_proposes_the_root_filesystem_cohort_permissions_per_declared_leaf() {
    use effects::TerminalAuthorityClass;
    use package_compilation::AcceptedSemanticBindingRole;
    use package_evidence::record::PackageReviewCanonicalRowKind;

    let fixture = FilesystemApplicationFixture::new();
    let closure = fixture.closure();
    let root = closure.graph().root().clone();
    let exact = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let discovery = compile_pass(
        &exact,
        &fixture.0.join("discovery"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Dependencies,
        None,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("preliminary pass");
    let proposals = candidate_semantic_binding_inputs(&discovery, &root).unwrap();
    let filesystem_proposals = proposals
        .iter()
        .filter(|input| {
            input.binding().role() == AcceptedSemanticBindingRole::FilesystemHostService
        })
        .collect::<Vec<_>>();
    let [input] = filesystem_proposals.as_slice() else {
        panic!("one root FilesystemHostService proposal: {proposals:#?}");
    };
    assert_eq!(input.consumer(), &root);
    let schema = discovery
        .review(&root)
        .unwrap()
        .semantic_binding_candidates()
        .iter()
        .find(|candidate| {
            candidate.binding().role() == AcceptedSemanticBindingRole::FilesystemHostService
        })
        .expect("the discovery review carries the filesystem candidate")
        .service_schema()
        .clone();
    let set_len = schema
        .methods
        .iter()
        .find(|method| method.name == "set_len")
        .expect("the fixture boundary declares set_len");
    let permissions = input.binding().terminal_authority_permissions();
    let [permission] = permissions else {
        panic!("one settled cohort permission per declared leaf: {permissions:?}");
    };
    assert_eq!(
        permission.requirement_identity(),
        set_len.requirement_identity
    );
    assert_eq!(
        permission.service_schema(),
        input.binding().normalized_schema_digest()
    );
    assert_eq!(
        permission.permitted().classes(),
        &[TerminalAuthorityClass::FilesystemContentWrite]
    );

    let reviews = compile_resolved_package_reviews_reusing(
        CandidateCompilation {
            target_closure: &exact,
            build_root: &fixture.0.join("final"),
            root_build_snapshot: None,
            restricted_build_checkpoint: None,
        },
        SemanticBindingReview::Discover,
        &mut preparation,
    )
    .expect("final pass consumes the proposed permissions");
    let root_review = reviews.review(&root).unwrap();
    let [service] = root_review.policy().terminal_permissions().services() else {
        panic!("root policy retains one permitted service");
    };
    assert_eq!(service.service().path(), "FilesystemHost");
    assert_eq!(service.permissions().len(), 1);
    for review in reviews.reviews() {
        if review.key() != &root {
            assert!(
                review.policy().terminal_permissions().services().is_empty(),
                "dependency reviews propose no permission"
            );
        }
    }
    let conflicts = crate::review::compare_review_only_initial_capabilities(
        &reviews,
        &exact,
        crate::review::ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("fresh conflicts");
    let permission_conflicts = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .filter(|conflict| {
            conflict.kind() == PackageReviewCanonicalRowKind::TerminalAuthorityPermission
        })
        .collect::<Vec<_>>();
    let [conflict] = permission_conflicts.as_slice() else {
        panic!("one blocking permission obligation per leaf: {permission_conflicts:?}");
    };
    assert!(conflict.is_blocking());
}
