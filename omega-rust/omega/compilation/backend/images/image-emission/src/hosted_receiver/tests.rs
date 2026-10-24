use super::{
    ProgramEntryPhysicalContractPlan, physical_contract_matches, receiver_pointer_matches,
    zero_valid_field, zero_valid_record_storage,
};
use calling_conventions::{
    IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement, ValueShape,
};

fn assert_corrupted_receiver_rejected(
    shape: ValueShape,
    placement: &ValuePlacement,
    receiver: MachineRegister,
    decoy: MachineRegister,
) {
    let indirect = |pointer, copy_stack_byte_offset, byte_size, alignment| ValuePlacement {
        shape,
        locations: vec![ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset,
            byte_size,
            alignment,
        }],
    };
    let pointer = IndirectPointerLocation::Register(receiver);
    for corrupted in [
        // A substituted register is a receiver substitution, not an alias.
        indirect(
            IndirectPointerLocation::Register(decoy),
            None,
            shape.byte_size,
            shape.alignment,
        ),
        indirect(
            IndirectPointerLocation::Stack {
                stack_byte_offset: 0,
                alignment: 8,
            },
            None,
            shape.byte_size,
            shape.alignment,
        ),
        indirect(pointer, Some(0), shape.byte_size, shape.alignment),
        indirect(pointer, None, shape.byte_size + 1, shape.alignment),
        indirect(pointer, None, shape.byte_size, shape.alignment * 2),
        ValuePlacement {
            shape,
            locations: Vec::new(),
        },
        ValuePlacement {
            shape,
            locations: vec![placement.locations[0]; 2],
        },
        ValuePlacement {
            shape,
            locations: vec![ValueLocation::Register {
                register: receiver,
                value_byte_offset: 0,
                byte_size: 8,
            }],
        },
        ValuePlacement {
            shape: ValueShape::integer(shape.byte_size, shape.alignment),
            locations: placement.locations.clone(),
        },
    ] {
        assert!(
            !receiver_pointer_matches(shape, &corrupted, receiver),
            "corrupted receiver: {corrupted:?}"
        );
    }
}

#[test]
fn retained_service_leaf_cannot_be_zero_filled_or_omit_its_establishment() {
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let service = StructuralTypeId::new(2).unwrap();
    let field = StructuralFieldDeclaration {
        storage: None,
        id: StructuralFieldId::new(1).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(1),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(service),
    };
    let declarations = [
        StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: root,
            identity: terminal_psi::TerminalIdentityIndex(2),
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![field.clone()],
            },
        },
        StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: service,
            identity: terminal_psi::TerminalIdentityIndex(3),
            shape: StructuralTypeShape::BindingCarrier,
        },
    ];
    let source = program_entry_plan::SelectedProgramEntrySignature::from_checked_declaration(
        target::TargetProfile::MacosArm64.program_entry_slot(),
        "package:test::Main::entry".to_owned(),
        "Main::entry".into(),
        "entry".into(),
        terminal_psi::TerminalIdentityIndex(1),
        program_entry_plan::ProgramEntryReceiverSignature::ProvisionedMutable {
            source_position: 0,
            normalized_type_identity: terminal_psi::TerminalIdentityIndex(2),
        },
        Vec::new(),
        terminal_psi::TerminalIdentityTable::from(vec![
            String::new(),
            "Main::entry()".to_owned(),
            "Main".to_owned(),
        ]),
    )
    .unwrap();
    assert!(!zero_valid_record_storage(
        &declarations,
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
    assert!(!zero_valid_record_storage(
        &declarations,
        service,
        &mut Vec::new(),
        zero_valid_field
    ));
    let mut used = super::ErasedRowUse::default();
    assert!(
        super::validate_receiver_fields(
            &declarations,
            &[field],
            &source,
            terminal_psi::TerminalIdentityIndex(2),
            terminal_psi::TerminalIdentityIndex(2),
            &Default::default(),
            super::ErasedRows {
                services: &[],
                determined_evidence: &[]
            },
            &mut Vec::new(),
            &mut vec![root],
            &mut used,
            &|| diagnostics::Diagnostic::error("unestablished service"),
        )
        .is_err()
    );
    assert_eq!(used.services, 0);
}
#[test]
fn zero_filled_receiver_byte_fields_require_owned_backing_or_a_shared_view() {
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let child = StructuralTypeId::new(2).unwrap();
    let field = |field_type| StructuralFieldDeclaration {
        storage: None,
        id: StructuralFieldId::new(1).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(5),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    for carrier in [
        ByteSequenceCarrier::BoundedOwned { capacity: 0 },
        ByteSequenceCarrier::BoundedOwned { capacity: 3 },
        ByteSequenceCarrier::BoundedOwned { capacity: 9 },
        ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        },
    ] {
        let declarations = [
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: root,
                identity: terminal_psi::TerminalIdentityIndex(6),
                shape: StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![field(StructuralFieldType::Structural(child))],
                },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: child,
                identity: terminal_psi::TerminalIdentityIndex(7),
                shape: StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![field(StructuralFieldType::ByteSequence(carrier))],
                },
            },
        ];
        // Zero owned storage has live length zero; a zero shared descriptor
        // is the empty view, which borrows nothing.
        let expected = matches!(
            carrier,
            ByteSequenceCarrier::BoundedOwned { .. }
                | ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                }
        );
        for selected in [root, child] {
            assert_eq!(
                zero_valid_record_storage(
                    &declarations,
                    selected,
                    &mut Vec::new(),
                    zero_valid_field
                ),
                expected,
                "carrier {carrier:?}, root {selected:?}",
            );
        }
    }
}

