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
