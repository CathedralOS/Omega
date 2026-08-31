use omega_compiler::compile_to_checked;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new(build: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-build-target-activation-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary Omega project");
        fs::write(path.join("main.omg"), "const ANSWER: u32 = 42;\n")
            .expect("write temporary Omega source");
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
    format!(
        "target windows_x86_64 {{ }}\nmachine build(builder: &mut Build) {{\n    builder.application(\"target-activation\");\n{body}\n}}\n"
    )
}

fn diagnostic_text(project: &TempProject) -> String {
    compile_to_checked(&project.main(), Some("windows_x86_64"))
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

    let checked = compile_to_checked(&project.main(), Some("windows_x86_64"))
        .expect("the selected target must be an ordinary readable Omega value");
    assert_eq!(
        checked.selected_target_profile(),
        Some(omega_target::TargetProfile::WindowsX64)
    );
    assert_eq!(checked.subsystem(), 2, "Windows branch must select Gui");
}

#[test]
fn legacy_and_canonical_cli_spellings_select_the_same_canonical_profile() {
    let project = TempProject::new(&exact_target_build(""));
    let legacy = compile_to_checked(&project.main(), Some("windows_x64"))
        .expect("legacy CLI alias should normalize before source selection");
    let canonical = compile_to_checked(&project.main(), Some("windows_x86_64"))
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

    let checked = compile_to_checked(&project.main(), None).expect("targetless check should pass");
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

#[test]
fn authored_legacy_build_is_rejected_instead_of_receiving_a_hidden_target() {
    let project = TempProject::new(
        r#"target windows_x86_64 { }
data Build {
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
    let checked = compile_to_checked(&baseline.main(), Some("windows_x86_64"))
        .expect("generic x86 baseline must remain available");
    assert_eq!(checked.x86_scalar_fma_provider(), None);

    let opted_in = TempProject::new(&exact_target_build(
        "    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;",
    ));
    let checked = compile_to_checked(&opted_in.main(), Some("windows_x86_64"))
        .expect("exact x86 build may select the canonical AVX+FMA3 deployment pair");
    let provider = checked
        .x86_scalar_fma_provider()
        .expect("explicit feature selection must retain one admitted provider");
    assert_eq!(provider.profile(), omega_target::TargetProfile::WindowsX64);
    assert!(provider.has_canonical_identity());
    assert_eq!(
        provider.deployment().features(),
        &omega_target::X86_SCALAR_FMA_REQUIRED_FEATURES
    );
}

#[test]
fn x86_fma_build_admission_binds_the_exact_selected_profile() {
    let project = TempProject::new(
        r#"target linux_x86_64 { }
target windows_x86_64 { }
machine build(builder: &mut Build) {
    builder.application("profile-bound-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let linux = compile_to_checked(&project.main(), Some("linux_x86_64"))
        .expect("Linux x86 deployment selection");
    let windows = compile_to_checked(&project.main(), Some("windows_x86_64"))
        .expect("Windows x86 deployment selection");
    let linux = linux.x86_scalar_fma_provider().expect("Linux admission");
    let windows = windows
        .x86_scalar_fma_provider()
        .expect("Windows admission");

    assert_eq!(linux.profile(), omega_target::TargetProfile::LinuxX64);
    assert_eq!(windows.profile(), omega_target::TargetProfile::WindowsX64);
    assert_ne!(linux.identity(), windows.identity());
}

#[test]
fn non_x86_profile_rejects_x86_deployment_feature_selection() {
    let project = TempProject::new(
        r#"target linux_arm64 { }
machine build(builder: &mut Build) {
    builder.application("invalid-arm-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let diagnostics = compile_to_checked(&project.main(), Some("linux_arm64"))
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
    let diagnostics = compile_to_checked(&project.main(), None)
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
