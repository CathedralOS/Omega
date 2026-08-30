use super::*;

#[path = "descriptor_error_state_tests/operand_bearing_errno_tests.rs"]
mod operand_bearing_errno_tests;

#[test]
fn baseline_retains_ordered_unknown_descriptor_failure_and_errno_replay_custody() {
    let fixtures = [
        (
            "unknown-close-errno",
            r#"let status: i32 = builder.filesystem.close(-1);
    let error: i32 = builder.filesystem.errno();"#,
            8,
        ),
        (
            "unknown-sync-errno",
            r#"let status: i32 = builder.filesystem.sync(-1);
    let error: i32 = builder.filesystem.errno();"#,
            43,
        ),
        (
            "unknown-sync-data-errno",
            r#"let status: i32 = builder.filesystem.sync_data(-1);
    let error: i32 = builder.filesystem.errno();"#,
            44,
        ),
        (
            "unknown-duplicate-errno",
            r#"let status: i32 = builder.filesystem.duplicate(-1);
    let error: i32 = builder.filesystem.errno();"#,
            45,
        ),
    ];

    for (label, statements, failure_tag) in fixtures {
        let replay = unknown_descriptor_failure_baseline(label, statements);
        let [failure, errno] = replay.attempts() else {
            panic!("descriptor failure and errno baseline retains exactly two operations")
        };
        assert_eq!(failure.operation_tag(), failure_tag);
        assert_eq!(
            failure.provider(),
            psi_checked_interpreter::FilesystemObservationProvider::RealScoped
        );
        assert_eq!(
            failure.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                -1
            ))
        );
        assert_eq!(failure.post_error(), Some(9));
        assert!(failure.scalar_operands().is_empty());
        assert!(failure.byte_operands().is_empty());
        assert!(failure.path_like_operands().is_empty());
        assert!(failure.rooted_path_operand_resolutions().is_empty());
        assert!(failure.returned_paths().is_empty());
        assert!(failure.observed_byte_regions().is_empty());
        assert!(failure.metadata_observations().is_empty());
        assert!(failure.mutable_byte_operand_resolutions().is_empty());
        assert!(failure.mutable_i64_operand_resolutions().is_empty());
        assert!(failure.mutable_byte_operands().is_empty());
        assert!(failure.mutable_i64_operands().is_empty());
        assert!(failure.authorized_paths().is_empty());
        let [descriptor] = failure.logical_handle_inputs() else {
            panic!("ordered descriptor failure retains one descriptor input")
        };
        assert_eq!(descriptor.operand_ordinal(), 0);
        assert_eq!(
            descriptor.kind(),
            psi_checked_interpreter::FilesystemLogicalHandleKind::Descriptor
        );
        assert_eq!(
            descriptor.resolution(),
            psi_checked_interpreter::FilesystemLogicalHandleInputResolution::Unknown
        );
        assert!(failure.logical_handle_output().is_none());
        assert!(failure.retired_logical_handles().is_empty());
        assert!(failure.grant_refusals().is_empty());

        assert_eq!(errno.operation_tag(), 50);
        assert_eq!(
            errno.provider(),
            psi_checked_interpreter::FilesystemObservationProvider::RealScoped
        );
        assert_eq!(
            errno.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                9
            ))
        );
        assert_eq!(errno.post_error(), Some(9));
        assert_empty_attempt_lanes(errno);
        assert!(!replay.has_output_attempts());
    }
}

fn assert_empty_attempt_lanes(attempt: &psi_checked_interpreter::FilesystemOperationAttempt) {
    assert!(attempt.scalar_operands().is_empty());
    assert!(attempt.byte_operands().is_empty());
    assert!(attempt.path_like_operands().is_empty());
    assert!(attempt.rooted_path_operand_resolutions().is_empty());
    assert!(attempt.returned_paths().is_empty());
    assert!(attempt.observed_byte_regions().is_empty());
    assert!(attempt.metadata_observations().is_empty());
    assert!(attempt.mutable_byte_operand_resolutions().is_empty());
    assert!(attempt.mutable_i64_operand_resolutions().is_empty());
    assert!(attempt.mutable_byte_operands().is_empty());
    assert!(attempt.mutable_i64_operands().is_empty());
    assert!(attempt.authorized_paths().is_empty());
    assert!(attempt.logical_handle_inputs().is_empty());
    assert!(attempt.logical_handle_output().is_none());
    assert!(attempt.retired_logical_handles().is_empty());
    assert!(attempt.grant_refusals().is_empty());
}
