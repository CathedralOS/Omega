use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemByteOperand,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemMutableByteOperand,
    BuildFilesystemMutableByteOperandResolution, BuildFilesystemReturnedPath,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind,
    BuildFilesystemScalarOperand, BuildObservationClass,
};

fn operand_free_unknown_descriptor_failure(operation_tag: u16) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag,
        provider: BuildFilesystemProvider::RealScoped,
        result: BuildFilesystemOperationResult::Scalar(-1),
        post_error: 9,
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Descriptor,
            resolution: BuildFilesystemLogicalHandleInputResolution::Unknown,
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

pub(super) fn summary(operation_tag: u16) -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: vec![operand_free_unknown_descriptor_failure(operation_tag)],
        canonical_source_metadata_identity: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::SourceInputsOnly,
        ),
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}

pub(super) fn unknown_descriptor_seek_summary(offset: i64, whence: i32) -> BuildObservationSummary {
    let mut summary = summary(10);
    summary.filesystem_operation_attempts[0].scalar_operands = vec![
        BuildFilesystemScalarOperand {
            operand_ordinal: 1,
            value: BuildFilesystemScalarOperandValue::I64(offset),
        },
        BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::I32(whence),
        },
    ];
    summary
}

pub(super) fn unknown_descriptor_at_summary(
    operation_tag: u16,
    relative_component: Vec<u8>,
    flags: i32,
) -> BuildObservationSummary {
    let mut summary = summary(operation_tag);
    summary.filesystem_operation_attempts[0].scalar_operands = vec![BuildFilesystemScalarOperand {
        operand_ordinal: 2,
        value: BuildFilesystemScalarOperandValue::I32(flags),
    }];
    summary.filesystem_operation_attempts[0].byte_operands = vec![BuildFilesystemByteOperand {
        operand_ordinal: 1,
        bytes: relative_component,
    }];
    summary
}

pub(super) fn unknown_descriptor_get_osfhandle_summary() -> BuildObservationSummary {
    let mut summary = summary(30);
    summary.filesystem_operation_attempts[0].result = BuildFilesystemOperationResult::Scalar(-2);
    summary.filesystem_operation_attempts[0].post_error = 0;
    summary
}

pub(super) fn unknown_native_handle_close_summary() -> BuildObservationSummary {
    let mut summary = summary(29);
    summary.filesystem_operation_attempts[0].result = BuildFilesystemOperationResult::Scalar(0);
    summary.filesystem_operation_attempts[0].post_error = 6;
    summary.filesystem_operation_attempts[0].logical_handle_inputs[0].kind =
        BuildFilesystemLogicalHandleKind::Native;
    summary
}

pub(super) fn unknown_native_handle_final_path_summary() -> BuildObservationSummary {
    let mut summary = summary(31);
    let carrier = vec![17; 47];
    summary.filesystem_operation_attempts[0].result = BuildFilesystemOperationResult::Scalar(0);
    summary.filesystem_operation_attempts[0].post_error = 6;
    summary.filesystem_operation_attempts[0].logical_handle_inputs[0].kind =
        BuildFilesystemLogicalHandleKind::Native;
    summary.filesystem_operation_attempts[0].scalar_operands = vec![
        BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::U64(47),
        },
        BuildFilesystemScalarOperand {
            operand_ordinal: 3,
            value: BuildFilesystemScalarOperandValue::U32(0),
        },
    ];
    summary.filesystem_operation_attempts[0].mutable_byte_operand_resolutions =
        vec![BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: carrier.clone(),
        }];
    summary.filesystem_operation_attempts[0].mutable_byte_operands =
        vec![BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: carrier.clone(),
            post_bytes: carrier,
        }];
    summary
}

pub(super) fn unknown_descriptor_write_summary(
    operation_tag: u16,
    values: &[BuildFilesystemScalarOperandValue],
) -> BuildObservationSummary {
    let mut summary = summary(operation_tag);
    summary.filesystem_operation_attempts[0].scalar_operands = values
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| BuildFilesystemScalarOperand {
            operand_ordinal: u8::try_from(index + 1).unwrap(),
            value,
        })
        .collect();
    summary
}

pub(super) fn unknown_descriptor_set_file_times_summary(times: Vec<u8>) -> BuildObservationSummary {
    let mut summary = summary(42);
    summary.filesystem_operation_attempts[0].mutable_byte_operand_resolutions =
        vec![BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: times.clone(),
        }];
    summary.filesystem_operation_attempts[0].mutable_byte_operands =
        vec![BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: times.clone(),
            post_bytes: times,
        }];
    summary
}

