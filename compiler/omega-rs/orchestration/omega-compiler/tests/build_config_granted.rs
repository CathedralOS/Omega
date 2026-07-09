//! The GRANTED build.omg round trip (owner answers #2/#4, OWNER_QUESTIONS
//! 2026-07-11i; gate landed 2026-07-11j): a build machine with a DECLARED
//! `filesystem_io` effect runs at compile time through the granted
//! interpreter entry (real filesystem, unscoped -- permissions explicitly
//! de-scoped by the owner) and stages an asset itself, while the augmented
//! Build's image facts flow into the pipeline. The fail halves live in
//! canaries/fail/build (undeclared effects; row-less boundary).

use omega_compiler::{CompileOptions, compile};
use std::path::PathBuf;
use std::process::Command;

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    }
}

#[test]
fn declared_filesystem_build_machine_stages_at_compile_time() {
    let project = std::env::temp_dir().join(format!(
        "omega-build-config-granted-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("stage")).expect("create project dirs");

    let stage = project.join("stage");
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

data Stager {{
    fs: FilesystemHost;
    fd: i32;
    n: i64;
}}

machine Stager::build(&mut self, b: &mut Build)
effects
    filesystem_io
{{
    self.fd = self.fs.create("{stage}/asset.bin", 438);
    transition self.fd >= 0 {{ true -> put(b) _ -> done(b) }}
    state put(&mut self, b: &mut Build) {{
        self.n = self.fs.write(self.fd, "staged by build\n");
        self.n = self.fs.close(self.fd);
        transition true {{ true -> done(b) _ -> done(b) }}
    }}
    state done(&mut self, b: &mut Build) {{
        b.freestanding = false;
    }}
}}
"#,
            stage = stage.display(),
        ),
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) { self.console.exit_process(70); }
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: PathBuf::from(project.join("main.omg")),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("declared-filesystem build.omg should compile");

    let staged = std::fs::read_to_string(stage.join("asset.bin"))
        .expect("the build machine should have staged stage/asset.bin at compile time");
    assert_eq!(staged, "staged by build\n");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("compiled program should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the staged program to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = std::fs::remove_dir_all(&project);
}
