//! Filesystem preparation tests.

use super::{
    FILETIME_BYTES, FIND_DATA_OUTPUT_BYTES, FilesystemHostOperation, FilesystemTransferCountError,
    MAX_FILESYSTEM_TRANSFER_BYTES, OVERLAPPED_BYTES, PATH_MAX_OUTPUT_BYTES, PreparedByteOutput,
    PreparedFilesystemCall, PreparedFilesystemLogicalHandleInput,
    PreparedFilesystemLogicalHandleOutput, PreparedI64Output, PreparedMutableByteInput,
    PreparedTransferCount, STAT_OUTPUT_BYTES, TIMESPEC_PAIR_BYTES, check_filesystem_arity,
    checked_filesystem_transfer_count, checked_relative_component,
    rooted_package_build_operation_refusal, synthetic_handle_fd,
};
use crate::interpreter::evaluator::{FilesystemLogicalHandleKind, Value};
use crate::{
    FilesystemMutableByteOperandResolution, FilesystemMutableI64OperandResolution,
    FilesystemScalarOperandValue,
};

fn array_output(length: usize) -> PreparedByteOutput {
    PreparedByteOutput::Array(
        (0..length)
            .map(|_| Value::Int(0).cell())
            .collect::<Vec<_>>(),
    )
}

fn transfer_count() -> PreparedTransferCount {
    PreparedTransferCount { raw: 1, host: 1 }
}

fn mutable_byte_input(length: usize) -> PreparedMutableByteInput {
    PreparedMutableByteInput {
        output: array_output(length),
        bytes: vec![0; length],
    }
}

#[test]
fn rooted_package_build_refuses_unrooted_or_host_absolute_protocols() {
    assert!(
        rooted_package_build_operation_refusal(FilesystemHostOperation::Canonicalize).is_some()
    );
    for operation in [
        FilesystemHostOperation::FindFirst,
        FilesystemHostOperation::FindNext,
        FilesystemHostOperation::FindClose,
    ] {
        assert!(rooted_package_build_operation_refusal(operation).is_some());
    }
    assert!(rooted_package_build_operation_refusal(FilesystemHostOperation::ReadDir).is_none());
}

fn mutable_i64() -> PreparedI64Output {
    PreparedI64Output {
        cell: Value::Int(7).cell(),
        initial: 7,
    }
}