pub(super) fn unknown_descriptor_read_file_metadata_summary(
    buffer: Vec<u8>,
) -> BuildObservationSummary {
    let mut summary = summary(39);
    summary.filesystem_operation_attempts[0].mutable_byte_operand_resolutions =
        vec![BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: buffer.clone(),
        }];
    summary.filesystem_operation_attempts[0].mutable_byte_operands =
        vec![BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: buffer.clone(),
            post_bytes: buffer,
        }];
    summary
}

pub(super) fn unknown_descriptor_read_summary(
    operation_tag: u16,
    buffer: Vec<u8>,
    scalar_values: &[BuildFilesystemScalarOperandValue],
) -> BuildObservationSummary {
    let mut summary = summary(operation_tag);
    summary.filesystem_operation_attempts[0].scalar_operands = scalar_values
        .iter()
        .enumerate()
        .map(|(index, value)| BuildFilesystemScalarOperand {
            operand_ordinal: u8::try_from(index + 2).unwrap(),
            value: *value,
        })
        .collect();
    summary.filesystem_operation_attempts[0].mutable_byte_operand_resolutions =
        vec![BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: buffer.clone(),
        }];
    summary.filesystem_operation_attempts[0].mutable_byte_operands =
        vec![BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: buffer.clone(),
            post_bytes: buffer,
        }];
    summary
}

pub(super) fn unknown_descriptor_write_payload_summary(
    operation_tag: u16,
    payload: Vec<u8>,
    offset: Option<i64>,
) -> BuildObservationSummary {
    let mut summary = summary(operation_tag);
    summary.filesystem_operation_attempts[0].byte_operands = vec![BuildFilesystemByteOperand {
        operand_ordinal: 1,
        bytes: payload,
    }];
    if let Some(offset) = offset {
        summary.filesystem_operation_attempts[0].scalar_operands =
            vec![BuildFilesystemScalarOperand {
                operand_ordinal: 2,
                value: BuildFilesystemScalarOperandValue::I64(offset),
            }];
    }
    summary
}

fn replay_scalar_values(
    values: &[BuildFilesystemScalarOperandValue],
) -> Vec<psi_checked_interpreter::FilesystemScalarOperandValue> {
    values
        .iter()
        .copied()
        .map(|value| match value {
            BuildFilesystemScalarOperandValue::I32(value) => {
                psi_checked_interpreter::FilesystemScalarOperandValue::I32(value)
            }
            BuildFilesystemScalarOperandValue::U32(value) => {
                psi_checked_interpreter::FilesystemScalarOperandValue::U32(value)
            }
            BuildFilesystemScalarOperandValue::I64(value) => {
                psi_checked_interpreter::FilesystemScalarOperandValue::I64(value)
            }
            BuildFilesystemScalarOperandValue::U64(value) => {
                psi_checked_interpreter::FilesystemScalarOperandValue::U64(value)
            }
        })
        .collect()
}

#[test]
fn operand_free_unknown_descriptor_failure_records_recover_and_rehydrate_exactly() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for operation_tag in [8, 43, 44, 45] {
        let captured =
            capture_verified_build_filesystem_replay_record(&summary(operation_tag), limits)
                .expect("exact operand-free unknown-descriptor failure encodes")
                .expect("verified failure retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("exact operand-free unknown-descriptor failure recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("exact failure rehydrates through its typed constructor");

        assert!(!replay.has_output_attempts());
        assert!(replay.output_entries().is_empty());
        let [attempt] = replay.attempts() else {
            panic!("unknown-descriptor failure replay must retain one exact attempt")
        };
        assert_eq!(attempt.operation_tag(), operation_tag);
        assert_eq!(
            attempt.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                -1
            ))
        );
        assert_eq!(attempt.post_error(), Some(9));
        assert!(attempt.retired_logical_handles().is_empty());
    }
}

#[test]
fn operand_free_unknown_descriptor_failures_reject_shape_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut null = summary(43);
    null.filesystem_operation_attempts[0].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Null;
    assert!(capture_verified_build_filesystem_replay_record(&null, limits).is_err());

    let mut resolved = summary(44);
    resolved.filesystem_operation_attempts[0].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Resolved(
            BuildFilesystemLogicalHandleIdentity::new(1).unwrap(),
        );
    assert!(capture_verified_build_filesystem_replay_record(&resolved, limits).is_err());

    let mut changed_error = summary(45);
    changed_error.filesystem_operation_attempts[0].post_error = 13;
    assert!(capture_verified_build_filesystem_replay_record(&changed_error, limits).is_err());

    let mut retired = summary(8);
    retired.filesystem_operation_attempts[0]
        .retired_logical_handles
        .push(BuildFilesystemLogicalHandleIdentity::new(1).unwrap());
    assert!(capture_verified_build_filesystem_replay_record(&retired, limits).is_err());

    let mut handoff = summary(43);
    handoff
        .included_source_handoffs
        .push(crate::BuildIncludedSourceHandoff {
            relative_path: b"impossible.omg".to_vec(),
            filesystem_attempt_ordinal: 1,
        });
    assert!(capture_verified_build_filesystem_replay_record(&handoff, limits).is_err());
}

