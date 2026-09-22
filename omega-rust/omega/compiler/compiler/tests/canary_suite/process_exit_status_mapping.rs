//! Source-produced canonical `ProcessExit::exit_process` status mapping: the
//! semantic `i32` status stays exact through the interpreter observation and
//! retained boundary custody, while a hosted target's physical presentation
//! keeps only the low byte. A host exposing fewer status bits never clamps or
//! substitutes the recorded semantic status (spec: the exit event preserves
//! the exact semantic `i32` status; physical presentation is the target
//! contract's).

use super::{
    CheckedCompileRequest, compile_reviewed_repository_fixture,
    compile_rooted_backend_canary_without_output_for_target, interpret, native_hosted_target,
    pass_canary,
};
/// Taken-branch semantic status: negative `i32`, presented by a Unix host as
/// its low byte (212). The untaken helper retains 300, so neither branch's
/// status fits the presented range and the two custody rows cannot alias.
const SEMANTIC_STATUS: i32 = -44;
#[cfg(unix)]
const HOST_PRESENTED_STATUS: i32 = SEMANTIC_STATUS & 255;

const STATUS_MAPPED_CANARY: &str = "host/process_exit_i32_status_mapped";

#[test]
fn process_exit_status_mapping_canary_interprets_exact_i32() {
    let canary = pass_canary(STATUS_MAPPED_CANARY);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("canonical ProcessExit status-mapping canary should reach checked trees");

    // The simulated domain ends with the exact semantic i32 status: -44, not
    // the 212 a Unix host presents. `exit_code` is the terminal observation,
    // not the physical encoding.
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "canonical ProcessExit should halt the simulated domain cleanly"
    );
    assert_eq!(outcome.exit_code, SEMANTIC_STATUS);
    assert!(outcome.stdout.is_empty());
}

#[test]
fn process_exit_status_mapping_realizes_custody_and_host_low_byte() {
    let canary = pass_canary(STATUS_MAPPED_CANARY);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{target} canonical ProcessExit status mapping reaches hosted exit: {diagnostics:#?}"
                )
            });
        let artifact = report
            .retained_native_artifact()
            .expect("source compilation retains native custody");
        let settlements = artifact.image().boundary_settlements();
        let exit_settlements = settlements
            .iter()
            .filter(|settlement| {
                settlement.settlement.execution
                    == native_realization::BoundaryExecutionRecord::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                    )
            })
            .collect::<Vec<_>>();
        // Each conditional exit helper settles its own exact boundary row:
        // `good` (semantic -44, presented 212) and `bad` (semantic 300,
        // presented 44) keep independent argument custody rather than sharing
        // a manufactured terminal tail.
        let [good_settlement, bad_settlement] = exit_settlements.as_slice() else {
            panic!(
                "one exact hosted exit boundary per conditional branch for {target}; \
                 settlements: {:#?}",
                settlements
                    .iter()
                    .map(|settlement| format!("{:?}", settlement.settlement))
                    .collect::<Vec<_>>()
            );
        };
        for settlement in [good_settlement, bad_settlement] {
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
        }
        let selected_source_value =
            |settlement: &machine_code::BoundarySettlementRecord| match settlement
                .runtime_scalar_arguments[0]
                .source
            {
                machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                    source_value,
                    ..
                } => source_value,
                ref other => panic!("non-ProcessExit exit argument custody: {other:?}"),
            };
        assert_ne!(
            selected_source_value(&good_settlement.settlement),
            selected_source_value(&bad_settlement.settlement),
            "each exit branch retains its own exact status custody for {target}"
        );
        assert!(artifact.physical_evidence().is_some());
        assert_host_low_byte_exit(target, &artifact.image().output().bytes);
    }
}

/// Execute the retained image on the matching host and require the physical
/// status to be the exact semantic status's low byte. This is the target
/// contract's presentation of an unchanged semantic argument; on a non-Unix
/// or mismatched host the run is an explicit skip rather than silent pass.
fn assert_host_low_byte_exit(target: &str, bytes: &[u8]) {
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
            "omega-source-exit-status-mapped-{}-{}",
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
            Some(HOST_PRESENTED_STATUS),
            "{target}: the host presents the exact semantic status {SEMANTIC_STATUS} \
             as its low byte, got {:?}",
            output.status
        );
        assert!(
            output.stdout.is_empty(),
            "{target}: this ProcessExit-only receiver produces no output"
        );
        assert!(output.stderr.is_empty());
    }
    #[cfg(not(unix))]
    let _ = bytes;
}

#[cfg(unix)]
struct ExecutableFile(PathBuf);
#[cfg(unix)]
impl Drop for ExecutableFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
