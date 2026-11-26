//! The local Build can read its admitted source and explicitly selected assets.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

#[test]
fn a_local_build_receives_its_helper_and_only_selected_assets() {
    let project = TemporaryDirectory::create("local-source-inventory", false).unwrap();
    let root = project.path();
    std::fs::write(root.join("note.txt"), b"test input").unwrap();
    std::fs::write(
        root.join("helper.omg"),
        "public machine ready() -> bool { true }",
    )
    .unwrap();
    let source = r#"use helper.omg;
machine readable(builder: @Build, name: @[u8]) -> bool {
    let path: BuildPath = BuildSource::resolve(@builder.source, name);
    let file: i32 = BuildSource::open(@builder.source, path, 0);
    transition file >= 0 { true -> close(builder, file), _ -> false }
    machine close(builder: @Build, file: i32) -> bool {
        _ = BuildSource::close(@builder.source, file);
        true
    }
}
machine build(builder: @mutable Build) crashes Trap {
    let helper_ok: bool = |helper|::ready() && readable(builder, "helper.omg");
    let asset_ok: bool = readable(builder, "note.txt") == EXPECTED;
    transition helper_ok && asset_ok { true -> publish(builder), _ -> failed() }
    machine publish(builder: @mutable Build) { Build::package(builder, "reader"); }
    machine failed() crashes Trap { crash Trap; }
}"#;
    for (inputs, visible) in [
        (&[][..], false),
        (&["build.omg", "helper.omg"][..], false),
        (&["build.omg", "helper.omg", "note.txt"][..], true),
    ] {
        std::fs::write(
            root.join("build.omg"),
            source.replace("EXPECTED", if visible { "true" } else { "false" }),
        )
        .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
        command.arg("build").current_dir(root).stdin(Stdio::null());
        for input in inputs {
            command.args(["--build-input", input]);
        }
        let result = command.arg("build.omg").output().unwrap();
        assert!(
            result.status.success(),
            "inputs {inputs:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn a_scoped_build_imports_its_nested_dependency_through_its_registration() {
    let project = TemporaryDirectory::create("nested-source-inventory", false).unwrap();
    let root = project.path();
    std::fs::create_dir(root.join("dep")).unwrap();
    std::fs::write(
        root.join("dep/build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "numbers"); }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("dep/api.omg"),
        "public machine value() -> i32 { 37 }",
    )
    .unwrap();
    std::fs::write(
        root.join("build.omg"),
        r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) crashes Trap {
    Build::application(builder, "reader");
    let mutable request: DependencyDescription = |dependency|::from_source(Source::Path { location: "dep" });
    |dependency|::with_alias(@mutable request, "numbers");
    let registration: DependencyRegistration = Build::dependency(builder, request);
    let path: BuildPath = BuildSource::resolve(@builder.source, "dep/api.omg");
    let opened: i32 = BuildSource::open(@builder.source, path, 0);
    transition opened < 0 {
        true -> enter(builder), _ -> exposed()
    }
    machine enter(builder: @mutable Build) { Build::bind(builder, linux_x86_64::ProgramEntry, main); }
    machine exposed() crashes Trap { crash Trap; }
}"#,
    )
    .unwrap();
    for (import, permitted) in [("{numbers}/api.omg", true), ("dep/api.omg", false)] {
        std::fs::write(
            root.join("main.omg"),
            format!(
                "use {import};\nmachine main() crashes Trap {{\n\
                   transition |api|::value() == 37 {{ true -> passed(), _ -> failed() }}\n\
                   machine passed() {{}}\n\
                   machine failed() crashes Trap {{ crash Trap; }}\n}}"
            ),
        )
        .unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args([
                "run",
                "--engine",
                "psi",
                "--target",
                "linux_x86_64",
                "--build-input",
                "main.omg",
                "--build-input",
                "build.omg",
                "build.omg",
            ])
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(result.status.success(), permitted, "{stdout}{stderr}");
        if permitted {
            assert!(stdout.lines().any(|line| line == "Psi exit 0"), "{stdout}");
        } else {
            assert!(
                stderr.contains("dep/api.omg")
                    && stderr.contains("belongs to a different reconciled package"),
                "{stderr}"
            );
        }
    }
}
