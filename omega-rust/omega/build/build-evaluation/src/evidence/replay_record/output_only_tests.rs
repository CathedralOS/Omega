use super::{
    BuildFilesystemReplayRecordLimits, capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
    rehydrate_review_only_build_filesystem_replay_record,
};
use crate::BuildFilesystemGrantAccess;
use crate::BuildFilesystemLogicalHandleInputResolution;
use crate::BuildFilesystemLogicalHandleKind;
use crate::BuildFilesystemLogicalHandleOutputSource;
use crate::BuildFilesystemOperationAttempt;
use crate::BuildFilesystemOperationResult;
use crate::BuildFilesystemProvider;
use crate::BuildFilesystemReplayDisposition;
use crate::BuildFilesystemReplayVerdict;
use crate::BuildFilesystemRoot;
use crate::BuildFilesystemScalarOperandValue;
use crate::BuildObservationSummary;
use crate::evidence::replay_record::shape_validation::validate_first_rung;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemRootedPathOperandResolution,
    BuildFilesystemScalarOperand, BuildIncludedSourceHandoff, BuildObservationClass,
};

const OUTPUT_PATH: &[u8] = b"generated.omg";

fn identity(value: u64) -> BuildFilesystemLogicalHandleIdentity {
    BuildFilesystemLogicalHandleIdentity::new(value).expect("test identity is nonzero")
}

fn attempt(
    operation_tag: u16,
    result: BuildFilesystemOperationResult,
) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag,
        provider: BuildFilesystemProvider::RealScoped,
        observation_class: crate::BuildFilesystemOperationObservationClass::Receipted,
        result,
        post_error: 0,
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
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn output_file_attempts() -> Vec<BuildFilesystemOperationAttempt> {
    let output_identity = identity(73);
    let mut create = attempt(
        1,
        BuildFilesystemOperationResult::LogicalHandle(output_identity),
    );
    create.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 1,
        value: BuildFilesystemScalarOperandValue::I32(
            checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE,
        ),
    });
    create
        .rooted_path_operand_resolutions
        .push(BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Output,
            relative_path: OUTPUT_PATH.to_vec(),
        });
    create.authorized_paths.push(BuildFilesystemAuthorizedPath {
        operand_ordinal: 0,
        access: BuildFilesystemGrantAccess::Write,
        root: BuildFilesystemRoot::Output,
        relative_path: OUTPUT_PATH.to_vec(),
    });
    create.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
        kind: BuildFilesystemLogicalHandleKind::Descriptor,
        identity: output_identity,
        source: BuildFilesystemLogicalHandleOutputSource::Created,
    });

    let mut close = attempt(8, BuildFilesystemOperationResult::Scalar(0));
    close
        .logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Descriptor,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(output_identity),
        });
    close.retired_logical_handles.push(output_identity);
    vec![create, close]
}

fn source_file_attempts(handle: u64, path: &[u8]) -> Vec<BuildFilesystemOperationAttempt> {
    let descriptor = identity(handle);
    let mut endpoints = output_file_attempts();
    let mut open = endpoints.remove(0);
    open.operation_tag = 2;
    open.result = BuildFilesystemOperationResult::LogicalHandle(descriptor);
    open.scalar_operands[0].value = BuildFilesystemScalarOperandValue::I32(0);
    open.logical_handle_output.as_mut().unwrap().identity = descriptor;
    open.rooted_path_operand_resolutions[0].root = BuildFilesystemRoot::Source;
    open.rooted_path_operand_resolutions[0].relative_path = path.to_vec();
    open.authorized_paths[0].root = BuildFilesystemRoot::Source;
    open.authorized_paths[0].access = BuildFilesystemGrantAccess::Read;
    open.authorized_paths[0].relative_path = path.to_vec();
    let mut close = endpoints.remove(0);
    close.logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Resolved(descriptor);
    close.retired_logical_handles = vec![descriptor];
    let mut read = attempt(4, BuildFilesystemOperationResult::Scalar(2));
    read.logical_handle_inputs = close.logical_handle_inputs.clone();
    read.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 2,
        value: BuildFilesystemScalarOperandValue::U64(2),
    });
    read.mutable_byte_operand_resolutions.push(
        crate::BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: b"....".to_vec(),
        },
    );
    read.mutable_byte_operands
        .push(crate::BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: b"....".to_vec(),
            post_bytes: b"ab..".to_vec(),
        });
    read.observed_byte_regions
        .push(crate::BuildFilesystemObservedByteRegion {
            output_operand_ordinal: 1,
            kind: crate::BuildFilesystemObservedByteRegionKind::SequentialFileRead,
            offset: 0,
            length: 2,
        });
    vec![open, read, close]
}

