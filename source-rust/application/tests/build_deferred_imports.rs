//! Check which imports a Build and its Product receive before each program runs.

use std::process::Command;

/// The Build has a local helper and registers `dep/` for the Product.
fn check(name: &str, imports: &str, body: &str, product: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-deferred-imports-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("dep")).expect("staging directory");
    std::fs::write(root.join("main.omg"), product).expect("Product source");
    std::fs::write(
        root.join("helper.omg"),
        "public machine value() -> i32 { 7 }",
    )
    .unwrap();
    std::fs::write(
        root.join("build.omg"),
        format!(
            "{imports}use {{omega}}/build/dependency.omg;
machine build(builder: @mutable Build) {{
    Build::application(builder, \"deferred\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    let mutable lib_request: DependencyDescription = |dependency|::from_source(Source::Path {{ location: \"dep\" }});
    |dependency|::with_alias(@mutable lib_request, \"lib\");
    let lib_registered: DependencyRegistration = Build::dependency(builder, lib_request);
{body}}}
"
        ),
    )
    .expect("staged Build");
    std::fs::write(
        root.join("dep/build.omg"),
        "machine build(builder: @mutable Build) {
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

const PRODUCT: &str = r#"use {lib}/main.omg as values;
machine main() {
    let seven: i32 = |values|::v();
}
"#;

#[test]
fn a_build_calls_its_local_helper_and_the_product_calls_its_own_dependency() {
    let (compiled, stderr) = check(
        "separate-libraries",
        "use helper.omg as helper;\n",
        "    let seven: i32 = |helper|::value();\n",
        PRODUCT,
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn an_unused_external_build_import_requires_a_supplied_library() {
    let (compiled, stderr) = check(
        "missing-build-library",
        "use {kit}/main.omg as kit;\n",
        "",
        PRODUCT,
    );
    assert!(
        !compiled
            && stderr.contains("Build import `{kit}/main.omg as kit` has no supplied source")
            && stderr.contains("build.omg:1:5"),
        "{stderr}"
    );
}

#[test]
fn the_product_must_resolve_its_own_files() {
    let (compiled, stderr) = check(
        "product-unknown",
        "",
        "",
        "\nuse missing.omg as missing;\nmachine main() {}",
    );
    assert!(
        !compiled
            && stderr.contains("no admitted logical file `missing.omg`")
            && stderr.contains("main.omg:2:5"),
        "{stderr}"
    );
}

#[test]
fn a_product_registration_cannot_supply_an_import_to_its_build() {
    let (compiled, stderr) = check(
        "wrong-scope",
        "use {lib}/main.omg as values;\n",
        "",
        PRODUCT,
    );
    assert!(
        !compiled
            && stderr.contains("Build import `{lib}/main.omg as values` has no supplied source")
            && stderr.contains("build.omg:1:5"),
        "{stderr}"
    );
}
