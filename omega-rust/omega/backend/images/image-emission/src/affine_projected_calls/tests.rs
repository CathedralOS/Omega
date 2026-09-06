use super::*;
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{
    EdgeId, MachineId, OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, StructuralAffineDiscard, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment,
};

fn type_id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

fn index(value: u64) -> StructuralPathSegment {
    StructuralPathSegment::FixedIndex(value)
}

fn field(identity: &str) -> StructuralPathSegment {
    StructuralPathSegment::Field(identity.into())
}

fn declaration(value: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: type_id(value),
        identity: format!("Type{value}"),
        shape,
    }
}

fn structural_field(value: u64, identity: &str, child: u64) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(value).unwrap(),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(type_id(child)),
    }
}

fn token() -> StructuralTypeDeclaration {
    declaration(
        1,
        StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            }],
        },
    )
}

fn residual(path: Vec<StructuralPathSegment>, child: u64) -> StructuralAffineDiscard {
    StructuralAffineDiscard {
        place: PlaceId::new(1).unwrap(),
        path,
        structural_type: type_id(child),
    }
}

fn partition(
    declarations: &[StructuralTypeDeclaration],
    root: u64,
    moved: &[(Vec<StructuralPathSegment>, u64)],
    residuals: &[StructuralAffineDiscard],
) -> bool {
    crate::exact_partial_cleanup_partition(
        declarations,
        type_id(root),
        &moved
            .iter()
            .map(|(path, child)| (path.as_slice(), type_id(*child)))
            .collect::<Vec<_>>(),
        &residuals.iter().collect::<Vec<_>>(),
    )
}

#[test]
fn wider_array_replays_reverse_residuals_and_rejects_partition_mutations() {
    let declarations = vec![
        token(),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: type_id(1),
                length: 5,
            },
        ),
    ];
    let moved = vec![(vec![index(2)], 1)];
    let residuals = [4, 3, 1, 0].map(|value| residual(vec![index(value)], 1));
    assert!(partition(&declarations, 2, &moved, &residuals));
    assert!(!partition(&declarations, 2, &moved, &[]));
    assert!(!partition(&declarations, 2, &moved, &residuals[..3]));
    let mut reordered = residuals.clone();
    reordered.swap(0, 1);
    assert!(!partition(&declarations, 2, &moved, &reordered));
    let mut mistyped = residuals.clone();
    mistyped[0].structural_type = type_id(2);
    assert!(!partition(&declarations, 2, &moved, &mistyped));
    let mut moved_twice = moved.clone();
    moved_twice.extend(moved.clone());
    assert!(!partition(&declarations, 2, &moved_twice, &residuals));
    assert!(!partition(
        &declarations,
        2,
        &[(vec![index(5)], 1)],
        &residuals
    ));
}

#[test]
fn mixed_paths_preserve_maximal_untouched_rows() {
    let declarations = vec![
        token(),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: type_id(1),
                length: 3,
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                fields: vec![
                    structural_field(1, "items", 2),
                    structural_field(2, "tail", 1),
                ],
            },
        ),
        declaration(
            4,
            StructuralTypeShape::FixedArray {
                element: type_id(3),
                length: 3,
            },
        ),
        declaration(
            5,
            StructuralTypeShape::Record {
                fields: vec![structural_field(1, "rows", 4)],
            },
        ),
    ];
    let moved = vec![(vec![field("rows"), index(1), field("items"), index(1)], 1)];
    let residuals = vec![
        residual(vec![field("rows"), index(2)], 3),
        residual(vec![field("rows"), index(1), field("tail")], 1),
        residual(vec![field("rows"), index(1), field("items"), index(2)], 1),
        residual(vec![field("rows"), index(1), field("items"), index(0)], 1),
        residual(vec![field("rows"), index(0)], 3),
    ];
    assert!(partition(&declarations, 5, &moved, &residuals));
    let mut expanded = residuals.clone();
    expanded.splice(
        0..1,
        [
            residual(vec![field("rows"), index(2), field("tail")], 1),
            residual(vec![field("rows"), index(2), field("items")], 2),
        ],
    );
    assert!(!partition(&declarations, 5, &moved, &expanded));
    let mut overlap = moved.clone();
    overlap.push((vec![field("rows"), index(1)], 3));
    assert!(!partition(&declarations, 5, &overlap, &residuals));
    let mut unknown = moved;
    unknown[0].0[2] = field("missing");
    assert!(!partition(&declarations, 5, &unknown, &residuals));
}