#[test]
fn nested_receiver_storage_rejects_missing_cyclic_erased_and_zero_excluded_leaves() {
    use semantic_vocabulary::{
        BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralFieldId,
        StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let child = StructuralTypeId::new(2).unwrap();
    let field = |field_type| StructuralFieldDeclaration {
        storage: None,
        id: StructuralFieldId::new(1).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(8),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let declarations = vec![
        StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: root,
            identity: terminal_psi::TerminalIdentityIndex(6),
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![field(StructuralFieldType::Structural(child))],
            },
        },
        StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: child,
            identity: terminal_psi::TerminalIdentityIndex(7),
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![field(StructuralFieldType::Scalar(ScalarType::Boolean))],
            },
        },
    ];
    assert!(zero_valid_record_storage(
        &declarations,
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
    for required in [false, true] {
        let mut refined = declarations.clone();
        refined[1].shape = StructuralTypeShape::Record {
            storage: terminal_psi::StructuralRecordStorage::Packed,
            fields: vec![field(StructuralFieldType::RefinedScalar(
                semantic_vocabulary::ScalarRefinement::new(
                    ScalarType::Boolean,
                    semantic_vocabulary::Proposition::Equal(
                        semantic_vocabulary::ScalarTerm::value(
                            semantic_vocabulary::ScalarRefinement::SUBJECT,
                            ScalarType::Boolean,
                        ),
                        semantic_vocabulary::ScalarTerm::boolean(required),
                    ),
                )
                .unwrap(),
            ))],
        };
        assert_eq!(
            zero_valid_record_storage(&refined, root, &mut Vec::new(), zero_valid_field),
            !required
        );
    }
    assert!(!zero_valid_record_storage(
        &declarations[..1],
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
    for field_type in [
        StructuralFieldType::Structural(root),
        StructuralFieldType::Erased {
            type_identity: terminal_psi::TerminalIdentityIndex(9),
        },
        StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                IntegerValue::Signed(1),
                IntegerValue::Signed(9),
            )
            .unwrap(),
        ),
    ] {
        let mut changed = declarations.clone();
        changed[1].shape = StructuralTypeShape::Record {
            storage: terminal_psi::StructuralRecordStorage::Packed,
            fields: vec![field(field_type)],
        };
        assert!(!zero_valid_record_storage(
            &changed,
            root,
            &mut Vec::new(),
            zero_valid_field
        ));
    }
    let mut duplicated = declarations.clone();
    duplicated.push(declarations[1].clone());
    assert!(!zero_valid_record_storage(
        &duplicated,
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
}

#[test]
fn nested_receiver_storage_follows_the_zero_tag_sum_case() {
    use semantic_vocabulary::{
        BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralCaseId,
        StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let sum = StructuralTypeId::new(2).unwrap();
    let field = |field_type| StructuralFieldDeclaration {
        storage: None,
        id: StructuralFieldId::new(1).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(8),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let case = |fields| StructuralCaseDeclaration {
        id: StructuralCaseId::new(1).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(10),
        fields,
    };
    let sum_declaration = |fields: Vec<StructuralFieldDeclaration>| StructuralTypeDeclaration {
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        copyable: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id: sum,
        identity: terminal_psi::TerminalIdentityIndex(11),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                case(fields),
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(2).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(12),
                    fields: vec![field(StructuralFieldType::Erased {
                        type_identity: terminal_psi::TerminalIdentityIndex(13),
                    })],
                },
            ],
        },
    };
    let declarations = |sum_shape: StructuralTypeDeclaration| {
        vec![
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: root,
                identity: terminal_psi::TerminalIdentityIndex(6),
                shape: StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![field(StructuralFieldType::Structural(sum))],
                },
            },
            sum_shape,
        ]
    };
    // A payloadless or zero-valid first case inhabits zero storage; later
    // cases — even erased or invalid ones — are never reached.
    assert!(zero_valid_record_storage(
        &declarations(sum_declaration(vec![])),
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
    assert!(zero_valid_record_storage(
        &declarations(sum_declaration(vec![field(StructuralFieldType::Scalar(
            ScalarType::Boolean
        ))])),
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
    for first_case_fields in [
        vec![field(StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                IntegerValue::Signed(1),
                IntegerValue::Signed(9),
            )
            .unwrap(),
        ))],
        // An exclusive view would alias storage provisioning cannot supply.
        vec![field(StructuralFieldType::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::MutableBorrow),
            },
        ))],
        vec![StructuralFieldDeclaration {
            storage: None,
            relevance: BindingRelevance::Erased,
            ..field(StructuralFieldType::Scalar(ScalarType::Boolean))
        }],
        vec![field(StructuralFieldType::Erased {
            type_identity: terminal_psi::TerminalIdentityIndex(14),
        })],
    ] {
        let label = format!("{first_case_fields:?}");
        assert!(
            !zero_valid_record_storage(
                &declarations(sum_declaration(first_case_fields)),
                root,
                &mut Vec::new(),
                zero_valid_field
            ),
            "first-case payload {label}"
        );
    }
    // A sum with no cases has no zero-storage inhabitant.
    let mut empty = sum_declaration(vec![]);
    empty.shape = StructuralTypeShape::Sum { cases: vec![] };
    assert!(!zero_valid_record_storage(
        &declarations(empty),
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
}

#[test]
fn nested_receiver_storage_checks_array_elements_and_mixed_common_fields() {
    use semantic_vocabulary::{
        BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralCaseId,
        StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let element = StructuralTypeId::new(3).unwrap();
    let nested_array = StructuralTypeId::new(5).unwrap();
    let leaf = StructuralTypeId::new(6).unwrap();
    let mixed = StructuralTypeId::new(4).unwrap();
    let field = |field_type| StructuralFieldDeclaration {
        storage: None,
        id: StructuralFieldId::new(1).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(8),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    // Record element arrays and nested fixed arrays are zero-valid when the
    // complete element chain is; the element judgment is length-independent.
    for (length, element_shape) in [
        (
            2,
            StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![field(StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                )))],
            },
        ),
        (0, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
        (
            3,
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: 4,
            }),
        ),
        (
            2,
            StructuralTypeShape::FixedArray {
                element: leaf,
                length: 5,
            },
        ),
    ] {
        let mut declarations = vec![
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: root,
                identity: terminal_psi::TerminalIdentityIndex(6),
                shape: StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![field(StructuralFieldType::Structural(array))],
                },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: array,
                identity: terminal_psi::TerminalIdentityIndex(15),
                shape: StructuralTypeShape::FixedArray { element, length },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: element,
                identity: terminal_psi::TerminalIdentityIndex(16),
                shape: element_shape.clone(),
            },
        ];
        if let StructuralTypeShape::FixedArray { .. } = element_shape {
            // A fixed array of fixed arrays bottoms out in a scalar leaf.
            declarations[1].shape = StructuralTypeShape::FixedArray {
                element: nested_array,
                length,
            };
            declarations[2].id = nested_array;
            declarations.push(StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: leaf,
                identity: terminal_psi::TerminalIdentityIndex(17),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            });
        }
        assert!(
            zero_valid_record_storage(&declarations, root, &mut Vec::new(), zero_valid_field),
            "length {length} element {element_shape:?}"
        );
    }
    // Elements that zero cannot establish reject through the same chain.
    for element_shape in [
        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        }),
        StructuralTypeShape::Record {
            storage: terminal_psi::StructuralRecordStorage::Packed,
            fields: vec![field(StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    IntegerValue::Signed(1),
                    IntegerValue::Signed(9),
                )
                .unwrap(),
            ))],
        },
    ] {
        let expected = matches!(
            element_shape,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
        );
        let declarations = vec![
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: root,
                identity: terminal_psi::TerminalIdentityIndex(6),
                shape: StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![field(StructuralFieldType::Structural(array))],
                },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: array,
                identity: terminal_psi::TerminalIdentityIndex(15),
                shape: StructuralTypeShape::FixedArray { element, length: 0 },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: element,
                identity: terminal_psi::TerminalIdentityIndex(16),
                shape: element_shape.clone(),
            },
        ];
        assert_eq!(
            zero_valid_record_storage(&declarations, root, &mut Vec::new(), zero_valid_field),
            expected,
            "element {element_shape:?}"
        );
    }
    // A cyclic element chain rejects even beneath an empty dimension.
    let cyclic = vec![
        StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: root,
            identity: terminal_psi::TerminalIdentityIndex(6),
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![field(StructuralFieldType::Structural(array))],
            },
        },
        StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: array,
            identity: terminal_psi::TerminalIdentityIndex(15),
            shape: StructuralTypeShape::FixedArray {
                element: array,
                length: 0,
            },
        },
    ];
    assert!(!zero_valid_record_storage(
        &cyclic,
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
    // A mixed shape composes common fields with the zero-tag case payload.
    let mixed_declaration =
        |common: Vec<StructuralFieldDeclaration>, case_fields| StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: mixed,
            identity: terminal_psi::TerminalIdentityIndex(18),
            shape: StructuralTypeShape::Mixed {
                fields: common,
                cases: vec![
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: terminal_psi::TerminalIdentityIndex(19),
                        fields: case_fields,
                    },
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).unwrap(),
                        identity: terminal_psi::TerminalIdentityIndex(20),
                        fields: vec![field(StructuralFieldType::Erased {
                            type_identity: terminal_psi::TerminalIdentityIndex(13),
                        })],
                    },
                ],
            },
        };
    let root_mixed = |mixed_declaration: StructuralTypeDeclaration| {
        vec![
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: root,
                identity: terminal_psi::TerminalIdentityIndex(6),
                shape: StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![field(StructuralFieldType::Structural(mixed))],
                },
            },
            mixed_declaration,
        ]
    };
    assert!(zero_valid_record_storage(
        &root_mixed(mixed_declaration(
            vec![field(StructuralFieldType::Scalar(ScalarType::Boolean))],
            vec![]
        )),
        root,
        &mut Vec::new(),
        zero_valid_field
    ));
    for (common, case_fields) in [
        (
            vec![field(StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    IntegerValue::Signed(1),
                    IntegerValue::Signed(9),
                )
                .unwrap(),
            ))],
            vec![],
        ),
        (
            vec![StructuralFieldDeclaration {
                storage: None,
                relevance: BindingRelevance::Erased,
                ..field(StructuralFieldType::Scalar(ScalarType::Boolean))
            }],
            vec![],
        ),
        (
            vec![field(StructuralFieldType::Scalar(ScalarType::Boolean))],
            vec![field(StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    IntegerValue::Signed(1),
                    IntegerValue::Signed(9),
                )
                .unwrap(),
            ))],
        ),
    ] {
        let label = format!("common {common:?} case {case_fields:?}");
        assert!(
            !zero_valid_record_storage(
                &root_mixed(mixed_declaration(common, case_fields)),
                root,
                &mut Vec::new(),
                zero_valid_field
            ),
            "{label}"
        );
    }
}