#[test]
fn unknown_descriptor_seek_failure_records_authored_scalars_exactly() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(
        &unknown_descriptor_seek_summary(-17, 2),
        limits,
    )
    .expect("exact unknown-descriptor seek failure encodes")
    .expect("verified seek failure retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact unknown-descriptor seek failure recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact seek failure rehydrates through its typed constructor");

    let [attempt] = replay.attempts() else {
        panic!("unknown-descriptor seek replay must retain one exact attempt")
    };
    assert_eq!(attempt.operation_tag(), 10);
    assert_eq!(
        attempt.result(),
        Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
            -1
        ))
    );
    assert_eq!(attempt.post_error(), Some(9));
    assert_eq!(
        attempt
            .scalar_operands()
            .iter()
            .map(|operand| (operand.operand_ordinal(), operand.value()))
            .collect::<Vec<_>>(),
        vec![
            (
                1,
                psi_checked_interpreter::FilesystemScalarOperandValue::I64(-17)
            ),
            (
                2,
                psi_checked_interpreter::FilesystemScalarOperandValue::I32(2)
            ),
        ]
    );
    assert!(!replay.has_output_attempts());
}

#[test]
fn unknown_descriptor_seek_failure_rejects_scalar_and_side_lane_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut wrong_type = unknown_descriptor_seek_summary(4, 1);
    wrong_type.filesystem_operation_attempts[0].scalar_operands[0].value =
        BuildFilesystemScalarOperandValue::U64(4);
    assert!(capture_verified_build_filesystem_replay_record(&wrong_type, limits).is_err());

    let mut wrong_ordinal = unknown_descriptor_seek_summary(4, 1);
    wrong_ordinal.filesystem_operation_attempts[0].scalar_operands[1].operand_ordinal = 3;
    assert!(capture_verified_build_filesystem_replay_record(&wrong_ordinal, limits).is_err());

    let mut missing = unknown_descriptor_seek_summary(4, 1);
    missing.filesystem_operation_attempts[0]
        .scalar_operands
        .pop();
    assert!(capture_verified_build_filesystem_replay_record(&missing, limits).is_err());

    let mut retired = unknown_descriptor_seek_summary(4, 1);
    retired.filesystem_operation_attempts[0]
        .retired_logical_handles
        .push(BuildFilesystemLogicalHandleIdentity::new(1).unwrap());
    assert!(capture_verified_build_filesystem_replay_record(&retired, limits).is_err());
}

#[test]
fn unknown_descriptor_at_failures_round_trip_exact_authored_inputs() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for (operation_tag, operation_name) in [(14, "open_at"), (15, "unlink_at")] {
        let summary =
            unknown_descriptor_at_summary(operation_tag, b"generated.omg".to_vec(), 0x241);
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .unwrap_or_else(|_| panic!("exact unknown-descriptor {operation_name} failure encodes"))
            .unwrap_or_else(|| panic!("verified {operation_name} failure retains replay custody"));
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .unwrap_or_else(|_| {
                    panic!("exact unknown-descriptor {operation_name} failure recovers")
                });
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .unwrap_or_else(|_| {
                panic!("exact {operation_name} failure rehydrates through its typed constructor")
            });

        let [attempt] = replay.attempts() else {
            panic!("unknown-descriptor {operation_name} replay must retain one exact attempt")
        };
        assert_eq!(attempt.operation_tag(), operation_tag);
        assert_eq!(
            attempt.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                -1
            ))
        );
        assert_eq!(attempt.post_error(), Some(9));
        let [component] = attempt.byte_operands() else {
            panic!("{operation_name} replay must retain one component")
        };
        assert_eq!(component.operand_ordinal(), 1);
        assert_eq!(component.bytes(), b"generated.omg");
        let [flags] = attempt.scalar_operands() else {
            panic!("{operation_name} replay must retain one flags operand")
        };
        assert_eq!(flags.operand_ordinal(), 2);
        assert_eq!(
            flags.value(),
            psi_checked_interpreter::FilesystemScalarOperandValue::I32(0x241)
        );
        assert!(!replay.has_output_attempts());
        assert!(replay.output_entries().is_empty());
    }
}

