/*

End-to-end CLI check that a checker refusal inside a dependency package says
which machine it is in and where that machine is written (`omega --check`).

The lowered-form checker refuses a construct it cannot judge yet with a
`terminal_check_tables::Refusal`, which holds the machine and, when the
construct has one, its source span. The refusal used to reach the user with
neither: the message named the machine only as text, and no span was set,
so a refusal inside a dependency such as `system` printed no file at all and the
author had to bisect the dependency's source to find it.

We stage a program whose dependency `kit` (declared package `kernel`) holds
one machine the checker refuses. The diagnostic must name that machine and
place it in the dependency's own file, at the line and column of the
machine's name.

*/

#[path = "support/project.rs"]
mod project;

#[test]
fn a_refusal_inside_a_dependency_names_its_machine_and_file_position() {
    let (success, stderr) = project::check(
        "dependency-refusal-location",
        &[
            (
                "main.omg",
                "use {kit}/main.omg as kit;\n\nmachine entry() -> i32 {\n    |kit|::value()\n}\n",
            ),
            (
                "build.omg",
                "use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"consumer\" };
    let mutable description: DependencyDescription = |dependency|::from_source(Source::Path { location: \"dep\" });
    |dependency|::with_alias(@mutable description, \"kit\");
    let registered: DependencyRegistration = Build::dependency(builder, description);
}
",
            ),
            // `7 % 2` is an anonymous remainder, which the checker cannot give an
            // exact meaning yet. The expression has no span of its own on the
            // refusal, so the refusal is placed at the machine's name, line 3
            // column 16.
            (
                "dep/main.omg",
                "// A dependency machine the checker cannot read yet.\n\n\
                 public machine value() -> i32 {\n    7 % 2\n}\n",
            ),
            (
                "dep/build.omg",
                "machine build(builder: @mutable BuildPlan) {\n    builder.product = BuildProduct::Package { name: \"kernel\" };\n}\n",
            ),
        ],
        &["--check", "main.omg"],
    );
    assert!(!success, "{stderr}");
    let refusal = stderr
        .lines()
        .position(|line| {
            line.contains(
                "not supported yet: expression anonymous operation without an exact meaning",
            )
        })
        .unwrap_or_else(|| panic!("the checker refuses the dependency's machine: {stderr}"));
    let lines: Vec<&str> = stderr.lines().collect();
    assert!(
        lines[refusal].contains("while reading the body of `value`"),
        "the refusal names its machine: {stderr}"
    );
    let location = lines
        .get(refusal + 1)
        .map(|line| line.trim())
        .unwrap_or_default();
    // The path is the captured package root, which may be the temporary
    // directory's canonical spelling; compare its tail.
    let path = location
        .strip_prefix("in package `kernel`, ")
        .and_then(|rest| rest.strip_suffix(":3:16"))
        .unwrap_or_else(|| panic!("the refusal is placed at kernel's 3:16: {stderr}"));
    assert!(
        std::path::Path::new(path).ends_with(std::path::Path::new("dep").join("main.omg")),
        "the refusal is placed in the dependency's file: {stderr}"
    );
}