/// One hosted policy's receiver arrival row: the register the bridge passes
/// the receiver through, the foreign registers whose placements must never
/// satisfy it, and the neighboring register a corrupted substitution uses.
struct ReceiverPlacementCase {
    policy: calling_conventions::CallingPolicy,
    register: MachineRegister,
    foreign_registers: &'static [MachineRegister],
    corrupted_register: MachineRegister,
}

#[test]
fn receiver_placement_accepts_only_each_policys_canonical_borrowed_pointer_placement() {
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    let cases = [
        ReceiverPlacementCase {
            policy: CallingPolicy::Aapcs64,
            register: MachineRegister::Aarch64X(0),
            foreign_registers: &[MachineRegister::X86Rdi],
            corrupted_register: MachineRegister::Aarch64X(1),
        },
        ReceiverPlacementCase {
            policy: CallingPolicy::MicrosoftX64,
            register: MachineRegister::X86Rcx,
            foreign_registers: &[MachineRegister::X86Rdi, MachineRegister::Aarch64X(0)],
            corrupted_register: MachineRegister::X86Rdx,
        },
        ReceiverPlacementCase {
            policy: CallingPolicy::SystemVAMD64,
            register: MachineRegister::X86Rdi,
            foreign_registers: &[MachineRegister::Aarch64X(0)],
            corrupted_register: MachineRegister::X86Rsi,
        },
    ];
    for case in cases {
        for shape in [
            ValueShape::borrowed_reference(0, 1),
            ValueShape::borrowed_reference(4, 4),
            ValueShape::borrowed_reference(32, 16),
        ] {
            let signature = CallSignature {
                parameters: vec![shape],
                result: None,
            };
            let plan = evaluate_call_plan(case.policy, &signature)
                .expect("canonical receiver pointer ABI");
            let placement = &plan.parameters[0];
            // Each policy passes the first integer argument through an
            // indirect location retaining the referent geometry; a borrowed
            // reference carries no caller copy.
            assert!(receiver_pointer_matches(shape, placement, case.register));
            // A foreign policy's placement register must never satisfy this
            // bridge's custody, and vice versa.
            for foreign in case.foreign_registers {
                assert!(!receiver_pointer_matches(shape, placement, *foreign));
            }
            assert_corrupted_receiver_rejected(
                shape,
                placement,
                case.register,
                case.corrupted_register,
            );
            // A non-canonical referent geometry or a non-reference class
            // never matches the canonical placement.
            assert!(!receiver_pointer_matches(
                ValueShape::borrowed_reference(shape.byte_size + 1, shape.alignment),
                placement,
                case.register
            ));
            assert!(!receiver_pointer_matches(
                ValueShape::integer(shape.byte_size, shape.alignment),
                placement,
                case.register
            ));
        }
    }
}