#[test]
fn unknown_descriptor_at_failures_reject_coordinate_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    for operation_tag in [14, 15] {
        for component in [
            Vec::new(),
            b".".to_vec(),
            b"..".to_vec(),
            b"nested/file".to_vec(),
            b"nested\\file".to_vec(),
            b"nul\0byte".to_vec(),
        ] {
            assert!(
                capture_verified_build_filesystem_replay_record(
                    &unknown_descriptor_at_summary(operation_tag, component, 0),
                    limits,
                )
                .is_err(),
                "non-component at-operation coordinates must reject"
            );
        }

        let mut wrong_component_ordinal =
            unknown_descriptor_at_summary(operation_tag, b"generated.omg".to_vec(), 0);
        wrong_component_ordinal.filesystem_operation_attempts[0].byte_operands[0].operand_ordinal =
            2;
        assert!(
            capture_verified_build_filesystem_replay_record(&wrong_component_ordinal, limits)
                .is_err()
        );

        let mut wrong_flags_ordinal =
            unknown_descriptor_at_summary(operation_tag, b"generated.omg".to_vec(), 0);
        wrong_flags_ordinal.filesystem_operation_attempts[0].scalar_operands[0].operand_ordinal = 1;
        assert!(
            capture_verified_build_filesystem_replay_record(&wrong_flags_ordinal, limits).is_err()
        );
    }
}

#[test]
fn unknown_descriptor_write_operation_failures_round_trip_exact_scalars() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let fixtures = [
        (17, vec![BuildFilesystemScalarOperandValue::U32(u32::MAX)]),
        (41, vec![BuildFilesystemScalarOperandValue::I64(i64::MIN)]),
        (46, vec![BuildFilesystemScalarOperandValue::I32(i32::MAX)]),
        (
            49,
            vec![
                BuildFilesystemScalarOperandValue::I32(-1),
                BuildFilesystemScalarOperandValue::I32(i32::MIN),
            ],
        ),
    ];

    for (operation_tag, values) in fixtures {
        let summary = unknown_descriptor_write_summary(operation_tag, &values);
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .expect("exact unknown-descriptor write operation encodes")
            .expect("verified write operation retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("exact unknown-descriptor write operation recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("exact write operation rehydrates through its typed constructor");
        let [attempt] = replay.attempts() else {
            panic!("unknown-descriptor write replay retains one attempt")
        };
        assert_eq!(attempt.operation_tag(), operation_tag);
        assert_eq!(
            attempt
                .scalar_operands()
                .iter()
                .map(|operand| operand.value())
                .collect::<Vec<_>>(),
            replay_scalar_values(&values)
        );
        assert_eq!(attempt.post_error(), Some(9));
        assert!(!replay.has_output_attempts());
    }
}

#[test]
fn unknown_descriptor_write_operation_failures_reject_scalar_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut wrong_type =
        unknown_descriptor_write_summary(17, &[BuildFilesystemScalarOperandValue::U32(0o755)]);
    wrong_type.filesystem_operation_attempts[0].scalar_operands[0].value =
        BuildFilesystemScalarOperandValue::I32(0o755);
    assert!(capture_verified_build_filesystem_replay_record(&wrong_type, limits).is_err());

    let mut extra =
        unknown_descriptor_write_summary(41, &[BuildFilesystemScalarOperandValue::I64(7)]);
    extra.filesystem_operation_attempts[0]
        .scalar_operands
        .push(BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::I32(0),
        });
    assert!(capture_verified_build_filesystem_replay_record(&extra, limits).is_err());

    let missing =
        unknown_descriptor_write_summary(49, &[BuildFilesystemScalarOperandValue::I32(-1)]);
    assert!(capture_verified_build_filesystem_replay_record(&missing, limits).is_err());
}

#[test]
fn unknown_descriptor_set_file_times_failure_round_trips_exact_carrier() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut times = vec![0; 47];
    times[0] = 11;
    times[16] = 29;
    times[46] = 173;
    let summary = unknown_descriptor_set_file_times_summary(times.clone());

    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("exact unknown-descriptor set_file_times failure encodes")
        .expect("verified set_file_times failure retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact set_file_times failure recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("set_file_times failure rehydrates through its typed constructor");

    let [attempt] = replay.attempts() else {
        panic!("unknown-descriptor set_file_times replay retains one attempt")
    };
    assert_eq!(attempt.operation_tag(), 42);
    let [resolution] = attempt.mutable_byte_operand_resolutions() else {
        panic!("set_file_times replay retains one resolution-time carrier")
    };
    let [carrier] = attempt.mutable_byte_operands() else {
        panic!("set_file_times replay retains one provider carrier")
    };
    assert_eq!(resolution.operand_ordinal(), 1);
    assert_eq!(resolution.bytes(), times);
    assert_eq!(carrier.operand_ordinal(), 1);
    assert_eq!(carrier.pre_bytes(), times);
    assert_eq!(carrier.post_bytes(), times);
    assert_eq!(attempt.post_error(), Some(9));
    assert!(!replay.has_output_attempts());
}

