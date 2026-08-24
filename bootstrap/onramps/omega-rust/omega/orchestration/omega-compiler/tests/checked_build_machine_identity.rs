use omega_compiler::{
    PackageCompilationInputs, PackageSourceBinding, compile_to_checked,
    compile_to_checked_with_packages,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-checked-build-machine-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary Omega project");
        Self(path)
    }

    fn write(&self, name: &str, source: &str) {
        fs::write(self.0.join(name), source).expect("write temporary Omega source");
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }

    fn root(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn absent_build_machine_retains_no_symbol_or_standalone_package_identity() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");

    let checked = compile_to_checked(&project.main(), None).expect("program should check");

    assert_eq!(checked.selected_build_machine_symbol(), None);
    assert_eq!(checked.package_identity(), None);
}

#[test]
fn present_build_machine_retains_its_exact_checked_symbol() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write("build.omg", "machine build(builder: &mut Build) { }\n");

    let checked = compile_to_checked(&project.main(), None).expect("build program should check");
    let build = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("checked build machine");

    assert_eq!(checked.selected_build_machine_symbol(), Some(build.symbol));
}

#[test]
fn package_aware_checked_compilation_retains_the_reconciled_root_identity() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    let root_identity = PackageKeyIdentity::from_digest([7; 32]).expect("nonzero package identity");
    let inputs = PackageCompilationInputs::new(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            project.root().to_owned(),
        )],
        Vec::new(),
    )
    .expect("root package inputs should validate");

    let checked = compile_to_checked_with_packages(&project.main(), None, inputs)
        .expect("package-aware program should check");

    assert_eq!(checked.package_identity(), Some(root_identity));
    assert_eq!(checked.selected_build_machine_symbol(), None);
}

#[test]
fn duplicate_build_machines_still_reject_before_symbol_retention() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"data Helper { }
machine build(builder: &mut Build) { }
machine Helper::build(&mut self, builder: &mut Build) { }
"#,
    );

    let diagnostics = compile_to_checked(&project.main(), None)
        .expect_err("duplicate build machines must reject checked compilation");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("two build machines exist")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}