/// One hosted target's exact contract row, as the plan crate declares it.
/// The tests read these constants independently of the production row, so a
/// misplaced slot, package, digest, identity, or boundary plan on either side
/// fails the other.
struct HostedContractCase {
    profile: target::TargetProfile,
    package: target::ProgramEntryPhysicalContractPackage,
    requirement_identity: &'static str,
    accepted_spelling: &'static str,
    package_source_digest:
        fn() -> program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
    parameter_type_identities: &'static [&'static str],
    result_type_identity: &'static str,
    boundary_entry_plan: fn() -> calling_conventions::ValidatedBoundaryEntryPlan,
    /// The case whose plan is presented on this target as a foreign contract
    /// substitution, not an alias.
    substitution: Option<usize>,
}

fn hosted_contract_cases() -> [HostedContractCase; 4] {
    use program_entry_plan as plan;
    [
        HostedContractCase {
            profile: target::TargetProfile::MacosArm64,
            package: target::ProgramEntryPhysicalContractPackage::MacosArm64,
            requirement_identity: plan::MACOS_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
            accepted_spelling: "accepted-package::MacosArm64PhysicalEntry::enter",
            package_source_digest: plan::exact_macos_arm64_physical_contract_package_source_digest,
            parameter_type_identities: &[
                plan::MACOS_ARM64_I32_TYPE_IDENTITY,
                plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            ],
            result_type_identity: plan::MACOS_ARM64_I32_TYPE_IDENTITY,
            boundary_entry_plan: plan::exact_macos_arm64_physical_boundary_entry_plan,
            substitution: None,
        },
        HostedContractCase {
            profile: target::TargetProfile::LinuxX64,
            package: target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
            requirement_identity: plan::LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
            accepted_spelling: "accepted-package::LinuxX86_64PhysicalEntry::enter",
            package_source_digest: plan::exact_linux_x86_64_physical_contract_package_source_digest,
            parameter_type_identities: &[plan::LINUX_X86_64_ADDRESS_TYPE_IDENTITY],
            result_type_identity: plan::LINUX_X86_64_I32_TYPE_IDENTITY,
            boundary_entry_plan: plan::exact_linux_x86_64_physical_boundary_entry_plan,
            substitution: Some(0),
        },
        HostedContractCase {
            profile: target::TargetProfile::LinuxArm64,
            package: target::ProgramEntryPhysicalContractPackage::LinuxArm64,
            requirement_identity: plan::LINUX_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
            accepted_spelling: "accepted-package::LinuxArm64PhysicalEntry::enter",
            package_source_digest: plan::exact_linux_arm64_physical_contract_package_source_digest,
            parameter_type_identities: &[plan::LINUX_ARM64_U64_TYPE_IDENTITY],
            result_type_identity: plan::LINUX_ARM64_I32_TYPE_IDENTITY,
            boundary_entry_plan: plan::exact_linux_arm64_physical_boundary_entry_plan,
            substitution: Some(1),
        },
        HostedContractCase {
            profile: target::TargetProfile::WindowsX64,
            package: target::ProgramEntryPhysicalContractPackage::WindowsX64,
            requirement_identity: plan::WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
            accepted_spelling: "accepted-package::WindowsProcessEntry::enter",
            package_source_digest:
                plan::exact_windows_x86_64_physical_contract_package_source_digest,
            parameter_type_identities: &[],
            result_type_identity: plan::WINDOWS_X86_64_U32_TYPE_IDENTITY,
            boundary_entry_plan: plan::exact_windows_x86_64_physical_boundary_entry_plan,
            substitution: Some(1),
        },
    ]
}

