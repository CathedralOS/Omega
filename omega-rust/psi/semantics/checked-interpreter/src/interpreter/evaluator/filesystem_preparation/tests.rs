//! Filesystem preparation tests.

use super::{
    FIND_DATA_OUTPUT_BYTES, FilesystemHostOperation, FilesystemTransferCountError,
    MAX_FILESYSTEM_TRANSFER_BYTES, PreparedByteOutput, PreparedFilesystemCall,
    PreparedFilesystemLogicalHandleInput, PreparedFilesystemLogicalHandleOutput,
    PreparedTransferCount, check_filesystem_arity, checked_filesystem_transfer_count,
    checked_relative_component, rooted_package_build_operation_refusal, synthetic_handle_fd,
};
use crate::interpreter::evaluator::{FilesystemLogicalHandleKind, Value};

fn array_output(length: usize) -> PreparedByteOutput {
    PreparedByteOutput::Array(
        (0..length)
            .map(|_| Value::Int(0).cell())
            .collect::<Vec<_>>(),
    )
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

#[test]
fn transfer_counts_are_bounded_before_host_conversion() {
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
    assert!(wrong_kind.validate_contents().is_err());
    assert!(wrong_kind.snapshot().is_err());
    let wrong_range = PreparedByteOutput::Array(vec![Value::Int(256).cell()]);
    assert!(wrong_range.validate_contents().is_err());
    assert!(wrong_range.snapshot().is_err());
}

#[test]
fn preflight_rechecks_aliased_output_carrier_without_snapshotting_it() {
    let cell = Value::Int(0).cell();
    let call = PreparedFilesystemCall::Read {
        fd: 3,
        buffer: PreparedByteOutput::Array(vec![cell.clone()]),
        count: PreparedTransferCount { host: 1 },
    };
    assert!(call.validate_output_carriers().is_ok());
    *cell.borrow_mut() = Value::Int(256);
    assert!(call.validate_output_carriers().is_err());
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
