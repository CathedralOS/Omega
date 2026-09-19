use super::CompileOutcomes;
use crate::{
    CompileOptions, CompileRequest, CompileTargetOutcome, ExplicitTargetSet,
    RequestedCompileProduct, TargetCompileConfiguration, admit_checked_compilation, compile,
};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../../tests/fixture_rosters/compiler_library.rs"]
mod fixture_roster;

#[test]
fn native_input_reuse_key_distinguishes_two_nonempty_optimization_suites() {
    use optimization_core::{Optimization, OptimizationSelections};
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root");
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: repository
                .join("tests/omega/pass/optimizer/no_selection_empty_entry/main.omg"),
            target_name: Some("linux_x86_64".to_owned()),
            build_dir: None,
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(CompileOutcomes::into_single_report)
    .expect("produce an exact Terminal identity");
    let identity = report
        .retained_native_artifact()
        .unwrap()
        .psi_artifact()
        .manifest()
        .identity();
    let key = |optimization| {
        native_realization::NativeInputReuseKey::from_parts(
            identity,
            proof_admission::AdmissionProfile::default(),
            OptimizationSelections::new([optimization])
                .unwrap()
                .project_post_terminal()
                .selections()
                .clone(),
        )
    };
    let first = key(Optimization::SelectedIncomingU12ExactAddImmediate);
    let second = key(Optimization::SelectedIncomingU12ExactSubtractImmediate);
    assert!(!first.post_terminal_optimizations().is_empty());
    assert!(!second.post_terminal_optimizations().is_empty());
    assert!(first != second);
    assert!(first == key(Optimization::SelectedIncomingU12ExactAddImmediate));
}

static NEXT_MULTI_TARGET_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct MultiTargetFixture {
    root: std::path::PathBuf,
    main: std::path::PathBuf,
}

impl MultiTargetFixture {
    fn new(main_source: &str, build_source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-multi-target-compiler-{}-{}",
            std::process::id(),
            NEXT_MULTI_TARGET_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create multi-target compiler fixture");
        let main = root.join("main.omg");
        fs::write(&main, main_source).expect("write multi-target compiler main");
        fs::write(root.join("build.omg"), build_source).expect("write multi-target compiler build");
        Self { root, main }
    }

    fn request(&self) -> CompileRequest {
        CompileRequest::new(CompileOptions {
            root_path: self.main.clone(),
            build_dir: None,
            target_name: None,
        })
    }

    fn target_configuration(&self, profile: target::TargetProfile) -> TargetCompileConfiguration {
        TargetCompileConfiguration::new(profile)
            .with_build_dir(self.root.join("build").join(profile.target_name()))
    }
}

impl Drop for MultiTargetFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn checked_admission_and_compilation_do_not_write_debug_dumps() {
    let fixture = MultiTargetFixture::new(
        "machine main() { }",
        r#"machine build(builder: &mut Build) { builder.application("no-dumps"); }"#,
    );
    let checked =
        crate::compile_to_checked(crate::CheckedCompileRequest::new(&fixture.main, None)).unwrap();
    assert!(checked.timings().phases().is_empty());
    let expected = admit_checked_compilation(&checked, &[])
        .unwrap()
        .into_settlement();
    let report = compile(fixture.request())
        .and_then(CompileOutcomes::into_single_report)
        .unwrap();
    assert_eq!(report.trust_admission_settlement(), &expected);
    assert!(!fixture.root.join("build").exists());
}

