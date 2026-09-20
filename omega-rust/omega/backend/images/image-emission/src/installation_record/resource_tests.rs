//! Installation-record fixtures and round-trip tests shared by the codec
//! children.

use super::record_shape::{
    installed_stack_facts_are_canonical, is_partial_cleanup_path,
    scalar_control_affine_cleanups_are_canonical,
};
use super::{
    CallSiteOwner, INSTALLATION_FORMAT_MARKER, InstallationError, InstalledForeignCallStack,
    InstalledFunction, InstalledInternalUnitCall, MachineId, Reader, StructuralTypeId,
    codec::structural_argument_codec, decode_installation_record, decode_structural_types,
    encode_structural_types, push_u16, push_u32, push_u64,
};
use super::{
    codec::function_affine_cleanup_codec::{
        decode_scalar_control_affine_cleanups, decode_unit_affine_cleanup,
        encode_scalar_control_affine_cleanups,
    },
    codec::function_codec::{decode_functions, encode_functions},
    codec::function_stack_codec::{decode_function_stack_facts, encode_function_stack_facts},
    codec::internal_unit_call_codec::{decode_internal_unit_calls, encode_internal_unit_calls},
};
use crate::installation_record::codec::envelope_codec::MAGIC;
use crate::installation_record::record_validation::installed_scalar_control_cleanups_match_object;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::{EdgeId, PlaceId, StructuralCaseId, StructuralFieldId, ValueId};

pub(super) fn installed_function_with_unit_call() -> InstalledFunction {
    InstalledFunction {
        machine: MachineId::new(1).expect("function"),
        attachment: None,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        parameter_abi: None,
        structural_call_scalar_return: None,
        text_offset: 24,
        byte_count: 16,
        unit_stack: Some(crate::ObjectUnitStack {
            frame_bytes: 0,
            local_peak_bytes: 16,
            stack_alignment: 16,
        }),
        scalar_stack: None,
        unit_call_stacks: vec![crate::ObjectUnitCallStack {
            owner: CallSiteOwner::Operation(OperationId::new(1).expect("call operation")),
            target: MachineId::new(2).expect("callee"),
            text_offset: 28,
            active_frame_bytes: 0,
            transient_bytes: 16,
            caller_live_bytes: 16,
        }],
        scalar_call_stacks: Vec::new(),
        foreign_call_stacks: vec![InstalledForeignCallStack {
            owner: CallSiteOwner::Operation(OperationId::new(2).expect("foreign operation")),
            text_offset: 32,
            caller_live_bytes: 16,
            provider_plan_report_identity: 7,
            contribution_report_identity:
                task_plans::AdmittedStackContributionReportId::from_normalized_identity(8)
                    .expect("contribution report"),
            contribution_commitment: task_plans::SameStackContributionCommitment::from_digest(
                [9; 32],
            ),
            contribution_bytes: 64,
            contribution_alignment: 16,
        }],
        unit_body: false,
        unit_parameters: Vec::new(),
        unit_parameter_homes: Vec::new(),
        unit_scalar_homes: Vec::new(),
        unit_integer_constants: Vec::new(),
        unit_affine_scalar_records: Vec::new(),
        unit_structural_scalar_field_stores: Vec::new(),
        unit_write_only_primitive_stores: Vec::new(),
        scalar_structural_scalar_field_stores: Vec::new(),
        unit_continuations: Vec::new(),
        unit_affine_cleanup: None,
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: Vec::new(),
        scalar_structural_parameter_homes: Vec::new(),
    }
}

fn scalar_control_cleanup(edge: u64, code_offset: usize) -> machine_code::UnitAffineCleanupRecord {
    machine_code::UnitAffineCleanupRecord {
        psi_edge: EdgeId::new(edge).expect("cleanup edge"),
        structural_types: Vec::new().into(),
        locals: Vec::new(),
        actions: vec![terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
            PlaceId::new(1).expect("cleanup place"),
        )],
        code_offset,
        byte_count: 4,
    }
}

fn scalar_control_object_cleanup(
    edge: u64,
    code_offset: usize,
) -> machine_code::ScalarControlAffineCleanupRecord {
    machine_code::ScalarControlAffineCleanupRecord {
        cleanup: scalar_control_cleanup(edge, code_offset),
        preservation: machine_code::ScalarCleanupPreservationEvidence {
            frame: machine_code::StackAdjustmentPair {
                byte_size: 16,
                allocation_offset: code_offset,
                allocation_byte_count: 4,
                release_offset: code_offset + 3,
                release_byte_count: 1,
            },
            result_byte_offset: 0,
            result_store_offset: code_offset + 1,
            result_load_offset: code_offset + 2,
            aarch64_return_link: None,
        },
    }
}