#[test]
fn unknown_descriptor_set_file_times_failure_rejects_carrier_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let too_short = unknown_descriptor_set_file_times_summary(vec![0; 31]);
    assert!(capture_verified_build_filesystem_replay_record(&too_short, limits).is_err());

    let mut wrong_resolution_ordinal = unknown_descriptor_set_file_times_summary(vec![0; 32]);
    wrong_resolution_ordinal.filesystem_operation_attempts[0].mutable_byte_operand_resolutions[0]
        .operand_ordinal = 2;
    assert!(
        capture_verified_build_filesystem_replay_record(&wrong_resolution_ordinal, limits).is_err()
    );

    let mut wrong_carrier_ordinal = unknown_descriptor_set_file_times_summary(vec![0; 32]);
    wrong_carrier_ordinal.filesystem_operation_attempts[0].mutable_byte_operands[0]
        .operand_ordinal = 2;
    assert!(
        capture_verified_build_filesystem_replay_record(&wrong_carrier_ordinal, limits).is_err()
    );

    let mut changed_pre = unknown_descriptor_set_file_times_summary(vec![0; 32]);
    changed_pre.filesystem_operation_attempts[0].mutable_byte_operands[0].pre_bytes[0] = 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed_pre, limits).is_err());

    let mut changed_post = unknown_descriptor_set_file_times_summary(vec![0; 32]);
    changed_post.filesystem_operation_attempts[0].mutable_byte_operands[0].post_bytes[31] = 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed_post, limits).is_err());
}

#[test]
fn unknown_descriptor_read_failures_round_trip_exact_scalars_and_carrier() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let fixtures = [
        (4, vec![BuildFilesystemScalarOperandValue::U64(23)]),
        (
            6,
            vec![
                BuildFilesystemScalarOperandValue::U64(19),
                BuildFilesystemScalarOperandValue::I64(-17),
            ],
        ),
    ];

    for (operation_tag, scalar_values) in fixtures {
        let mut buffer = vec![0; 47];
        buffer[0] = 11;
        buffer[23] = 29;
        buffer[46] = 173;
        let summary =
            unknown_descriptor_read_summary(operation_tag, buffer.clone(), &scalar_values);
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .expect("exact unknown-descriptor read failure encodes")
            .expect("verified read failure retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("exact read failure recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("read failure rehydrates through its typed constructor");

        let [attempt] = replay.attempts() else {
            panic!("unknown-descriptor read replay retains one attempt")
        };
        assert_eq!(attempt.operation_tag(), operation_tag);
        assert_eq!(
            attempt
                .scalar_operands()
                .iter()
                .map(|operand| operand.value())
                .collect::<Vec<_>>(),
            replay_scalar_values(&scalar_values)
        );
        let [resolution] = attempt.mutable_byte_operand_resolutions() else {
            panic!("read replay retains one resolution-time carrier")
        };
        let [carrier] = attempt.mutable_byte_operands() else {
            panic!("read replay retains one provider carrier")
        };
        assert_eq!(resolution.operand_ordinal(), 1);
        assert_eq!(resolution.bytes(), buffer);
        assert_eq!(carrier.operand_ordinal(), 1);
        assert_eq!(carrier.pre_bytes(), buffer);
        assert_eq!(carrier.post_bytes(), buffer);
        assert_eq!(attempt.post_error(), Some(9));
        assert!(!replay.has_output_attempts());
    }
}

#[test]
fn unknown_descriptor_read_failures_reject_scalar_and_carrier_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut count_exceeds_carrier = unknown_descriptor_read_summary(
        4,
        vec![0; 31],
        &[BuildFilesystemScalarOperandValue::U64(32)],
    );
    assert!(
        capture_verified_build_filesystem_replay_record(&count_exceeds_carrier, limits).is_err()
    );

    count_exceeds_carrier.filesystem_operation_attempts[0].scalar_operands[0].value =
        BuildFilesystemScalarOperandValue::U64(31);
    assert!(
        capture_verified_build_filesystem_replay_record(&count_exceeds_carrier, limits).is_ok()
    );

    let mut wrong_scalar = unknown_descriptor_read_summary(
        6,
        vec![0; 32],
        &[
            BuildFilesystemScalarOperandValue::U64(32),
            BuildFilesystemScalarOperandValue::U64(0),
        ],
    );
    assert!(capture_verified_build_filesystem_replay_record(&wrong_scalar, limits).is_err());
    wrong_scalar.filesystem_operation_attempts[0].scalar_operands[1].value =
        BuildFilesystemScalarOperandValue::I64(0);
    wrong_scalar.filesystem_operation_attempts[0].scalar_operands[1].operand_ordinal = 2;
    assert!(capture_verified_build_filesystem_replay_record(&wrong_scalar, limits).is_err());

    let mut changed_resolution = unknown_descriptor_read_summary(
        4,
        vec![0; 32],
        &[BuildFilesystemScalarOperandValue::U64(7)],
    );
    changed_resolution.filesystem_operation_attempts[0].mutable_byte_operand_resolutions[0].bytes
        [0] = 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed_resolution, limits).is_err());

    let mut changed_post = unknown_descriptor_read_summary(
        4,
        vec![0; 32],
        &[BuildFilesystemScalarOperandValue::U64(7)],
    );
    changed_post.filesystem_operation_attempts[0].mutable_byte_operands[0].post_bytes[31] = 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed_post, limits).is_err());
}