fn prepared_call_fixture(operation: FilesystemHostOperation) -> PreparedFilesystemCall {
    let path = || b"/root/file".to_vec();
    let name = || b"entry".to_vec();
    match operation {
        FilesystemHostOperation::Create => PreparedFilesystemCall::Create {
            path: path(),
            mode: -1,
        },
        FilesystemHostOperation::Open => PreparedFilesystemCall::Open {
            path: path(),
            flags: -1,
        },
        FilesystemHostOperation::OpenCreate => PreparedFilesystemCall::OpenCreate {
            path: path(),
            flags: -1,
            mode: -1,
        },
        FilesystemHostOperation::Read => PreparedFilesystemCall::Read {
            fd: 3,
            buffer: array_output(1),
            count: transfer_count(),
        },
        FilesystemHostOperation::Write => PreparedFilesystemCall::Write {
            fd: 3,
            bytes: b"payload".to_vec(),
        },
        FilesystemHostOperation::ReadAt => PreparedFilesystemCall::ReadAt {
            fd: 3,
            buffer: array_output(1),
            count: transfer_count(),
            offset: i64::MIN,
        },
        FilesystemHostOperation::WriteAt => PreparedFilesystemCall::WriteAt {
            fd: 3,
            bytes: b"payload".to_vec(),
            offset: i64::MIN,
        },
        FilesystemHostOperation::Close => PreparedFilesystemCall::Close { fd: 3 },
        FilesystemHostOperation::Remove => PreparedFilesystemCall::Remove { path: path() },
        FilesystemHostOperation::Seek => PreparedFilesystemCall::Seek {
            fd: 3,
            offset: i64::MIN,
            whence: -1,
        },
        FilesystemHostOperation::CreateDir => PreparedFilesystemCall::CreateDir {
            path: path(),
            mode: -1,
        },
        FilesystemHostOperation::RemoveDir => PreparedFilesystemCall::RemoveDir { path: path() },
        FilesystemHostOperation::CreateDirName => PreparedFilesystemCall::CreateDirName {
            name: path(),
            mode: -1,
        },
        FilesystemHostOperation::OpenAt => PreparedFilesystemCall::OpenAt {
            dirfd: 3,
            name: name(),
            flags: -1,
        },
        FilesystemHostOperation::UnlinkAt => PreparedFilesystemCall::UnlinkAt {
            dirfd: 3,
            name: name(),
            flags: -1,
        },
        FilesystemHostOperation::SetPermissions => PreparedFilesystemCall::SetPermissions {
            path: path(),
            mode: u32::MAX,
        },
        FilesystemHostOperation::SetFilePermissions => PreparedFilesystemCall::SetFilePermissions {
            fd: 3,
            mode: u32::MAX,
        },
        FilesystemHostOperation::Rename => PreparedFilesystemCall::Rename {
            from: path(),
            to: path(),
        },
        FilesystemHostOperation::HardLink => PreparedFilesystemCall::HardLink {
            original: path(),
            link: path(),
        },
        FilesystemHostOperation::Symlink => PreparedFilesystemCall::Symlink {
            target: path(),
            link: path(),
        },
        FilesystemHostOperation::ReadLink => PreparedFilesystemCall::ReadLink {
            path: path(),
            buffer: array_output(1),
            count: transfer_count(),
        },
        FilesystemHostOperation::Canonicalize => PreparedFilesystemCall::Canonicalize {
            path: path(),
            buffer: array_output(PATH_MAX_OUTPUT_BYTES),
        },
        FilesystemHostOperation::ReadDir => PreparedFilesystemCall::ReadDir {
            fd: 3,
            buffer: array_output(1),
            count: transfer_count(),
            position: mutable_i64(),
        },
        FilesystemHostOperation::FindFirst => PreparedFilesystemCall::FindFirst {
            pattern: path(),
            data: array_output(FIND_DATA_OUTPUT_BYTES),
        },
        FilesystemHostOperation::FindNext => PreparedFilesystemCall::FindNext {
            handle: 11,
            data: array_output(FIND_DATA_OUTPUT_BYTES),
        },
        FilesystemHostOperation::FindClose => PreparedFilesystemCall::FindClose { handle: 11 },
        FilesystemHostOperation::CreateHardLink => PreparedFilesystemCall::CreateHardLink {
            link: path(),
            existing: path(),
            security_attributes: i64::MIN,
        },
        FilesystemHostOperation::OpenPathHandle => PreparedFilesystemCall::OpenPathHandle {
            path: path(),
            desired_access: u32::MAX,
            share_mode: u32::MAX,
            security_attributes: i64::MIN,
            creation_disposition: u32::MAX,
            flags_and_attributes: u32::MAX,
            template_file: 0,
        },
        FilesystemHostOperation::CloseHandle => PreparedFilesystemCall::CloseHandle { handle: 3 },
        FilesystemHostOperation::GetOsfHandle => PreparedFilesystemCall::GetOsfHandle { fd: 3 },
        FilesystemHostOperation::FinalPathNameByHandle => {
            PreparedFilesystemCall::FinalPathNameByHandle {
                handle: 3,
                buffer: array_output(1),
                capacity: transfer_count(),
                flags: u32::MAX,
            }
        }
        FilesystemHostOperation::SetFileTime => PreparedFilesystemCall::SetFileTime {
            handle: 3,
            creation: i64::MIN,
            last_access: vec![1; FILETIME_BYTES + 1],
            last_write: vec![2; FILETIME_BYTES + 1],
        },
        FilesystemHostOperation::LockFileEx => PreparedFilesystemCall::LockFileEx {
            handle: 3,
            flags: u32::MAX,
            reserved: u32::MAX,
            length_low: u32::MAX,
            length_high: u32::MAX,
            overlapped: mutable_byte_input(OVERLAPPED_BYTES),
        },
        FilesystemHostOperation::UnlockFile => PreparedFilesystemCall::UnlockFile {
            handle: 3,
            offset_low: u32::MAX,
            offset_high: u32::MAX,
            length_low: u32::MAX,
            length_high: u32::MAX,
        },
        FilesystemHostOperation::GetLastError => PreparedFilesystemCall::GetLastError,
        FilesystemHostOperation::RemoveName => PreparedFilesystemCall::RemoveName { path: path() },
        FilesystemHostOperation::RemoveDirName => {
            PreparedFilesystemCall::RemoveDirName { path: path() }
        }
        FilesystemHostOperation::ReadMetadata => PreparedFilesystemCall::ReadMetadata {
            path: path(),
            buffer: array_output(STAT_OUTPUT_BYTES),
        },
        FilesystemHostOperation::ReadFileMetadata => PreparedFilesystemCall::ReadFileMetadata {
            fd: 3,
            buffer: array_output(STAT_OUTPUT_BYTES),
        },
        FilesystemHostOperation::ReadSymlinkMetadata => {
            PreparedFilesystemCall::ReadSymlinkMetadata {
                path: path(),
                buffer: array_output(STAT_OUTPUT_BYTES),
            }
        }
        FilesystemHostOperation::SetLen => PreparedFilesystemCall::SetLen {
            fd: 3,
            length: i64::MIN,
        },
        FilesystemHostOperation::SetFileTimes => PreparedFilesystemCall::SetFileTimes {
            fd: 3,
            times: mutable_byte_input(TIMESPEC_PAIR_BYTES),
        },
        FilesystemHostOperation::Sync => PreparedFilesystemCall::Sync { fd: 3 },
        FilesystemHostOperation::SyncData => PreparedFilesystemCall::SyncData { fd: 3 },
        FilesystemHostOperation::Duplicate => PreparedFilesystemCall::Duplicate { fd: 3 },
        FilesystemHostOperation::LockFile => PreparedFilesystemCall::LockFile {
            fd: 3,
            operation: -1,
        },
        FilesystemHostOperation::ChangeOwner => PreparedFilesystemCall::ChangeOwner {
            path: path(),
            uid: -1,
            gid: -1,
        },
        FilesystemHostOperation::ChangeOwnerNoFollow => {
            PreparedFilesystemCall::ChangeOwnerNoFollow {
                path: path(),
                uid: -1,
                gid: -1,
            }
        }
        FilesystemHostOperation::ChangeFileOwner => PreparedFilesystemCall::ChangeFileOwner {
            fd: 3,
            uid: -1,
            gid: -1,
        },
        FilesystemHostOperation::Errno => PreparedFilesystemCall::Errno,
    }
}

