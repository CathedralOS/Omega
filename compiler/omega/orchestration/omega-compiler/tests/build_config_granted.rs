//! The GRANTED build.omg round trip (owner answers #2/#4, OWNER_QUESTIONS
//! 2026-07-11i; gate landed 2026-07-11j): a build machine with a declared
//! `FilesystemHost` service ceiling runs at compile time through the granted
//! interpreter entry (real filesystem, unscoped -- permissions explicitly
//! de-scoped by the owner) and stages an asset itself, while the augmented
//! Build's image facts flow into the pipeline. Console rows (#5) are
//! served: a declared `Console` boundary write passes the gate, the
//! granted evaluator serves it, and the bytes flush to the compiler's
//! real streams. The fail halves live in canaries/fail/build
//! (undeclared services; unpinned custom boundary).

use omega_compiler::{CompileOptions, compile, compile_to_checked};
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
    let profile = omega_target::TargetProfile::host();
    let project =
        std::env::temp_dir().join(format!("omega-build-config-granted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("stage")).expect("create project dirs");

    let stage = project.join("stage");
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::console;
use omega::language::std::filesystem_host;

target {target} {{}}

data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

data Stager {{
    fs: FilesystemHost;
    log: Console;
    fd: i32;
    n: i64;
}}

machine Stager::build(&mut self, b: &mut Build)
reaches
    FilesystemHost + Console
{{
    b.roots.bind({root_owner}::ProgramEntry, Main::main);
    self.log.write_line("build: staging");
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
            // Forward slashes so the embedded path lexes on windows too
            // (`C:\Users\...` would read `\U` as an escape sequence); every
            // host fs API accepts them.
            stage = stage.display().to_string().replace('\\', "/"),
            target = profile.target_name(),
            root_owner = profile.root_slot_owner_name(),
        ),
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::std::console;
data Main { console: Console; }
machine Main::main(&mut self) { self.console.exit_process(70); }
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("checked build evaluation should succeed");
    assert_eq!(checked.selected_program_entry_machine(), Some("Main::main"));
    let checked_usage = checked
        .build_evaluation_usage()
        .expect("build machine evaluation must publish precursor usage");
    assert_eq!(checked_usage.usage_schema_version, 1);
    assert_eq!(checked_usage.step_schedule_marker, 1);
    assert!(checked_usage.fuel_units > 0);
    assert!(checked_usage.result_cells > 0);

    let report = compile(CompileOptions {
        root_path: PathBuf::from(project.join("main.omg")),
        build_dir: Some(build_dir.clone()),
        target_name: Some(profile.target_name().to_owned()),
        write_output: true,
    })
    .expect("declared filesystem+console build.omg should compile (console rows are SERVED, not backstopped)");
    assert!(report.wrote_output());
    assert_eq!(report.build_evaluation_usage, Some(checked_usage));

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
