use super::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

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

    fn request(&self, profile: omega_target::TargetProfile) -> CompileRequest {
        CompileRequest::new(CompileOptions {
            root_path: self.main.clone(),
            build_dir: Some(self.root.join("build").join(profile.target_name())),
            target_name: None,
        })
    }
}

impl Drop for MultiTargetFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn native_product_stop_rejoins_every_hosted_target() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("compiler crate should have the repository above it");
    let root = repository.join("tests/omega/pass/optimizer/no_selection_empty_entry/main.omg");
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
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly);
        let report = driver::compile(request)
            .unwrap_or_else(|diagnostics| panic!("{target}: {diagnostics:#?}"));
        let profile = omega_target::TargetProfile::from_omega_target_name(Some(target))
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
fn exact_target_batch_is_canonical_matches_standalone_and_continues_after_failure() {
    let fixture = MultiTargetFixture::new(
        "const ANSWER: u32 = 42;\n",
        r#"target linux_arm64 {
    host: omega::language::missing_provider { }
}
target linux_x86_64 { }
target windows_x86_64 { }
machine build(builder: &mut Build) {
    builder.application("multi-target-compiler");
}
"#,
    );
    let targets = ExplicitTargetSet::from_caller_names(["windows_x64", "linux_arm64", "linux_x64"])
        .expect("explicit target set should canonicalize");
    let batch =
        MultiTargetCompileRequest::from_target_set(targets, |profile| fixture.request(profile))
            .expect("target factory should not duplicate target identity");
    let outcomes = compile_targets(batch).expect("batch request should admit");
    assert_eq!(outcomes.outcomes().len(), 3);
    assert_eq!(
        outcomes
            .outcomes()
            .iter()
            .map(ExactTargetCompileOutcome::target_profile)
            .collect::<Vec<_>>(),
        [
            omega_target::TargetProfile::LinuxArm64,
            omega_target::TargetProfile::LinuxX64,
            omega_target::TargetProfile::WindowsX64,
        ],
    );
    assert!(!outcomes.outcomes()[0].succeeded());
    assert!(
        outcomes.outcomes()[0]
            .diagnostics()
            .expect("Linux Arm64 child should reject")
            .iter()
            .any(|diagnostic| diagnostic.message.contains("missing_provider")),
    );
    assert!(outcomes.outcomes()[1].succeeded());
    assert!(outcomes.outcomes()[2].succeeded());

    let mut standalone_request = fixture.request(omega_target::TargetProfile::LinuxX64);
    standalone_request.options.target_name = Some("linux_x86_64".to_owned());
    let standalone = compile(standalone_request).expect("standalone Linux child");
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
        r#"target linux_x86_64 { }
target windows_x86_64 { }
machine build(builder: &mut Build) {
    builder.application("multi-target-source-failure");
}
"#,
    );
    let targets = ExplicitTargetSet::from_caller_names(["windows_x64", "linux_x64"])
        .expect("explicit target set should canonicalize");
    let batch =
        MultiTargetCompileRequest::from_target_set(targets, |profile| fixture.request(profile))
            .expect("target factory should not duplicate target identity");
    let outcomes = compile_targets(batch).expect("batch request should admit");
    assert_eq!(outcomes.outcomes().len(), 2);
    let linux = outcomes.outcomes()[0]
        .diagnostics()
        .expect("Linux child should retain shared parse failure");
    let windows = outcomes.outcomes()[1]
        .diagnostics()
        .expect("Windows child should retain shared parse failure");
    assert_eq!(linux, windows);
}