#[test]
fn empty_complement_requires_every_structural_descendant() {
    let declarations = vec![
        token(),
        declaration(
            2,
            StructuralTypeShape::Record {
                fields: vec![
                    structural_field(1, "first", 1),
                    structural_field(2, "last", 1),
                ],
            },
        ),
    ];
    let moved = vec![(vec![field("last")], 1), (vec![field("first")], 1)];
    assert!(partition(&declarations, 2, &moved, &[]));
    assert!(!partition(&declarations, 2, &moved[..1], &[]));
    assert!(!partition(&declarations, 2, &[], &[]));
    assert!(!partition(&declarations, 2, &[(Vec::new(), 2)], &[]));
}

#[test]
fn huge_dimension_is_rejected_against_supplied_cleanup_before_enumeration() {
    let declarations = vec![
        token(),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: type_id(1),
                length: u64::MAX,
            },
        ),
    ];
    assert!(!partition(&declarations, 2, &[(vec![index(0)], 1)], &[]));
    assert!(!partition(
        &declarations,
        2,
        &[(vec![index(0)], 1)],
        &[residual(vec![index(1)], 1)]
    ));
}

#[test]
fn malformed_type_graphs_do_not_authorize_empty_cleanup() {
    let declarations = vec![
        token(),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: type_id(1),
                length: 1,
            },
        ),
    ];
    let moved = [(vec![index(0)], 1)];
    assert!(partition(&declarations, 2, &moved, &[]));
    let mut duplicate = declarations.clone();
    duplicate[1].identity = duplicate[0].identity.clone();
    assert!(!partition(&duplicate, 2, &moved, &[]));
    let mut cycle = declarations.clone();
    cycle[0].shape = StructuralTypeShape::FixedArray {
        element: type_id(2),
        length: 1,
    };
    assert!(!partition(&cycle, 2, &moved, &[]));
    let mut missing = declarations.clone();
    missing[1].shape = StructuralTypeShape::FixedArray {
        element: type_id(3),
        length: 1,
    };
    assert!(!partition(&missing, 2, &moved, &[]));
    let mut reordered = declarations;
    reordered.reverse();
    assert!(!partition(&reordered, 2, &moved, &[]));
}

// These records exercise the local metadata replay helpers. Executable bytes,
// ABI locations and relocation joins are validated by the surrounding consumers.
fn home(root: u64, bytes: u16) -> UnitParameterHomeRecord {
    let shape = ValueShape::integer(bytes, 1);
    UnitParameterHomeRecord {
        place: PlaceId::new(1).unwrap(),
        structural_type: type_id(root),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        shape,
        source: ValuePlacement {
            shape,
            locations: Vec::new(),
        },
        location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
        indirect: false,
    }
}

fn argument(
    home: &UnitParameterHomeRecord,
    path: Vec<StructuralPathSegment>,
    offset: u32,
    length: Option<u64>,
    stride: Option<u32>,
) -> InternalUnitCallArgumentRecord {
    let shape = ValueShape::integer(1, 1);
    InternalUnitCallArgumentRecord {
        place: home.place,
        access: StructuralAccess::Owned,
        path,
        root_structural_type: home.structural_type,
        structural_type: type_id(1),
        shape,
        source_byte_offset: offset,
        source_location: home.location,
        call_stack_bytes: 0,
        fixed_array_length: length,
        element_stride: stride,
        source: home.source.clone(),
        destination: ValuePlacement {
            shape,
            locations: Vec::new(),
        },
        code_offset: 0,
        byte_count: 0,
        bytes: Vec::new(),
    }
}