#[test]
fn cleanup_decoder_rejects_impossible_capacity_before_allocation() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        decode_unit_affine_cleanup(&mut reader),
        Err(InstallationError::UnexpectedEnd)
    );
}

#[test]
fn scalar_control_cleanup_codec_accepts_finite_leaf_sets() {
    for count in [0_usize, 2, 3, 4] {
        let cleanups = (0..count)
            .map(|index| scalar_control_cleanup(index as u64 + 1, index * 8))
            .collect::<Vec<_>>();
        let mut bytes = Vec::new();
        encode_scalar_control_affine_cleanups(&mut bytes, &cleanups)
            .expect("encode finite cleanup leaf set");
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_scalar_control_affine_cleanups(&mut reader)
                .expect("decode finite cleanup leaf set"),
            cleanups
        );
        assert_eq!(reader.remaining(), 0);
    }

    let invalid = vec![scalar_control_cleanup(1, 0)];
    assert_eq!(
        encode_scalar_control_affine_cleanups(&mut Vec::new(), &invalid),
        Err(InstallationError::InvalidScalarControlAffineCleanupCount(1))
    );
    let mut encoded_count = Vec::new();
    push_u32(&mut encoded_count, 1);
    assert_eq!(
        decode_scalar_control_affine_cleanups(&mut Reader::new(&encoded_count)),
        Err(InstallationError::InvalidScalarControlAffineCleanupCount(1))
    );

    let mut impossible_capacity = Vec::new();
    push_u32(&mut impossible_capacity, u32::MAX);
    assert_eq!(
        decode_scalar_control_affine_cleanups(&mut Reader::new(&impossible_capacity)),
        Err(InstallationError::UnexpectedEnd)
    );
}

#[test]
fn scalar_control_cleanup_canonicality_rejects_edge_and_interval_corruption() {
    let cleanups = vec![
        scalar_control_cleanup(1, 4),
        scalar_control_cleanup(2, 12),
        scalar_control_cleanup(3, 20),
        scalar_control_cleanup(4, 28),
    ];
    assert!(scalar_control_affine_cleanups_are_canonical(&cleanups, 32));

    let mut duplicate_edge = cleanups.clone();
    duplicate_edge[1].psi_edge = duplicate_edge[0].psi_edge;
    assert!(!scalar_control_affine_cleanups_are_canonical(
        &duplicate_edge,
        32
    ));

    let mut overlapping = cleanups.clone();
    overlapping[1].code_offset = 7;
    assert!(!scalar_control_affine_cleanups_are_canonical(
        &overlapping,
        32
    ));

    let mut reordered = cleanups.clone();
    reordered.swap(0, 1);
    assert!(!scalar_control_affine_cleanups_are_canonical(
        &reordered, 32
    ));

    let mut changed_actions = cleanups.clone();
    changed_actions[2].actions.clear();
    assert!(!scalar_control_affine_cleanups_are_canonical(
        &changed_actions,
        32
    ));

    assert!(!scalar_control_affine_cleanups_are_canonical(&cleanups, 33));
}

#[test]
fn installed_scalar_control_cleanup_projection_binds_the_exact_object_records() {
    let emitted = vec![
        scalar_control_object_cleanup(1, 4),
        scalar_control_object_cleanup(2, 12),
        scalar_control_object_cleanup(3, 20),
    ];
    let mut installed = emitted
        .iter()
        .map(|record| record.cleanup.clone())
        .collect::<Vec<_>>();
    assert!(installed_scalar_control_cleanups_match_object(
        &installed, &emitted
    ));

    installed[1].psi_edge = EdgeId::new(9).expect("different edge");
    assert!(!installed_scalar_control_cleanups_match_object(
        &installed, &emitted
    ));
    installed[1] = emitted[1].cleanup.clone();
    installed[2].actions.clear();
    assert!(!installed_scalar_control_cleanups_match_object(
        &installed, &emitted
    ));
}

