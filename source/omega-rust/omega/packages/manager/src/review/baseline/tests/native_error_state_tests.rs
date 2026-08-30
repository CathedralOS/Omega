use super::*;

#[test]
fn baseline_retains_ordered_native_handle_mutation_and_last_error_replay_custody() {
    let fixtures = [
        (
            "unknown-set-file-time-last-error",
            r#"let mut access: [u8; 8];
    let mut last_write: [u8; 12];
    access[0] = 11;
    last_write[11] = 29;
    let status: i32 = builder.filesystem.set_file_time(-1, 37, &access, &last_write);
    let error: i32 = builder.filesystem.get_last_error();"#,
            32,
        ),
        (
            "unknown-lock-file-ex-last-error",
            r#"let mut overlapped: [u8; 40];
    overlapped[0] = 41;
    overlapped[39] = 173;
    let status: i32 = builder.filesystem.lock_file_ex(-1, 1, 0, 4294967295, 4294967295, &mut overlapped);
    let error: i32 = builder.filesystem.get_last_error();"#,
            33,
        ),
        (
            "unknown-unlock-file-last-error",
            r#"let status: i32 = builder.filesystem.unlock_file(-1, 3, 5, 7, 11);
    let error: i32 = builder.filesystem.get_last_error();"#,
            34,
        ),
    ];

    for (label, statements, mutation_tag) in fixtures {
        let replay = unknown_descriptor_failure_baseline(label, statements);
        let [mutation, last_error] = replay.attempts() else {
            panic!("native mutation and last-error baseline retains exactly two operations")
        };
        assert_eq!(mutation.operation_tag(), mutation_tag);
        assert_eq!(
            mutation.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                0
            ))
        );
        assert_eq!(mutation.post_error(), Some(6));
        let [handle] = mutation.logical_handle_inputs() else {
            panic!("ordered native mutation baseline retains one native-handle input")
        };
        assert_eq!(
            handle.kind(),
            psi_checked_interpreter::FilesystemLogicalHandleKind::Native
        );
        assert_eq!(
            handle.resolution(),
            psi_checked_interpreter::FilesystemLogicalHandleInputResolution::Unknown
        );

        assert_eq!(last_error.operation_tag(), 35);
        assert_eq!(
            last_error.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                6
            ))
        );
        assert_eq!(last_error.post_error(), Some(6));
        assert!(last_error.scalar_operands().is_empty());
        assert!(last_error.byte_operands().is_empty());
        assert!(last_error.path_like_operands().is_empty());
        assert!(last_error.rooted_path_operand_resolutions().is_empty());
        assert!(last_error.returned_paths().is_empty());
        assert!(last_error.observed_byte_regions().is_empty());
        assert!(last_error.metadata_observations().is_empty());
        assert!(last_error.mutable_byte_operand_resolutions().is_empty());
        assert!(last_error.mutable_i64_operand_resolutions().is_empty());
        assert!(last_error.mutable_byte_operands().is_empty());
        assert!(last_error.mutable_i64_operands().is_empty());
        assert!(last_error.authorized_paths().is_empty());
        assert!(last_error.logical_handle_inputs().is_empty());
        assert!(last_error.logical_handle_output().is_none());
        assert!(last_error.retired_logical_handles().is_empty());
        assert!(last_error.grant_refusals().is_empty());
        assert!(!replay.has_output_attempts());
    }
}
