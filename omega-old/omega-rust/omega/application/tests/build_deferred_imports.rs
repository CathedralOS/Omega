/*

End-to-end CLI check that a Build's import of a dependency may wait
(`omega --check`).

A `build.omg` that will call a build dependency writes `use kit;` at its top,
as any source names what it uses. When the Build is first checked, `kit` is
not a package yet: the Build has not run, so it has registered nothing.
`wiki/spec/language/modules.md` ("Names and imports") says what such an
import is: "A logical `use dependency;` may remain deferred until explicit
dependency registration and preparation make its exact declaration scope
available to a subsequent checked continuation." An import that names no
declaration of the Build's own package is therefore not an error. It selects
nothing, and nothing of the package's Product source is checked as Build code
on its account.

That is all the import gets. The same page: "A later request cannot justify
earlier code", and "Ordinary product source retains complete name resolution".
The programs below are one project with a line changed:

- The Build imports `kit` and uses nothing of it; the Product source imports
  its own Product dependency. It compiles.
- The Build uses `kit::v()` with no stage that prepared `kit`. The name is
  unknown, and the Build is rejected for it before anything runs.
- A Product source's import that names nothing is still an error.
- The Build imports `lib`, which it registers as a Product dependency. Once
  the Build has run, the import names a dependency of the wrong scope and is
  rejected for that.

*/

use std::process::Command;

/// `omega --check` on a project whose Build registers `dep/` twice: as the
/// build dependency `kit` and as the Product dependency `lib`. `imports` is
/// written above the Build entry, `body` ends it, and `product` is the whole
/// Product source.
fn check(name: &str, imports: &str, body: &str, product: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-deferred-imports-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("dep")).expect("staging directory");
    std::fs::write(root.join("main.omg"), product).expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "{imports}machine build(builder: &mut Build) {{
    Build::application(builder, \"deferred\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    let mut kit_request: DependencyDescription = dependency::from_source(Source::Path {{ location: \"dep\" }});
    dependency::with_alias(&mut kit_request, \"kit\");
    let kit_registered: DependencyRegistration = Build::build_dependency(builder, kit_request);
    let mut lib_request: DependencyDescription = dependency::from_source(Source::Path {{ location: \"dep\" }});
    dependency::with_alias(&mut lib_request, \"lib\");
    let lib_registered: DependencyRegistration = Build::dependency(builder, lib_request);
{body}}}
"
        ),
    )
    .expect("staged Build");
    std::fs::write(
        root.join("dep/build.omg"),
        "machine build(builder: &mut Build) {
    Build::package(builder, \"kit_package\");
}
",
    )
    .expect("staged dependency Build");
    std::fs::write(
        root.join("dep/main.omg"),
        "public machine v() -> i32 { 7 }\n",
    )
    .expect("staged dependency source");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

const PRODUCT: &str = "use lib;

machine main() {
    let seven: i32 = lib::v();
}
";

#[test]
fn a_build_import_of_a_dependency_waits_while_the_product_keeps_its_own() {
    let (compiled, stderr) = check("deferred", "use kit;\n\n", "", PRODUCT);
    assert!(compiled, "{stderr}");
}

#[test]
fn a_name_used_with_no_stage_that_prepared_its_dependency_is_unknown() {
    let (compiled, stderr) = check(
        "used-unprepared",
        "use kit;\n\n",
        "    let seven: i32 = kit::v();\n",
        PRODUCT,
    );
    assert!(!compiled);
    assert!(
        stderr.contains("names a value nothing the program declares"),
        "{stderr}"
    );
}

#[test]
fn a_product_import_that_names_nothing_is_still_an_error() {
    let (compiled, stderr) = check(
        "product-unknown",
        "",
        "",
        "use nothing_here;

machine main() {
}
",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("import `nothing_here` does not select"),
        "{stderr}"
    );
}

#[test]
fn a_build_import_of_a_product_dependency_is_refused_for_its_scope() {
    let (compiled, stderr) = check("wrong-scope", "use lib;\n\n", "", PRODUCT);
    assert!(!compiled);
    assert!(
        stderr.contains("a build import may only select build dependencies"),
        "{stderr}"
    );
}