fn expected_immutable_byte_ordinals(operation: FilesystemHostOperation) -> &'static [u8] {
    match operation {
        FilesystemHostOperation::Write | FilesystemHostOperation::WriteAt => &[1],
        FilesystemHostOperation::OpenAt | FilesystemHostOperation::UnlinkAt => &[1],
        FilesystemHostOperation::SetFileTime => &[2, 3],
        FilesystemHostOperation::Create
        | FilesystemHostOperation::Open
        | FilesystemHostOperation::OpenCreate
        | FilesystemHostOperation::Read
        | FilesystemHostOperation::ReadAt
        | FilesystemHostOperation::Close
        | FilesystemHostOperation::Remove
        | FilesystemHostOperation::Seek
        | FilesystemHostOperation::CreateDir
        | FilesystemHostOperation::RemoveDir
        | FilesystemHostOperation::CreateDirName
        | FilesystemHostOperation::SetPermissions
        | FilesystemHostOperation::SetFilePermissions
        | FilesystemHostOperation::Rename
        | FilesystemHostOperation::HardLink
        | FilesystemHostOperation::Symlink
        | FilesystemHostOperation::ReadLink
        | FilesystemHostOperation::Canonicalize
        | FilesystemHostOperation::ReadDir
        | FilesystemHostOperation::FindFirst
        | FilesystemHostOperation::FindNext
        | FilesystemHostOperation::FindClose
        | FilesystemHostOperation::CreateHardLink
        | FilesystemHostOperation::OpenPathHandle
        | FilesystemHostOperation::CloseHandle
        | FilesystemHostOperation::GetOsfHandle
        | FilesystemHostOperation::FinalPathNameByHandle
        | FilesystemHostOperation::LockFileEx
        | FilesystemHostOperation::UnlockFile
        | FilesystemHostOperation::GetLastError
        | FilesystemHostOperation::RemoveName
        | FilesystemHostOperation::RemoveDirName
        | FilesystemHostOperation::ReadMetadata
        | FilesystemHostOperation::ReadFileMetadata
        | FilesystemHostOperation::ReadSymlinkMetadata
        | FilesystemHostOperation::SetLen
        | FilesystemHostOperation::SetFileTimes
        | FilesystemHostOperation::Sync
        | FilesystemHostOperation::SyncData
        | FilesystemHostOperation::Duplicate
        | FilesystemHostOperation::LockFile
        | FilesystemHostOperation::ChangeOwner
        | FilesystemHostOperation::ChangeOwnerNoFollow
        | FilesystemHostOperation::ChangeFileOwner
        | FilesystemHostOperation::Errno => &[],
    }
}

