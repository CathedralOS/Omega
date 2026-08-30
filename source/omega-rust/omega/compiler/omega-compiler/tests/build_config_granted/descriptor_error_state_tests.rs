use super::*;

#[path = "descriptor_error_state_tests/operand_bearing_errno_tests.rs"]
mod operand_bearing_errno_tests;

#[test]
fn unknown_descriptor_failure_then_errno_replays_as_one_ordered_receipt() {
    let fixtures = [
        (
            "close-ebadf-errno",
            r#"    self.code = self.filesystem.close(-1);
    self.code = self.filesystem.errno();"#,
            vec![8, 50],
        ),
        (
            "sync-ebadf-errno-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.code = self.filesystem.sync(-1);
    self.code = self.filesystem.errno();"#,
            vec![2, 4, 8, 43, 50],
        ),
        (
            "sync-data-ebadf-errno",
            r#"    self.code = self.filesystem.sync_data(-1);
    self.code = self.filesystem.errno();"#,
            vec![44, 50],
        ),
        (
            "duplicate-ebadf-errno",
            r#"    self.code = self.filesystem.duplicate(-1);
    self.code = self.filesystem.errno();"#,
            vec![45, 50],
        ),
    ];

    for (label, body, operation_tags) in fixtures {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-descriptor failure and immediate errno read should compile");
        let summary = compilation
            .build_observation_summary()
            .expect("ordered descriptor failure and errno read retain observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("ordered descriptor failure retains empty Output custody")
                .entry_count(),
            0
        );
        assert!(summary.included_source_handoffs().is_empty());
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );

        let attempts = summary.filesystem_operation_attempts();
        let failure = &attempts[attempts.len() - 2];
        let errno = &attempts[attempts.len() - 1];
        assert!(matches!(failure.operation_tag(), 8 | 43 | 44 | 45));
        assert_eq!(failure.provider(), BuildFilesystemProvider::RealScoped);
        assert_eq!(failure.result(), BuildFilesystemOperationResult::Scalar(-1));
        assert_eq!(failure.post_error(), 9);
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
            panic!("failed descriptor operation retains one descriptor input")
        };
        assert_eq!(descriptor.operand_ordinal(), 0);
        assert_eq!(
            descriptor.kind(),
            BuildFilesystemLogicalHandleKind::Descriptor
        );
        assert_eq!(
            descriptor.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );
        assert!(failure.logical_handle_output().is_none());
        assert!(failure.retired_logical_handles().is_empty());
        assert!(failure.grant_refusals().is_empty());

        assert_operand_free_errno_attempt(errno);

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified ordered descriptor failure must encode")
            .expect("ordered descriptor failure and errno read retain review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical ordered descriptor failure record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after ordered descriptor failure capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("ordered descriptor failure replay must not invoke the host provider");
        let replayed_summary = replayed
            .build_observation_summary()
            .expect("replayed ordered descriptor failure retains observations");
        assert!(replayed_summary.included_source_handoffs().is_empty());
        assert_eq!(
            replayed_summary.filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn standalone_errno_remains_outside_complete_replay_grammar() {
    let (project, profile) = rooted_build_probe_project(
        "standalone-errno",
        "    self.code = self.filesystem.errno();",
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("standalone errno observation remains valid build code");
    let summary = compilation
        .build_observation_summary()
        .expect("standalone errno read retains observations");
    assert!(!summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Volatile);
    assert!(summary.included_source_handoffs().is_empty());
    let [errno] = summary.filesystem_operation_attempts() else {
        panic!("standalone errno fixture retains exactly one operation")
    };
    assert_eq!(errno.operation_tag(), 50);
    assert_eq!(errno.provider(), BuildFilesystemProvider::RealScoped);
    assert_empty_attempt_lanes(errno);

    let _ = std::fs::remove_dir_all(project);
}

fn assert_operand_free_errno_attempt(
    errno: &omega_build_evaluation::BuildFilesystemOperationAttempt,
) {
    assert_eq!(errno.operation_tag(), 50);
    assert_eq!(errno.provider(), BuildFilesystemProvider::RealScoped);
    assert_eq!(errno.result(), BuildFilesystemOperationResult::Scalar(9));
    assert_eq!(errno.post_error(), 9);
    assert_empty_attempt_lanes(errno);
}

fn assert_empty_attempt_lanes(attempt: &omega_build_evaluation::BuildFilesystemOperationAttempt) {
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
