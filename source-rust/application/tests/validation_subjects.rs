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
use main.omg as program;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"acceptance_app\" };
    let mutable vendor_dependency: DependencyDescription = |dependency|::from_source(Source::Path { location: \"../vendor\" });
    |dependency|::with_alias(@mutable vendor_dependency, \"vendor\");
    let vendor_registration: DependencyRegistration = Build::dependency(builder, vendor_dependency);
    Build::bind(builder, linux_x86_64::ProgramEntry, |program|::main);
}
";
const APP_MAIN: &str = "use {vendor}/api.omg as vendor;
public machine main() {
    let value: u64 = |vendor|::count(3u64);
}
";
const VENDOR_BUILD: &str = "machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: \"acceptance_vendor\" }; }\n";
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
        std::fs::create_dir_all(root.join("app")).expect("consumer directory");
        std::fs::create_dir_all(root.join("vendor")).expect("project directory");
        for (path, text) in [
            ("app/build.omg", APP_BUILD),
            ("app/main.omg", self.app_main.as_str()),
            ("vendor/build.omg", self.vendor_build.as_str()),
            ("vendor/api.omg", self.vendor_main.as_str()),
        ] {
            std::fs::write(root.join(path), text).expect("project source");
        }
        root.join("app")
    }
}

struct Outcome {
    success: bool,
    stdout: String,
    stderr: String,
}

fn check(directory: &Path) -> Outcome {
    check_entry(directory, "build.omg")
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
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
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
    let supplier = root.join("../vendor");
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
    assert_refused_in(&root.join("../vendor"), "bump");
}

#[test]
fn an_unused_subject_helper_that_reaches_the_invalid_dependency_body_fails() {
    let project = Project {
        app_main: format!(
            "{APP_MAIN}machine unused(value: u64) -> u64 {{ |vendor|::bump(value) }}\n"
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
    assert_refused_in(&root.join("../vendor"), "unrelated");

    let reached = Project {
        vendor_build: "machine build(builder: @mutable BuildPlan) {
    let size: u64 = grow(3u64);
    builder.product = BuildProduct::Package { name: \"acceptance_vendor\" };
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
machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "root_tests" }; }
machine unused() reaches |service|::Pulse { |service|::Pulse::tick(); }
"#;
    let mut project = Project::valid();
    project.vendor_build = build.to_owned();
    project.vendor_main.clear();
    let package = project.write("build_boundary_owner").join("../vendor");
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

#[test]
fn validate_follows_a_copied_registration_and_checks_its_entire_package_once() {
    let root = Project::valid().write("selected_registration");
    let validating = APP_BUILD.replace("    Build::bind(", "    let copy: DependencyRegistration = vendor_registration;\n    Build::validate(builder, @copy);\n    Build::validate(builder, @vendor_registration);\n    Build::bind(");
    std::fs::write(root.join("build.omg"), &validating).unwrap();
    let accepted = check(&root);
    assert!(accepted.success, "{}", accepted.stderr);
    assert_eq!(
        accepted
            .stdout
            .matches("validated package acceptance_vendor (product, linux_x86_64)")
            .count(),
        1,
        "{}",
        accepted.stdout
    );

    std::fs::write(
        root.join("../vendor/unused.omg"),
        "public machine invalid() -> u32 { true }",
    )
    .unwrap();
    let rejected = check(&root);
    assert!(
        !rejected.success && rejected.stderr.contains("unused.omg"),
        "{}",
        rejected.stderr
    );
    std::fs::write(root.join("build.omg"), APP_BUILD).unwrap();
    assert_accepted(&root);

    std::fs::remove_file(root.join("../vendor/unused.omg")).unwrap();
    std::fs::write(
        root.join("../vendor/build.omg"),
        format!("{VENDOR_BUILD}machine bad_build_helper(count: u64) -> u64 {{ count + 1u64 }}"),
    )
    .unwrap();
    assert_accepted(&root);
    std::fs::write(root.join("build.omg"), &validating).unwrap();
    assert_refused_in(&root, "bad_build_helper");

    std::fs::write(
        root.join("build.omg"),
        validating.replace("= vendor_registration;", "= DependencyRegistration {};"),
    )
    .unwrap();
    let forged = check(&root);
    assert!(
        !forged.success
            && forged
                .stderr
                .contains("Build::validate requires a registration this activation issued"),
        "{}",
        forged.stderr
    );
}

#[test]
fn validate_distinguishes_identical_aliases_in_build_and_product_scope() {
    let root = Project::valid().write("registration_purpose");
    let tools = root.join("../build_tools");
    std::fs::create_dir(&tools).unwrap();
    std::fs::write(
        tools.join("build.omg"),
        "machine build(b: @mutable BuildPlan) { b.product = BuildProduct::Package { name: \"build_tools\" }; }",
    )
    .unwrap();
    std::fs::write(
        tools.join("unused.omg"),
        "public machine unrelated_build_library_body(count: u64) -> u64 { count + 1u64 }",
    )
    .unwrap();
    let prepare = r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    let mutable tool: DependencyDescription = |dependency|::from_source(Source::Path { location: "../build_tools" });
    |dependency|::with_alias(@mutable tool, "vendor");
    let registered: DependencyRegistration = |dependency|::add(builder, tool);
    builder.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };
}"#;
    let configure = r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "consumer" };
    let mutable vendor: DependencyDescription = |dependency|::from_source(Source::Path { location: "../vendor" });
    |dependency|::with_alias(@mutable vendor, "vendor");
    let registered: DependencyRegistration = |dependency|::add(builder, vendor);
    Build::validate(builder, @registered);
}"#;
    std::fs::write(root.join("configure.omg"), configure).unwrap();
    std::fs::write(root.join("build.omg"), prepare).unwrap();
    assert_accepted(&root);
    let selected = prepare.replace(
        "    builder.product = BuildProduct::InterpretedBuild",
        "    Build::validate(builder, @registered);\n    builder.product = BuildProduct::InterpretedBuild",
    );
    std::fs::write(root.join("build.omg"), selected).unwrap();
    assert_refused_in(&root, "unrelated_build_library_body");

    // The successor can check its incoming library under the same alias as its
    // independent Product registration. The preparing Build requests no validation.
    std::fs::write(root.join("build.omg"), prepare).unwrap();
    let incoming = configure.replace(
        "    Build::validate(builder, @registered);",
        r#"    let incoming: PackageBinding = |dependency|::input(builder, "vendor");
    Build::validate(builder, @incoming);
    Build::validate(builder, @incoming);
    Build::validate(builder, @registered);"#,
    );
    std::fs::write(root.join("configure.omg"), &incoming).unwrap();
    assert_refused_in(&root, "unrelated_build_library_body");
    std::fs::write(
        tools.join("unused.omg"),
        "public machine valid() -> u64 { 1u64 }",
    )
    .unwrap();
    let report = check(&root);
    assert!(report.success, "{}", report.stderr);
    assert!(
        report
            .stdout
            .matches("validated package build_tools (build, ")
            .count()
            == 1,
        "{}",
        report.stdout
    );
    assert!(
        report
            .stdout
            .contains("validated package acceptance_vendor (product, linux_x86_64)"),
        "{}",
        report.stdout
    );
    let forged = incoming.replace(
        r#"|dependency|::input(builder, "vendor")"#,
        "PackageBinding {}",
    );
    std::fs::write(root.join("configure.omg"), forged).unwrap();
    let denied = check(&root);
    assert!(
        !denied.success
            && denied
                .stderr
                .contains("requires a PackageBinding this activation issued"),
        "{}",
        denied.stderr
    );
}
