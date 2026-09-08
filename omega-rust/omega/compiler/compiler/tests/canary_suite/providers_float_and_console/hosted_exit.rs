//! Source-produced i32 exit operands and execution of the retained native image.
use super::*;

#[test]
fn hosted_exit_normalizes_source_i32_status_through_selected_custody() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_EXIT_I32_STATUS);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!("{target} source i32 status reaches hosted exit: {diagnostics:#?}")
            });
        let artifact = report
            .retained_native_artifact()
            .expect("source compilation retains native custody");
        let [settlement] = artifact.image().boundary_settlements() else {
            panic!("one exact hosted exit boundary for {target}");
        };
        assert_eq!(
            settlement.settlement.execution,
            native_realization::BoundaryExecutionRecord::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedExitProcessI32
            )
        );
        assert!(settlement.settlement.native_result.is_unit());
        assert_eq!(settlement.settlement.runtime_scalar_arguments.len(), 1);
        assert!(matches!(
            settlement.settlement.runtime_scalar_arguments[0].source,
            machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
        ));
        assert!(
            settlement.settlement.scalar_arguments.is_empty(),
            "the scalar definition is an actual runtime carrier, not a manufactured immediate"
        );
        assert!(artifact.physical_evidence().is_some());
        assert_matching_host_exit(target, &artifact.image().output().bytes, 37);
    }
}

pub(super) fn assert_matching_host_exit(target: &str, bytes: &[u8], status: i32) {
    if target != native_hosted_target() {
        eprintln!(
            "SKIP: source-produced {target} executable cannot run on {}",
            native_hosted_target()
        );
        return;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omega-source-hosted-exit-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("create unique source-produced executable");
        let cleanup = ExecutableFile(path);
        file.write_all(bytes)
            .expect("write exact compiler-produced executable");
        file.set_permissions(fs::Permissions::from_mode(0o700))
            .unwrap();
        drop(file);
        // No C replacement, external signing, or executable-byte mutation.
        let output = Command::new(&cleanup.0)
            .output()
            .expect("run source-produced hosted exit");
        assert_eq!(
            output.status.code(),
            Some(status & 255),
            "{target}: {:?}",
            output.status
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    #[cfg(not(unix))]
    let _ = (bytes, status);
}

#[cfg(unix)]
struct ExecutableFile(PathBuf);
#[cfg(unix)]
impl Drop for ExecutableFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
