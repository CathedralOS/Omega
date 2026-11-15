/*

End-to-end CLI check of a Build that names its dependency with an alias
(`omega --check`).

`dependency::from_source(Source::Path { location: "dep" })` builds a
`Source`, a sum whose `Path` case holds a byte view, and
`dependency::with_alias` then stores another byte view into the description.
The Build therefore holds byte views of more than one region. The checker
derives what each helper writes and reads the type at the end of every
written path; for a byte-view field it found that type by its shape, which
names nothing once two view declarations share the shape, so the path to
`Source::Path.location` had no type and the whole program was refused as a
checker defect ("write frame ... storage write query does not follow its
declared type").

We stage a program with one aliased dependency and require it to compile.

*/

use std::process::Command;

#[test]
fn a_build_that_aliases_a_path_dependency_compiles() {
    let root = std::env::temp_dir().join(format!(
        "omega-build-dependency-alias-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("dep")).expect("staging directory");
    std::fs::write(
        root.join("main.omg"),
        "use kit;\n\nmachine entry() -> i32 {\n    kit::value()\n}\n",
    )
    .expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {
    Build::application(builder, \"consumer\");
    let mut description: DependencyDescription = dependency::from_source(Source::Path { location: \"dep\" });
    dependency::with_alias(&mut description, \"kit\");
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
    std::fs::write(
        root.join("dep").join("build.omg"),
        "machine build(builder: &mut Build) {\n    Build::package(builder, \"kernel\");\n}\n",
    )
    .expect("dependency Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
}
