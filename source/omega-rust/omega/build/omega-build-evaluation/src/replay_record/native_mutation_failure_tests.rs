use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemByteOperand,
    BuildFilesystemLogicalHandleInput, BuildFilesystemLogicalHandleInputResolution,
    BuildFilesystemLogicalHandleKind, BuildFilesystemMutableByteOperand,
    BuildFilesystemMutableByteOperandResolution, BuildFilesystemReplayDisposition,
    BuildFilesystemReplayVerdict, BuildFilesystemScalarOperand, BuildObservationClass,
};

fn summary(attempt: BuildFilesystemOperationAttempt) -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: vec![attempt],
        canonical_source_metadata_identity: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::SourceInputsOnly,
        ),
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
    }
}

fn native_failure(operation_tag: u16) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag,
        provider: BuildFilesystemProvider::RealScoped,
        result: BuildFilesystemOperationResult::Scalar(0),
        post_error: 6,
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
            kind: BuildFilesystemLogicalHandleKind::Native,
            resolution: BuildFilesystemLogicalHandleInputResolution::Unknown,
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn set_file_time_summary() -> BuildObservationSummary {
    let mut attempt = native_failure(32);
    attempt.scalar_operands = vec![BuildFilesystemScalarOperand {
        operand_ordinal: 1,
        value: BuildFilesystemScalarOperandValue::I64(37),
    }];
    attempt.byte_operands = vec![
        BuildFilesystemByteOperand {
            operand_ordinal: 2,
            bytes: vec![11; 12],
        },
        BuildFilesystemByteOperand {
            operand_ordinal: 3,
            bytes: vec![19; 16],
        },
    ];
    summary(attempt)
}

fn lock_file_ex_summary() -> BuildObservationSummary {
    let mut attempt = native_failure(33);
    attempt.scalar_operands = [1, 0, 0xffff_ffff, 0xffff_ffff]
        .into_iter()
        .enumerate()
        .map(|(index, value)| BuildFilesystemScalarOperand {
            operand_ordinal: u8::try_from(index + 1).unwrap(),
            value: BuildFilesystemScalarOperandValue::U32(value),
        })
        .collect();
    let overlapped = vec![23; 40];
    attempt.mutable_byte_operand_resolutions = vec![BuildFilesystemMutableByteOperandResolution {
        operand_ordinal: 5,
        bytes: overlapped.clone(),
    }];
    attempt.mutable_byte_operands = vec![BuildFilesystemMutableByteOperand {
        operand_ordinal: 5,
        pre_bytes: overlapped.clone(),
        post_bytes: overlapped,
    }];
    summary(attempt)
}

fn unlock_file_summary() -> BuildObservationSummary {
    let mut attempt = native_failure(34);
    attempt.scalar_operands = [3, 5, 7, 11]
        .into_iter()
        .enumerate()
        .map(|(index, value)| BuildFilesystemScalarOperand {
            operand_ordinal: u8::try_from(index + 1).unwrap(),
            value: BuildFilesystemScalarOperandValue::U32(value),
        })
        .collect();
    summary(attempt)
}

#[test]
fn unknown_native_handle_mutation_family_round_trips_exact_inputs() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for summary in [
        set_file_time_summary(),
        lock_file_ex_summary(),
        unlock_file_summary(),
    ] {
        let expected = &summary.filesystem_operation_attempts[0];
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .expect("exact native mutation failure encodes")
            .expect("verified native mutation failure retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("exact native mutation failure recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("exact native mutation failure rehydrates through its typed constructor");
        let [actual] = replay.attempts() else {
            panic!("native mutation replay retains one exact attempt")
        };

        assert_eq!(actual.operation_tag(), expected.operation_tag);
        assert_eq!(
            actual.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                0
            ))
        );
        assert_eq!(actual.post_error(), Some(6));
        assert_eq!(
            actual.scalar_operands().len(),
            expected.scalar_operands.len()
        );
        assert_eq!(actual.byte_operands().len(), expected.byte_operands.len());
        assert_eq!(
            actual.mutable_byte_operand_resolutions().len(),
            expected.mutable_byte_operand_resolutions.len()
        );
        assert_eq!(
            actual.mutable_byte_operands().len(),
            expected.mutable_byte_operands.len()
        );
        assert!(!replay.has_output_attempts());
    }
}

#[test]
fn unknown_native_handle_mutation_family_rejects_shape_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut short_filetime = set_file_time_summary();
    short_filetime.filesystem_operation_attempts[0].byte_operands[0]
        .bytes
        .truncate(7);
    assert!(capture_verified_build_filesystem_replay_record(&short_filetime, limits).is_err());

    let mut changed_overlapped = lock_file_ex_summary();
    changed_overlapped.filesystem_operation_attempts[0].mutable_byte_operands[0].post_bytes[0] ^= 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed_overlapped, limits).is_err());

    let mut invented_payload = unlock_file_summary();
    invented_payload.filesystem_operation_attempts[0]
        .byte_operands
        .push(BuildFilesystemByteOperand {
            operand_ordinal: 5,
            bytes: vec![0],
        });
    assert!(capture_verified_build_filesystem_replay_record(&invented_payload, limits).is_err());

    let mut wrong_domain = unlock_file_summary();
    wrong_domain.filesystem_operation_attempts[0].logical_handle_inputs[0].kind =
        BuildFilesystemLogicalHandleKind::Descriptor;
    assert!(capture_verified_build_filesystem_replay_record(&wrong_domain, limits).is_err());
}
