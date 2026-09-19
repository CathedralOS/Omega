//! Tests for filesystem call evaluation.

use super::super::directory_entries::dirent_record_extent;
use super::super::filesystem_preparation::{
    FilesystemLogicalHandleResultSuccess, FilesystemLogicalHandleRetirementSuccess,
    FilesystemTransferCountError, MAX_FILESYSTEM_TRANSFER_BYTES,
    PreparedFilesystemLogicalHandleInput, PreparedFilesystemLogicalHandlePlan,
    PreparedFilesystemLogicalHandleRetirement, PreparedI64Output, PreparedTransferCount,
    checked_filesystem_transfer_count,
};
use super::super::{MAX_DIRECTORY_ENTRY_NAME_BYTES, MAX_DIRECTORY_SNAPSHOT_BYTES, TypedTrees};
use crate::FilesystemObservedByteRegion;
use crate::FilesystemObservedByteRegionKind;
use crate::FilesystemReturnedPathCompleteness;
use crate::FilesystemReturnedPathKind;
use crate::FilesystemRootedPathOperandResolution;
use crate::FilesystemScalarOperand;
use crate::interpreter::evaluator::BuildEvaluationSponsor;
use crate::interpreter::evaluator::Evaluator;
use crate::interpreter::evaluator::FIND_DATA_OUTPUT_BYTES;
use crate::interpreter::evaluator::FilesystemHostOperation;
use crate::interpreter::evaluator::FilesystemLogicalHandleInput;
use crate::interpreter::evaluator::FilesystemLogicalHandleInputResolution;
use crate::interpreter::evaluator::FilesystemLogicalHandleKind;
use crate::interpreter::evaluator::FilesystemObservationProvider;
use crate::interpreter::evaluator::FilesystemOperationAttempt;
use crate::interpreter::evaluator::FilesystemOperationAttemptOutcome;
use crate::interpreter::evaluator::FilesystemOperationResult;
use crate::interpreter::evaluator::Halt;
use crate::interpreter::evaluator::MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES;
use crate::interpreter::evaluator::PreparedByteOutput;
use crate::interpreter::evaluator::PreparedFilesystemCall;
use crate::interpreter::evaluator::PreparedFilesystemLogicalHandleOutput;
use crate::interpreter::evaluator::VIRTUAL_GID;
use crate::interpreter::evaluator::VIRTUAL_UID;
use crate::interpreter::evaluator::Value;
use crate::interpreter::evaluator::checked_directory_name_snapshot_total;
use crate::interpreter::evaluator::checked_directory_record_snapshot_total;
use crate::interpreter::evaluator::filesystem::filesystem_calls::checked_observation_evidence_total;
use crate::interpreter::evaluator::filesystem::replay::replay_prepared_inputs_match;
use crate::interpreter::evaluator::pack_dirent_records;
use crate::interpreter::evaluator::portable_directory_entry_name;
use crate::{
    BuildEvaluationSponsorLimits, FilesystemGrantRootIdentity, FilesystemScalarOperandValue,
};

fn evaluator_with_pending_attempt(program: &TypedTrees) -> Evaluator<'_> {
    let mut evaluator = Evaluator::new(program, &[]);
    evaluator
        .filesystem_operation_attempts
        .push(FilesystemOperationAttempt::pending(
            1,
            FilesystemObservationProvider::Virtual,
        ));
    evaluator
}

fn returned_attempt(operation: FilesystemHostOperation) -> FilesystemOperationAttempt {
    let mut attempt = FilesystemOperationAttempt::pending(
        operation.operation_tag(),
        FilesystemObservationProvider::RealScoped,
    );
    attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 0,
    });
    attempt
}

#[test]
fn owned_handle_capacity_is_reserved_before_provider_service() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    let sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(100, 100, 100, 1, 100, 100, 100, 100)
            .expect("nonzero limits"),
    );
    evaluator.build_evaluation_sponsor = Some(sponsor.clone());
    let plan = PreparedFilesystemLogicalHandlePlan {
        inputs: Vec::new(),
        input_success: None,
        output: Some(PreparedFilesystemLogicalHandleOutput::Created {
            kind: FilesystemLogicalHandleKind::Descriptor,
            success: FilesystemLogicalHandleResultSuccess::NonNegative,
        }),
        retirement: None,
    };

    let reservation = evaluator
        .reserve_prepared_live_filesystem_handle(&plan)
        .unwrap_or_else(|_| panic!("first provider call may be reserved"))
        .expect("sponsored calls own leases");
    assert_eq!(sponsor.live_filesystem_handles(), 1);
    let error = evaluator
        .reserve_prepared_live_filesystem_handle(&plan)
        .expect_err("the second provider call is refused before service");
    assert!(matches!(error, Halt::Resource(_)));
    drop(reservation);
    assert_eq!(sponsor.live_filesystem_handles(), 0);
    assert_eq!(sponsor.peak_live_filesystem_handles(), 1);
}

