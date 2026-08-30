use super::*;

#[test]
fn baseline_retains_operand_bearing_descriptor_failure_and_errno_replay_custody() {
    let fixtures = [
        (
            "unknown-seek-errno",
            r#"let position: i64 = builder.filesystem.seek(-1, -17, 2);
    let error: i32 = builder.filesystem.errno();"#,
            10,
        ),
        (
            "unknown-open-at-errno",
            r#"let descriptor: i32 = builder.filesystem.open_at(-1, "generated.omg", 577);
    let error: i32 = builder.filesystem.errno();"#,
            14,
        ),
        (
            "unknown-read-dir-errno",
            r#"let mut buffer: [u8; 47];
    buffer[0] = 11;
    buffer[46] = 173;
    let mut position: i64 = -19;
    let count: i64 = builder.filesystem.read_dir(-1, &mut buffer, 31, &mut position);
    let error: i32 = builder.filesystem.errno();"#,
            23,
        ),
        (
            "unknown-set-file-times-errno",
            r#"let mut times: [u8; 32];
    times[0] = 11;
    times[16] = 29;
    times[31] = 173;
    let status: i32 = builder.filesystem.set_file_times(-1, &mut times);
    let error: i32 = builder.filesystem.errno();"#,
            42,
        ),
        (
            "unknown-write-at-errno",
            r#"let mut payload: [u8; 47];
    payload[0] = 11;
    payload[23] = 29;
    payload[46] = 173;
    let count: i64 = builder.filesystem.write_at(-1, &payload, -17);
    let error: i32 = builder.filesystem.errno();"#,
            7,
        ),
        (
            "unknown-read-file-metadata-errno",
            r#"let mut buffer: [u8; 144];
    buffer[0] = 11;
    buffer[71] = 29;
    buffer[143] = 173;
    let status: i32 = builder.filesystem.read_file_metadata(-1, &mut buffer);
    let error: i32 = builder.filesystem.errno();"#,
            39,
        ),
    ];

    for (label, statements, failure_tag) in fixtures {
        let replay = unknown_descriptor_failure_baseline(label, statements);
        let [failure, errno] = replay.attempts() else {
            panic!("descriptor failure and errno baseline retains exactly two operations")
        };
        assert_eq!(failure.operation_tag(), failure_tag);
        assert_eq!(
            failure.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                -1
            ))
        );
        assert_eq!(failure.post_error(), Some(9));
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
        assert_exact_baseline_failure_operands(failure_tag, failure);
        assert_errno_attempt(errno);
        assert!(!replay.has_output_attempts());
    }
}

fn assert_exact_baseline_failure_operands(
    operation_tag: u16,
    failure: &psi_checked_interpreter::FilesystemOperationAttempt,
) {
    use psi_checked_interpreter::FilesystemScalarOperandValue;

    match operation_tag {
        10 => assert_eq!(
            failure
                .scalar_operands()
                .iter()
                .map(|operand| (operand.operand_ordinal(), operand.value()))
                .collect::<Vec<_>>(),
            vec![
                (1, FilesystemScalarOperandValue::I64(-17)),
                (2, FilesystemScalarOperandValue::I32(2))
            ]
        ),
        14 => {
            let [component] = failure.byte_operands() else {
                panic!("open_at baseline retains one exact relative component")
            };
            assert_eq!(component.operand_ordinal(), 1);
            assert_eq!(component.bytes(), b"generated.omg");
            let [flags] = failure.scalar_operands() else {
                panic!("open_at baseline retains one exact flags operand")
            };
            assert_eq!(flags.value(), FilesystemScalarOperandValue::I32(577));
        }
        23 => {
            let [count] = failure.scalar_operands() else {
                panic!("read_dir baseline retains one exact requested count")
            };
            assert_eq!(count.value(), FilesystemScalarOperandValue::U64(31));
            let [buffer] = failure.mutable_byte_operand_resolutions() else {
                panic!("read_dir baseline retains one exact buffer carrier")
            };
            assert_eq!(buffer.bytes().len(), 47);
            assert_eq!((buffer.bytes()[0], buffer.bytes()[46]), (11, 173));
            let [position] = failure.mutable_i64_operand_resolutions() else {
                panic!("read_dir baseline retains one exact position carrier")
            };
            assert_eq!(position.value(), -19);
        }
        42 => {
            let [times] = failure.mutable_byte_operand_resolutions() else {
                panic!("set_file_times baseline retains one exact time carrier")
            };
            assert_eq!(times.bytes().len(), 32);
            assert_eq!(
                (times.bytes()[0], times.bytes()[16], times.bytes()[31]),
                (11, 29, 173)
            );
        }
        7 => {
            let [payload] = failure.byte_operands() else {
                panic!("write_at baseline retains one exact payload")
            };
            assert_eq!(payload.bytes().len(), 47);
            assert_eq!(
                (payload.bytes()[0], payload.bytes()[23], payload.bytes()[46]),
                (11, 29, 173)
            );
            let [offset] = failure.scalar_operands() else {
                panic!("write_at baseline retains one exact offset")
            };
            assert_eq!(offset.value(), FilesystemScalarOperandValue::I64(-17));
        }
        39 => {
            let [metadata] = failure.mutable_byte_operand_resolutions() else {
                panic!("read_file_metadata baseline retains one exact metadata carrier")
            };
            assert_eq!(metadata.bytes().len(), 144);
            assert_eq!(
                (
                    metadata.bytes()[0],
                    metadata.bytes()[71],
                    metadata.bytes()[143]
                ),
                (11, 29, 173)
            );
        }
        _ => panic!("unexpected representative descriptor failure tag {operation_tag}"),
    }
}

fn assert_errno_attempt(errno: &psi_checked_interpreter::FilesystemOperationAttempt) {
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
}