#[test]
fn stack_fact_codec_round_trips_exact_emitter_evidence() {
    let function = installed_function_with_unit_call();
    let mut bytes = Vec::new();
    encode_function_stack_facts(&mut bytes, &function).expect("encode stack facts");
    let mut reader = Reader::new(&bytes);
    let (unit, scalar, unit_calls, scalar_calls, foreign_calls) =
        decode_function_stack_facts(&mut reader).expect("decode stack facts");
    assert_eq!(unit, function.unit_stack);
    assert_eq!(scalar, function.scalar_stack);
    assert_eq!(unit_calls, function.unit_call_stacks);
    assert_eq!(scalar_calls, function.scalar_call_stacks);
    assert_eq!(foreign_calls, function.foreign_call_stacks);
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn function_codec_round_trips_structural_call_scalar_result_evidence() {
    let mut function = installed_function_with_unit_call();
    function.structural_call_scalar_return =
        Some(machine_code::StructuralCallScalarReturnEvidence {
            psi_edge: EdgeId::new(1).expect("return edge"),
            psi_operation: OperationId::new(1).expect("call operation"),
            source_value: ValueId::new(1).expect("source value"),
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
            callee: MachineId::new(2).expect("callee"),
        });
    let mut bytes = Vec::new();
    encode_functions(&mut bytes, 1, std::slice::from_ref(&function))
        .expect("encode function result evidence");
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        decode_functions(&mut reader).expect("decode function result evidence"),
        [function]
    );
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn internal_unit_call_codec_round_trips_exact_semantic_result() {
    let installed = InstalledInternalUnitCall {
        machine: MachineId::new(1).expect("caller"),
        text_offset: 8,
        custody: machine_code::InternalUnitCallRecord {
            source: machine_code::InternalUnitCallSource::Authored,
            owner: CallSiteOwner::Operation(OperationId::new(1).expect("call operation")),
            target: MachineId::new(2).expect("callee"),
            result: Some(semantic_vocabulary::ScalarType::Boolean),
            semantic_result: Some(abstract_operations::AbstractResult {
                value: ValueId::new(1).expect("call result"),
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            }),
            structural_result: None,
            scalar_arguments: Vec::new(),
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            operation_ordinal: 0,
            code_offset: 8,
            byte_count: 5,
        },
    };
    let mut bytes = Vec::new();
    encode_internal_unit_calls(&mut bytes, 1, std::slice::from_ref(&installed))
        .expect("encode semantic call result");
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        decode_internal_unit_calls(&mut reader).expect("decode semantic call result"),
        [installed]
    );
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn installed_stack_fact_shape_rejects_nonlocal_or_forged_call_inputs() {
    let functions = [
        (MachineId::new(1).expect("caller"), None),
        (MachineId::new(2).expect("callee"), None),
    ]
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>();
    let valid = installed_function_with_unit_call();
    assert!(installed_stack_facts_are_canonical(&valid, &functions));

    let mut nonlocal_offset = valid.clone();
    nonlocal_offset.unit_call_stacks[0].text_offset =
        nonlocal_offset.text_offset + nonlocal_offset.byte_count;
    assert!(!installed_stack_facts_are_canonical(
        &nonlocal_offset,
        &functions
    ));

    let mut forged_live_bytes = valid.clone();
    forged_live_bytes.unit_call_stacks[0].caller_live_bytes += 1;
    assert!(!installed_stack_facts_are_canonical(
        &forged_live_bytes,
        &functions
    ));

    let mut zero_provider_plan = valid.clone();
    zero_provider_plan.foreign_call_stacks[0].provider_plan_report_identity = 0;
    assert!(!installed_stack_facts_are_canonical(
        &zero_provider_plan,
        &functions
    ));

    let mut unsupported_foreign_alignment = valid.clone();
    unsupported_foreign_alignment.foreign_call_stacks[0].contribution_alignment = 32;
    assert!(!installed_stack_facts_are_canonical(
        &unsupported_foreign_alignment,
        &functions
    ));

    let mut zero_foreign_commitment = valid.clone();
    zero_foreign_commitment.foreign_call_stacks[0].contribution_commitment =
        task_plans::SameStackContributionCommitment::from_digest([0; 32]);
    assert!(!installed_stack_facts_are_canonical(
        &zero_foreign_commitment,
        &functions
    ));

    let mut invalid_alignment = valid;
    invalid_alignment
        .unit_stack
        .as_mut()
        .expect("unit stack")
        .stack_alignment = 3;
    assert!(!installed_stack_facts_are_canonical(
        &invalid_alignment,
        &functions
    ));
}

#[test]
fn native_reference_shapes_and_projections_carry_metadata_not_storage() {
    let primitive = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let declarations = vec![
        terminal_psi::StructuralTypeDeclaration {
            id: primitive,
            identity: "Boolean".into(),
            shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Boolean,
            ),
        },
        terminal_psi::StructuralTypeDeclaration {
            id: reference,
            identity: "MutableBooleanReference".into(),
            shape: terminal_psi::StructuralTypeShape::Reference {
                referent: primitive,
                access: terminal_psi::StructuralAccess::MutableBorrow,
            },
        },
    ];
    let mut bytes = Vec::new();
    encode_structural_types(&mut bytes, &declarations).expect("encode reference type");
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        decode_structural_types(&mut reader).expect("decode reference type"),
        declarations
    );
    assert_eq!(reader.remaining(), 0);
    // A carrier's value shape is the canonical empty aggregate slot; the loan
    // is metadata and never pointer-sized storage.
    assert_eq!(
        crate::object_artifact::replay::structural::condition_layout::replay_structural_value_shape(
            reference,
            &declarations,
        ),
        Some(calling_conventions::ValueShape::integer(0, 1))
    );
    let path = vec![terminal_psi::StructuralPathSegment::Referent];
    // A referent crossing is custody metadata, never a physical byte
    // projection or a partial-cleanup subtree.
    assert_eq!(
        crate::object_artifact::replay::structural::condition_layout::replay_structural_projection(
            reference,
            &path,
            &declarations,
        ),
        None
    );
    assert!(!is_partial_cleanup_path(&path));
    let argument = terminal_psi::StructuralArgument {
        place: PlaceId::new(1).unwrap(),
        path,
        access: terminal_psi::StructuralAccess::MutableBorrow,
    };
    let mut argument_bytes = Vec::new();
    structural_argument_codec::encode_structural_argument(&mut argument_bytes, &argument)
        .expect("encode referent argument");
    let mut reader = Reader::new(&argument_bytes);
    assert_eq!(
        structural_argument_codec::decode_structural_argument(&mut reader)
            .expect("decode referent argument"),
        argument
    );
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn boundary_opaque_application_custody_round_trips() {
    use super::codec::opaque_application_codec::{
        decode_boundary_opaque_applications, encode_boundary_opaque_applications,
    };
    let custody = boundary_applications::BoundaryOpaqueRepresentationApplications::new(vec![
        boundary_applications::BoundaryOpaqueRepresentationApplication {
            requirement_identity: "core::system::Table".into(),
            shape_root: 4,
            application_report_fingerprint: 0xA55A,
            selected_application_commitment: [0x11; 32],
        },
        boundary_applications::BoundaryOpaqueRepresentationApplication {
            requirement_identity: "core::ptr::Ptr".into(),
            shape_root: 2,
            application_report_fingerprint: 0xB66B,
            selected_application_commitment: [0x22; 32],
        },
    ])
    .expect("custody");
    let mut bytes = Vec::new();
    encode_boundary_opaque_applications(&mut bytes, &custody);
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        decode_boundary_opaque_applications(&mut reader).expect("decode custody"),
        custody
    );
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn boundary_opaque_application_custody_rejects_drifted_edge() {
    use super::codec::opaque_application_codec::{
        decode_boundary_opaque_applications, encode_boundary_opaque_applications,
    };
    // One edge coordinate cannot retain two different commitments: decode
    // fails closed rather than picking either application.
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 2);
    for commitment in [0x33_u8, 0x44_u8] {
        push_u32(&mut bytes, 4);
        bytes.extend_from_slice(b"edge");
        push_u16(&mut bytes, 1);
        push_u64(&mut bytes, 7);
        bytes.extend_from_slice(&[commitment; 32]);
    }
    let mut reader = Reader::new(&bytes);
    assert!(matches!(
        decode_boundary_opaque_applications(&mut reader),
        Err(InstallationError::InvalidBoundaryOpaqueApplicationCustody(
            _
        ))
    ));

    let custody = boundary_applications::BoundaryOpaqueRepresentationApplications::new(vec![
        boundary_applications::BoundaryOpaqueRepresentationApplication {
            requirement_identity: "edge".into(),
            shape_root: 1,
            application_report_fingerprint: 7,
            selected_application_commitment: [0x33; 32],
        },
    ])
    .expect("custody");
    let mut encoded = Vec::new();
    encode_boundary_opaque_applications(&mut encoded, &custody);
    // An empty requirement identity cannot encode one edge.
    let zero_identity = encoded.iter().position(|b| *b == 0).is_some();
    assert!(zero_identity, "sanity");
    let mut forged = encoded.clone();
    // identity len field begins at byte 4 (count prefix is u32)
    forged[4..8].copy_from_slice(&0_u32.to_le_bytes());
    forged.drain(8..12);
    let mut reader = Reader::new(&forged);
    assert_eq!(
        decode_boundary_opaque_applications(&mut reader),
        Err(InstallationError::InvalidBoundaryOpaqueApplicationIdentity)
    );
}