fn expected_path_like_byte_ordinals(operation: FilesystemHostOperation) -> &'static [u8] {
    match operation {
        FilesystemHostOperation::CreateDirName
        | FilesystemHostOperation::Symlink
        | FilesystemHostOperation::FindFirst
        | FilesystemHostOperation::RemoveName
        | FilesystemHostOperation::RemoveDirName => &[0],
        _ => &[],
    }
}

#[test]
fn transfer_counts_are_bounded_without_losing_the_authored_value() {
    assert_eq!(
        checked_filesystem_transfer_count(-1),
        Err(FilesystemTransferCountError::NegativeOrUnrepresentable)
    );
    assert_eq!(
        checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64 + 1),
        Err(FilesystemTransferCountError::ExceedsEvaluatorLimit)
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
fn prepared_output_rejects_truncation_and_keeps_the_resolved_cells() {
    let output = array_output(2);
    let cells = match &output {
        PreparedByteOutput::Array(cells) => cells.clone(),
        PreparedByteOutput::Text { .. } => unreachable!(),
    };
    assert!(output.write(&[1, 2, 3]).is_err());
    assert!(output.write(&[7, 8]).is_ok());
    assert_eq!(cells[0].borrow().as_int(), Some(7));
    assert_eq!(cells[1].borrow().as_int(), Some(8));
}

#[test]
fn prepared_text_output_preserves_its_original_capacity_across_short_writes() {
    let Value::Str(text) = Value::bytes(vec![0; 4]) else {
        unreachable!()
    };
    let output = PreparedByteOutput::Text {
        text: text.clone(),
        capacity: 4,
    };
    assert!(output.write(&[1]).is_ok());
    assert!(output.write(&[2, 3, 4, 5]).is_ok());
    assert_eq!(&*text.borrow(), &[2, 3, 4, 5]);
}

#[test]
fn synthetic_handles_never_alias_after_narrowing() {
    assert_eq!(synthetic_handle_fd(3), Some(3));
    assert_eq!(synthetic_handle_fd(0x1_0000_0003), None);
    assert_eq!(synthetic_handle_fd(i64::from(i32::MIN) - 1), None);
}

#[test]
fn at_family_names_are_exact_portable_relative_components() {
    let Ok(accepted) = checked_relative_component(b"entry.bin".to_vec()) else {
        panic!("one ordinary component must be accepted")
    };
    assert_eq!(accepted, b"entry.bin");
    for rejected in [
        b"".as_slice(),
        b".".as_slice(),
        b"..".as_slice(),
        b"nested/entry".as_slice(),
        b"nested\\entry".as_slice(),
        b"nul\0entry".as_slice(),
    ] {
        assert!(
            checked_relative_component(rejected.to_vec()).is_err(),
            "unexpected accepted relative component: {rejected:?}"
        );
    }
}

#[test]
fn every_canonical_operation_rejects_wrong_arity_before_cursor_creation() {
    for operation in FilesystemHostOperation::ALL {
        let expected = operation.operand_kinds().len();
        assert!(check_filesystem_arity(operation, expected).is_ok());
        assert!(check_filesystem_arity(operation, expected + 1).is_err());
        if expected > 0 {
            assert!(check_filesystem_arity(operation, expected - 1).is_err());
        }
    }
}

#[test]
fn prepared_byte_snapshots_reject_wrong_element_kinds_and_ranges() {
    let wrong_kind = PreparedByteOutput::Array(vec![Value::Bool(true).cell()]);
    assert!(wrong_kind.snapshot().is_err());
    let wrong_range = PreparedByteOutput::Array(vec![Value::Int(256).cell()]);
    assert!(wrong_range.snapshot().is_err());
}

#[test]
fn provider_boundaries_only_accept_prepared_calls() {
    let virtual_source = include_str!("../filesystem/filesystem_calls.rs");
    let real_source = include_str!("../real_filesystem.rs");
    let virtual_signatures = virtual_source
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        virtual_signatures
            .contains("fn serve_filesystem_call( &mut self, call: PreparedFilesystemCall, )")
    );
    assert!(real_source.contains("call: PreparedFilesystemCall,"));
    assert!(!real_source.contains("ExpressionHandle"));
    assert!(!real_source.contains("frame: &Frame"));
    assert!(!virtual_source.contains("handle as i32"));
    assert!(!real_source.contains("handle as i32"));
}