fn round_trip_summary(summary: &BuildObservationSummary) -> checked_interpreter::FilesystemReplay {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("record captures")
        .expect("record is replayed");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("record recovers");
    assert_eq!(captured, recovered);
    rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("record rehydrates")
}

#[test]
fn source_output_source_record_round_trip_preserves_every_attempt_and_global_handoff() {
    let first = source_file_attempts(71, b"first.txt");
    let second = source_file_attempts(72, b"second.txt");
    let mut output = output_file_attempts();
    let mut write = attempt(5, BuildFilesystemOperationResult::Scalar(2));
    write.logical_handle_inputs = output[1].logical_handle_inputs.clone();
    write.byte_operands.push(crate::BuildFilesystemByteOperand {
        operand_ordinal: 1,
        bytes: b"ab".to_vec(),
    });
    output.insert(1, write);
    let mut summary = output_only_summary(9);
    summary.filesystem_operation_attempts = first
        .iter()
        .chain(&output)
        .chain(&second)
        .cloned()
        .collect();
    let sequential = round_trip_summary(&summary);
    assert_eq!(
        sequential
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 8, 2, 4, 8]
    );
    // Both inputs and Output are live together, then Output closes before the
    // last input. Its handoff ordinal still counts the original global stream.
    let order = [0, 1, 3, 4, 6, 7, 2, 5, 8];
    let baseline = summary.filesystem_operation_attempts.clone();
    summary.filesystem_operation_attempts = order
        .iter()
        .map(|position| baseline[*position].clone())
        .collect();
    summary.included_source_handoffs[0].filesystem_attempt_ordinal = 8;
    let interleaved = round_trip_summary(&summary);
    let expected = order
        .iter()
        .map(|position| sequential.attempts()[*position].clone())
        .collect::<Vec<_>>();
    assert_eq!(interleaved.attempts(), expected);
    assert_eq!(
        interleaved.output_files()[0].replayed_bytes().unwrap(),
        b"ab"
    );
    assert_eq!(
        interleaved.expected_included_sources()[0].filesystem_attempt_ordinal(),
        8
    );

    summary.included_source_handoffs[0].filesystem_attempt_ordinal = 7;
    assert!(
        capture_verified_build_filesystem_replay_record(
            &summary,
            BuildFilesystemReplayRecordLimits::default()
        )
        .is_err()
    );
}

#[test]
fn interleaved_source_only_record_preserves_complete_carriers_and_lifetimes() {
    let first = source_file_attempts(71, b"first.txt");
    let second = source_file_attempts(72, b"second.txt");
    let mut summary = output_only_summary(0);
    summary.included_source_handoffs.clear();
    summary.filesystem_operation_attempts = first.iter().chain(&second).cloned().collect();
    let sequential = round_trip_summary(&summary);
    let baseline = summary.filesystem_operation_attempts.clone();
    let order = [0, 3, 1, 4, 5, 2];
    summary.filesystem_operation_attempts = order
        .iter()
        .map(|position| baseline[*position].clone())
        .collect();
    let replay = round_trip_summary(&summary);
    assert_eq!(
        replay.attempts(),
        order
            .iter()
            .map(|position| sequential.attempts()[*position].clone())
            .collect::<Vec<_>>()
    );
    assert!(replay.output_files().is_empty());
}