fn contract_plan(
    case: &HostedContractCase,
    requirement: &str,
    source: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
) -> ProgramEntryPhysicalContractPlan {
    let boundary = (case.boundary_entry_plan)();
    ProgramEntryPhysicalContractPlan::new(
        case.profile.program_entry_slot(),
        requirement.into(),
        case.package,
        source,
        0,
        case.parameter_type_identities
            .iter()
            .map(|identity| (*identity).to_owned())
            .collect(),
        case.result_type_identity.into(),
        boundary.contract_report_fingerprint(),
        boundary.plan().clone(),
    )
    .expect("well-shaped physical plan")
}

fn contract_case(profile: target::TargetProfile) -> HostedContractCase {
    hosted_contract_cases()
        .into_iter()
        .find(|case| case.profile == profile)
        .expect("hosted contract case")
}

/// The canonical plan of one hosted target's row, for callers that need the
/// exact contract before any requirement spelling is under test.
fn canonical_contract_plan(profile: target::TargetProfile) -> ProgramEntryPhysicalContractPlan {
    let case = contract_case(profile);
    contract_plan(
        &case,
        case.requirement_identity,
        (case.package_source_digest)(),
    )
}

#[test]
fn hosted_physical_replay_keeps_source_bytes_for_package_qualified_requirements() {
    let cases = hosted_contract_cases();
    for (index, case) in cases.iter().enumerate() {
        let source = (case.package_source_digest)();
        for requirement in [case.requirement_identity, case.accepted_spelling] {
            let plan = contract_plan(case, requirement, source);
            assert!(physical_contract_matches(
                &plan,
                case.profile.native_target()
            ));
            // The exact contract must not satisfy a different bridge target.
            for foreign in &cases {
                if foreign.profile != case.profile {
                    assert!(!physical_contract_matches(
                        &plan,
                        foreign.profile.native_target()
                    ));
                }
            }
        }
        let changed_source =
            program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                case.package,
                b"different target implementation",
            );
        assert!(!physical_contract_matches(
            &contract_plan(case, case.accepted_spelling, changed_source),
            case.profile.native_target()
        ));
        // A foreign contract presented on this target is a contract
        // substitution, not an alias.
        if let Some(foreign_index) = case.substitution {
            let foreign = &cases[foreign_index];
            let plan = contract_plan(
                foreign,
                foreign.requirement_identity,
                (foreign.package_source_digest)(),
            );
            assert!(
                !physical_contract_matches(&plan, case.profile.native_target()),
                "case {index} accepted a foreign contract substitution"
            );
        }
    }
}
fn hosted_receiver_binding(
    physical: ProgramEntryPhysicalContractPlan,
    ceiling_bytes: u64,
    receiver_byte_count: u64,
    receiver_alignment: u64,
) -> super::HostedReceiverBinding {
    let target = physical.target_slot().owner.native_target();
    let source = program_entry_plan::SelectedProgramEntrySignature::from_checked_declaration(
        physical.target_slot(),
        "package:test::Main::main".to_owned(),
        "Main".into(),
        "main".into(),
        terminal_psi::TerminalIdentityIndex(1),
        program_entry_plan::ProgramEntryReceiverSignature::ProvisionedMutable {
            source_position: 0,
            normalized_type_identity: terminal_psi::TerminalIdentityIndex(2),
        },
        Vec::new(),
        terminal_psi::TerminalIdentityTable::from(vec![
            String::new(),
            "Main::main".to_owned(),
            "named(name(Main))".to_owned(),
        ]),
    )
    .expect("well-shaped source signature");
    let checked_entry = terminal_psi::CheckedProgramEntryTerminalReceipt::new(
        source.identity().bytes(),
        source.machine_name().into(),
        terminal_psi::NominalDeclarationIdentity::new(terminal_psi::TerminalIdentityIndex(4)),
        terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([7; 32]),
        },
        semantic_vocabulary::MachineId::new(1).unwrap(),
        None,
    );
    super::HostedReceiverBinding {
        source,
        checked_entry,
        physical,
        services: Vec::new(),
        private_entry_ram: Vec::new(),
        determined_evidence: Vec::new(),
        demand: crate::StackDemand {
            psi: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([7; 32]),
            },
            target,
            entry: semantic_vocabulary::MachineId::new(1).unwrap(),
            ceiling_bytes,
            stack_alignment: 16,
            contributing_machines: Default::default(),
            admitted_contribution_report_identities: Default::default(),
            admitted_contribution_commitments: Default::default(),
            private_demands: Vec::new(),
        },
        receiver_byte_count,
        receiver_alignment,
        cleanup_occupancy: false,
    }
}

