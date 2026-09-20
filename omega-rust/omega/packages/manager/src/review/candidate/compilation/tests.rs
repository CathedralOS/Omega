use super::super::PackageSourceVerificationPhase;
use super::{
    CandidateSourcePreparation, CompileResolvedPackageReviewsError, SemanticBindingReview,
    TargetEntryDiscovery, candidate_semantic_binding_inputs, compile_pass,
    compile_resolved_package_candidate_for_check,
    compile_resolved_package_candidate_for_production, compile_resolved_package_reviews,
    compile_resolved_package_reviews_reusing,
};
use crate::resolution::graph::GitResolutionOptions;
use crate::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure,
};
use package_source::PrimaryGitChoices;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

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

#[test]
fn check_retains_requested_package_entry_after_disposal_without_production() {
    let fixture = SourcePreparationFixture::new();
    fs::write(
        fixture.0.join("package/main.omg"),
        "invalid unselected source",
    )
    .unwrap();
    fs::write(
        fixture.0.join("package/entry.omg"),
        "pub const VALUE: u32 = 8;\n",
    )
    .unwrap();
    let closure = fixture.closure();
    let target = target::TargetProfile::WindowsX64;
    let entry = closure
        .source_root(closure.graph().root())
        .unwrap()
        .join("entry.omg");
    let build = fixture.0.join("check");
    let checked = compile_resolved_package_candidate_for_check(
        &closure.for_exact_target(target),
        &build,
        &entry,
        None,
    )
    .expect("check accepts a package and does not read its unselected main");
    assert_eq!(checked.selected_target_profile(), Some(target));
    assert!(fs::read_dir(&build).unwrap().next().is_none());
    checked.verify_current_source_consumption().unwrap();
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
            "machine build(builder: &mut Build) { builder.package(\"prepared-package\"); }\n",
        )
        .unwrap();
        fs::write(root.join("package/main.omg"), main_source).unwrap();
        Self(root)
    }

    fn closure(&self) -> crate::resolution::graph::ResolvedPackageSourceClosure {
        let storage = SourceResolverStorage::for_hardened_base(
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
        TargetEntryDiscovery::Dependencies,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("discovery retains immutable preparation");
    assert!(preparation.slots.iter().all(Option::is_some));
    assert_eq!(
        preparation.fresh_preparation_count(),
        closure.graph().packages().len()
    );
    assert!(
        candidate_semantic_binding_inputs(&discovery, closure.graph().root())
            .unwrap()
            .is_empty()
    );
    let consumed = compile_pass(
        &exact,
        &fixture.0.join("consumed"),
        &[],
        None,
        None,
        TargetEntryDiscovery::Disabled,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("final pass reuses preparation under fresh sponsors");
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
        &exact,
        &fixture.0.join("candidate"),
        SemanticBindingReview::Discover,
        None,
        &mut preparation,
    )
    .expect("a repeated candidate reuses the same preparation");
    assert_eq!(
        preparation.fresh_preparation_count(),
        closure.graph().packages().len(),
        "an unchanged repeated candidate prepared no source again"
    );
    let reference = independent.review(closure.graph().root()).unwrap();
    for reviews in [&discovery, &consumed, &candidate] {
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
        TargetEntryDiscovery::Dependencies,
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
        TargetEntryDiscovery::Disabled,
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
                &exact,
                &fixture
                    .0
                    .join(format!("reused-{}", exact.target_profile().target_name())),
                SemanticBindingReview::Discover,
                None,
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
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &fixture.0.join("drifted"),
        SemanticBindingReview::Discover,
        None,
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
    via Binding::CompilerIntrinsic
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

/// An application root over an ordinary Console package whose provider also
/// selects itself as the package's own default, so both consumers discover
/// the Console exit binding.
struct ConsoleApplicationFixture(PathBuf);

impl ConsoleApplicationFixture {
    fn new(console_source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-candidate-console-permission-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("console")).unwrap();
        fs::create_dir_all(root.join("application")).unwrap();
        fs::write(
            root.join("console/build.omg"),
            "machine build(builder: &mut Build) { builder.package(\"ordinary-console\"); }\n",
        )
        .unwrap();
        fs::write(root.join("console/main.omg"), console_source).unwrap();
        fs::write(
            root.join("application/build.omg"),
            r#"machine build(builder: &mut Build) {
    builder.application("console-consumer");
    builder.depend_as("ordinary_console", Source::Path { location: "../console" });
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("application/main.omg"),
            r#"use ordinary_console::main;
use omega::language::core::service;

data Main { console: Service<Console> in Bound; }
machine Main::main(&mut self)
reaches Console
{
    self.console.exit_process(70);
}
"#,
        )
        .unwrap();
        Self(root)
    }

    fn closure(&self) -> crate::resolution::graph::ResolvedPackageSourceClosure {
        let storage = SourceResolverStorage::for_hardened_base(
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

impl Drop for ConsoleApplicationFixture {
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
    use package_compilation::AcceptedSemanticBindingRole;
    use package_evidence::record::PackageReviewCanonicalRowKind;

    let fixture = ConsoleApplicationFixture::new(console_source);
    // A source read in either discovery or final checking must observe the
    // caller's inventory. Full package custody still includes this sibling.
    let build_path = fixture.0.join("application/build.omg");
    let build = fs::read_to_string(&build_path).unwrap().replace(
        "\n}\n",
        r#"
    let path: BuildPath = builder.source.resolve("not-an-input.txt");
    let descriptor: i32 = builder.source.open(path, 0);
    transition descriptor < 0 {
        true -> admitted()
        _ -> rejected(builder)
    }
    state admitted() {}
    state rejected(builder: &mut Build) {
        let output: RequiredOutput = builder.output.require("inventory-verdict");
        builder.output.fail(output, "review pass widened the root input inventory");
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
        TargetEntryDiscovery::Dependencies,
        &mut preparation,
    )
    .map(|compiled| compiled.reviews)
    .expect("preliminary pass");
    let proposals = candidate_semantic_binding_inputs(&discovery, &root).unwrap();
    let console_proposals = proposals
        .iter()
        .filter(|input| {
            input.binding().role() == AcceptedSemanticBindingRole::ConsoleExitProcessI32
        })
        .collect::<Vec<_>>();
    assert_eq!(console_proposals.len(), 2, "{proposals:#?}");
    let schema = discovery
        .review(&root)
        .unwrap()
        .projection()
        .selected_providers()
        .iter()
        .find(|provider| provider.service_schema() == "Console")
        .expect("selected Console provider")
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
    for input in console_proposals {
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
        &exact,
        &fixture.0.join("final"),
        SemanticBindingReview::Discover,
        Some(&snapshot),
        &mut preparation,
    )
    .expect("final pass consumes the proposed permissions");
    let root_review = reviews.review(&root).unwrap();
    let [service] = root_review.policy().terminal_permissions().services() else {
        panic!("root policy retains one permitted service");
    };
    assert_eq!(service.service().path(), "Console");
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

/// A dependency package compiled as its own component: its build seals the
/// `Pick` requirement with the fused vtable provider while the checked
/// `PickProvider` adapter beside it is the realization a consumer may deploy
/// independently. The component entry's own call puts that adapter in the
/// module's realization roster, which is what the published description
/// exports — nothing here asserts a roster.
const INDEPENDENT_COMPONENT_SOURCE: &str = r#"pub boundary trait Pick {
    machine mark(value: i32);
}

pub data VtablePick { mark: addr; }
pub machine VtablePick::mark(value: i32)
satisfies Pick::mark
via Binding::VtableField(mark);

pub data PickProvider { }
pub machine PickProvider::mark_adapter(value: i32) satisfies Pick::mark { }

pub data ComponentEntry { pick: Pick; }
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
    builder.package("pick-component");
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
    builder.application("independent-consumer");
    builder.depend_as("pick_component", Source::Path { location: "../pick-component" });
    builder.select_provider<Pick, PickProvider>(CompositionMode::Independent);
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
        let storage = SourceResolverStorage::for_hardened_base(
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
            "machine build(builder: &mut Build) { builder.package(\"ordinary-filesystem\"); }\n",
        )
        .unwrap();
        fs::write(root.join("filesystem/main.omg"), SET_LEN_FILESYSTEM).unwrap();
        fs::write(
            root.join("application/build.omg"),
            r#"machine build(builder: &mut Build) {
    builder.application("filesystem-consumer");
    builder.depend_as("ordinary_filesystem", Source::Path { location: "../filesystem" });
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("application/main.omg"),
            r#"use ordinary_filesystem::main;

pub data Main { files: FilesystemHost; rc: i32; }
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
        let storage = SourceResolverStorage::for_hardened_base(
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
        TargetEntryDiscovery::Dependencies,
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
        &exact,
        &fixture.0.join("final"),
        SemanticBindingReview::Discover,
        None,
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