#[test]
fn unknown_descriptor_write_payload_failures_round_trip_exact_inputs() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let fixtures = [(5, None), (7, Some(-47))];
    for (operation_tag, offset) in fixtures {
        let payload = vec![0, 11, 29, 47, 83, 173, 255];
        let summary =
            unknown_descriptor_write_payload_summary(operation_tag, payload.clone(), offset);
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .expect("exact unknown-descriptor write payload encodes")
            .expect("verified write payload retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("exact unknown-descriptor write payload recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("exact write payload rehydrates through its typed constructor");

        let [attempt] = replay.attempts() else {
            panic!("unknown-descriptor write payload replay retains one attempt")
        };
        assert_eq!(attempt.operation_tag(), operation_tag);
        assert_eq!(
            attempt.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                -1
            ))
        );
        assert_eq!(attempt.post_error(), Some(9));
        let [bytes] = attempt.byte_operands() else {
            panic!("unknown-descriptor write retains one immutable payload")
        };
        assert_eq!(bytes.operand_ordinal(), 1);
        assert_eq!(bytes.bytes(), payload);
        assert_eq!(
            attempt
                .scalar_operands()
                .iter()
                .map(|operand| (operand.operand_ordinal(), operand.value()))
                .collect::<Vec<_>>(),
            offset
                .map(|offset| vec![(
                    2,
                    psi_checked_interpreter::FilesystemScalarOperandValue::I64(offset),
                )])
                .unwrap_or_default()
        );
        assert!(!replay.has_output_attempts());
    }
}

#[test]
fn unknown_descriptor_write_payload_failures_reject_lane_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut wrong_payload_ordinal =
        unknown_descriptor_write_payload_summary(5, vec![1, 2, 3], None);
    wrong_payload_ordinal.filesystem_operation_attempts[0].byte_operands[0].operand_ordinal = 2;
    assert!(
        capture_verified_build_filesystem_replay_record(&wrong_payload_ordinal, limits).is_err()
    );

    let mut missing_payload = unknown_descriptor_write_payload_summary(5, Vec::new(), None);
    missing_payload.filesystem_operation_attempts[0]
        .byte_operands
        .clear();
    assert!(capture_verified_build_filesystem_replay_record(&missing_payload, limits).is_err());

    let mut wrong_scalar_type =
        unknown_descriptor_write_payload_summary(7, vec![1, 2, 3], Some(-47));
    wrong_scalar_type.filesystem_operation_attempts[0].scalar_operands[0].value =
        BuildFilesystemScalarOperandValue::U64(47);
    assert!(capture_verified_build_filesystem_replay_record(&wrong_scalar_type, limits).is_err());

    let mut unexpected_scalar = unknown_descriptor_write_payload_summary(5, vec![1, 2, 3], None);
    unexpected_scalar.filesystem_operation_attempts[0]
        .scalar_operands
        .push(BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::I64(0),
        });
    assert!(capture_verified_build_filesystem_replay_record(&unexpected_scalar, limits).is_err());

    let first = unknown_descriptor_write_payload_summary(5, vec![1, 2, 3], None);
    let second = unknown_descriptor_write_payload_summary(5, vec![1, 2, 4], None);
    let first = capture_verified_build_filesystem_replay_record(&first, limits)
        .unwrap()
        .unwrap();
    let second = capture_verified_build_filesystem_replay_record(&second, limits)
        .unwrap()
        .unwrap();
    assert_ne!(first.commitment(), second.commitment());
}