#[test]
fn owned_handle_close_reuses_capacity_and_teardown_releases_leases() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    let sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(100, 100, 100, 1, 100, 100, 100, 100)
            .expect("nonzero limits"),
    );
    evaluator.build_evaluation_sponsor = Some(sponsor.clone());
    let created = || PreparedFilesystemLogicalHandlePlan {
        inputs: Vec::new(),
        input_success: None,
        output: Some(PreparedFilesystemLogicalHandleOutput::Created {
            kind: FilesystemLogicalHandleKind::Descriptor,
            success: FilesystemLogicalHandleResultSuccess::NonNegative,
        }),
        retirement: None,
    };

    let mut lease = evaluator
        .reserve_prepared_live_filesystem_handle(&created())
        .unwrap_or_else(|_| panic!("first open reserves before provider entry"));
    evaluator
        .complete_logical_handle_observations(0, &created(), 3, &mut lease)
        .unwrap_or_else(|_| panic!("successful open transfers its lease"));
    assert!(lease.is_none());
    assert_eq!(sponsor.live_filesystem_handles(), 1);
    assert!(
        evaluator
            .reserve_prepared_live_filesystem_handle(&created())
            .is_err()
    );

    evaluator
        .filesystem_operation_attempts
        .push(FilesystemOperationAttempt::pending(
            2,
            FilesystemObservationProvider::Virtual,
        ));
    evaluator.record_prepared_filesystem_logical_handle_input(
        1,
        0,
        FilesystemLogicalHandleKind::Descriptor,
        3,
        false,
    );
    let close = PreparedFilesystemLogicalHandlePlan {
        inputs: vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            raw: 3,
            null_allowed: false,
        }],
        input_success: Some(FilesystemLogicalHandleResultSuccess::Zero),
        output: None,
        retirement: Some(PreparedFilesystemLogicalHandleRetirement {
            operand_ordinal: 0,
            success: FilesystemLogicalHandleRetirementSuccess::Zero,
        }),
    };
    evaluator
        .complete_logical_handle_observations(1, &close, 0, &mut None)
        .unwrap_or_else(|_| panic!("successful close retires its lease"));
    assert_eq!(sponsor.live_filesystem_handles(), 0);

    let mut replacement = evaluator
        .reserve_prepared_live_filesystem_handle(&created())
        .unwrap_or_else(|_| panic!("close makes capacity reusable"));
    evaluator
        .filesystem_operation_attempts
        .push(FilesystemOperationAttempt::pending(
            3,
            FilesystemObservationProvider::Virtual,
        ));
    evaluator
        .complete_logical_handle_observations(2, &created(), 4, &mut replacement)
        .unwrap_or_else(|_| panic!("replacement open transfers its lease"));
    assert_eq!(sponsor.live_filesystem_handles(), 1);
    drop(evaluator);
    assert_eq!(sponsor.live_filesystem_handles(), 0);
    assert_eq!(sponsor.peak_live_filesystem_handles(), 1);
}

#[test]
fn borrowed_native_view_uses_its_descriptor_lease() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    let sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(100, 100, 100, 1, 100, 100, 100, 100)
            .expect("nonzero limits"),
    );
    evaluator.build_evaluation_sponsor = Some(sponsor.clone());
    let created = PreparedFilesystemLogicalHandlePlan {
        inputs: Vec::new(),
        input_success: None,
        output: Some(PreparedFilesystemLogicalHandleOutput::Created {
            kind: FilesystemLogicalHandleKind::Descriptor,
            success: FilesystemLogicalHandleResultSuccess::NonNegative,
        }),
        retirement: None,
    };
    let mut lease = evaluator
        .reserve_prepared_live_filesystem_handle(&created)
        .unwrap_or_else(|_| panic!("descriptor reserves one resource"));
    evaluator
        .complete_logical_handle_observations(0, &created, 3, &mut lease)
        .unwrap_or_else(|_| panic!("descriptor is retained"));

    evaluator
        .filesystem_operation_attempts
        .push(FilesystemOperationAttempt::pending(
            2,
            FilesystemObservationProvider::Virtual,
        ));
    evaluator.record_prepared_filesystem_logical_handle_input(
        1,
        0,
        FilesystemLogicalHandleKind::Descriptor,
        3,
        false,
    );
    let borrowed = PreparedFilesystemLogicalHandlePlan {
        inputs: vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            raw: 3,
            null_allowed: false,
        }],
        input_success: Some(FilesystemLogicalHandleResultSuccess::NonNegative),
        output: Some(PreparedFilesystemLogicalHandleOutput::Borrowed {
            source_operand_ordinal: 0,
            success: FilesystemLogicalHandleResultSuccess::NonNegative,
        }),
        retirement: None,
    };
    let mut borrowed_lease = evaluator
        .reserve_prepared_live_filesystem_handle(&borrowed)
        .unwrap_or_else(|_| panic!("borrow does not reserve"));
    assert!(borrowed_lease.is_none());
    evaluator
        .complete_logical_handle_observations(1, &borrowed, 103, &mut borrowed_lease)
        .unwrap_or_else(|_| panic!("native view borrows descriptor lifetime"));
    assert_eq!(sponsor.live_filesystem_handles(), 1);

    evaluator
        .filesystem_operation_attempts
        .push(FilesystemOperationAttempt::pending(
            3,
            FilesystemObservationProvider::Virtual,
        ));
    evaluator.record_prepared_filesystem_logical_handle_input(
        2,
        0,
        FilesystemLogicalHandleKind::Native,
        103,
        false,
    );
    let close_borrowed = PreparedFilesystemLogicalHandlePlan {
        inputs: vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Native,
            raw: 103,
            null_allowed: false,
        }],
        input_success: Some(FilesystemLogicalHandleResultSuccess::NonZero),
        output: None,
        retirement: Some(PreparedFilesystemLogicalHandleRetirement {
            operand_ordinal: 0,
            success: FilesystemLogicalHandleRetirementSuccess::NonZero,
        }),
    };
    evaluator
        .complete_logical_handle_observations(2, &close_borrowed, 1, &mut None)
        .unwrap_or_else(|_| panic!("closing borrowed view retires its owner"));
    assert_eq!(sponsor.live_filesystem_handles(), 0);
}

#[test]
fn replay_cursor_rejects_reordered_extra_and_missing_events() {
    let program = TypedTrees::default();
    let directory = crate::FilesystemOutputDirectoryReplayRecord::new(
        FilesystemGrantRootIdentity::new(1).expect("nonzero Output root"),
        b"generated".to_vec(),
    )
    .expect("canonical directory");
    let record = crate::FilesystemInputOutputTreeReplayRecord::output_only(
        vec![crate::FilesystemOutputTreeEntryReplayRecord::Directory(
            directory,
        )],
        Vec::new(),
    )
    .expect("one complete Output event");
    let replay =
        crate::FilesystemReplay::from_input_output_tree_record(record).expect("validated replay");
    let expected = replay.attempts()[0].clone();
    let mut evaluator = Evaluator::new(&program, &[]);
    evaluator.filesystem_replay = Some(replay);

    assert!(matches!(
        evaluator.expected_filesystem_replay_attempt(0, FilesystemHostOperation::CreateDir),
        Ok(Some(actual)) if actual == expected
    ));
    assert!(matches!(
        evaluator.expected_filesystem_replay_attempt(0, FilesystemHostOperation::Read),
        Err(Halt::Trap(message)) if message.contains("changed order")
    ));
    assert!(matches!(
        evaluator.expected_filesystem_replay_attempt(1, FilesystemHostOperation::CreateDir),
        Err(Halt::Trap(message)) if message.contains("extra event")
    ));
    assert!(matches!(
        evaluator.finish_filesystem_replay(),
        Err(Halt::Trap(message)) if message.contains("record contains 1")
    ));
    evaluator.filesystem_operation_attempts.push(expected);
    assert!(
        evaluator
            .serve_filesystem_call(PreparedFilesystemCall::CreateDir {
                path: b"/root/1/generated".to_vec(),
                mode: crate::FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE,
            })
            .is_ok(),
        "the completed event also establishes its Output namespace"
    );
    assert!(evaluator.finish_filesystem_replay().is_ok());
}

