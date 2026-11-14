/*

End-to-end CLI check that the native bindings a Build makes are held to the
Product (`omega --check`).

`Build::bind<Requirement>(builder, target, locator)` says where one boundary
requirement is found on one target (`wiki/spec/build/foreign_bindings.md`).
The requirement is a declaration of the Product, which the Build cannot
check while it runs, so the binding is admitted when the Product is typed:
it must name one visible boundary requirement, and two bindings of one
requirement for one target must name the same place.

We stage a project with one boundary trait and vary only the Build's
bindings. The control binds two requirements; whatever it is stopped by
later, it is not the admission of its bindings. Each twin changes one
binding and is refused for it, at the line it is on.

The last test is the dependency case, on a kept program: its dependency is
the standard library, whose Build binds dozens of requirements and hands
them to the program that uses it.

*/

use std::path::PathBuf;
use std::process::Command;

const PRODUCT: &str = "\
public boundary trait Host {
    machine tick() reaches none crashes none;
    machine tock() reaches none crashes none;
}
machine main() { }
";

const SLEEP: &str = "ForeignBinding::DllImport { import: DllImport::PeByName { library: \"kernel32.dll\", export: \"Sleep\" } }";
const OTHER: &str = "ForeignBinding::DllImport { import: DllImport::PeByName { library: \"kernel32.dll\", export: \"SleepEx\" } }";

fn omega_check(directory: &std::path::Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn check(name: &str, bindings: &str) -> String {
    check_product(name, PRODUCT, bindings)
}

fn check_product(name: &str, product: &str, bindings: &str) -> String {
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
            "use omega::language::core::DllImport;
use omega::language::core::ForeignBinding;
machine build(builder: &mut Build) {{
    Build::application(builder, \"bound\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
{bindings}}}
"
        ),
    )
    .expect("staged Build");
    let stderr = omega_check(&root);
    let _ = std::fs::remove_dir_all(&root);
    stderr
}

/// What the admission of a Build's native bindings says when it refuses.
const ADMISSION_REFUSALS: [&str; 4] = [
    "native binding admission",
    "native binding must name",
    "conflicting native bindings",
    "Product operand preparation",
];

#[test]
fn a_binding_of_each_requirement_is_admitted() {
    let stderr = check(
        "control",
        &format!(
            "    Build::bind<Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind<Host::tock>(builder, windows_x86_64, {OTHER});
"
        ),
    );
    for refusal in ADMISSION_REFUSALS {
        assert!(!stderr.contains(refusal), "{stderr}");
    }
}

#[test]
fn two_places_for_one_requirement_on_one_target_are_refused() {
    let stderr = check(
        "two-places",
        &format!(
            "    Build::bind<Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind<Host::tick>(builder, windows_x86_64, {OTHER});
"
        ),
    );
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
    let stderr = check(
        "two-targets",
        &format!(
            "    Build::bind<Host::tick>(builder, windows_x86_64, {SLEEP});
    Build::bind<Host::tick>(builder, linux_x86_64, {OTHER});
"
        ),
    );
    for refusal in ADMISSION_REFUSALS {
        assert!(!stderr.contains(refusal), "{stderr}");
    }
}

#[test]
fn a_binding_that_names_no_requirement_is_refused() {
    let stderr = check(
        "no-requirement",
        &format!("    Build::bind<Host::missing>(builder, windows_x86_64, {SLEEP});\n"),
    );
    assert!(
        stderr.contains("native binding must name one exact visible package-owned requirement"),
        "{stderr}"
    );
    assert!(stderr.contains("build.omg:6:"), "{stderr}");
}

/// A package's own bindings are judged with the package, even when no
/// machine calls the requirement: the control's twin binds a requirement that
/// declares no crash ceiling, and nothing calls it.
#[test]
fn a_requirement_the_package_binds_is_judged_though_nothing_calls_it() {
    let product = PRODUCT.replace(
        "machine tock() reaches none crashes none;",
        "machine tock() reaches none;",
    );
    let stderr = check_product(
        "uncalled-binding",
        &product,
        &format!("    Build::bind<Host::tock>(builder, windows_x86_64, {OTHER});\n"),
    );
    assert!(
        stderr.contains("boundary requirement must declare its crash ceiling"),
        "{stderr}"
    );
}

/// A dependency whose Build binds a requirement that declares no crash
/// ceiling, and a public machine that calls it.
const BOUND_LIBRARY: &str = "\
public boundary trait Raw {
    machine read() reaches none;
}
public machine reads() reaches none { Raw::read(); }
public machine answer() -> u32 { 73u32 }
";

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
            "use omega::language::core::DllImport;
use omega::language::core::ForeignBinding;
machine build(builder: &mut Build) {{
    Build::package(builder, \"bound_library\");
    Build::bind<Raw::read>(builder, macos_arm64, {SLEEP});
}}
"
        ),
    )
    .expect("staged library Build");
    std::fs::write(
        root.join("main.omg"),
        format!("use bound;\n\nmachine main() {{\n{body}}}\n"),
    )
    .expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {
    Build::application(builder, \"bound_user\");
    let mut library: DependencyDescription = dependency::from_source(Source::Path { location: \"bound_library\" });
    dependency::with_alias(&mut library, \"bound\");
    let registration: DependencyRegistration = Build::dependency(builder, library);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
}
",
    )
    .expect("staged Build");
    let stderr = omega_check(&root);
    let _ = std::fs::remove_dir_all(&root);
    stderr
}

/// A dependency's bindings are judged where the program demands them, not
/// wholesale: a Windows check of a program that reaches nothing of the
/// library does not judge the library's macOS binding.
#[test]
fn a_dependency_binding_nothing_reaches_is_not_judged() {
    let stderr = check_with_bound_library("unreached-dependency-binding", "");
    assert!(
        !stderr.contains("boundary requirement must declare its crash ceiling"),
        "{stderr}"
    );
}

/// Its twin reaches the library's machine that calls the bound requirement,
/// so the requirement is judged at that call, in the library's own package.
#[test]
fn a_dependency_binding_the_program_reaches_is_judged() {
    let stderr = check_with_bound_library("reached-dependency-binding", "    bound::reads();\n");
    assert!(
        stderr.contains("boundary requirement must declare its crash ceiling"),
        "{stderr}"
    );
}

/// A program that depends on the standard library receives the bindings and
/// provider choices its Build made. Nothing about them stops the program.
#[test]
fn a_program_receives_the_bindings_of_the_standard_library() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/fixtures/run/providers/default_console_provider_reached");
    let stderr = omega_check(&directory);
    for refusal in ADMISSION_REFUSALS
        .into_iter()
        .chain(["provider selection", "captured native"])
    {
        assert!(!stderr.contains(refusal), "{stderr}");
    }
    // Its Build stage is over: what remains is said of a package's own
    // checking, not of source discovery.
    assert!(
        !stderr.contains("project source discovery failed"),
        "{stderr}"
    );
}
