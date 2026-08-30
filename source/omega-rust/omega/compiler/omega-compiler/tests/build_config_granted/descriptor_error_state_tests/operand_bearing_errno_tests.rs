use super::*;

#[test]
fn operand_bearing_unknown_descriptor_failures_then_errno_replay_without_a_provider() {
    let fixtures = [
        (
            "seek-ebadf-errno",
            r#"    self.position = self.filesystem.seek(-1, -17, 2);
    self.code = self.filesystem.errno();"#,
            10,
        ),
        (
            "open-at-ebadf-errno",
            r#"    self.descriptor = self.filesystem.open_at(-1, "generated.omg", 577);
    self.code = self.filesystem.errno();"#,
            14,
        ),
        (
            "read-dir-ebadf-errno",
            r#"    self.position = -19;
    self.result = self.filesystem.read_dir(-1, &mut self.buffer, 31, &mut self.position);
    self.code = self.filesystem.errno();"#,
            23,
        ),
        (
            "set-file-times-ebadf-errno",
            r#"    self.times[0] = 11;
    self.times[16] = 29;
    self.times[31] = 173;
    self.code = self.filesystem.set_file_times(-1, &mut self.times);
    self.code = self.filesystem.errno();"#,
            42,
        ),
        (
            "write-at-ebadf-errno",
            r#"    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[4095] = 173;
    self.result = self.filesystem.write_at(-1, &self.buffer, -17);
    self.code = self.filesystem.errno();"#,
            7,
        ),
        (
            "read-file-metadata-ebadf-errno",
            r#"    self.buffer[0] = 11;
    self.buffer[71] = 29;
    self.buffer[4095] = 173;
    self.code = self.filesystem.read_file_metadata(-1, &mut self.buffer);
    self.code = self.filesystem.errno();"#,
            39,
        ),
    ];

    for (label, body, failure_tag) in fixtures {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("operand-bearing descriptor failure and immediate errno should compile");
        let summary = compilation
            .build_observation_summary()
            .expect("ordered descriptor failure and errno retain observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert!(summary.included_source_handoffs().is_empty());
        let [failure, errno] = summary.filesystem_operation_attempts() else {
            panic!("descriptor failure and errno fixture retains exactly two operations")
        };
        assert_eq!(failure.operation_tag(), failure_tag);
        assert_eq!(failure.provider(), BuildFilesystemProvider::RealScoped);
        assert_eq!(failure.result(), BuildFilesystemOperationResult::Scalar(-1));
        assert_eq!(failure.post_error(), 9);
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
        assert_exact_representative_failure_operands(failure_tag, failure);
        assert_operand_free_errno_attempt(errno);

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified descriptor failure and errno must encode")
            .expect("verified descriptor failure and errno retain replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical descriptor failure and errno record must recover");
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
        .expect("descriptor failure and errno replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed descriptor failure retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn get_osfhandle_then_errno_is_not_an_ebadf_error_state_receipt() {
    let (project, profile) = rooted_build_probe_project(
        "get-osfhandle-errno-excluded",
        r#"    self.result = self.filesystem.get_osfhandle(-1);
    self.code = self.filesystem.errno();"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("get_osfhandle and errno remain valid build code");
    let summary = compilation
        .build_observation_summary()
        .expect("get_osfhandle and errno retain observations");
    assert!(!summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Volatile);
    let [get_osfhandle, errno] = summary.filesystem_operation_attempts() else {
        panic!("get_osfhandle and errno fixture retains exactly two operations")
    };
    assert_eq!(get_osfhandle.operation_tag(), 30);
    assert_eq!(
        get_osfhandle.result(),
        BuildFilesystemOperationResult::Scalar(-2)
    );
    assert_eq!(get_osfhandle.post_error(), 0);
    assert_eq!(errno.operation_tag(), 50);
    assert_eq!(errno.result(), BuildFilesystemOperationResult::Scalar(0));
    assert_eq!(errno.post_error(), 0);

    let _ = std::fs::remove_dir_all(project);
}

fn assert_exact_representative_failure_operands(
    operation_tag: u16,
    failure: &omega_build_evaluation::BuildFilesystemOperationAttempt,
) {
    match operation_tag {
        10 => assert_eq!(
            failure
                .scalar_operands()
                .iter()
                .map(|operand| (operand.operand_ordinal(), operand.value()))
                .collect::<Vec<_>>(),
            vec![
                (1, BuildFilesystemScalarOperandValue::I64(-17)),
                (2, BuildFilesystemScalarOperandValue::I32(2)),
            ]
        ),
        14 => {
            let [component] = failure.byte_operands() else {
                panic!("open_at retains one exact relative component")
            };
            assert_eq!(component.operand_ordinal(), 1);
            assert_eq!(component.bytes(), b"generated.omg");
            let [flags] = failure.scalar_operands() else {
                panic!("open_at retains one exact flags operand")
            };
            assert_eq!(flags.operand_ordinal(), 2);
            assert_eq!(flags.value(), BuildFilesystemScalarOperandValue::I32(577));
        }
        23 => {
            let [count] = failure.scalar_operands() else {
                panic!("read_dir retains one exact requested count")
            };
            assert_eq!(count.value(), BuildFilesystemScalarOperandValue::U64(31));
            let [buffer] = failure.mutable_byte_operand_resolutions() else {
                panic!("read_dir retains one exact buffer carrier")
            };
            assert_eq!(buffer.bytes().len(), 4096);
            let [position] = failure.mutable_i64_operand_resolutions() else {
                panic!("read_dir retains one exact position carrier")
            };
            assert_eq!(position.value(), -19);
        }
        42 => {
            let [times] = failure.mutable_byte_operand_resolutions() else {
                panic!("set_file_times retains one exact time carrier")
            };
            assert_eq!(times.bytes().len(), 32);
            assert_eq!(
                (times.bytes()[0], times.bytes()[16], times.bytes()[31]),
                (11, 29, 173)
            );
        }
        7 => {
            let [payload] = failure.byte_operands() else {
                panic!("write_at retains one exact payload")
            };
            assert_eq!(payload.bytes().len(), 4096);
            assert_eq!(
                (
                    payload.bytes()[0],
                    payload.bytes()[23],
                    payload.bytes()[4095]
                ),
                (11, 29, 173)
            );
            let [offset] = failure.scalar_operands() else {
                panic!("write_at retains one exact offset")
            };
            assert_eq!(offset.value(), BuildFilesystemScalarOperandValue::I64(-17));
        }
        39 => {
            let [metadata] = failure.mutable_byte_operand_resolutions() else {
                panic!("read_file_metadata retains one exact metadata carrier")
            };
            assert_eq!(metadata.bytes().len(), 4096);
            assert_eq!(
                (
                    metadata.bytes()[0],
                    metadata.bytes()[71],
                    metadata.bytes()[4095]
                ),
                (11, 29, 173)
            );
        }
        _ => panic!("unexpected representative descriptor failure tag {operation_tag}"),
    }
}