#[test]
fn replay_preparation_comparison_rejects_changed_input_lanes() {
    let mut current = returned_attempt(FilesystemHostOperation::Open);
    current.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 1,
        value: FilesystemScalarOperandValue::I32(0),
    });
    let mut changed = current.clone();
    changed.scalar_operands[0].value = FilesystemScalarOperandValue::I32(1);
    assert!(replay_prepared_inputs_match(&current, &current));
    assert!(!replay_prepared_inputs_match(&current, &changed));

    changed = current.clone();
    changed
        .rooted_path_operand_resolutions
        .push(FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: FilesystemGrantRootIdentity::new(1).unwrap(),
            relative_path: b"changed.omg".to_vec(),
        });
    assert!(!replay_prepared_inputs_match(&current, &changed));
}

#[test]
fn transfer_count_rejects_wrap_and_unbounded_allocation() {
    assert_eq!(
        checked_filesystem_transfer_count(-1),
        Err(FilesystemTransferCountError::NegativeOrUnrepresentable)
    );
    assert_eq!(
        checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64 + 1),
        Err(FilesystemTransferCountError::ExceedsEvaluatorLimit)
    );
}

#[test]
fn transfer_count_accepts_the_closed_interval_through_the_limit() {
    assert_eq!(
        checked_filesystem_transfer_count(0),
        Ok(PreparedTransferCount { raw: 0, host: 0 })
    );
    assert_eq!(
        checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64),
        Ok(PreparedTransferCount {
            raw: MAX_FILESYSTEM_TRANSFER_BYTES as u64,
            host: MAX_FILESYSTEM_TRANSFER_BYTES,
        })
    );
}

#[test]
fn directory_snapshot_payload_lanes_accept_the_exact_ceiling_only() {
    assert_eq!(
        checked_directory_name_snapshot_total(MAX_DIRECTORY_SNAPSHOT_BYTES - 1, 1)
            .unwrap_or_else(|_| panic!("exact retained-name ceiling is admitted")),
        MAX_DIRECTORY_SNAPSHOT_BYTES
    );
    assert!(matches!(
        checked_directory_name_snapshot_total(MAX_DIRECTORY_SNAPSHOT_BYTES, 1),
        Err(Halt::Resource(_))
    ));

    let one_byte_name_record =
        dirent_record_extent(1).unwrap_or_else(|_| panic!("one-byte dirent extent"));
    assert_eq!(one_byte_name_record, 32);
    assert_eq!(
        checked_directory_record_snapshot_total(
            MAX_DIRECTORY_SNAPSHOT_BYTES - one_byte_name_record,
            1,
        )
        .unwrap_or_else(|_| panic!("exact packed-record ceiling is admitted")),
        MAX_DIRECTORY_SNAPSHOT_BYTES
    );
    assert!(matches!(
        checked_directory_record_snapshot_total(
            MAX_DIRECTORY_SNAPSHOT_BYTES - one_byte_name_record + 1,
            1,
        ),
        Err(Halt::Resource(_))
    ));
}

#[test]
fn dirent_layout_truncates_names_to_the_existing_std_carrier() {
    assert_eq!(
        dirent_record_extent(MAX_DIRECTORY_ENTRY_NAME_BYTES)
            .unwrap_or_else(|_| panic!("largest portable directory record")),
        280
    );
    let long_name = vec![b'x'; MAX_DIRECTORY_ENTRY_NAME_BYTES + 1];
    assert_eq!(
        portable_directory_entry_name(&long_name).len(),
        MAX_DIRECTORY_ENTRY_NAME_BYTES
    );
    let records = pack_dirent_records(&[(long_name, 8)])
        .unwrap_or_else(|_| panic!("packer applies std name truncation"));
    assert_eq!(records.len(), 280);
    assert_eq!(u16::from_le_bytes([records[18], records[19]]), 255);
    assert!(
        records[21..21 + MAX_DIRECTORY_ENTRY_NAME_BYTES]
            .iter()
            .all(|byte| *byte == b'x')
    );
}

#[test]
fn virtual_directory_snapshot_truncates_names_before_retention_and_packing() {
    let program = TypedTrees::default();
    let mut evaluator = Evaluator::new(&program, &[]);
    evaluator.virtual_dirs.insert(b"dir".to_vec());
    let mut path = b"dir/".to_vec();
    path.extend(std::iter::repeat_n(
        b'x',
        MAX_DIRECTORY_ENTRY_NAME_BYTES + 1,
    ));
    evaluator.virtual_files.insert(path, Vec::new());

    let records = evaluator
        .build_dirent_records(b"dir")
        .unwrap_or_else(|_| panic!("long virtual name packs through std truncation"));
    let child_start = 64;
    assert_eq!(
        u16::from_le_bytes([records[child_start + 18], records[child_start + 19]]),
        255
    );
    let entries = evaluator
        .build_find_entries(b"dir")
        .unwrap_or_else(|_| panic!("long virtual name enters a bounded find snapshot"));
    assert_eq!(entries[2].0.len(), MAX_DIRECTORY_ENTRY_NAME_BYTES);
}

