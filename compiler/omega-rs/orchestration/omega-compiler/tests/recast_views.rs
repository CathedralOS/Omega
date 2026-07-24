//! Focused native canaries for programmable-layout recast views.
//!
//! These live outside the monolithic canary suite so each new view rung can
//! carry its own end-to-end oracle without making that shared file responsible
//! for another subsystem.

use omega_compiler::{CompileOptions, compile, compile_to_checked};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-compiler lives under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

fn compile_and_run(canary_rel: &str, tag: &str) -> std::process::Output {
    let canary = repo_root().join("canaries/pass").join(canary_rel);
    let build_dir = std::env::temp_dir().join(format!("omega-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|diagnostics| panic!("{canary_rel} should compile:\n{diagnostics:#?}"));

    let output = Command::new(build_dir.join("omega-program"))
        .output()
        .expect("canary should run");
    let _ = std::fs::remove_dir_all(&build_dir);
    output
}

#[test]
fn mutable_recast_accepts_bidirectionally_equivalent_domain_facts() {
    let canary = "recast/runtime_mutable_equivalent_domain_recast_exit";
    let output = compile_and_run(canary, "mutable-equivalent-domain-recast");
    assert_eq!(
        output.status.code(),
        Some(70),
        "{canary} expected exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn mutable_recast_rejects_equal_looking_cross_carrier_domains() {
    let canary = repo_root()
        .join("canaries/fail/recast/recast_mut_cross_carrier_domain_not_equivalent/main.omg");
    let diagnostics = compile_to_checked(&canary, None)
        .expect_err("unproved cross-carrier domain equivalence must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("source and target constraints are not proven representation-equivalent"),
        "wrong cross-carrier recast diagnostic:\n{combined}"
    );
}
