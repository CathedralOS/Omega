/*

End-to-end CLI check of a Build that names its dependency with an alias
(`omega --check`).

`|dependency|::from_source(Source::Path { location: "dep" })` builds a
`Source`, a sum whose `Path` case holds a byte view, and
`|dependency|::with_alias` then stores another byte view into the description.
The Build therefore holds byte views of more than one region. The checker
derives what each helper writes and reads the type at the end of every
written path; for a byte-view field it found that type by its shape, which
names nothing once two view declarations share the shape, so the path to
`Source::Path.location` had no type and the whole program was refused as a
checker defect ("write frame ... storage write query does not follow its
declared type").

We stage a program with one aliased dependency and require it to compile.

*/

#[path = "support/project.rs"]
mod project;

#[test]
fn a_build_that_aliases_a_path_dependency_compiles() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = format!(
        r#"use {{omega}}/build/dependency.omg;
machine build(builder: @mutable Build) {{
    Build::application(builder, "consumer");
    let mutable description: DependencyDescription = |dependency|::from_source(Source::Path {{ location: "dep" }});
    |dependency|::with_alias(@mutable description, "kit");
    _ = Build::dependency(builder, description);
    Build::bind(builder, {}::ProgramEntry, main);
}}
"#,
        host.target_name()
    );
    let (compiled, stderr) = project::check(
        "build-dependency-alias",
        &[
            (
                "main.omg",
                "use {kit}/main.omg as answers;\n\nmachine main() {\n    _ = |answers|::value();\n}\n",
            ),
            ("build.omg", &build),
            (
                "dep/main.omg",
                "public machine value() -> i32 {\n    7\n}\n",
            ),
            (
                "dep/build.omg",
                "machine build(builder: @mutable Build) {\n    Build::package(builder, \"kernel\");\n}\n",
            ),
        ],
        &["--check", "main.omg"],
    );
    assert!(compiled, "{stderr}");
}
