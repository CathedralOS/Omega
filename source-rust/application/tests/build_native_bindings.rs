/*

End-to-end CLI check that the native bindings a Build makes are held to the
Product (`omega --check`).

`Build::bind_native::<Requirement>(builder, target, locator)` says where one boundary
requirement is found on one target (`wiki/spec/build/foreign_bindings.md`).
The requirement is a declaration of the Product, which the Build cannot
check while it runs, so the binding is admitted when the Product is typed:
it must name one visible boundary requirement, and two bindings of one
requirement for one target must name the same place.

The control binds two requirements and must compile. Changing just one
binding either selects a different target successfully or produces the
expected diagnostic at its source location.

The system package binds the native requirements used by a console program.
The dependency coverage checks that program on Linux and emits it for Windows.

*/

mod support;

use std::process::Command;

const PRODUCT: &str = "\
public boundary trait Host {
    machine tick() reaches none crashes none;
    machine tock() reaches none crashes none;
}
machine main() { }
";

const SLEEP: &str = "|foreign|::ForeignBinding::DllImport { import: |foreign|::DllImport::PeByName { library: \"kernel32.dll\", export: \"Sleep\" } }";
const OTHER: &str = "|foreign|::ForeignBinding::DllImport { import: |foreign|::DllImport::PeByName { library: \"kernel32.dll\", export: \"SleepEx\" } }";

fn omega_check(directory: &std::path::Path, target: &str) -> Result<(), String> {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "main.omg", "--target", target])
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn check(name: &str, bindings: &str) -> Result<(), String> {
    check_product(name, PRODUCT, bindings)
}

fn check_product(name: &str, product: &str, bindings: &str) -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "omega-build-native-bindings-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), product).expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "use {{omega}}/language/core/external_binding.omg as foreign;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: \"bound\" }};
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
{bindings}}}
"
        ),
    )
    .expect("staged Build");
    let result = omega_check(&root, "linux_x86_64");
    let _ = std::fs::remove_dir_all(&root);
    result
}

#[test]
fn a_binding_of_each_requirement_is_admitted() {
    check(
        "control",
        &format!(
            "    Build::bind_native::<Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind_native::<Host::tock>(builder, windows_x86_64, {OTHER});
"
        ),
    )
    .expect("the package must compile with both native bindings");
}

#[test]
fn two_places_for_one_requirement_on_one_target_are_refused() {
    let stderr = check(
        "two-places",
        &format!(
            "    Build::bind_native::<Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind_native::<Host::tick>(builder, windows_x86_64, {OTHER});
"
        ),
    )
    .expect_err("the invalid binding must be refused");
    assert!(
        stderr.contains(
            "conflicting native bindings for the same requirement, target and supplier precedence"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("build.omg:7:"), "{stderr}");
}

/// The same two places for two targets do not conflict.
#[test]
fn one_requirement_may_be_found_in_a_different_place_on_each_target() {
    check(
        "two-targets",
        &format!(
            "    Build::bind_native::<Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind_native::<Host::tick>(builder, linux_x86_64, {OTHER});
"
        ),
    )
    .expect("the package must compile with both native bindings");
}

#[test]
fn a_binding_that_names_no_requirement_is_refused() {
    let stderr = check(
        "no-requirement",
        &format!("    Build::bind_native::<Host::missing>(builder, windows_x86_64, {SLEEP});\n"),
    )
    .expect_err("the invalid binding must be refused");
    assert!(
        stderr
            .contains("Product requirement `Host::missing` does not name one visible declaration"),
        "{stderr}"
    );
    assert!(stderr.contains("build.omg:6:"), "{stderr}");
}

/// A package's own bindings are judged with the package, even when no
/// machine calls the requirement: the control's twin binds a requirement with
/// a default body, which the checker does not read, and nothing calls it.
#[test]
fn a_requirement_the_package_binds_is_judged_though_nothing_calls_it() {
    let product = PRODUCT.replace(
        "machine tock() reaches none crashes none;",
        "machine tock() reaches none crashes none { }",
    );
    let stderr = check_product(
        "uncalled-binding",
        &product,
        &format!("    Build::bind_native::<Host::tock>(builder, windows_x86_64, {OTHER});\n"),
    )
    .expect_err("the invalid binding must be refused");
    assert!(stderr.contains(DEFAULT_BODY_REFUSAL), "{stderr}");
}

/// A dependency whose Build binds a requirement with a default body, which
/// the checker does not read, and a public machine that calls it.
const BOUND_LIBRARY: &str = "\
public boundary trait Raw {
    machine read() reaches none crashes none { }
}
public machine reads() reaches none { Raw::read(); }
public machine answer() -> u32 { 73u32 }
";

