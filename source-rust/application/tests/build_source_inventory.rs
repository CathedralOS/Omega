//! The local Build can read its admitted source and explicitly selected assets.
#[path = "support/project.rs"]
mod project;

#[test]
fn a_build_and_its_successor_read_their_helper_and_only_selected_assets() {
    let project = project::stage(
        "local-source-inventory",
        &[
            ("note.txt", "test input"),
            ("helper.omg", "public machine ready() -> bool { true }"),
        ],
    );
    let root = project.path();
    let source = r#"use helper.omg;
machine readable(builder: @BuildPlan, name: @[u8]) -> bool {
    let path: BuildPath = BuildSource::resolve(@builder.source, name);
    let file: i32 = BuildSource::open(@builder.source, path, 0);
    transition file >= 0 { true -> close(builder, file), _ -> false }
    machine close(builder: @BuildPlan, file: i32) -> bool {
        _ = BuildSource::close(@builder.source, file);
        true
    }
}
machine build(builder: @mutable BuildPlan) crashes Trap {
    let helper_ok: bool = |helper|::ready() && readable(builder, "helper.omg");
    let asset_ok: bool = readable(builder, "note.txt") == EXPECTED;
    transition helper_ok && asset_ok { true -> publish(builder), _ -> failed() }
    machine publish(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "reader" }; }
    machine failed() crashes Trap { crash Trap; }
}"#;
    for successor in [false, true] {
        let entry = if successor { "next.omg" } else { "build.omg" };
        if successor {
            std::fs::write(
                root.join("build.omg"),
                r#"machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::InterpretedBuild { entry: "next.omg" };
}"#,
            )
            .unwrap();
        }
        for explicit in [None, Some(false), Some(true)] {
            let visible = explicit == Some(true);
            std::fs::write(
                root.join(entry),
                source.replace("EXPECTED", if visible { "true" } else { "false" }),
            )
            .unwrap();
            let mut inputs = Vec::new();
            if explicit.is_some() {
                inputs.extend(["build.omg", "helper.omg"]);
                if successor {
                    inputs.push(entry);
                }
                if visible {
                    inputs.push("note.txt");
                }
            }
            let mut arguments = vec!["build"];
            for &input in &inputs {
                arguments.extend(["--build-input", input]);
            }
            arguments.push("build.omg");
            let result = project::omega(root, &arguments);
            assert!(
                result.status.success(),
                "successor={successor}, inputs {inputs:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}

#[test]
fn a_scoped_build_imports_its_nested_dependency_through_its_registration() {
    let project = project::stage(
        "nested-source-inventory",
        &[
            (
                "dep/build.omg",
                r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "numbers" }; }"#,
            ),
            ("dep/api.omg", "public machine value() -> i32 { 37 }"),
            (
                "build.omg",
                r#"use {omega}/build/dependency.omg;
use main.omg as app;
machine build(builder: @mutable BuildPlan) crashes Trap {
    builder.product = BuildProduct::Application { name: "reader" };
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable request, "numbers");
    _ = |dependency|::add(builder, request);
    let path: BuildPath = BuildSource::resolve(@builder.source, "dep/api.omg");
    let opened: i32 = BuildSource::open(@builder.source, path, 0);
    transition opened < 0 {
        true -> enter(builder), _ -> exposed()
    }
    machine enter(builder: @mutable BuildPlan) { Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main); }
    machine exposed() crashes Trap { crash Trap; }
}"#,
            ),
        ],
    );
    let root = project.path();
    for (import, permitted) in [("{numbers}/api.omg", true), ("dep/api.omg", false)] {
        std::fs::write(
            root.join("main.omg"),
            format!(
                "use {import};\npublic machine main() crashes Trap {{\n\
                   transition |api|::value() == 37 {{ true -> passed(), _ -> failed() }}\n\
                   machine passed() {{}}\n\
                   machine failed() crashes Trap {{ crash Trap; }}\n}}"
            ),
        )
        .unwrap();
        let result = project::omega(
            root,
            &[
                "run",
                "--engine",
                "psi",
                "--outputs",
                "observation",
                "--target",
                "linux_x86_64",
                "--build-input",
                "main.omg",
                "--build-input",
                "build.omg",
                "build.omg",
            ],
        );
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(result.status.success(), permitted, "{stdout}{stderr}");
        if permitted {
            assert_eq!(
                std::fs::read_to_string(root.join("observation/termination.tsv")).unwrap(),
                "psi\treturn\n"
            );
        } else {
            assert!(
                stderr.contains("dep/api.omg")
                    && stderr.contains("belongs to a different reconciled package"),
                "{stderr}"
            );
        }
    }
}