#[test]
fn virtual_hard_link_variants_copy_all_modeled_file_metadata() {
    let program = TypedTrees::default();
    let mut evaluator = Evaluator::new(&program, &[]);
    let source = b"source".to_vec();
    let unix_link = b"unix-link".to_vec();
    let windows_link = b"windows-link".to_vec();
    evaluator
        .virtual_files
        .insert(source.clone(), b"payload".to_vec());
    evaluator.virtual_perms.insert(source.clone(), 0o751);
    evaluator.virtual_times.insert(source.clone(), 1_234_567);

    let unix_result = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::HardLink {
            original: source.clone(),
            link: unix_link.clone(),
        })
        .unwrap_or_else(|_| panic!("virtual unix hard link is served"));
    assert!(matches!(unix_result, Value::Int(0)));

    let windows_result = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::CreateHardLink {
            link: windows_link.clone(),
            existing: source,
            security_attributes: 0,
        })
        .unwrap_or_else(|_| panic!("virtual Win32 hard link is served"));
    assert!(matches!(windows_result, Value::Int(1)));

    for link in [unix_link, windows_link] {
        assert_eq!(
            evaluator.virtual_files.get(&link),
            Some(&b"payload".to_vec())
        );
        assert_eq!(evaluator.virtual_perms.get(&link), Some(&0o751));
        assert_eq!(evaluator.virtual_times.get(&link), Some(&1_234_567));
    }
}

#[test]
fn virtual_change_file_owner_preserves_exact_failure_and_error_sequence() {
    let program = TypedTrees::default();
    let mut evaluator = Evaluator::new(&program, &[]);
    let fd = evaluator.virtual_open(b"owned.bin".to_vec(), true, true);

    let unchanged = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::ChangeFileOwner {
            fd,
            uid: -1,
            gid: -1,
        })
        .unwrap_or_else(|_| panic!("unchanged ownership is served"));
    assert!(matches!(unchanged, Value::Int(0)));
    assert_eq!(evaluator.virtual_errno, 0);

    let denied = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::ChangeFileOwner { fd, uid: 0, gid: 0 })
        .unwrap_or_else(|_| panic!("denied ownership is served"));
    assert!(matches!(denied, Value::Int(-1)));
    assert_eq!(evaluator.virtual_errno, 1);

    let current = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::ChangeFileOwner {
            fd,
            uid: VIRTUAL_UID as i32,
            gid: VIRTUAL_GID as i32,
        })
        .unwrap_or_else(|_| panic!("current ownership is served"));
    assert!(matches!(current, Value::Int(0)));
    assert_eq!(
        evaluator.virtual_errno, 1,
        "successful ownership changes do not erase prior provider error state"
    );
    assert!(evaluator.virtual_fds.contains_key(&fd));
}

#[test]
fn virtual_hard_link_variants_reject_all_occupied_destination_kinds() {
    let program = TypedTrees::default();
    let mut evaluator = Evaluator::new(&program, &[]);
    let source = b"source".to_vec();
    let file = b"occupied-file".to_vec();
    let directory = b"occupied-directory".to_vec();
    let symlink = b"occupied-symlink".to_vec();
    evaluator
        .virtual_files
        .insert(source.clone(), b"source bytes".to_vec());
    evaluator
        .virtual_files
        .insert(file.clone(), b"existing bytes".to_vec());
    evaluator.virtual_dirs.insert(directory.clone());
    evaluator
        .virtual_symlinks
        .insert(symlink.clone(), b"target".to_vec());
    evaluator.virtual_perms.insert(file.clone(), 0o600);
    evaluator.virtual_times.insert(file.clone(), 99);
    evaluator.virtual_perms.insert(directory.clone(), 0o700);
    evaluator.virtual_times.insert(directory.clone(), 100);

    for destination in [&file, &directory, &symlink] {
        let result = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::HardLink {
                original: source.clone(),
                link: destination.clone(),
            })
            .unwrap_or_else(|_| panic!("virtual unix collision is served"));
        assert!(matches!(result, Value::Int(-1)));
        assert_eq!(evaluator.virtual_errno, 17);
    }

    for destination in [&file, &directory, &symlink] {
        let result = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::CreateHardLink {
                link: destination.clone(),
                existing: source.clone(),
                security_attributes: 0,
            })
            .unwrap_or_else(|_| panic!("virtual Win32 collision is served"));
        assert!(matches!(result, Value::Int(0)));
        assert_eq!(evaluator.virtual_errno, 183);
    }

    assert_eq!(
        evaluator.virtual_files.get(&file),
        Some(&b"existing bytes".to_vec())
    );
    assert!(!evaluator.virtual_files.contains_key(&directory));
    assert!(!evaluator.virtual_files.contains_key(&symlink));
    assert_eq!(
        evaluator.virtual_symlinks.get(&symlink),
        Some(&b"target".to_vec())
    );
    assert_eq!(evaluator.virtual_perms.get(&file), Some(&0o600));
    assert_eq!(evaluator.virtual_times.get(&file), Some(&99));
    assert_eq!(evaluator.virtual_perms.get(&directory), Some(&0o700));
    assert_eq!(evaluator.virtual_times.get(&directory), Some(&100));
    assert!(!evaluator.virtual_perms.contains_key(&symlink));
    assert!(!evaluator.virtual_times.contains_key(&symlink));
}

#[test]
fn observation_evidence_budget_accepts_exact_limit_and_rejects_overflow() {
    assert_eq!(
        checked_observation_evidence_total(MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 1, 1),
        Some(MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES)
    );
    assert_eq!(
        checked_observation_evidence_total(MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES, 1),
        None
    );
    assert_eq!(checked_observation_evidence_total(usize::MAX, 1), None);
}

fn mutable_byte_output(bytes: &[u8]) -> PreparedByteOutput {
    PreparedByteOutput::Array(
        bytes
            .iter()
            .map(|byte| Value::Int(i64::from(*byte)).cell())
            .collect(),
    )
}