#[test]
fn previous_installation_marker_is_not_accepted() {
    for previous_marker in [95, INSTALLATION_FORMAT_MARKER - 1] {
        let mut bytes = MAGIC.to_vec();
        push_u16(&mut bytes, previous_marker);
        assert_eq!(
            decode_installation_record(&bytes),
            Err(InstallationError::UnsupportedFormatMarker(previous_marker))
        );
    }
}

#[test]
fn ieee_structural_and_scalar_field_formats_round_trip_in_installations() {
    let declarations = vec![terminal_psi::StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).expect("structural type"),
        identity: "Samples".into(),
        shape: terminal_psi::StructuralTypeShape::Record {
            fields: vec![
                terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).expect("f32 field"),
                    identity: "single".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ),
                },
                terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(2).expect("f64 field"),
                    identity: "double".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ),
                },
                terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(3).expect("scalar f32 field"),
                    identity: "scalar_single".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::IeeeFloat(
                            semantic_vocabulary::IeeeFloatFormat::Binary32,
                        ),
                    ),
                },
                terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(4).expect("scalar f64 field"),
                    identity: "scalar_double".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::IeeeFloat(
                            semantic_vocabulary::IeeeFloatFormat::Binary64,
                        ),
                    ),
                },
            ],
        },
    }];
    let mut bytes = Vec::new();
    encode_structural_types(&mut bytes, &declarations).expect("encode structural types");
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        decode_structural_types(&mut reader),
        Ok(declarations.clone())
    );
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn byte_sequence_carriers_round_trip_in_installations() {
    let declarations = vec![terminal_psi::StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).expect("structural type"),
        identity: "TextFields".into(),
        shape: terminal_psi::StructuralTypeShape::Record {
            fields: vec![
                terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).expect("borrowed field"),
                    identity: "borrowed".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                    ),
                },
                terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(2).expect("bounded field"),
                    identity: "bounded".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                    ),
                },
            ],
        },
    }];
    let mut bytes = Vec::new();
    encode_structural_types(&mut bytes, &declarations).expect("encode structural types");
    let mut reader = Reader::new(&bytes);
    assert_eq!(decode_structural_types(&mut reader), Ok(declarations));
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn payload_sum_case_fields_round_trip_in_installations() {
    let declarations = vec![terminal_psi::StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).expect("structural type"),
        identity: "Mode".into(),
        shape: terminal_psi::StructuralTypeShape::Sum {
            cases: vec![
                terminal_psi::StructuralCaseDeclaration {
                    id: StructuralCaseId::new(1).expect("off case"),
                    identity: "Off".into(),
                    fields: Vec::new(),
                },
                terminal_psi::StructuralCaseDeclaration {
                    id: StructuralCaseId::new(2).expect("on case"),
                    identity: "On".into(),
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).expect("payload field"),
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(
                            semantic_vocabulary::ScalarType::Integer(
                                semantic_vocabulary::IntegerType::new(
                                    semantic_vocabulary::IntegerSign::Signed,
                                    32,
                                )
                                .expect("i32"),
                            ),
                        ),
                    }],
                },
            ],
        },
    }];
    let mut bytes = Vec::new();
    encode_structural_types(&mut bytes, &declarations).expect("encode structural sum");
    let mut reader = Reader::new(&bytes);
    assert_eq!(decode_structural_types(&mut reader), Ok(declarations));
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn mixed_common_fields_and_cases_round_trip_in_installations() {
    let declarations = vec![terminal_psi::StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).expect("structural type"),
        identity: "Message".into(),
        shape: terminal_psi::StructuralTypeShape::Mixed {
            fields: vec![terminal_psi::StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).expect("common field"),
                identity: "active".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::Scalar(
                    semantic_vocabulary::ScalarType::Boolean,
                ),
            }],
            cases: vec![
                terminal_psi::StructuralCaseDeclaration {
                    id: StructuralCaseId::new(1).expect("empty case"),
                    identity: "Empty".into(),
                    fields: Vec::new(),
                },
                terminal_psi::StructuralCaseDeclaration {
                    id: StructuralCaseId::new(2).expect("data case"),
                    identity: "Data".into(),
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).expect("payload field"),
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(
                            semantic_vocabulary::ScalarType::Integer(
                                semantic_vocabulary::IntegerType::new(
                                    semantic_vocabulary::IntegerSign::Signed,
                                    32,
                                )
                                .expect("i32"),
                            ),
                        ),
                    }],
                },
            ],
        },
    }];
    let mut bytes = Vec::new();
    encode_structural_types(&mut bytes, &declarations).expect("encode mixed structural type");
    let mut reader = Reader::new(&bytes);
    assert_eq!(decode_structural_types(&mut reader), Ok(declarations));
    assert_eq!(reader.remaining(), 0);
}