#[test]
fn logical_handle_plan_distinguishes_descriptor_native_find_and_pointer_scalars() {
    let descriptor_open = PreparedFilesystemCall::Create {
        path: b"file".to_vec(),
        mode: 0,
    }
    .logical_handle_plan();
    assert!(descriptor_open.inputs.is_empty());
    assert!(matches!(
        descriptor_open.output,
        Some(PreparedFilesystemLogicalHandleOutput::Created {
            kind: FilesystemLogicalHandleKind::Descriptor,
            ..
        })
    ));

    let borrowed = PreparedFilesystemCall::GetOsfHandle { fd: 7 }.logical_handle_plan();
    assert_eq!(
        borrowed.inputs,
        vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            raw: 7,
            null_allowed: false,
        }]
    );
    assert!(matches!(
        borrowed.output,
        Some(PreparedFilesystemLogicalHandleOutput::Borrowed {
            source_operand_ordinal: 0,
            ..
        })
    ));

    let native_open = PreparedFilesystemCall::OpenPathHandle {
        path: b"file".to_vec(),
        desired_access: 0,
        share_mode: 0,
        security_attributes: 123,
        creation_disposition: 0,
        flags_and_attributes: 0,
        template_file: 0,
    }
    .logical_handle_plan();
    assert_eq!(
        native_open.inputs,
        vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 6,
            kind: FilesystemLogicalHandleKind::Native,
            raw: 0,
            null_allowed: true,
        }],
        "security_attributes is pointer-shaped but only template_file is a handle"
    );

    let find = PreparedFilesystemCall::FindFirst {
        pattern: b"dir/*".to_vec(),
        data: array_output(FIND_DATA_OUTPUT_BYTES),
    }
    .logical_handle_plan();
    assert!(matches!(
        find.output,
        Some(PreparedFilesystemLogicalHandleOutput::Created {
            kind: FilesystemLogicalHandleKind::Find,
            ..
        })
    ));
    assert_eq!(
        PreparedFilesystemCall::FindNext {
            handle: 41,
            data: array_output(FIND_DATA_OUTPUT_BYTES),
        }
        .logical_handle_plan()
        .inputs,
        vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Find,
            raw: 41,
            null_allowed: false,
        }]
    );

    assert_eq!(
        PreparedFilesystemCall::CloseHandle { handle: 73 }
            .logical_handle_plan()
            .inputs,
        vec![PreparedFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Native,
            raw: 73,
            null_allowed: false,
        }]
    );

    let hard_link = PreparedFilesystemCall::CreateHardLink {
        link: b"link".to_vec(),
        existing: b"file".to_vec(),
        security_attributes: 123,
    }
    .logical_handle_plan();
    assert!(hard_link.inputs.is_empty());
    assert!(hard_link.output.is_none());
}