#[test]
fn mutable_resolution_prefix_survives_its_own_capacity_failure() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    let output = mutable_byte_output(&[3, 1, 4]);

    evaluator
        .record_prepared_filesystem_mutable_byte_operand_resolution(0, 5, &output)
        .unwrap_or_else(|_| panic!("resolution fixture must be representable"));
    evaluator.record_prepared_filesystem_mutable_i64_operand_resolution(0, 6, -9);
    assert!(output.require_capacity(4).is_err());

    let attempt = &evaluator.filesystem_operation_attempts[0];
    assert_eq!(attempt.mutable_byte_operand_resolutions.len(), 1);
    assert_eq!(
        attempt.mutable_byte_operand_resolutions[0].operand_ordinal(),
        5
    );
    assert_eq!(
        attempt.mutable_byte_operand_resolutions[0].bytes(),
        &[3, 1, 4]
    );
    assert_eq!(attempt.mutable_i64_operand_resolutions.len(), 1);
    assert_eq!(
        attempt.mutable_i64_operand_resolutions[0].operand_ordinal(),
        6
    );
    assert_eq!(attempt.mutable_i64_operand_resolutions[0].value(), -9);
    assert!(attempt.mutable_byte_operands.is_empty());
    assert!(attempt.mutable_i64_operands.is_empty());
    assert_eq!(evaluator.filesystem_observation_evidence_bytes, 3);
}

#[test]
fn completed_mutable_plan_accepts_alias_drift_and_retains_all_three_byte_snapshots() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    let output = mutable_byte_output(&[1, 2]);
    evaluator
        .record_prepared_filesystem_mutable_byte_operand_resolution(0, 1, &output)
        .unwrap_or_else(|_| panic!("resolution fixture must be representable"));

    output
        .write(&[8, 9])
        .unwrap_or_else(|_| panic!("alias fixture write must fit"));
    let call = PreparedFilesystemCall::Read {
        fd: 3,
        buffer: output.clone(),
        count: PreparedTransferCount { raw: 1, host: 1 },
    };
    let (scalar_operands, byte_operands, path_like_operands) = call.operand_observation_plan();
    evaluator.filesystem_operation_attempts[0].scalar_operands = scalar_operands.clone();
    let plan = call
        .mutable_observation_plan()
        .unwrap_or_else(|_| panic!("mutable fixture must be representable"));
    evaluator
        .record_operand_observations(
            0,
            scalar_operands,
            byte_operands,
            path_like_operands,
            &[],
            &plan,
        )
        .unwrap_or_else(|_| panic!("matching resolution roles must validate"));

    assert_eq!(
        evaluator.filesystem_operation_attempts[0].mutable_byte_operand_resolutions[0].bytes(),
        &[1, 2],
        "resolution-time contents must not be overwritten by provider pre-state"
    );
    assert_eq!(
        evaluator.filesystem_operation_attempts[0].mutable_byte_operands[0].pre_bytes(),
        &[8, 9]
    );
    assert_eq!(
        evaluator.filesystem_observation_evidence_bytes, 6,
        "resolution, provider pre-state, and provider post-state share one sponsor"
    );

    output
        .write(&[7])
        .unwrap_or_else(|_| panic!("provider fixture write must fit"));
    evaluator
        .complete_mutable_observations(0, &plan)
        .unwrap_or_else(|_| panic!("completed fixture must be representable"));
    let attempt = &evaluator.filesystem_operation_attempts[0];
    assert_eq!(attempt.mutable_byte_operand_resolutions[0].bytes(), &[1, 2]);
    assert_eq!(attempt.mutable_byte_operands[0].pre_bytes(), &[8, 9]);
    assert_eq!(attempt.mutable_byte_operands[0].post_bytes(), &[7, 9]);
}

#[test]
fn completed_rooted_path_sidecar_requires_exact_portable_coordinates() {
    let program = TypedTrees::default();
    let root = crate::FilesystemGrantRootIdentity::new(1)
        .unwrap_or_else(|| panic!("fixture root identity is nonzero"));
    let resolution = FilesystemRootedPathOperandResolution {
        operand_ordinal: 0,
        root,
        relative_path: b"inputs/table.txt".to_vec(),
    };
    let call = PreparedFilesystemCall::Remove {
        path: b"/physical/source/inputs/table.txt".to_vec(),
    };
    let (scalar_operands, byte_operands, path_like_operands) = call.operand_observation_plan();
    let mutable_plan = call
        .mutable_observation_plan()
        .unwrap_or_else(|_| panic!("remove has no mutable observation failure"));

    let mut matching = evaluator_with_pending_attempt(&program);
    matching
        .record_prepared_filesystem_rooted_path_operand_resolution(0, resolution.clone())
        .unwrap_or_else(|_| panic!("rooted fixture fits evidence sponsor"));
    matching
        .record_operand_observations(
            0,
            scalar_operands.clone(),
            byte_operands.clone(),
            path_like_operands.clone(),
            std::slice::from_ref(&resolution),
            &mutable_plan,
        )
        .unwrap_or_else(|_| panic!("exact rooted sidecar must validate"));

    let mut mismatching = evaluator_with_pending_attempt(&program);
    mismatching
        .record_prepared_filesystem_rooted_path_operand_resolution(0, resolution)
        .unwrap_or_else(|_| panic!("rooted fixture fits evidence sponsor"));
    let wrong = FilesystemRootedPathOperandResolution {
        operand_ordinal: 0,
        root,
        relative_path: b"inputs/other.txt".to_vec(),
    };
    assert!(
        mismatching
            .record_operand_observations(
                0,
                scalar_operands,
                byte_operands,
                path_like_operands,
                &[wrong],
                &mutable_plan,
            )
            .is_err(),
        "physical provider bytes must not substitute for exact portable rooted coordinates"
    );
}

#[test]
fn rooted_path_resolution_budget_failure_is_atomic() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.filesystem_observation_evidence_bytes = MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 2;
    let resolution = FilesystemRootedPathOperandResolution {
        operand_ordinal: 0,
        root: crate::FilesystemGrantRootIdentity::new(1)
            .unwrap_or_else(|| panic!("fixture root identity is nonzero")),
        relative_path: b"abc".to_vec(),
    };

    assert!(
        evaluator
            .record_prepared_filesystem_rooted_path_operand_resolution(0, resolution)
            .is_err()
    );
    assert_eq!(
        evaluator.filesystem_observation_evidence_bytes,
        MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 2
    );
    assert!(
        evaluator.filesystem_operation_attempts[0]
            .rooted_path_operand_resolutions
            .is_empty()
    );
}