#[test]
fn unknown_descriptor_read_file_metadata_failure_round_trips_exact_carrier() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for buffer in [
        vec![0; psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES],
        vec![11; psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES + 19],
    ] {
        let summary = unknown_descriptor_read_file_metadata_summary(buffer.clone());
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .expect("exact unknown-descriptor read_file_metadata encodes")
            .expect("verified read_file_metadata retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("exact unknown-descriptor read_file_metadata recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("exact read_file_metadata rehydrates through its typed constructor");

        let [attempt] = replay.attempts() else {
            panic!("unknown-descriptor read_file_metadata replay retains one attempt")
        };
        assert_eq!(attempt.operation_tag(), 39);
        assert_eq!(
            attempt.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                -1
            ))
        );
        assert_eq!(attempt.post_error(), Some(9));
        let [resolution] = attempt.mutable_byte_operand_resolutions() else {
            panic!("read_file_metadata retains one resolution-time carrier")
        };
        let [carrier] = attempt.mutable_byte_operands() else {
            panic!("read_file_metadata retains one provider carrier")
        };
        assert_eq!(resolution.operand_ordinal(), 1);
        assert_eq!(resolution.bytes(), buffer);
        assert_eq!(carrier.operand_ordinal(), 1);
        assert_eq!(carrier.pre_bytes(), buffer);
        assert_eq!(carrier.post_bytes(), buffer);
        assert!(!replay.has_output_attempts());
    }
}

#[test]
fn unknown_descriptor_read_file_metadata_failure_rejects_carrier_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let carrier = || vec![1; psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES];

    let mut wrong_resolution_ordinal = unknown_descriptor_read_file_metadata_summary(carrier());
    wrong_resolution_ordinal.filesystem_operation_attempts[0].mutable_byte_operand_resolutions[0]
        .operand_ordinal = 2;
    assert!(
        capture_verified_build_filesystem_replay_record(&wrong_resolution_ordinal, limits).is_err()
    );

    let mut missing_resolution = unknown_descriptor_read_file_metadata_summary(carrier());
    missing_resolution.filesystem_operation_attempts[0]
        .mutable_byte_operand_resolutions
        .clear();
    assert!(capture_verified_build_filesystem_replay_record(&missing_resolution, limits).is_err());

    let mut changed_pre = unknown_descriptor_read_file_metadata_summary(carrier());
    changed_pre.filesystem_operation_attempts[0].mutable_byte_operands[0].pre_bytes[0] = 4;
    assert!(capture_verified_build_filesystem_replay_record(&changed_pre, limits).is_err());

    let mut changed_post = unknown_descriptor_read_file_metadata_summary(carrier());
    changed_post.filesystem_operation_attempts[0].mutable_byte_operands[0].post_bytes[2] = 4;
    assert!(capture_verified_build_filesystem_replay_record(&changed_post, limits).is_err());

    let short = unknown_descriptor_read_file_metadata_summary(vec![
        1;
        psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
            - 1
    ]);
    assert!(capture_verified_build_filesystem_replay_record(&short, limits).is_err());

    let first = unknown_descriptor_read_file_metadata_summary(carrier());
    let mut changed = carrier();
    changed[37] = 2;
    let second = unknown_descriptor_read_file_metadata_summary(changed);
    let first = capture_verified_build_filesystem_replay_record(&first, limits)
        .unwrap()
        .unwrap();
    let second = capture_verified_build_filesystem_replay_record(&second, limits)
        .unwrap()
        .unwrap();
    assert_ne!(first.commitment(), second.commitment());
}

#[test]
fn unknown_descriptor_get_osfhandle_failure_round_trips_exact_model() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let summary = unknown_descriptor_get_osfhandle_summary();
    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("exact unknown-descriptor get_osfhandle encodes")
        .expect("verified get_osfhandle failure retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact unknown-descriptor get_osfhandle recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact get_osfhandle rehydrates through its typed constructor");

    let [attempt] = replay.attempts() else {
        panic!("unknown-descriptor get_osfhandle replay retains one attempt")
    };
    assert_eq!(attempt.operation_tag(), 30);
    assert_eq!(
        attempt.result(),
        Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
            -2
        ))
    );
    assert_eq!(attempt.post_error(), Some(0));
    assert!(attempt.logical_handle_output().is_none());
    assert!(!replay.has_output_attempts());
}

#[test]
fn unknown_descriptor_get_osfhandle_failure_rejects_model_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut changed_result = unknown_descriptor_get_osfhandle_summary();
    changed_result.filesystem_operation_attempts[0].result =
        BuildFilesystemOperationResult::Scalar(-1);
    assert!(capture_verified_build_filesystem_replay_record(&changed_result, limits).is_err());

    let mut changed_error = unknown_descriptor_get_osfhandle_summary();
    changed_error.filesystem_operation_attempts[0].post_error = 9;
    assert!(capture_verified_build_filesystem_replay_record(&changed_error, limits).is_err());

    let mut invented_output = unknown_descriptor_get_osfhandle_summary();
    invented_output.filesystem_operation_attempts[0].logical_handle_output =
        Some(BuildFilesystemLogicalHandleOutput {
            identity: BuildFilesystemLogicalHandleIdentity::new(2).unwrap(),
            kind: BuildFilesystemLogicalHandleKind::Native,
            source: BuildFilesystemLogicalHandleOutputSource::Borrowed(
                BuildFilesystemLogicalHandleIdentity::new(1).unwrap(),
            ),
        });
    assert!(capture_verified_build_filesystem_replay_record(&invented_output, limits).is_err());
}

