//! Build-scope sources are checked for the admitted build execution profile,
//! not the product target (`wiki/spec/build/scoped_execution.md`).

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

/// A root whose build entry declares one target-scoped helper machine, for
/// `helper_target` only, and calls it while building.
struct HelperFixture {
    root: std::path::PathBuf,
    main: std::path::PathBuf,
}

impl HelperFixture {
    fn new(helper_target: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-build-execution-profile-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create execution-profile fixture");
        let main = root.join("main.omg");
        fs::write(&main, "const ANSWER: u32 = 42;\n").expect("write main");
        fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    builder.package(\"scoped-helper\");\n    probe();\n}}\n\n{helper_target} machine probe() {{}}\n"
            ),
        )
        .expect("write build");
        Self { root, main }
    }
}

impl Drop for HelperFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A product target that is not the compiler host.
fn foreign_product_target() -> target::TargetProfile {
    match target::TargetProfile::host() {
        target::TargetProfile::LinuxX64 => target::TargetProfile::MacosArm64,
        _ => target::TargetProfile::LinuxX64,
    }
}

/// A profile that is neither the host nor the foreign product target.
fn third_profile() -> target::TargetProfile {
    let taken = [target::TargetProfile::host(), foreign_product_target()];
    [
        target::TargetProfile::WindowsX64,
        target::TargetProfile::LinuxArm64,
        target::TargetProfile::LinuxX64,
    ]
    .into_iter()
    .find(|profile| !taken.contains(profile))
    .expect("three hosted profiles remain")
}

#[test]
fn build_helper_selects_its_host_row_while_the_product_targets_another_profile() {
    let host = target::TargetProfile::host();
    let fixture = HelperFixture::new(host.target_name());
    let product = foreign_product_target();
    let checked = super::compile_to_checked(super::CheckedCompileRequest::new(
        &fixture.main,
        Some(product.target_name()),
    ))
    .unwrap_or_else(|diagnostics| {
        panic!("the build helper's {host:?} row must select under a {product:?} product: {diagnostics:#?}")
    });
    assert_eq!(checked.selected_target_profile(), Some(product));
}

#[test]
fn an_admitted_execution_profile_without_a_helper_row_leaves_the_helper_inert() {
    let host = target::TargetProfile::host();
    let fixture = HelperFixture::new(host.target_name());
    let mut request = super::CheckedCompileRequest::new(
        &fixture.main,
        Some(foreign_product_target().target_name()),
    );
    request.build_execution_profile = Some(third_profile());
    let diagnostics = super::compile_to_checked(request)
        .expect_err("a helper row for the host only is inert under another execution profile");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("probe")),
        "{diagnostics:#?}"
    );
}