pub(super) fn output_only_summary(handoff_ordinal: u64) -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: output_file_attempts(),
        canonical_source_metadata_identity: None,
        replay_activation: crate::BuildReplayActivation::default(),
        captured_source_inventory: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::Complete,
        ),
        included_source_handoffs: vec![BuildIncludedSourceHandoff {
            relative_path: OUTPUT_PATH.to_vec(),
            filesystem_attempt_ordinal: handoff_ordinal,
        }],
        required_output_settlements: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}

#[test]
fn output_only_record_recovers_and_rehydrates_from_attempt_zero() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&output_only_summary(2), limits)
        .expect("exact Output-only record encodes")
        .expect("verified Output-only record retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact Output-only record recovers");
    assert_eq!(recovered, captured);

    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact Output-only record rehydrates through the no-Source constructor");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![1, 8]
    );
    assert_eq!(replay.output_files()[0].output_relative_path(), OUTPUT_PATH);
    let [included] = replay.expected_included_sources() else {
        panic!("Output-only replay retains its one generated-source handoff")
    };
    assert_eq!(included.relative_path(), OUTPUT_PATH);
    assert_eq!(included.filesystem_attempt_ordinal(), 2);
}

#[test]
fn output_only_record_rejects_handoff_before_its_attempt_zero_file_closes() {
    assert!(
        capture_verified_build_filesystem_replay_record(
            &output_only_summary(1),
            BuildFilesystemReplayRecordLimits::default(),
        )
        .is_err()
    );
}

fn interleaved_output_summary() -> BuildObservationSummary {
    let mut summary = output_only_summary(4);
    let first = output_file_attempts();
    let mut second = output_file_attempts();
    let second_identity = identity(74);
    second[0].result = BuildFilesystemOperationResult::LogicalHandle(second_identity);
    second[0].logical_handle_output.as_mut().unwrap().identity = second_identity;
    second[0].rooted_path_operand_resolutions[0].relative_path = b"other.txt".to_vec();
    second[0].authorized_paths[0].relative_path = b"other.txt".to_vec();
    second[1].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Resolved(second_identity);
    second[1].retired_logical_handles = vec![second_identity];
    summary.filesystem_operation_attempts = vec![
        first[0].clone(),
        second[0].clone(),
        second[1].clone(),
        first[1].clone(),
    ];
    summary
}

#[test]
fn interleaved_output_record_preserves_event_order_and_exact_close_handoff() {
    let summary = interleaved_output_summary();
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("interleaved files encode")
        .expect("complete replay record");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("interleaved files recover");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("interleaved files rehydrate");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![1, 1, 8, 8]
    );
    assert_eq!(
        replay.expected_included_sources()[0].filesystem_attempt_ordinal(),
        4
    );
    assert_eq!(replay.output_files().len(), 2);

    let mut early = summary;
    early.included_source_handoffs[0].filesystem_attempt_ordinal = 3;
    assert!(capture_verified_build_filesystem_replay_record(&early, limits).is_err());
}

#[test]
fn interleaved_output_record_rejects_invalid_descriptor_lifetimes() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut unclosed = interleaved_output_summary();
    unclosed.filesystem_operation_attempts.pop();
    assert!(capture_verified_build_filesystem_replay_record(&unclosed, limits).is_err());

    let mut retired = interleaved_output_summary();
    retired
        .filesystem_operation_attempts
        .push(retired.filesystem_operation_attempts[2].clone());
    assert!(capture_verified_build_filesystem_replay_record(&retired, limits).is_err());

    let mut reused = interleaved_output_summary();
    reused.filesystem_operation_attempts[1] = reused.filesystem_operation_attempts[0].clone();
    assert!(capture_verified_build_filesystem_replay_record(&reused, limits).is_err());

    let mut missing_path = interleaved_output_summary();
    missing_path.filesystem_operation_attempts[0]
        .rooted_path_operand_resolutions
        .clear();
    assert!(capture_verified_build_filesystem_replay_record(&missing_path, limits).is_err());
}