#[test]
fn full_path_layout_keeps_root_array_metadata_and_record_root_absence() {
    let mut declarations = vec![
        token(),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: type_id(1),
                length: 3,
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                fields: vec![
                    structural_field(1, "items", 2),
                    structural_field(2, "tail", 1),
                ],
            },
        ),
        declaration(
            4,
            StructuralTypeShape::FixedArray {
                element: type_id(3),
                length: 5,
            },
        ),
    ];
    let array_home = home(4, 20);
    let projected = argument(
        &array_home,
        vec![index(2), field("items"), index(1)],
        9,
        Some(5),
        Some(4),
    );
    assert!(exact_owned_projection(
        &projected,
        &array_home,
        &declarations
    ));
    for changed in [
        InternalUnitCallArgumentRecord {
            source_byte_offset: 8,
            ..projected.clone()
        },
        InternalUnitCallArgumentRecord {
            fixed_array_length: Some(3),
            ..projected.clone()
        },
        InternalUnitCallArgumentRecord {
            element_stride: Some(1),
            ..projected.clone()
        },
        InternalUnitCallArgumentRecord {
            structural_type: type_id(2),
            ..projected.clone()
        },
        InternalUnitCallArgumentRecord {
            source_location: machine_code::StructuralSourceLocation::Stack { byte_offset: 1 },
            ..projected.clone()
        },
    ] {
        assert!(!exact_owned_projection(
            &changed,
            &array_home,
            &declarations
        ));
    }
    declarations.push(declaration(
        5,
        StructuralTypeShape::Record {
            fields: vec![structural_field(1, "rows", 4)],
        },
    ));
    let record_home = home(5, 20);
    let projected = argument(
        &record_home,
        vec![field("rows"), index(2), field("items"), index(1)],
        9,
        None,
        None,
    );
    assert!(exact_owned_projection(
        &projected,
        &record_home,
        &declarations
    ));
    let forged = InternalUnitCallArgumentRecord {
        fixed_array_length: Some(5),
        element_stride: Some(4),
        ..projected
    };
    assert!(!exact_owned_projection(
        &forged,
        &record_home,
        &declarations
    ));
}

#[test]
fn function_closure_checks_calls_and_partition_even_when_cleanup_is_empty() {
    let parameter = home(2, 5);
    let mut cleanup = UnitAffineCleanupRecord {
        psi_edge: EdgeId::new(1).unwrap(),
        structural_types: vec![
            token(),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element: type_id(1),
                    length: 5,
                },
            ),
        ],
        locals: Vec::new(),
        actions: Vec::new(),
        code_offset: 20,
        byte_count: 4,
    };
    let calls = [2, 4, 0, 3, 1]
        .into_iter()
        .enumerate()
        .map(|(ordinal, moved)| InternalUnitCallRecord {
            source: machine_code::InternalUnitCallSource::Authored,
            owner: CallSiteOwner::Operation(
                OperationId::new(u64::try_from(ordinal + 1).unwrap()).unwrap(),
            ),
            target: MachineId::new(1).unwrap(),
            result: None,
            semantic_result: None,
            structural_result: None,
            scalar_arguments: Vec::new(),
            arguments: vec![argument(
                &parameter,
                vec![index(moved)],
                u32::try_from(moved).unwrap(),
                Some(5),
                Some(1),
            )],
            claim_transfers: Vec::new(),
            operation_ordinal: ordinal,
            code_offset: ordinal * 4,
            byte_count: 4,
        })
        .collect::<Vec<_>>();
    let homes = [parameter];
    assert!(exact_fully_consumed_affine_parameter(
        &homes,
        &calls,
        Some(&cleanup)
    ));
    assert!(!exact_fully_consumed_affine_parameter(
        &homes,
        &calls[..4],
        Some(&cleanup)
    ));
    let mut bad_owner = calls.clone();
    bad_owner[2].owner = bad_owner[0].owner;
    assert!(!exact_fully_consumed_affine_parameter(
        &homes,
        &bad_owner,
        Some(&cleanup)
    ));
    let mut bad_order = calls.clone();
    bad_order[1].operation_ordinal = 0;
    assert!(!exact_fully_consumed_affine_parameter(
        &homes,
        &bad_order,
        Some(&cleanup)
    ));
    let mut bad_span = calls.clone();
    bad_span[1].code_offset = 3;
    assert!(!exact_fully_consumed_affine_parameter(
        &homes,
        &bad_span,
        Some(&cleanup)
    ));
    cleanup.actions = [4, 3, 1, 0]
        .map(|value| TerminalAffineCleanupAction::DiscardResidual(residual(vec![index(value)], 1)))
        .to_vec();
    assert!(exact_partially_consumed_affine_parameter(
        &homes,
        &calls[..1],
        Some(&cleanup)
    ));
    assert!(!exact_fully_consumed_affine_parameter(
        &homes,
        &calls,
        Some(&cleanup)
    ));
    cleanup.actions.swap(0, 1);
    assert!(!exact_partially_consumed_affine_parameter(
        &homes,
        &calls[..1],
        Some(&cleanup)
    ));
}

fn projected_row_copy(
    destination: Vec<calling_conventions::ValueLocation>,
    call_stack_bytes: u32,
) -> InternalUnitCallArgumentRecord {
    use calling_conventions::{IndirectPointerLocation, MachineRegister, ValueLocation};
    let mut parameter = home(2, 48);
    parameter.shape = ValueShape::integer(48, 8);
    parameter.source = ValuePlacement {
        shape: parameter.shape,
        locations: vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdi),
            copy_stack_byte_offset: Some(0),
            byte_size: 48,
            alignment: 8,
        }],
    };
    let mut copy = argument(&parameter, vec![index(1)], 24, Some(2), Some(24));
    copy.shape = ValueShape::integer(24, 8);
    copy.destination = ValuePlacement {
        shape: copy.shape,
        locations: destination,
    };
    copy.call_stack_bytes = call_stack_bytes;
    copy
}