#[test]
fn returned_path_rows_retain_exact_bytes_and_completeness() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.filesystem_operation_attempt_stack.push(0);
    evaluator
        .record_returned_path_observation(
            1,
            FilesystemReturnedPathKind::ReadLinkPayload,
            FilesystemReturnedPathCompleteness::Complete,
            b"ab",
        )
        .unwrap_or_else(|_| panic!("complete returned path fits sponsor"));
    evaluator
        .record_returned_path_observation(
            1,
            FilesystemReturnedPathKind::ReadLinkPayload,
            FilesystemReturnedPathCompleteness::LimitReached,
            b"prefix",
        )
        .unwrap_or_else(|_| panic!("limited returned path fits sponsor"));
    let [returned, limited] = &evaluator.filesystem_operation_attempts[0].returned_paths[..] else {
        panic!("fixture retains both returned paths")
    };
    assert_eq!(
        returned.kind(),
        crate::FilesystemReturnedPathKind::ReadLinkPayload
    );
    assert_eq!(returned.operand_ordinal(), 1);
    assert_eq!(returned.bytes(), b"ab");
    assert_eq!(
        returned.completeness(),
        crate::FilesystemReturnedPathCompleteness::Complete
    );
    assert_eq!(
        limited.completeness(),
        crate::FilesystemReturnedPathCompleteness::LimitReached
    );
    assert_eq!(limited.bytes(), b"prefix");
    assert_eq!(evaluator.filesystem_observation_evidence_bytes, 8);
}

#[test]
fn virtual_read_link_provider_distinguishes_exact_fit_from_truncation() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.filesystem_operation_attempt_stack.push(0);
    evaluator
        .virtual_symlinks
        .insert(b"exact".to_vec(), b"abcd".to_vec());
    evaluator
        .virtual_symlinks
        .insert(b"limited".to_vec(), b"abcdef".to_vec());

    let exact_output = mutable_byte_output(&[9; 6]);
    let exact_result = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::ReadLink {
            path: b"exact".to_vec(),
            buffer: exact_output.clone(),
            count: PreparedTransferCount { raw: 4, host: 4 },
        })
        .unwrap_or_else(|_| panic!("exact-fit read_link succeeds"));
    assert!(matches!(exact_result, Value::Int(4)));
    assert_eq!(
        exact_output
            .snapshot()
            .unwrap_or_else(|_| panic!("exact output remains readable")),
        b"abcd\x09\x09"
    );

    let limited_output = mutable_byte_output(&[9; 6]);
    let limited_result = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::ReadLink {
            path: b"limited".to_vec(),
            buffer: limited_output.clone(),
            count: PreparedTransferCount { raw: 4, host: 4 },
        })
        .unwrap_or_else(|_| panic!("truncated read_link succeeds"));
    assert!(matches!(limited_result, Value::Int(4)));
    assert_eq!(
        limited_output
            .snapshot()
            .unwrap_or_else(|_| panic!("limited output remains readable")),
        b"abcd\x09\x09"
    );

    let [exact, limited] = &evaluator.filesystem_operation_attempts[0].returned_paths[..] else {
        panic!("provider records both successful writes")
    };
    assert_eq!(exact.bytes(), b"abcd");
    assert_eq!(
        exact.completeness(),
        FilesystemReturnedPathCompleteness::Complete
    );
    assert_eq!(limited.bytes(), b"abcd");
    assert_eq!(
        limited.completeness(),
        FilesystemReturnedPathCompleteness::LimitReached
    );
}

#[test]
fn returned_path_budget_failure_is_atomic() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.filesystem_operation_attempt_stack.push(0);
    evaluator.filesystem_observation_evidence_bytes = MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 1;

    assert!(
        evaluator
            .record_returned_path_observation(
                1,
                FilesystemReturnedPathKind::ReadLinkPayload,
                FilesystemReturnedPathCompleteness::Complete,
                b"ab",
            )
            .is_err()
    );
    assert_eq!(
        evaluator.filesystem_observation_evidence_bytes,
        MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 1
    );
    assert!(
        evaluator.filesystem_operation_attempts[0]
            .returned_paths
            .is_empty()
    );
}

#[test]
fn virtual_file_reads_retain_only_exact_observed_bytes() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.filesystem_operation_attempt_stack.push(0);
    evaluator
        .virtual_files
        .insert(b"input".to_vec(), b"abcdef".to_vec());
    let descriptor = evaluator.virtual_open(b"input".to_vec(), false, false);

    let sequential_output = mutable_byte_output(&[9; 8]);
    let sequential = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::Read {
            fd: descriptor,
            buffer: sequential_output.clone(),
            count: PreparedTransferCount { raw: 4, host: 4 },
        })
        .unwrap_or_else(|_| panic!("sequential read succeeds"));
    assert!(matches!(sequential, Value::Int(4)));
    assert_eq!(
        sequential_output
            .snapshot()
            .unwrap_or_else(|_| panic!("sequential output remains readable")),
        b"abcd\x09\x09\x09\x09"
    );

    let positioned_output = mutable_byte_output(&[9; 8]);
    let positioned = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::ReadAt {
            fd: descriptor,
            buffer: positioned_output,
            count: PreparedTransferCount { raw: 3, host: 3 },
            offset: 1,
        })
        .unwrap_or_else(|_| panic!("positioned read succeeds"));
    assert!(matches!(positioned, Value::Int(3)));

    let tail_output = mutable_byte_output(&[9; 8]);
    let tail = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::Read {
            fd: descriptor,
            buffer: tail_output,
            count: PreparedTransferCount { raw: 8, host: 8 },
        })
        .unwrap_or_else(|_| panic!("tail read succeeds"));
    assert!(matches!(tail, Value::Int(2)));

    let eof_output = mutable_byte_output(&[9; 8]);
    let eof = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::Read {
            fd: descriptor,
            buffer: eof_output,
            count: PreparedTransferCount { raw: 8, host: 8 },
        })
        .unwrap_or_else(|_| panic!("EOF read succeeds"));
    assert!(matches!(eof, Value::Int(0)));

    let failed_output = mutable_byte_output(&[9; 8]);
    let failed = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::Read {
            fd: -1,
            buffer: failed_output.clone(),
            count: PreparedTransferCount { raw: 8, host: 8 },
        })
        .unwrap_or_else(|_| panic!("unknown descriptor is a provider error"));
    assert!(matches!(failed, Value::Int(-1)));
    assert_eq!(
        failed_output
            .snapshot()
            .unwrap_or_else(|_| panic!("failed output remains readable")),
        &[9; 8]
    );

    let rows = &evaluator.filesystem_operation_attempts[0].observed_byte_regions;
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().all(|row| row.output_operand_ordinal() == 1));
    assert_eq!(
        rows[0].kind(),
        FilesystemObservedByteRegionKind::SequentialFileRead
    );
    assert_eq!(
        rows[1].kind(),
        FilesystemObservedByteRegionKind::PositionedFileRead
    );
    assert_eq!(
        rows[2].kind(),
        FilesystemObservedByteRegionKind::SequentialFileRead
    );
    assert_eq!(
        rows[3].kind(),
        FilesystemObservedByteRegionKind::SequentialFileRead
    );
    assert_eq!(rows[0].offset(), 0);
    assert_eq!(rows[0].length(), 4);
    assert_eq!(rows[1].length(), 3);
    assert_eq!(rows[2].length(), 2);
    assert_eq!(rows[3].length(), 0);
    assert_eq!(
        evaluator.filesystem_observation_evidence_bytes, 0,
        "semantic regions reference the already-custodied mutable post-state"
    );
}

