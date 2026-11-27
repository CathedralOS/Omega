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

#[path = "support/project.rs"]
mod project;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;

const PRODUCT: &str = "\
public boundary trait Host {
    machine tick() reaches none crashes none;
    machine tock() reaches none crashes none;
}
machine main() { }
";

const SLEEP: &str = "|native|::ForeignBinding::DllImport { import: |native|::DllImport::PeByName { library: \"kernel32.dll\", export: \"Sleep\" } }";
const OTHER: &str = "|native|::ForeignBinding::DllImport { import: |native|::DllImport::PeByName { library: \"kernel32.dll\", export: \"SleepEx\" } }";

fn omega_check(directory: &std::path::Path, target: &str) -> Result<(), String> {
    let output = project::omega(directory, &["--check", "main.omg", "--target", target]);
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
    let build = format!(
        "use main.omg as app;
use {{omega}}/language/core/external_binding.omg as native;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: \"bound\" }};
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
{bindings}}}
"
    );
    let scratch = project::stage(name, &[("main.omg", product), ("build.omg", &build)]);
    omega_check(scratch.path(), "linux_x86_64")
}

#[test]
fn a_binding_of_each_requirement_is_admitted() {
    check(
        "control",
        &format!(
            "    Build::bind_native::<|app|::Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind_native::<|app|::Host::tock>(builder, windows_x86_64, {OTHER});
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
            "    Build::bind_native::<|app|::Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind_native::<|app|::Host::tick>(builder, windows_x86_64, {OTHER});
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
            "    Build::bind_native::<|app|::Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind_native::<|app|::Host::tick>(builder, linux_x86_64, {OTHER});
"
        ),
    )
    .expect("the package must compile with both native bindings");
}

#[test]
fn a_binding_that_names_no_requirement_is_refused() {
    let stderr = check(
        "no-requirement",
        &format!(
            "    Build::bind_native::<|app|::Host::missing>(builder, windows_x86_64, {SLEEP});\n"
        ),
    )
    .expect_err("the invalid binding must be refused");
    assert!(
        stderr.contains(
            "Product requirement `|app|::Host::missing` does not name one visible declaration"
        ),
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
        &format!(
            "    Build::bind_native::<|app|::Host::tock>(builder, windows_x86_64, {OTHER});\n"
        ),
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
    let library_build = format!(
        "use main.omg as app;
use {{omega}}/language/core/external_binding.omg as native;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Package {{ name: \"bound_library\" }};
    Build::bind_native::<|app|::Raw::read>(builder, macos_arm64, {SLEEP});
}}
"
    );
    let main = format!("use {{bound}}/main.omg as bound;\n\nmachine main() {{\n{body}}}\n");
    let scratch = project::stage(
        name,
        &[
            ("bound_library/main.omg", BOUND_LIBRARY),
            ("bound_library/build.omg", &library_build),
            ("main.omg", &main),
            (
                "build.omg",
                "use main.omg as app;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"bound_user\" };
    let mutable library: DependencyDescription = |dependency|::from_source(Source::Path { location: \"bound_library\" });
    |dependency|::with_alias(@mutable library, \"bound\");
    let registration: DependencyRegistration = Build::dependency(builder, library);
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
}
",
            ),
        ],
    );
    omega_check(scratch.path(), "windows_x86_64")
        .err()
        .unwrap_or_default()
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

fn compile_windows_console(target: &str, machine: u16) -> TemporaryDirectory {
    let directory = support::repository_root()
        .join("tests/fixtures/providers/run/default_console_provider_reached");
    let build = project::stage("windows-console", &[]);
    let output = project::omega(
        &directory,
        &[
            "build",
            "--target",
            target,
            "--build-dir",
            build.path().to_str().unwrap(),
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "build.omg",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let image = std::fs::read(build.path().join("omega-program.exe")).unwrap();
    assert!(image.starts_with(b"MZ"), "{target}");
    let pe = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    assert_eq!(&image[pe..pe + 4], b"PE\0\0", "{target}");
    assert_eq!(&image[pe + 4..pe + 6], &machine.to_le_bytes(), "{target}");
    for import in [b"msvcrt.dll\0".as_slice(), b"_write\0"] {
        assert!(
            image.windows(import.len()).any(|bytes| bytes == import),
            "{target}: missing import {import:?}"
        );
    }
    build
}

/// The system selects its shared Windows console adapter on ARM64, reached
/// through MSVCRT imports placed by AAPCS64.
#[test]
fn windows_arm64_console_emits_with_its_authored_imports() {
    compile_windows_console("windows_arm64", 0xaa64);
}

#[test]
fn windows_console_emits_with_its_authored_imports() {
    let build = compile_windows_console("windows_x86_64", 0x8664);
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        let compiled = std::process::Command::new(build.path().join("omega-program.exe"))
            .output()
            .unwrap();
        assert!(compiled.status.success(), "{compiled:?}");
        assert_eq!(compiled.stdout, b"K\n");
        let directory = support::repository_root()
            .join("tests/fixtures/providers/run/default_console_provider_reached");
        let interpreted = project::omega(
            &directory,
            &[
                "run",
                "--build-input",
                "build.omg",
                "--build-input",
                "main.omg",
                "build.omg",
            ],
        );
        assert!(interpreted.status.success(), "{interpreted:?}");
        assert_eq!(interpreted.stdout, b"K\n");
        assert!(
            String::from_utf8_lossy(&interpreted.stderr)
                .lines()
                .any(|line| line == "Psi exit 0"),
            "{interpreted:?}"
        );
    }
}