/// What the checker says when it judges a requirement with a default body.
const DEFAULT_BODY_REFUSAL: &str = "machine default or native-callback requirement";

/// Stage a program that uses `BOUND_LIBRARY` through `body`, and check it.
fn check_with_bound_library(name: &str, body: &str) -> String {
    let root = std::env::temp_dir().join(format!(
        "omega-build-native-bindings-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let library = root.join("bound_library");
    std::fs::create_dir_all(&library).expect("staging directory");
    std::fs::write(library.join("main.omg"), BOUND_LIBRARY).expect("staged library");
    std::fs::write(
        library.join("build.omg"),
        format!(
            "use {{omega}}/language/core/external_binding.omg as foreign;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Package {{ name: \"bound_library\" }};
    Build::bind_native::<Raw::read>(builder, macos_arm64, {SLEEP});
}}
"
        ),
    )
    .expect("staged library Build");
    std::fs::write(
        root.join("main.omg"),
        format!("use {{bound}}/main.omg as bound;\n\nmachine main() {{\n{body}}}\n"),
    )
    .expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        "use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"bound_user\" };
    let mutable library: DependencyDescription = |dependency|::from_source(Source::Path { location: \"bound_library\" });
    |dependency|::with_alias(@mutable library, \"bound\");
    let registration: DependencyRegistration = Build::dependency(builder, library);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
}
",
    )
    .expect("staged Build");
    let stderr = omega_check(&root, "windows_x86_64").err().unwrap_or_default();
    let _ = std::fs::remove_dir_all(&root);
    stderr
}

/// A dependency's bindings are judged where the program demands them, not
/// wholesale: a Windows check of a program that reaches nothing of the
/// library does not judge the library's macOS binding.
#[test]
fn a_dependency_binding_nothing_reaches_is_not_judged() {
    let stderr = check_with_bound_library("unreached-dependency-binding", "");
    assert!(!stderr.contains(DEFAULT_BODY_REFUSAL), "{stderr}");
}

/// Its twin reaches the library's machine that calls the bound requirement,
/// so the requirement is judged at that call, in the library's own package.
#[test]
fn a_dependency_binding_the_program_reaches_is_judged() {
    let stderr = check_with_bound_library("reached-dependency-binding", "    |bound|::reads();\n");
    assert!(stderr.contains(DEFAULT_BODY_REFUSAL), "{stderr}");
}

/// The system package supplies the target's providers and native bindings.
#[test]
fn a_program_receives_the_system_packages_native_bindings() {
    let directory = support::repository_root()
        .join("tests/fixtures/providers/run/default_console_provider_reached");
    omega_check(&directory, "linux_x86_64").expect("the console program must compile");
}

/// The same Console program cross-compiles for Windows ARM64: system selects its
/// shared Windows console adapter there, reached through MSVCRT imports placed
/// by AAPCS64. No ARM64 host runs it here.
#[test]
fn windows_arm64_console_emits_with_its_authored_imports() {
    let directory = support::repository_root()
        .join("tests/fixtures/providers/run/default_console_provider_reached");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "run",
            "--engine",
            "native",
            "--target",
            "windows_arm64",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "build.omg",
        ])
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("compiled for target `windows_arm64`"),
        "{stderr}"
    );
}

#[test]
fn windows_console_emits_with_its_authored_imports() {
    let directory = support::repository_root()
        .join("tests/fixtures/providers/run/default_console_provider_reached");
    let hosted = cfg!(all(target_os = "windows", target_arch = "x86_64"));
    let engines = if hosted {
        &["native", "psi"][..]
    } else {
        &["native"][..]
    };
    for engine in engines {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
        command.args(["run", "--engine", engine]);
        if !hosted {
            command.args(["--target", "windows_x86_64"]);
        }
        let output = command
            .args([
                "--build-input",
                "build.omg",
                "--build-input",
                "main.omg",
                "build.omg",
            ])
            .current_dir(&directory)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("omega runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        if hosted {
            assert_eq!(output.stdout, b"K\n");
            let expected = if *engine == "native" {
                "native exit 0"
            } else {
                "Psi exit 0"
            };
            assert!(stderr.lines().any(|line| line == expected), "{stderr}");
        } else {
            assert!(
                stderr.contains("compiled for target `windows_x86_64`"),
                "{stderr}"
            );
        }
    }
}