#[test]
fn operand_observation_plan_preserves_width_payload_and_safe_components() {
    let open_at = PreparedFilesystemCall::OpenAt {
        dirfd: 7,
        name: b"entry.bin".to_vec(),
        flags: -1,
    };
    let (scalars, bytes, path_like) = open_at.operand_observation_plan();
    assert_eq!(scalars.len(), 1);
    assert_eq!(scalars[0].operand_ordinal(), 2);
    assert_eq!(scalars[0].value(), FilesystemScalarOperandValue::I32(-1));
    assert_eq!(bytes.len(), 1);
    assert_eq!(bytes[0].operand_ordinal(), 1);
    assert_eq!(bytes[0].bytes(), b"entry.bin");
    assert!(path_like.is_empty());

    let write = PreparedFilesystemCall::Write {
        fd: 3,
        bytes: b"a\0b".to_vec(),
    };
    let (scalars, bytes, path_like) = write.operand_observation_plan();
    assert!(scalars.is_empty(), "raw descriptor tokens are not scalars");
    assert_eq!(bytes[0].bytes(), b"a\0b");
    assert!(path_like.is_empty());

    let set_time = PreparedFilesystemCall::SetFileTime {
        handle: 3,
        creation: i64::MIN,
        last_access: (0u8..12).collect(),
        last_write: (20u8..32).collect(),
    };
    let (scalars, bytes, path_like) = set_time.operand_observation_plan();
    assert_eq!(
        scalars[0].value(),
        FilesystemScalarOperandValue::I64(i64::MIN)
    );
    assert_eq!(bytes[0].bytes(), &(0u8..12).collect::<Vec<_>>());
    assert_eq!(bytes[1].bytes(), &(20u8..32).collect::<Vec<_>>());
    assert!(path_like.is_empty());

    let native_open = PreparedFilesystemCall::OpenPathHandle {
        path: b"file".to_vec(),
        desired_access: u32::MAX,
        share_mode: 2,
        security_attributes: 123,
        creation_disposition: 4,
        flags_and_attributes: 5,
        template_file: 99,
    };
    let (scalars, bytes, path_like) = native_open.operand_observation_plan();
    assert_eq!(
        scalars
            .iter()
            .map(|operand| operand.operand_ordinal())
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5],
        "path and logical-handle ordinals stay outside scalar evidence"
    );
    assert_eq!(
        scalars[0].value(),
        FilesystemScalarOperandValue::U32(u32::MAX)
    );
    assert!(bytes.is_empty());
    assert!(path_like.is_empty());
}

