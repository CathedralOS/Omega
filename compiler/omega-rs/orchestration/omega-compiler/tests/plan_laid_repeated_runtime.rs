//! End-to-end oracle for a plan-laid outer fixed array whose validated
//! destinations retain a constant physical stride larger than element width.

use omega_compiler::{CompileOptions, compile, compile_to_checked};
use psi_checked_interpreter::interpret_entry;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CANARY: &str = "layouts/runtime_plan_laid_tiled_outer_array_view_exit";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-compiler lives under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

struct TemporaryBuildDirectory(PathBuf);

impl TemporaryBuildDirectory {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryBuildDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unique_build_dir(tag: &str) -> TemporaryBuildDirectory {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should follow the Unix epoch")
        .as_nanos();
    TemporaryBuildDirectory(
        std::env::temp_dir().join(format!("omega-{tag}-{}-{nonce}", std::process::id())),
    )
}

#[test]
fn gapped_outer_array_interprets_runs_natively_and_cross_compiles() {
    let canary = repo_root().join("canaries/pass").join(CANARY);
    let host = omega_target::TargetProfile::host();
    let checked = compile_to_checked(&canary.join("main.omg"), Some(host.target_name()))
        .expect("gapped outer-array canary should reach checked trees");
    let interpreted = interpret_entry(
        &checked,
        checked
            .selected_program_entry_machine()
            .expect("gapped outer-array canary selects an exact ProgramEntry"),
        &[],
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "interpreter must preserve the validated element stride: {interpreted:?}"
    );

    let host_build = unique_build_dir("plan-laid-gapped-host");
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(host_build.path().to_path_buf()),
        target_name: Some(host.target_name().to_owned()),
        write_output: true,
    })
    .unwrap_or_else(|diagnostics| panic!("host compile should succeed:\n{diagnostics:#?}"));
    let executable = if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    };
    let output = Command::new(host_build.path().join(executable))
        .output()
        .expect("gapped outer-array executable should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native gapped outer-array canary failed; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let host_build_path = host_build.path().to_path_buf();
    drop(host_build);
    assert!(
        !host_build_path.exists(),
        "host build directory should be removed after execution"
    );

    for target in ["windows_x64", "linux_arm64"] {
        let cross_build = unique_build_dir(&format!("plan-laid-gapped-{target}"));
        compile(CompileOptions {
            root_path: canary.join("main.omg"),
            build_dir: Some(cross_build.path().to_path_buf()),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| panic!("{target} compile should succeed:\n{diagnostics:#?}"));
        let cross_build_path = cross_build.path().to_path_buf();
        drop(cross_build);
        assert!(
            !cross_build_path.exists(),
            "{target} build directory should be removed after compilation"
        );
    }
}