#[test]
fn owned_subtree_stack_fragments_have_independent_exact_copy_bytes() {
    use calling_conventions::ValueLocation;
    let copy = projected_row_copy(
        (0..3)
            .map(|fragment| ValueLocation::Stack {
                stack_byte_offset: fragment * 8,
                value_byte_offset: u16::try_from(fragment * 8).unwrap(),
                byte_size: 8,
                alignment: 8,
            })
            .collect(),
        32,
    );
    let expected = vec![
        0x4c, 0x8b, 0x5c, 0x24, 0x20, 0x49, 0x8b, 0x43, 0x18, 0x48, 0x89, 0x44, 0x24, 0x00, 0x49,
        0x8b, 0x43, 0x20, 0x48, 0x89, 0x44, 0x24, 0x08, 0x49, 0x8b, 0x43, 0x28, 0x48, 0x89, 0x44,
        0x24, 0x10,
    ];
    assert_eq!(
        crate::unit_call_custody::expected_projected_copy_bytes(
            target::NativeTarget::linux_x64(),
            &copy,
        ),
        Some(expected)
    );
    let mut gap = copy.clone();
    gap.destination.locations.remove(1);
    assert!(
        crate::unit_call_custody::expected_projected_copy_bytes(
            target::NativeTarget::linux_x64(),
            &gap,
        )
        .is_none()
    );
    let mut wrong_offset = copy;
    wrong_offset.source_byte_offset += 8;
    assert!(
        crate::unit_call_custody::expected_projected_copy_bytes(
            target::NativeTarget::linux_x64(),
            &wrong_offset,
        )
        .is_none()
    );
}

#[test]
fn owned_subtree_indirect_temporary_has_exact_x86_and_aarch64_bytes() {
    use calling_conventions::{IndirectPointerLocation, MachineRegister, ValueLocation};
    let windows = projected_row_copy(
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: Some(32),
            byte_size: 24,
            alignment: 8,
        }],
        64,
    );
    let expected = vec![
        0x4c, 0x8b, 0x5c, 0x24, 0x40, 0x49, 0x8b, 0x43, 0x18, 0x48, 0x89, 0x44, 0x24, 0x20, 0x49,
        0x8b, 0x43, 0x20, 0x48, 0x89, 0x44, 0x24, 0x28, 0x49, 0x8b, 0x43, 0x28, 0x48, 0x89, 0x44,
        0x24, 0x30, 0x48, 0x8d, 0x8c, 0x24, 0x20, 0x00, 0x00, 0x00,
    ];
    assert_eq!(
        crate::unit_call_custody::expected_projected_copy_bytes(
            target::NativeTarget::windows_x64(),
            &windows,
        ),
        Some(expected)
    );
    let mut aarch64 = projected_row_copy(
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
            copy_stack_byte_offset: Some(0),
            byte_size: 24,
            alignment: 8,
        }],
        32,
    );
    aarch64.source.locations = vec![ValueLocation::Indirect {
        pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
        copy_stack_byte_offset: Some(0),
        byte_size: 48,
        alignment: 8,
    }];
    let instructions: [u32; 8] = [
        0xf940_13e9,
        0xf940_0d2a,
        0xf900_03ea,
        0xf940_112a,
        0xf900_07ea,
        0xf940_152a,
        0xf900_0bea,
        0x9100_03e0,
    ];
    assert_eq!(
        crate::unit_call_custody::expected_projected_copy_bytes(
            target::NativeTarget::linux_arm64(),
            &aarch64,
        ),
        Some(
            instructions
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect()
        )
    );
    for location in [
        ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: None,
            byte_size: 24,
            alignment: 8,
        },
        ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: Some(48),
            byte_size: 24,
            alignment: 8,
        },
        ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Stack {
                stack_byte_offset: 32,
                alignment: 8,
            },
            copy_stack_byte_offset: Some(32),
            byte_size: 24,
            alignment: 8,
        },
    ] {
        let mut forged = windows.clone();
        forged.destination.locations = vec![location];
        assert!(
            crate::unit_call_custody::expected_projected_copy_bytes(
                target::NativeTarget::windows_x64(),
                &forged,
            )
            .is_none()
        );
    }
}
