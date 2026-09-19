//! Tests for filesystem call evaluation.

use super::super::directory_entries::dirent_record_extent;
use super::super::filesystem_preparation::{
    FilesystemLogicalHandleResultSuccess, FilesystemLogicalHandleRetirementSuccess,
    FilesystemTransferCountError, MAX_FILESYSTEM_TRANSFER_BYTES,
    PreparedFilesystemLogicalHandleInput, PreparedFilesystemLogicalHandlePlan,
    PreparedFilesystemLogicalHandleRetirement, PreparedTransferCount,
    checked_filesystem_transfer_count,
};
use super::super::{MAX_DIRECTORY_ENTRY_NAME_BYTES, MAX_DIRECTORY_SNAPSHOT_BYTES, TypedTrees};
use crate::BuildEvaluationSponsorLimits;
use crate::FilesystemRootedPathOperandResolution;
use crate::interpreter::evaluator::BuildEvaluationSponsor;
use crate::interpreter::evaluator::Evaluator;
use crate::interpreter::evaluator::FilesystemLogicalHandleInput;
use crate::interpreter::evaluator::FilesystemLogicalHandleInputResolution;
use crate::interpreter::evaluator::FilesystemLogicalHandleKind;
use crate::interpreter::evaluator::FilesystemObservationProvider;
use crate::interpreter::evaluator::FilesystemOperationAttempt;
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
use crate::interpreter::evaluator::pack_dirent_records;
use crate::interpreter::evaluator::portable_directory_entry_name;

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
        Ok(PreparedTransferCount { host: 0 })
    );
    assert_eq!(
        checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64),
        Ok(PreparedTransferCount {
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
fn virtual_read_preserves_result_length_and_untouched_buffer_tail() {
    let program = TypedTrees::default();
    let mut evaluator = Evaluator::new(&program, &[]);
    evaluator
        .virtual_files
        .insert(b"input".to_vec(), b"abc".to_vec());
    let fd = evaluator.virtual_open(b"input".to_vec(), false, false);
    let output = mutable_byte_output(&[9; 5]);
    let result = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::Read {
            fd,
            buffer: output.clone(),
            count: PreparedTransferCount { host: 5 },
        })
        .unwrap_or_else(|_| panic!("read succeeds"));
    assert!(matches!(result, Value::Int(3)));
    assert_eq!(output.snapshot().unwrap_or_default(), b"abc\x09\x09");
    let result = evaluator
        .serve_filesystem_call(PreparedFilesystemCall::Read {
            fd,
            buffer: output.clone(),
            count: PreparedTransferCount { host: 5 },
        })
        .unwrap_or_else(|_| panic!("EOF succeeds"));
    assert!(matches!(result, Value::Int(0)));
    assert_eq!(output.snapshot().unwrap_or_default(), b"abc\x09\x09");
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
            count: PreparedTransferCount { host: 4 },
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
            count: PreparedTransferCount { host: 4 },
        })
        .unwrap_or_else(|_| panic!("truncated read_link succeeds"));
    assert!(matches!(limited_result, Value::Int(4)));
    assert_eq!(
        limited_output
            .snapshot()
            .unwrap_or_else(|_| panic!("limited output remains readable")),
        b"abcd\x09\x09"
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
