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

const APP_BUILD: &str = "machine build(builder: &mut Build) {
    Build::application(builder, \"acceptance_app\");
    let mut vendor_dependency: DependencyDescription = dependency::from_source(Source::Path { location: \"vendor\" });
    dependency::with_alias(&mut vendor_dependency, \"vendor\");
    let vendor_registration: DependencyRegistration = Build::dependency(builder, vendor_dependency);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
";
const APP_MAIN: &str = "use vendor::main;
machine main() {
    let value: u64 = count(3u64);
}
";
const VENDOR_BUILD: &str =
    "machine build(builder: &mut Build) { Build::package(builder, \"acceptance_vendor\"); }\n";
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
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("--offline")
        .arg("main.omg")
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
        app_main: format!("{APP_MAIN}machine unused(value: u64) -> u64 {{ bump(value) }}\n"),
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
        vendor_build: "machine build(builder: &mut Build) {
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