#[test]
fn observed_byte_region_validation_binds_result_kind_and_post_carrier() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.filesystem_operation_attempts[0]
        .observed_byte_regions
        .push(FilesystemObservedByteRegion {
            output_operand_ordinal: 1,
            kind: FilesystemObservedByteRegionKind::SequentialFileRead,
            offset: 0,
            length: 4,
        });
    evaluator.filesystem_operation_attempts[0]
        .mutable_byte_operands
        .push(crate::FilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: vec![9; 6],
            post_bytes: b"abcd\x09\x09".to_vec(),
        });

    evaluator
        .validate_observed_byte_regions(0, FilesystemHostOperation::Read, 4)
        .unwrap_or_else(|_| panic!("exact read region validates"));
    assert!(
        evaluator
            .validate_observed_byte_regions(0, FilesystemHostOperation::Read, 3)
            .is_err(),
        "scalar result must equal semantic region length"
    );
    assert!(
        evaluator
            .validate_observed_byte_regions(0, FilesystemHostOperation::ReadAt, 4)
            .is_err(),
        "positioned and sequential read regions are distinct"
    );
    evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].kind =
        FilesystemObservedByteRegionKind::DirectoryRecords;
    evaluator
        .validate_observed_byte_regions(0, FilesystemHostOperation::ReadDir, 4)
        .unwrap_or_else(|_| panic!("directory record region validates"));

    evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].kind =
        FilesystemObservedByteRegionKind::FindEntry;
    evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].length =
        FIND_DATA_OUTPUT_BYTES;
    evaluator.filesystem_operation_attempts[0].mutable_byte_operands[0]
        .pre_bytes
        .resize(FIND_DATA_OUTPUT_BYTES, 9);
    evaluator.filesystem_operation_attempts[0].mutable_byte_operands[0]
        .post_bytes
        .resize(FIND_DATA_OUTPUT_BYTES, 0);
    evaluator
        .validate_observed_byte_regions(0, FilesystemHostOperation::FindFirst, 7)
        .unwrap_or_else(|_| panic!("find_first record region validates"));
    evaluator
        .validate_observed_byte_regions(0, FilesystemHostOperation::FindNext, 1)
        .unwrap_or_else(|_| panic!("find_next record region validates"));
    evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].length = 0;
    evaluator
        .validate_observed_byte_regions(0, FilesystemHostOperation::FindNext, 0)
        .unwrap_or_else(|_| panic!("find_next empty region validates"));

    evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].kind =
        FilesystemObservedByteRegionKind::SequentialFileRead;
    evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].length = 4;
    evaluator.filesystem_operation_attempts[0]
        .mutable_byte_operands
        .clear();
    assert!(
        evaluator
            .validate_observed_byte_regions(0, FilesystemHostOperation::Read, 4)
            .is_err(),
        "semantic region requires its retained post-carrier"
    );
}

