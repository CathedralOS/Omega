use omega_compiler::{compile, CompileOptions};
use std::fs;
use std::path::{Path, PathBuf};

const HOSTED_TARGETS: &[&str] = &["windows_x64", "linux_x64", "linux_arm64", "macos_arm64"];
const SAMPLES: &[&str] = &[
    "samples/cli/rendering/bouncing_ball_2d/main.omg",
    "samples/cli/simulation/particle_sim/main.omg",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under compiler/omega-rs/orchestration/omega-compiler")
        .to_path_buf()
}

#[test]
fn mutable_float_machine_fields_lower_for_every_hosted_target() {
    for relative in SAMPLES {
        let main_path = repo_root().join(relative);
        for target in HOSTED_TARGETS {
            let build_dir = std::env::temp_dir().join(format!(
                "omega-mutable-float-machine-fields-{target}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&build_dir);
            compile(CompileOptions {
                root_path: main_path.clone(),
                build_dir: Some(build_dir.clone()),
                target_name: Some((*target).to_owned()),
                write_output: false,
            })
            .unwrap_or_else(|diagnostics| {
                panic!("{relative}/{target} should lower directly: {diagnostics:#?}")
            });
            let _ = fs::remove_dir_all(build_dir);
        }
    }
}