#[test]
fn unknown_native_handle_close_failure_round_trips_exact_model() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let summary = unknown_native_handle_close_summary();
    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("exact unknown-native-handle close encodes")
        .expect("verified close failure retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact unknown-native-handle close recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact native-handle close rehydrates through its typed constructor");

    let [attempt] = replay.attempts() else {
        panic!("unknown-native-handle close replay retains one attempt")
    };
    assert_eq!(attempt.operation_tag(), 29);
    assert_eq!(
        attempt.result(),
        Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
            0
        ))
    );
    assert_eq!(attempt.post_error(), Some(6));
    assert!(attempt.retired_logical_handles().is_empty());
    assert!(!replay.has_output_attempts());
}

#[test]
fn unknown_native_handle_close_failure_rejects_model_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut changed_result = unknown_native_handle_close_summary();
    changed_result.filesystem_operation_attempts[0].result =
        BuildFilesystemOperationResult::Scalar(1);
    assert!(capture_verified_build_filesystem_replay_record(&changed_result, limits).is_err());

    let mut changed_error = unknown_native_handle_close_summary();
    changed_error.filesystem_operation_attempts[0].post_error = 0;
    assert!(capture_verified_build_filesystem_replay_record(&changed_error, limits).is_err());

    let mut wrong_kind = unknown_native_handle_close_summary();
    wrong_kind.filesystem_operation_attempts[0].logical_handle_inputs[0].kind =
        BuildFilesystemLogicalHandleKind::Descriptor;
    assert!(capture_verified_build_filesystem_replay_record(&wrong_kind, limits).is_err());

    let mut invented_retirement = unknown_native_handle_close_summary();
    invented_retirement.filesystem_operation_attempts[0]
        .retired_logical_handles
        .push(BuildFilesystemLogicalHandleIdentity::new(1).unwrap());
    assert!(capture_verified_build_filesystem_replay_record(&invented_retirement, limits).is_err());
}

#[test]
fn unknown_native_handle_final_path_failure_round_trips_exact_carrier() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let summary = unknown_native_handle_final_path_summary();
    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("exact unknown-native-handle final path encodes")
        .expect("verified final-path failure retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact unknown-native-handle final path recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact final-path failure rehydrates through its typed constructor");

    let [attempt] = replay.attempts() else {
        panic!("unknown-native-handle final-path replay retains one attempt")
    };
    assert_eq!(attempt.operation_tag(), 31);
    assert_eq!(
        attempt.result(),
        Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
            0
        ))
    );
    assert_eq!(attempt.post_error(), Some(6));
    let [resolution] = attempt.mutable_byte_operand_resolutions() else {
        panic!("final-path failure retains one resolution-time carrier")
    };
    let [carrier] = attempt.mutable_byte_operands() else {
        panic!("final-path failure retains one provider carrier")
    };
    assert_eq!(resolution.bytes(), carrier.pre_bytes());
    assert_eq!(carrier.pre_bytes(), carrier.post_bytes());
    assert!(attempt.returned_paths().is_empty());
    assert!(!replay.has_output_attempts());
}

#[test]
fn unknown_native_handle_final_path_failure_rejects_carrier_and_scalar_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut oversized_capacity = unknown_native_handle_final_path_summary();
    oversized_capacity.filesystem_operation_attempts[0].scalar_operands[0].value =
        BuildFilesystemScalarOperandValue::U64(48);
    assert!(capture_verified_build_filesystem_replay_record(&oversized_capacity, limits).is_err());

    let mut changed_post = unknown_native_handle_final_path_summary();
    changed_post.filesystem_operation_attempts[0].mutable_byte_operands[0].post_bytes[0] ^= 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed_post, limits).is_err());

    let mut invented_path = unknown_native_handle_final_path_summary();
    invented_path.filesystem_operation_attempts[0]
        .returned_paths
        .push(BuildFilesystemReturnedPath {
            operand_ordinal: 1,
            kind: BuildFilesystemReturnedPathKind::FinalPath,
            completeness: BuildFilesystemReturnedPathCompleteness::Complete,
            bytes: b"invented".to_vec(),
        });
    assert!(capture_verified_build_filesystem_replay_record(&invented_path, limits).is_err());
}