#[test]
fn every_canonical_operation_has_exact_scalar_byte_and_path_like_roles() {
    use super::super::filesystem_host_operation::FilesystemHostOperandKind as Kind;

    for operation in FilesystemHostOperation::ALL {
        let call = prepared_call_fixture(operation);
        let logical_plan = call.logical_handle_plan();
        let mutable_plan = call
            .mutable_observation_plan()
            .unwrap_or_else(|_| panic!("mutable observation fixture must be representable"));
        let logical_ordinals = logical_plan
            .inputs
            .iter()
            .map(|input| input.operand_ordinal)
            .collect::<std::collections::BTreeSet<_>>();
        let (scalars, bytes, path_like) = call.operand_observation_plan();
        let actual_scalars = scalars
            .iter()
            .map(|operand| {
                let kind = match operand.value() {
                    FilesystemScalarOperandValue::I32(_) => Kind::I32,
                    FilesystemScalarOperandValue::U32(_) => Kind::U32,
                    FilesystemScalarOperandValue::I64(_) => Kind::I64,
                    FilesystemScalarOperandValue::U64(_) => Kind::U64,
                };
                (operand.operand_ordinal(), kind)
            })
            .collect::<Vec<_>>();
        let expected_scalars = operation
            .operand_kinds()
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(ordinal, kind)| {
                let ordinal = u8::try_from(ordinal).unwrap();
                matches!(kind, Kind::I32 | Kind::U32 | Kind::I64 | Kind::U64)
                    .then_some((ordinal, kind))
                    .filter(|(ordinal, _)| !logical_ordinals.contains(ordinal))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual_scalars, expected_scalars,
            "scalar evidence role drift for `{operation}`"
        );

        let actual_bytes = bytes
            .iter()
            .map(|operand| operand.operand_ordinal())
            .collect::<Vec<_>>();
        assert_eq!(
            actual_bytes,
            expected_immutable_byte_ordinals(operation),
            "immutable byte evidence role drift for `{operation}`"
        );
        let actual_path_like = path_like
            .iter()
            .map(|operand| operand.operand_ordinal())
            .collect::<Vec<_>>();
        assert_eq!(
            actual_path_like,
            expected_path_like_byte_ordinals(operation),
            "path-like byte evidence role drift for `{operation}`"
        );
        for operand in &path_like {
            assert_eq!(
                operand.bytes(),
                b"/root/file",
                "path-like byte evidence content drift for `{operation}`"
            );
        }
        let expected_mutable_bytes = operation
            .operand_kinds()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, kind)| {
                (*kind == Kind::MutableBytes).then_some(u8::try_from(ordinal).unwrap())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            mutable_plan
                .byte_operands
                .iter()
                .map(|operand| operand.operand_ordinal)
                .collect::<Vec<_>>(),
            expected_mutable_bytes,
            "mutable byte evidence role drift for `{operation}`"
        );
        let expected_mutable_i64 = operation
            .operand_kinds()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, kind)| {
                (*kind == Kind::MutableI64).then_some(u8::try_from(ordinal).unwrap())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            mutable_plan
                .i64_operands
                .iter()
                .map(|operand| operand.operand_ordinal)
                .collect::<Vec<_>>(),
            expected_mutable_i64,
            "mutable i64 evidence role drift for `{operation}`"
        );

        let mut observed_ordinals = logical_ordinals;
        for ordinal in actual_scalars
            .iter()
            .map(|(ordinal, _)| *ordinal)
            .chain(actual_bytes.iter().copied())
            .chain(actual_path_like.iter().copied())
        {
            assert!(
                observed_ordinals.insert(ordinal),
                "operand `{ordinal}` of `{operation}` entered two evidence roles"
            );
        }
    }
}