#[test]
fn exact_target_invocation_needs_no_authored_target_declaration() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should have the repository above it");
    let root = repository
        .join("tests/omega/pass")
        .join(fixture_roster::NO_SELECTION_EMPTY_ENTRY)
        .join("main.omg");
    for target in [
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
        "windows_x86_64",
    ] {
        let request = CompileRequest::new(CompileOptions {
            root_path: root.clone(),
            build_dir: Some(std::env::temp_dir().join(format!(
                "omega-private-native-receipt-{target}-{}",
                std::process::id()
            ))),
            target_name: Some(target.to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
        let report = compile(request)
            .and_then(crate::CompileOutcomes::into_single_report)
            .unwrap_or_else(|diagnostics| panic!("{target}: {diagnostics:#?}"));
        let profile = target::TargetProfile::from_omega_target_name(Some(target))
            .expect("hosted target fixture must name a canonical target");
        assert_eq!(
            profile.native_target(),
            report
                .retained_native_artifact()
                .expect("paired report must retain its artifact")
                .target()
        );
        report
            .into_retained_native_artifact()
            .expect("paired report must transfer its retained artifact")
            .validate()
            .expect("retained artifact must replay");
    }
}

#[test]
fn native_batch_reuses_exact_terminal_input_before_distinct_target_lowering() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should have the repository above it");
    let root = repository
        .join("tests/omega/pass")
        .join(fixture_roster::NO_SELECTION_EMPTY_ENTRY)
        .join("main.omg");
    let targets = ExplicitTargetSet::from_caller_names(["linux_x64", "linux_arm64"])
        .expect("hosted targets should canonicalize");
    let batch = CompileRequest::new(CompileOptions {
        root_path: root.clone(),
        build_dir: None,
        target_name: None,
    })
    .with_target_configurations(
        targets
            .profiles()
            .iter()
            .map(|&profile| {
                TargetCompileConfiguration::new(profile).with_build_dir(std::env::temp_dir().join(
                    format!(
                        "omega-native-input-reuse-{}-{}",
                        std::process::id(),
                        profile.target_name(),
                    ),
                ))
            })
            .collect(),
    )
    .with_requested_product(RequestedCompileProduct::NativeArtifact);

    let outcomes = compile(batch).expect("native batch request should admit");
    assert_eq!(outcomes.prepared_terminal_native_input_count(), 1);
    let artifacts = outcomes
        .outcomes()
        .iter()
        .map(|outcome| {
            outcome
                .report()
                .unwrap_or_else(|| {
                    panic!(
                        "{:?}: {:#?}",
                        outcome.target_profile(),
                        outcome.diagnostics()
                    )
                })
                .retained_native_artifact()
                .expect("native report retains its artifact")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        artifacts[0].psi_artifact().manifest().identity(),
        artifacts[1].psi_artifact().manifest().identity(),
    );
    assert_ne!(artifacts[0].target(), artifacts[1].target());
    assert_ne!(artifacts[0].identity(), artifacts[1].identity());
    for outcome in outcomes.outcomes() {
        let profile = outcome
            .target_profile()
            .expect("batch selected an exact target");
        let standalone = compile(
            CompileRequest::new(CompileOptions {
                root_path: root.clone(),
                build_dir: Some(std::env::temp_dir().join(format!(
                    "omega-native-input-standalone-{}-{}",
                    std::process::id(),
                    profile.target_name(),
                ))),
                target_name: Some(profile.target_name().to_owned()),
            })
            .with_requested_product(RequestedCompileProduct::NativeArtifact),
        )
        .and_then(crate::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{profile:?}: {diagnostics:#?}"));
        assert_eq!(
            outcome
                .report()
                .expect("batched native child")
                .retained_native_artifact()
                .expect("batched artifact")
                .identity(),
            standalone
                .retained_native_artifact()
                .expect("standalone artifact")
                .identity(),
        );
    }
}

#[test]
fn native_batch_does_not_reuse_target_specific_terminal_input() {
    let fixture = MultiTargetFixture::new(
        r#"data LinuxMain { }
machine LinuxMain::main(&mut self) { }
data ArmMain { }
machine ArmMain::main(&mut self) { }
"#,
        r#"machine build(builder: &mut Build) {
    builder.application("target-specific-terminal-input");
    builder.roots.bind(linux_x86_64::ProgramEntry, LinuxMain::main);
    builder.roots.bind(linux_arm64::ProgramEntry, ArmMain::main);
}
"#,
    );
    let targets = ExplicitTargetSet::from_caller_names(["linux_x64", "linux_arm64"])
        .expect("hosted targets should canonicalize");
    let batch = fixture
        .request()
        .with_target_configurations(
            targets
                .profiles()
                .iter()
                .map(|&profile| fixture.target_configuration(profile))
                .collect(),
        )
        .with_requested_product(RequestedCompileProduct::NativeArtifact);

    let outcomes = compile(batch).expect("native batch request should admit");
    let terminal_identities = outcomes
        .outcomes()
        .iter()
        .map(|outcome| {
            outcome
                .report()
                .unwrap_or_else(|| {
                    panic!(
                        "{:?}: {:#?}",
                        outcome.target_profile(),
                        outcome.diagnostics()
                    )
                })
                .retained_native_artifact()
                .expect("native report retains its artifact")
                .psi_artifact()
                .manifest()
                .identity()
        })
        .collect::<Vec<_>>();
    assert_eq!(outcomes.prepared_terminal_native_input_count(), 2);
    assert_ne!(terminal_identities[0], terminal_identities[1]);
}

#[test]
fn exact_target_batch_is_canonical_and_matches_standalone() {
    let fixture = MultiTargetFixture::new(
        "const ANSWER: u32 = 42;\n",
        r#"machine build(builder: &mut Build) {
    builder.application("multi-target-compiler");
}
"#,
    );
    let targets = ExplicitTargetSet::from_caller_names(["windows_x64", "linux_arm64", "linux_x64"])
        .expect("explicit target set should canonicalize");
    let batch = fixture.request().with_target_configurations(
        targets
            .profiles()
            .iter()
            .map(|&profile| fixture.target_configuration(profile))
            .collect(),
    );
    let outcomes = compile(batch).expect("batch request should admit");
    assert_eq!(outcomes.outcomes().len(), 3);
    assert_eq!(
        outcomes
            .outcomes()
            .iter()
            .map(CompileTargetOutcome::target_profile)
            .collect::<Vec<_>>(),
        [
            Some(target::TargetProfile::LinuxArm64),
            Some(target::TargetProfile::LinuxX64),
            Some(target::TargetProfile::WindowsX64),
        ],
    );
    assert!(outcomes.outcomes()[0].succeeded());
    assert!(outcomes.outcomes()[1].succeeded());
    assert!(outcomes.outcomes()[2].succeeded());

    let standalone_request = CompileRequest::new(CompileOptions {
        root_path: fixture.main.clone(),
        build_dir: Some(fixture.root.join("build").join("linux_x86_64")),
        target_name: Some("linux_x86_64".to_owned()),
    });
    let standalone = compile(standalone_request)
        .and_then(crate::CompileOutcomes::into_single_report)
        .expect("standalone Linux child");
    let batched = outcomes.outcomes()[1]
        .report()
        .expect("batched Linux child should compile");
    assert_eq!(batched.root_path(), standalone.root_path());
    assert_eq!(batched.source_file_count, standalone.source_file_count);
    assert_eq!(batched.output_kind(), standalone.output_kind());
    assert_eq!(batched.wrote_output(), standalone.wrote_output());
    assert_eq!(
        batched.trust_admission_settlement(),
        standalone.trust_admission_settlement(),
    );
}

#[test]
fn shared_source_failure_is_retained_for_every_exact_target() {
    let fixture = MultiTargetFixture::new(
        "machine broken( {\n",
        r#"machine build(builder: &mut Build) {
    builder.application("multi-target-source-failure");
}
"#,
    );
    let targets = ExplicitTargetSet::from_caller_names(["windows_x64", "linux_x64"])
        .expect("explicit target set should canonicalize");
    let batch = fixture.request().with_target_configurations(
        targets
            .profiles()
            .iter()
            .map(|&profile| fixture.target_configuration(profile))
            .collect(),
    );
    let outcomes = compile(batch).expect("batch request should admit");
    assert_eq!(outcomes.outcomes().len(), 2);
    let linux = outcomes.outcomes()[0]
        .diagnostics()
        .expect("Linux child should retain shared parse failure");
    let windows = outcomes.outcomes()[1]
        .diagnostics()
        .expect("Windows child should retain shared parse failure");
    assert_eq!(linux, windows);
}

#[test]
fn core_drop_consuming_call_site_checks() {
    // `omega::language::core::drop` is declared in the bundled core library;
    // the corpus call site consumes a nominal-cleanup `Guard` and a `Carrier`
    // whose only nominal member is erased, so both die by transfer and the
    // fixture checks without a `Carrier::drop`.
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should have the repository above it");
    let root = repository
        .join("tests/omega/pass")
        .join(fixture_roster::CORE_DROP_EXPLICIT_CONSUME)
        .join("main.omg");
    crate::compile_to_checked(crate::CheckedCompileRequest::new(&root, None))
        .expect("the corpus `omega::language::core::drop` call site must check");
}
