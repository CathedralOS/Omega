use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemLogicalHandleInput, BuildFilesystemLogicalHandleInputResolution,
    BuildFilesystemLogicalHandleKind, BuildFilesystemMutableByteOperand,
    BuildFilesystemMutableByteOperandResolution, BuildFilesystemMutableI64Operand,
    BuildFilesystemMutableI64OperandResolution, BuildFilesystemScalarOperand,
    BuildObservationClass,
};

pub(super) fn summary(
    buffer: Vec<u8>,
    requested_count: u64,
    position: i64,
) -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: vec![BuildFilesystemOperationAttempt {
            operation_tag: 23,
            provider: BuildFilesystemProvider::RealScoped,
            observation_class: crate::BuildFilesystemOperationObservationClass::Receipted,
            result: BuildFilesystemOperationResult::Scalar(-1),
            post_error: 9,
            scalar_operands: vec![BuildFilesystemScalarOperand {
                operand_ordinal: 2,
                value: BuildFilesystemScalarOperandValue::U64(requested_count),
            }],
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: vec![BuildFilesystemMutableByteOperandResolution {
                operand_ordinal: 1,
                bytes: buffer.clone(),
            }],
            mutable_i64_operand_resolutions: vec![BuildFilesystemMutableI64OperandResolution {
                operand_ordinal: 3,
                value: position,
            }],
            mutable_byte_operands: vec![BuildFilesystemMutableByteOperand {
                operand_ordinal: 1,
                pre_bytes: buffer.clone(),
                post_bytes: buffer,
            }],
            mutable_i64_operands: vec![BuildFilesystemMutableI64Operand {
                operand_ordinal: 3,
                pre_value: position,
                post_value: position,
            }],
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![BuildFilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: BuildFilesystemLogicalHandleKind::Descriptor,
                resolution: BuildFilesystemLogicalHandleInputResolution::Unknown,
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        }],
        canonical_source_metadata_identity: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::SourceInputsOnly,
        ),
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}

#[test]
fn unknown_descriptor_read_dir_round_trips_exact_carriers_and_count() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let summary = summary(vec![0x5a; 47], 31, -19);
    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("exact unknown-descriptor read_dir failure encodes")
        .expect("verified read_dir failure retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact unknown-descriptor read_dir failure recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact read_dir failure rehydrates through its typed constructor");

    let [attempt] = replay.attempts() else {
        panic!("unknown-descriptor read_dir replay must retain one exact attempt")
    };
    assert_eq!(attempt.operation_tag(), 23);
    assert_eq!(
        attempt.result(),
        Some(checked_interpreter::FilesystemOperationResult::Scalar(-1))
    );
    assert_eq!(attempt.post_error(), Some(9));
    let [count] = attempt.scalar_operands() else {
        panic!("read_dir replay must retain one requested count")
    };
    assert_eq!(count.operand_ordinal(), 2);
    assert_eq!(
        count.value(),
        checked_interpreter::FilesystemScalarOperandValue::U64(31)
    );
    let [buffer] = attempt.mutable_byte_operands() else {
        panic!("read_dir replay must retain one byte carrier")
    };
    assert_eq!(buffer.pre_bytes(), &[0x5a; 47]);
    assert_eq!(buffer.post_bytes(), &[0x5a; 47]);
    let [position] = attempt.mutable_i64_operands() else {
        panic!("read_dir replay must retain one position carrier")
    };
    assert_eq!(position.pre_value(), -19);
    assert_eq!(position.post_value(), -19);
    assert!(!replay.has_output_attempts());
}

#[test]
fn unknown_descriptor_read_dir_rejects_carrier_and_lane_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    assert!(
        capture_verified_build_filesystem_replay_record(&summary(vec![0; 4], 5, 0), limits)
            .is_err()
    );

    let mut changed = summary(vec![0; 4], 4, 0);
    changed.filesystem_operation_attempts[0].mutable_byte_operands[0].post_bytes[0] = 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed, limits).is_err());

    let mut changed = summary(vec![0; 4], 4, 0);
    changed.filesystem_operation_attempts[0].mutable_i64_operands[0].post_value = 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed, limits).is_err());

    let mut changed = summary(vec![0; 4], 4, 0);
    changed.filesystem_operation_attempts[0]
        .authorized_paths
        .push(BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Read,
            root: BuildFilesystemRoot::Source,
            relative_path: b"invented".to_vec(),
        });
    assert!(capture_verified_build_filesystem_replay_record(&changed, limits).is_err());
}
