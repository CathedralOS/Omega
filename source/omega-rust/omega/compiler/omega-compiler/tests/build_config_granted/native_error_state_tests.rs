use super::*;

#[test]
fn unknown_native_handle_mutation_then_get_last_error_replays_as_one_ordered_receipt() {
    let fixtures = [
        (
            "set-file-time-invalid-handle-last-error",
            r#"    self.buffer[0] = 11;
    self.buffer[4095] = 173;
    self.times[0] = 29;
    self.times[31] = 197;
    self.code = self.filesystem.set_file_time(-1, 37, &self.buffer, &self.times);
    self.code = self.filesystem.get_last_error();"#,
            vec![32, 35],
        ),
        (
            "lock-file-ex-invalid-handle-last-error-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.times[0] = 41;
    self.times[31] = 211;
    self.code = self.filesystem.lock_file_ex(-1, 1, 0, 4294967295, 4294967295, &mut self.times);
    self.code = self.filesystem.get_last_error();"#,
            vec![2, 4, 8, 33, 35],
        ),
        (
            "unlock-file-invalid-handle-last-error",
            r#"    self.code = self.filesystem.unlock_file(-1, 3, 5, 7, 11);
    self.code = self.filesystem.get_last_error();"#,
            vec![34, 35],
        ),
    ];

    for (label, body, operation_tags) in fixtures {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-native-handle mutation and last-error read should compile");
        let summary = compilation
            .build_observation_summary()
            .expect("ordered native failure and last-error read retain observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("ordered failure receipt retains empty Output custody")
                .entry_count(),
            0
        );
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );

        let attempts = summary.filesystem_operation_attempts();
        let mutation = &attempts[attempts.len() - 2];
        let last_error = &attempts[attempts.len() - 1];
        assert!(matches!(mutation.operation_tag(), 32 | 33 | 34));
        assert_eq!(mutation.provider(), BuildFilesystemProvider::RealScoped);
        assert_eq!(mutation.result(), BuildFilesystemOperationResult::Scalar(0));
        assert_eq!(mutation.post_error(), 6);
        let [handle] = mutation.logical_handle_inputs() else {
            panic!("failed native mutation retains one native-handle input")
        };
        assert_eq!(handle.operand_ordinal(), 0);
        assert_eq!(handle.kind(), BuildFilesystemLogicalHandleKind::Native);
        assert_eq!(
            handle.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );

        assert_eq!(last_error.operation_tag(), 35);
        assert_eq!(last_error.provider(), BuildFilesystemProvider::RealScoped);
        assert_eq!(
            last_error.result(),
            BuildFilesystemOperationResult::Scalar(6)
        );
        assert_eq!(last_error.post_error(), 6);
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

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified ordered native failure must encode")
            .expect("ordered native failure and last-error read retain review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical ordered native failure record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after ordered native failure capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("ordered native failure replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed ordered native failure retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn standalone_get_last_error_remains_outside_complete_replay_grammar() {
    let (project, profile) = rooted_build_probe_project(
        "standalone-get-last-error",
        "    self.code = self.filesystem.get_last_error();",
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("standalone last-error observation remains valid build code");
    let summary = compilation
        .build_observation_summary()
        .expect("standalone last-error read retains observations");
    assert!(!summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Volatile);
    let [last_error] = summary.filesystem_operation_attempts() else {
        panic!("standalone last-error fixture retains exactly one operation")
    };
    assert_eq!(last_error.operation_tag(), 35);

    let _ = std::fs::remove_dir_all(project);
}