#[test]
fn virtual_directory_enumeration_designates_exact_output_regions() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.filesystem_operation_attempt_stack.push(0);
    evaluator.virtual_dirs.insert(b"dir".to_vec());
    evaluator
        .virtual_files
        .insert(b"dir/entry".to_vec(), b"payload".to_vec());
    let descriptor = evaluator.virtual_open_flags(b"dir".to_vec(), 0);
    assert!(descriptor >= 0);

    let directory_output = mutable_byte_output(&[9; 512]);
    let directory = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::ReadDir {
            fd: descriptor,
            buffer: directory_output.clone(),
            count: PreparedTransferCount {
                raw: 512,
                host: 512,
            },
            position: PreparedI64Output::test_fixture(0),
        })
        .unwrap_or_else(|_| panic!("directory enumeration succeeds"));
    let Value::Int(directory_length) = directory else {
        panic!("read_dir returns a byte length")
    };
    assert!(directory_length > 0);
    let directory_length = usize::try_from(directory_length)
        .unwrap_or_else(|_| panic!("directory fixture length is representable"));
    let directory_bytes = directory_output
        .snapshot()
        .unwrap_or_else(|_| panic!("directory output remains readable"));
    assert!(
        directory_bytes[directory_length..]
            .iter()
            .all(|byte| *byte == 9)
    );

    let eof_output = mutable_byte_output(&[9; 512]);
    let eof = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::ReadDir {
            fd: descriptor,
            buffer: eof_output.clone(),
            count: PreparedTransferCount {
                raw: 512,
                host: 512,
            },
            position: PreparedI64Output::test_fixture(i64::MAX),
        })
        .unwrap_or_else(|_| panic!("directory EOF succeeds"));
    assert!(matches!(eof, Value::Int(0)));
    assert_eq!(
        eof_output
            .snapshot()
            .unwrap_or_else(|_| panic!("EOF output remains readable")),
        &[9; 512]
    );

    let first_output = mutable_byte_output(&[9; 322]);
    let first = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::FindFirst {
            pattern: b"dir/*".to_vec(),
            data: first_output.clone(),
        })
        .unwrap_or_else(|_| panic!("find_first succeeds"));
    let Value::Int(find_handle) = first else {
        panic!("find_first returns its handle")
    };
    assert!(find_handle >= 0);
    assert_eq!(
        &first_output
            .snapshot()
            .unwrap_or_else(|_| panic!("first find output remains readable"))
            [FIND_DATA_OUTPUT_BYTES..],
        &[9, 9]
    );

    let next_output = mutable_byte_output(&[9; 322]);
    let next = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::FindNext {
            handle: find_handle,
            data: next_output,
        })
        .unwrap_or_else(|_| panic!("find_next succeeds"));
    assert!(matches!(next, Value::Int(1)));

    let empty_output = mutable_byte_output(&[9; 322]);
    let empty = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::FindNext {
            handle: -1,
            data: empty_output.clone(),
        })
        .unwrap_or_else(|_| panic!("unknown find handle returns no entry"));
    assert!(matches!(empty, Value::Int(0)));
    assert_eq!(
        empty_output
            .snapshot()
            .unwrap_or_else(|_| panic!("empty find output remains readable")),
        &[9; 322]
    );

    let rows = &evaluator.filesystem_operation_attempts[0].observed_byte_regions;
    assert_eq!(rows.len(), 5);
    assert_eq!(
        rows[0].kind(),
        FilesystemObservedByteRegionKind::DirectoryRecords
    );
    assert_eq!(rows[0].length(), directory_length);
    assert_eq!(
        rows[1].kind(),
        FilesystemObservedByteRegionKind::DirectoryRecords
    );
    assert_eq!(rows[1].length(), 0);
    assert!(rows[2..].iter().all(|row| {
        row.kind() == FilesystemObservedByteRegionKind::FindEntry
            && row.output_operand_ordinal() == 1
            && row.offset() == 0
    }));
    assert_eq!(rows[2].length(), FIND_DATA_OUTPUT_BYTES);
    assert_eq!(rows[3].length(), FIND_DATA_OUTPUT_BYTES);
    assert_eq!(rows[4].length(), 0);
}

#[test]
fn observed_byte_region_rejects_out_of_bounds_without_a_row() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.filesystem_operation_attempt_stack.push(0);
    let output = mutable_byte_output(&[9]);

    assert!(
        evaluator
            .record_observed_byte_region(
                1,
                FilesystemObservedByteRegionKind::SequentialFileRead,
                &output,
                0,
                2,
            )
            .is_err()
    );
    assert!(
        evaluator.filesystem_operation_attempts[0]
            .observed_byte_regions
            .is_empty()
    );
}

#[test]
fn incremental_logical_handle_inputs_retain_exact_resolution_prefix() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    let descriptor = evaluator
        .filesystem_logical_handles
        .create(FilesystemLogicalHandleKind::Descriptor, 7)
        .unwrap();

    evaluator.record_prepared_filesystem_logical_handle_input(
        0,
        0,
        FilesystemLogicalHandleKind::Descriptor,
        7,
        false,
    );
    evaluator.record_prepared_filesystem_logical_handle_input(
        0,
        1,
        FilesystemLogicalHandleKind::Find,
        7,
        false,
    );
    evaluator.record_prepared_filesystem_logical_handle_input(
        0,
        2,
        FilesystemLogicalHandleKind::Native,
        0,
        true,
    );
    evaluator.record_prepared_filesystem_logical_handle_input(
        0,
        3,
        FilesystemLogicalHandleKind::Native,
        0,
        false,
    );

    assert_eq!(
        evaluator.filesystem_operation_attempts[0].logical_handle_inputs,
        vec![
            FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(descriptor),
            },
            FilesystemLogicalHandleInput {
                operand_ordinal: 1,
                kind: FilesystemLogicalHandleKind::Find,
                resolution: FilesystemLogicalHandleInputResolution::Unknown,
            },
            FilesystemLogicalHandleInput {
                operand_ordinal: 2,
                kind: FilesystemLogicalHandleKind::Native,
                resolution: FilesystemLogicalHandleInputResolution::Null,
            },
            FilesystemLogicalHandleInput {
                operand_ordinal: 3,
                kind: FilesystemLogicalHandleKind::Native,
                resolution: FilesystemLogicalHandleInputResolution::Unknown,
            },
        ],
        "a later preparation halt must retain every already typed handle input",
    );
}

#[test]
fn completed_logical_handle_plan_cross_checks_without_overwriting_evidence() {
    let program = TypedTrees::default();
    let mut evaluator = evaluator_with_pending_attempt(&program);
    evaluator.record_prepared_filesystem_logical_handle_input(
        0,
        0,
        FilesystemLogicalHandleKind::Descriptor,
        7,
        false,
    );
    let retained = evaluator.filesystem_operation_attempts[0]
        .logical_handle_inputs
        .clone();
    let matching = PreparedFilesystemLogicalHandlePlan {
        inputs: vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            raw: 7,
            null_allowed: false,
        }],
        input_success: None,
        output: None,
        retirement: None,
    };
    assert!(
        evaluator
            .validate_incremental_logical_handle_inputs(0, &matching)
            .is_ok()
    );
    assert_eq!(
        evaluator.filesystem_operation_attempts[0].logical_handle_inputs,
        retained,
    );

    let disagreement = PreparedFilesystemLogicalHandlePlan {
        inputs: vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Find,
            raw: 7,
            null_allowed: false,
        }],
        ..matching
    };
    assert!(
        evaluator
            .validate_incremental_logical_handle_inputs(0, &disagreement)
            .is_err()
    );
    assert_eq!(
        evaluator.filesystem_operation_attempts[0].logical_handle_inputs,
        retained,
    );
}
