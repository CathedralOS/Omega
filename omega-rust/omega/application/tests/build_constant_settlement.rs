/*

End-to-end CLI check that a Build script may read a constant whose value was
recorded when the source was resolved (`omega --check`).

A project's Build runs first, and the Product is then typed again with the
Build source in it. Whatever the Build read has to mean the same thing
there. A target constant such as `linux_x86_64` is a toolchain declaration
with a recorded value: no stage ran anything to produce it, so the later
frontier holds the same declaration at the same value and nothing is left to
evaluate. We stage a project whose Build passes that constant to a helper
machine and run `omega --check`: the project compiles.

The control passes no constant, so a failure of the first project belongs to
the constant and not to the staging.

*/

use std::path::PathBuf;
use std::process::Command;

fn check(name: &str, build_body: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-constant-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine observe(target: BuildTarget) {{ }}
machine build(builder: &mut Build) {{
    Build::application(builder, \"settled\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
{build_body}}}
"
        ),
    )
    .expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(PathBuf::from(&root));
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_build_that_names_no_constant_compiles() {
    let (compiled, stderr) = check("control", "");
    assert!(compiled, "{stderr}");
}

#[test]
fn a_build_that_reads_a_target_constant_compiles() {
    let (compiled, stderr) = check(
        "target-constant",
        "    observe(linux_x86_64);\n    observe(windows_x86_64);\n",
    );
    assert!(compiled, "{stderr}");
}