#[test]
fn interleaved_wire_record_rejects_early_handoff_and_reused_identity() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured =
        capture_verified_build_filesystem_replay_record(&interleaved_output_summary(), limits)
            .expect("valid stream encodes")
            .expect("complete record");
    let mut handoff_marker = OUTPUT_PATH.to_vec();
    handoff_marker.extend_from_slice(&4u64.to_le_bytes());
    let offsets = captured
        .canonical_bytes()
        .windows(handoff_marker.len())
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == handoff_marker).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1);
    let mut early = captured.canonical_bytes().to_vec();
    let ordinal = offsets[0] + OUTPUT_PATH.len();
    early[ordinal..ordinal + 8].copy_from_slice(&3u64.to_le_bytes());
    assert!(recover_review_only_build_filesystem_replay_record(&early, limits).is_err());

    // Replace every occurrence of the second descriptor's identity, retaining
    // internally agreeing create/result/close lanes but violating global freshness.
    let second_identity = 74u64.to_le_bytes();
    let offsets = captured
        .canonical_bytes()
        .windows(8)
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == second_identity).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 4);
    let mut reused = captured.canonical_bytes().to_vec();
    for position in offsets {
        reused[position..position + 8].copy_from_slice(&73u64.to_le_bytes());
    }
    assert!(recover_review_only_build_filesystem_replay_record(&reused, limits).is_err());
}

fn encoded_attempts(attempts: &[BuildFilesystemOperationAttempt]) -> Vec<u8> {
    use super::attempt_codec::{Encoder, encode_attempt};
    let mut encoder = Encoder::new(BuildFilesystemReplayRecordLimits::default().maximum_bytes);
    for attempt in attempts {
        encode_attempt(&mut encoder, attempt).expect("test attempt encodes");
    }
    encoder.finish().expect("test attempts fit")
}

#[test]
fn interleaved_wire_record_rejects_early_hard_link_and_missing_create_path() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut summary = interleaved_output_summary();
    let mut link = attempt(19, BuildFilesystemOperationResult::Scalar(0));
    for (operand_ordinal, relative_path) in [(0, OUTPUT_PATH), (1, b"alias.omg".as_slice())] {
        link.rooted_path_operand_resolutions
            .push(BuildFilesystemRootedPathOperandResolution {
                operand_ordinal,
                root: BuildFilesystemRoot::Output,
                relative_path: relative_path.to_vec(),
            });
        link.authorized_paths.push(BuildFilesystemAuthorizedPath {
            operand_ordinal,
            access: BuildFilesystemGrantAccess::Write,
            root: BuildFilesystemRoot::Output,
            relative_path: relative_path.to_vec(),
        });
    }
    summary.filesystem_operation_attempts.push(link);
    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("closed source may be linked")
        .expect("complete record");
    let encoded = encoded_attempts(&summary.filesystem_operation_attempts);
    assert!(captured.canonical_bytes().ends_with(&encoded));
    let prefix = &captured.canonical_bytes()[..captured.canonical_bytes().len() - encoded.len()];

    let mut early_link = summary.filesystem_operation_attempts.clone();
    early_link.swap(3, 4);
    let mut tampered = prefix.to_vec();
    tampered.extend(encoded_attempts(&early_link));
    assert!(recover_review_only_build_filesystem_replay_record(&tampered, limits).is_err());

    let mut missing_path = summary.filesystem_operation_attempts;
    missing_path[0].rooted_path_operand_resolutions.clear();
    let mut tampered = prefix.to_vec();
    tampered.extend(encoded_attempts(&missing_path));
    assert!(recover_review_only_build_filesystem_replay_record(&tampered, limits).is_err());
}

