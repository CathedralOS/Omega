/*

End-to-end CLI check of a dependency whose directory has no `build.omg`
(`omega --check`).

A package without a build file is given the compiler's default Build. For the
program being compiled that is how a loose `main.omg` works. A dependency is
admitted after the program's own sources, so its default Build is not the
first source the compilation ever loaded; the parser's primary coordinate was
left at that first source and the dependency's Build stopped with an internal
message, "consumer cohort lost its parser's primary source coordinate".

We stage a program that registers such a directory as a dependency. The
dependency must be refused for what is wrong with it: its default Build names
no package, so it has no named owner to be a dependency under. The twin gives
the same directory a `build.omg` that names the package, and compiles.

*/

use std::process::Command;

fn check(name: &str, dependency_build: Option<&str>) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-dependency-without-build-file-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("dep")).expect("staging directory");
    std::fs::write(
        root.join("main.omg"),
        "machine entry() -> i32 {\n    0\n}\n",
    )
    .expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {
    Build::application(builder, \"consumer\");
    let mut description: DependencyDescription = dependency::from_source(Source::Path { location: \"dep\" });
    dependency::with_alias(&mut description, \"dep\");
    let registered: DependencyRegistration = Build::dependency(builder, description);
    builder.freestanding = true;
}
",
    )
    .expect("staged Build");
    std::fs::write(
        root.join("dep").join("main.omg"),
        "public machine value() -> i32 {\n    7\n}\n",
    )
    .expect("staged dependency");
    if let Some(build) = dependency_build {
        std::fs::write(root.join("dep").join("build.omg"), build).expect("dependency Build");
    }
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

#[test]
fn a_dependency_with_a_build_file_compiles() {
    let (compiled, stderr) = check(
        "declared",
        Some("machine build(builder: &mut Build) {\n    Build::package(builder, \"kernel\");\n}\n"),
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn a_dependency_without_a_build_file_is_refused_for_having_no_named_owner() {
    let (compiled, stderr) = check("undeclared", None);
    assert!(!compiled);
    assert!(
        stderr.contains("default Build has no authored named source owner"),
        "{stderr}"
    );
    assert!(!stderr.contains("consumer cohort"), "{stderr}");
}