#[test]
fn hosted_receiver_partitions_reserve_disjoint_aligned_residences() {
    let linux = |ceiling_bytes: u64, receiver_byte_count: u64, receiver_alignment: u64| {
        hosted_receiver_binding(
            canonical_contract_plan(target::TargetProfile::LinuxX64),
            ceiling_bytes,
            receiver_byte_count,
            receiver_alignment,
        )
    };
    for (existing, ceiling, byte_count, alignment) in [
        (0u64, 0, 0, 1),
        (3, 5, 17, 8),
        (48, 33, 64, 64),
        (1024, 4096, 4, 256),
    ] {
        let partitions = linux(ceiling, byte_count, alignment)
            .partitions(existing)
            .unwrap_or_else(|error| {
                panic!("{existing}/{ceiling}/{byte_count}/{alignment}: {error:?}")
            });
        let saved_end = partitions.saved_continuation_offset + 16;
        let stack_end = partitions.stack_offset + partitions.stack_byte_count;
        assert!(
            partitions.saved_continuation_offset >= existing
                && partitions.saved_continuation_offset % 16 == 0
                && saved_end <= partitions.stack_offset
                && partitions.stack_byte_count >= ceiling.max(16)
                && partitions.stack_byte_count % 16 == 0
                && stack_end <= partitions.receiver_offset
                && partitions.receiver_offset % alignment.max(16) == 0
                && partitions.receiver_byte_count == byte_count.max(1)
                && partitions.receiver_offset + partitions.receiver_byte_count
                    == partitions.end_offset,
            "{existing}/{ceiling}/{byte_count}/{alignment}: {partitions:?}"
        );
    }
    // The Windows caller owes shadow space plus the pushed return address on
    // top of the checked demand; the private stack partition carries it.
    let windows = hosted_receiver_binding(
        canonical_contract_plan(target::TargetProfile::WindowsX64),
        0,
        8,
        8,
    )
    .partitions(0)
    .unwrap();
    assert_eq!(windows.stack_byte_count, 64);
    // Occupancy that cannot be represented rejects instead of overflowing.
    assert!(linux(u64::MAX, 8, 8).partitions(0).is_err());
    assert!(linux(8, 8, 8).partitions(u64::MAX - 4).is_err());
    // A non-power-of-two receiver alignment has no valid residence.
    assert!(linux(8, 8, 24).partitions(0).is_err());
}