#[test]
fn interleaved_unrelated_events_preserve_bounded_duplicate_and_lock_pairs() {
    for duplicate_pair in [true, false] {
        let mut summary = interleaved_output_summary();
        let original = summary.filesystem_operation_attempts.clone();
        let mut first = attempt(
            if duplicate_pair { 45 } else { 46 },
            BuildFilesystemOperationResult::Scalar(0),
        );
        first.logical_handle_inputs = original[3].logical_handle_inputs.clone();
        let second = if duplicate_pair {
            let duplicate_identity = identity(75);
            first.result = BuildFilesystemOperationResult::LogicalHandle(duplicate_identity);
            first.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
                kind: BuildFilesystemLogicalHandleKind::Descriptor,
                identity: duplicate_identity,
                source: BuildFilesystemLogicalHandleOutputSource::Duplicated(identity(73)),
            });
            let mut close = original[3].clone();
            close.logical_handle_inputs[0].resolution =
                BuildFilesystemLogicalHandleInputResolution::Resolved(duplicate_identity);
            close.retired_logical_handles = vec![duplicate_identity];
            close
        } else {
            first.scalar_operands.push(BuildFilesystemScalarOperand {
                operand_ordinal: 1,
                value: BuildFilesystemScalarOperandValue::I32(6),
            });
            let mut release = first.clone();
            release.scalar_operands[0].value = BuildFilesystemScalarOperandValue::I32(8);
            release
        };
        summary.filesystem_operation_attempts = vec![
            original[0].clone(),
            first,
            original[1].clone(),
            second,
            original[2].clone(),
            original[3].clone(),
        ];
        summary.included_source_handoffs[0].filesystem_attempt_ordinal = 6;
        let limits = BuildFilesystemReplayRecordLimits::default();
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .expect("unrelated event may occur inside pair")
            .expect("complete record");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("paired record recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("paired record rehydrates");
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            summary
                .filesystem_operation_attempts
                .iter()
                .map(|attempt| attempt.operation_tag)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn output_only_record_rejects_a_non_output_prefix() {
    let mut summary = output_only_summary(3);
    summary
        .filesystem_operation_attempts
        .insert(0, attempt(10, BuildFilesystemOperationResult::Scalar(0)));
    assert!(
        capture_verified_build_filesystem_replay_record(
            &summary,
            BuildFilesystemReplayRecordLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn output_only_record_rejects_empty_attempts_and_tampered_output_shape() {
    assert!(validate_first_rung(&[]).is_err());

    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&output_only_summary(2), limits)
        .expect("exact Output-only record encodes")
        .expect("verified Output-only record retains custody");
    let mut create_prefix = Vec::new();
    create_prefix.extend_from_slice(&1u16.to_le_bytes());
    create_prefix.push(2);
    create_prefix.push(0);
    create_prefix.push(1);
    create_prefix.extend_from_slice(&73u64.to_le_bytes());
    create_prefix.extend_from_slice(&0i32.to_le_bytes());
    let offsets = captured
        .canonical_bytes()
        .windows(create_prefix.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == create_prefix).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "Output create framing is unique");

    let mut tampered = captured.canonical_bytes().to_vec();
    tampered[offsets[0] + 2] = 1;
    assert!(recover_review_only_build_filesystem_replay_record(&tampered, limits).is_err());
}

#[test]
fn replay_record_rejects_a_volatile_operation_class() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut summary = output_only_summary(2);
    summary.filesystem_operation_attempts[0].observation_class =
        crate::BuildFilesystemOperationObservationClass::Volatile;

    assert_eq!(
        capture_verified_build_filesystem_replay_record(&summary, limits).unwrap(),
        None,
        "a replay verdict cannot launder a volatile operation into a receipt"
    );
}