#[test]
fn mutable_observation_plan_retains_complete_pre_and_post_carriers() {
    let buffer = array_output(3);
    buffer
        .write(&[9, 8, 7])
        .unwrap_or_else(|_| panic!("fixture write must fit"));
    let call = PreparedFilesystemCall::Read {
        fd: 3,
        buffer: buffer.clone(),
        count: PreparedTransferCount { raw: 1, host: 1 },
    };
    let plan = call
        .mutable_observation_plan()
        .unwrap_or_else(|_| panic!("mutable observation fixture must be representable"));
    assert_eq!(plan.reserved_bytes(), Some(6));
    let (initial_bytes, initial_i64) = plan.initial_rows();
    assert!(initial_i64.is_empty());
    assert_eq!(initial_bytes[0].pre_bytes(), &[9, 8, 7]);
    assert_eq!(initial_bytes[0].post_bytes(), &[9, 8, 7]);

    buffer
        .write(&[1])
        .unwrap_or_else(|_| panic!("fixture write must fit"));
    let (completed_bytes, completed_i64) = plan
        .completed_rows()
        .unwrap_or_else(|_| panic!("completed fixture must be representable"));
    assert!(completed_i64.is_empty());
    assert_eq!(completed_bytes[0].pre_bytes(), &[9, 8, 7]);
    assert_eq!(
        completed_bytes[0].post_bytes(),
        &[1, 8, 7],
        "unchanged mutable tail remains explicit"
    );

    let position = mutable_i64();
    let call = PreparedFilesystemCall::ReadDir {
        fd: 3,
        buffer: array_output(1),
        count: transfer_count(),
        position: position.clone(),
    };
    let plan = call
        .mutable_observation_plan()
        .unwrap_or_else(|_| panic!("mutable observation fixture must be representable"));
    position
        .write(12)
        .unwrap_or_else(|_| panic!("fixture cursor write must fit"));
    let (_, completed_i64) = plan
        .completed_rows()
        .unwrap_or_else(|_| panic!("completed fixture must be representable"));
    assert_eq!(completed_i64[0].pre_value(), 7);
    assert_eq!(completed_i64[0].post_value(), 12);
}

#[test]
fn mutable_resolution_validation_uses_only_ordered_role_and_capacity() {
    let position = mutable_i64();
    let call = PreparedFilesystemCall::ReadDir {
        fd: 3,
        buffer: array_output(2),
        count: transfer_count(),
        position,
    };
    let plan = call
        .mutable_observation_plan()
        .unwrap_or_else(|_| panic!("mutable fixture must be representable"));
    let matching_bytes = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: vec![91, 92],
    }];
    let matching_i64 = vec![FilesystemMutableI64OperandResolution {
        operand_ordinal: 3,
        value: i64::MIN,
    }];
    assert!(
        plan.matches_resolution_roles(&matching_bytes, &matching_i64),
        "resolution values may differ from provider pre-state after aliasing"
    );

    let wrong_byte_role = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 0,
        bytes: vec![91, 92],
    }];
    assert!(!plan.matches_resolution_roles(&wrong_byte_role, &matching_i64));

    let wrong_capacity = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: vec![91],
    }];
    assert!(!plan.matches_resolution_roles(&wrong_capacity, &matching_i64));

    let wrong_i64_role = vec![FilesystemMutableI64OperandResolution {
        operand_ordinal: 2,
        value: 7,
    }];
    assert!(!plan.matches_resolution_roles(&matching_bytes, &wrong_i64_role));
    assert!(!plan.matches_resolution_roles(&[], &matching_i64));
    assert!(!plan.matches_resolution_roles(&matching_bytes, &[]));
}

#[test]
fn operation_attempt_encloses_canonical_preparation() {
    let source = include_str!("../filesystem/filesystem_calls.rs");
    let push = source.find("push(attempt_index)").expect("attempt push");
    let prepare = source
        .find(".prepare_filesystem_call(operation, arguments, frame)")
        .expect("canonical preparation");
    let pop = source[prepare..]
        .find(".pop()")
        .map(|offset| prepare + offset)
        .expect("attempt pop");
    assert!(push < prepare && prepare < pop);
}
