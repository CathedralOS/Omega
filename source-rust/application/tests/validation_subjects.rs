/*

End-to-end checks that `omega --check` claims what the validation-subjects rule
says it claims (wiki/spec/build/validation_subjects.md): every declaration of
the package the request names, including unused ones, and of a dependency only
what that package or the dependency's executed Build reaches.

Each case is an application `app` with one path dependency `vendor`. The
invalid declaration in every case is the same unproved `count + 1u64`, so a
case differs from the valid program only by where that declaration sits and
what reaches it. The packages are written under Cargo's per-test temporary
directory because the defect must sit in an exact place of a two-package
graph, which the corpus's single-directory fixtures cannot express within the
repository's fixture budget.

*/

use std::path::{Path, PathBuf};
use std::process::Command;

const UNPROVED: &str = "may overflow `u64`";

const APP_BUILD: &str = "use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
    Build::application(builder, \"acceptance_app\");
    let mutable vendor_dependency: DependencyDescription = |dependency|::from_source(Source::Path { location: \"vendor\" });
    |dependency|::with_alias(@mutable vendor_dependency, \"vendor\");
    let vendor_registration: DependencyRegistration = Build::dependency(builder, vendor_dependency);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
";
const APP_MAIN: &str = "use vendor;
machine main() {
    let value: u64 = vendor\\count(3u64);
}
";
const VENDOR_BUILD: &str =
    "machine build(builder: @mutable Build) { Build::package(builder, \"acceptance_vendor\"); }\n";
const VENDOR_MAIN: &str = "public machine count(value: u64) -> u64 { value }\n";

struct Project {
    app_main: String,
    vendor_build: String,
    vendor_main: String,
}

impl Project {
    fn valid() -> Self {
        Self {
            app_main: APP_MAIN.to_owned(),
            vendor_build: VENDOR_BUILD.to_owned(),
            vendor_main: VENDOR_MAIN.to_owned(),
        }
    }

    fn write(&self, name: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("validation_subjects")
            .join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("previous project is removable");
        }
        std::fs::create_dir_all(root.join("vendor")).expect("project directory");
        for (path, text) in [
            ("build.omg", APP_BUILD),
            ("main.omg", self.app_main.as_str()),
            ("vendor/build.omg", self.vendor_build.as_str()),
            ("vendor/main.omg", self.vendor_main.as_str()),
        ] {
            std::fs::write(root.join(path), text).expect("project source");
        }
        root
    }
}

struct Outcome {
    success: bool,
    stderr: String,
}

fn check(directory: &Path) -> Outcome {
    check_entry(directory, "main.omg")
}

fn check_entry(directory: &Path, entry: &str) -> Outcome {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("--offline")
        .args(["--target", "linux_x86_64"])
        .arg(entry)
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    Outcome {
        success: output.status.success(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[track_caller]
fn assert_accepted(directory: &Path) {
    let outcome = check(directory);
    assert!(
        outcome.success,
        "{} must check: {}",
        directory.display(),
        outcome.stderr
    );
}

#[track_caller]
fn assert_refused_in(directory: &Path, machine: &str) {
    let outcome = check(directory);
    assert!(
        !outcome.success,
        "{} must be refused for `{machine}`",
        directory.display()
    );
    let named = format!("machine `{machine}`");
    assert!(
        outcome.stderr.contains(UNPROVED) && outcome.stderr.contains(&named),
        "{} must be refused for `{machine}`: {}",
        directory.display(),
        outcome.stderr
    );
}

#[test]
fn the_valid_program_checks() {
    assert_accepted(&Project::valid().write("valid"));
}

#[test]
fn an_unused_invalid_declaration_of_the_subject_fails() {
    let project = Project {
        app_main: format!("{APP_MAIN}machine unused(count: u64) -> u64 {{ count + 1u64 }}\n"),
        ..Project::valid()
    };
    assert_refused_in(&project.write("subject_unused_invalid"), "unused");
}

#[test]
fn an_entryless_subject_checks_its_unimported_module_in_any_filename() {
    let root = Project::valid().write("entryless_subject_module");
    let supplier = root.join("vendor");
    std::fs::rename(supplier.join("main.omg"), supplier.join("supply.omg")).unwrap();
    std::fs::create_dir(supplier.join("units")).unwrap();
    std::fs::write(
        supplier.join("units/conversion.omg"),
        "public machine from_kelvin(count: u64) -> u64 { count + 1u64 }",
    )
    .unwrap();

    let consumer = check_entry(&root, "build.omg");
    assert!(consumer.success, "{}", consumer.stderr);
    let checked_supplier = check_entry(&supplier, "build.omg");
    assert!(!checked_supplier.success);
    assert!(
        checked_supplier.stderr.contains(UNPROVED)
            && checked_supplier.stderr.contains("from_kelvin"),
        "{}",
        checked_supplier.stderr
    );
}

#[test]
fn an_unreached_invalid_dependency_body_fails_only_when_the_dependency_is_the_subject() {
    let project = Project {
        vendor_main: format!(
            "{VENDOR_MAIN}public machine bump(count: u64) -> u64 {{ count + 1u64 }}\n"
        ),
        ..Project::valid()
    };
    let root = project.write("dependency_unused_invalid");
    assert_accepted(&root);
    assert_refused_in(&root.join("vendor"), "bump");
}

#[test]
fn an_unused_subject_helper_that_reaches_the_invalid_dependency_body_fails() {
    let project = Project {
        app_main: format!(
            "{APP_MAIN}machine unused(value: u64) -> u64 {{ vendor\\bump(value) }}\n"
        ),
        vendor_main: format!(
            "{VENDOR_MAIN}public machine bump(count: u64) -> u64 {{ count + 1u64 }}\n"
        ),
        ..Project::valid()
    };
    assert_refused_in(&project.write("subject_helper_reaches_invalid"), "bump");
}

#[test]
fn a_dependency_build_checks_what_it_executes_and_not_its_unrelated_declarations() {
    let unrelated = Project {
        vendor_build: format!(
            "{VENDOR_BUILD}machine unrelated(count: u64) -> u64 {{ count + 1u64 }}\n"
        ),
        ..Project::valid()
    };
    let root = unrelated.write("dependency_build_unrelated");
    assert_accepted(&root);
    assert_refused_in(&root.join("vendor"), "unrelated");

    let reached = Project {
        vendor_build: "machine build(builder: @mutable Build) {
    let size: u64 = grow(3u64);
    Build::package(builder, \"acceptance_vendor\");
}
machine grow(count: u64) -> u64 { count + 1u64 }
"
        .to_owned(),
        ..Project::valid()
    };
    assert_refused_in(&reached.write("dependency_build_reached"), "grow");
}

#[test]
fn a_direct_package_checks_its_unused_build_declarations_under_its_owner() {
    let build = r#"use service.omg;
machine build(builder: @mutable Build) { Build::package(builder, "root_tests"); }
machine unused() reaches |service|::Pulse { |service|::Pulse::tick(); }
"#;
    let mut project = Project::valid();
    project.vendor_build = build.to_owned();
    project.vendor_main.clear();
    let package = project.write("build_boundary_owner").join("vendor");
    std::fs::write(
        package.join("service.omg"),
        "public boundary trait Pulse { machine tick() reaches none crashes none; }",
    )
    .unwrap();
    assert_accepted(&package);

    std::fs::write(
        package.join("build.omg"),
        format!("{build}machine invalid() -> u32 ensures result == 3u32 {{ 2u32 }}"),
    )
    .unwrap();
    let refused = check(&package);
    assert!(
        !refused.success,
        "an unused Build helper broke its guarantee"
    );
    assert!(
        refused
            .stderr
            .contains("cannot prove ensures contract for exit from invalid"),
        "{}",
        refused.stderr
    );
}
