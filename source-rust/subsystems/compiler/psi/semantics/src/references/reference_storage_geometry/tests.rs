use super::*;
use semantic_vocabulary::{
    AuthoredRecordStorage, IntegerSign, IntegerType, StructuralFieldId,
};
use terminal_psi::{BindingRelevance, StructuralFieldType, TerminalIdentityIndex};

fn identity(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).expect("type identity")
}
fn scalar(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).expect("signed scalar"))
}
fn field(
    value: u64,
    field_type: StructuralFieldType,
    storage: Option<AuthoredFieldStorage>,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: StructuralFieldId::new(value).expect("field identity"),
        identity: TerminalIdentityIndex(4),
        relevance: BindingRelevance::Relevant,
        field_type,
        storage,
    }
}

#[test]
fn primitive_placement_uses_target_addresses_without_granting_byte_codecs() {
    let address = ScalarType::Integer(IntegerType::address(64).unwrap());
    let narrow = scalar(13);
    let types = [address, narrow, scalar(32)]
        .into_iter()
        .enumerate()
        .map(|(position, scalar)| StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: identity(position as u64 + 1),
            identity: TerminalIdentityIndex(position as u32 + 1),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
        })
        .collect::<Vec<_>>();
    let target = |size, alignment| TargetCarrierLayout {
        pointer_byte_size: size,
        pointer_byte_alignment: alignment,
        descriptor_byte_size: 16,
        descriptor_byte_alignment: 8,
        case_tag_byte_size: 4,
        case_tag_byte_alignment: 4,
    };
    let layout = |root, carriers| {
        ReferenceStorageGeometry::new(types.iter(), |application| {
            Err(StorageGeometryError::UnresolvedExtent(application))
        })
        .with_target_carriers(carriers)
        .layout(root)
    };
    // The displayed 64-bit carrier supplies no target placement.
    assert_eq!(
        layout(identity(1), None),
        Err(StorageGeometryError::MissingCarrierLayout)
    );
    assert_eq!(layout(identity(1), Some(target(4, 2))), Ok((4, 2)));
    assert_eq!(layout(identity(1), Some(target(16, 8))), Ok((16, 8)));
    assert_eq!(
        layout(identity(1), Some(target(0, 8))),
        Err(StorageGeometryError::InvalidCarrierLayout)
    );
    assert_eq!(
        layout(identity(1), Some(target(8, 3))),
        Err(StorageGeometryError::InvalidCarrierLayout)
    );
    assert_eq!(layout(identity(2), None), Ok((2, 2)));
    assert_eq!(layout(identity(3), None), Ok((4, 4)));
    assert_eq!(crate::reference_storage_byte_width(address), None);
    assert_eq!(crate::reference_storage_byte_width(narrow), None);
    assert!(!crate::reference_storage_view_is_compatible(
        address, address
    ));
}

#[test]
fn physical_carriers_preserve_source_defaults_and_lookup_errors() {
    let declaration = |raw, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        id: identity(raw),
        identity: TerminalIdentityIndex(raw as u32),
        shape,
        requires: Vec::new(),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        scalar_restriction: None,
    };
    let bytes = terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 13 };
    let types = [
        declaration(1, StructuralTypeShape::PrimitiveScalar(scalar(32))),
        declaration(
            2,
            StructuralTypeShape::Reference {
                referent: identity(1),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                fields: vec![
                    field(1, StructuralFieldType::ByteSequence(bytes), None),
                    StructuralFieldDeclaration {
                        identity: TerminalIdentityIndex(5),
                        ..field(2, StructuralFieldType::Scalar(scalar(8)), None)
                    },
                ],
                storage: StructuralRecordStorage::Packed,
            },
        ),
    ];
    let carriers = TargetCarrierLayout {
        pointer_byte_size: 8,
        pointer_byte_alignment: 8,
        descriptor_byte_size: 16,
        descriptor_byte_alignment: 8,
        case_tag_byte_size: 4,
        case_tag_byte_alignment: 4,
    };
    fn unresolved(application: ScalarApplicationId) -> Result<u64, StorageGeometryError> {
        Err(StorageGeometryError::UnresolvedExtent(application))
    }
    let mut source = ReferenceStorageGeometry::new(types.iter(), unresolved)
        .with_target_carriers(Some(carriers));
    assert_eq!(source.layout(identity(2)), Ok((8, 8)));
    assert!(matches!(
        source.layout(identity(3)),
        Err(StorageGeometryError::UnsupportedByteSequence(_))
    ));

    struct Physical {
        unavailable: bool,
    }
    impl RecordStorageResolver for Physical {
        fn carrier_layout(
            &self,
            owner: StructuralTypeId,
            carrier: &StructuralFieldType,
        ) -> Result<Option<(u64, u64)>, StorageGeometryError> {
            if self.unavailable {
                return Err(StorageGeometryError::UnresolvedLayout(owner));
            }
            match carrier {
                StructuralFieldType::Structural(id)
                    if *id == identity(2) && owner == *id =>
                {
                    Ok(Some((16, 8)))
                }
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 13 },
                ) if owner == identity(3) => Ok(Some((21, 8))),
                _ => Err(StorageGeometryError::InvalidRecordStorage(owner)),
            }
        }
        fn realization(
            &self,
            record: StructuralTypeId,
            _application: LayoutApplicationId,
        ) -> Result<&TargetLayoutRealization, StorageGeometryError> {
            Err(StorageGeometryError::UnresolvedLayout(record))
        }
    }
    let mut physical = ReferenceStorageGeometry::new(types.iter(), unresolved)
        .with_target_carriers(Some(carriers))
        .with_record_storage(Physical { unavailable: false });
    assert_eq!(physical.layout(identity(2)), Ok((16, 8)));
    assert_eq!(physical.layout(identity(3)), Ok((24, 8)));
    let region = physical
        .region(
            identity(3),
            &[StructuralPathSegment::Field(TerminalIdentityIndex(5))],
        )
        .unwrap();
    assert_eq!(
        region.byte_offset, 21,
        "the inline carrier receives its real owner and keeps its unrounded size"
    );
    assert_eq!(region.byte_length, 1);
    let mut unavailable = ReferenceStorageGeometry::new(types.iter(), unresolved)
        .with_target_carriers(Some(carriers))
        .with_record_storage(Physical { unavailable: true });
    assert_eq!(
        unavailable.layout(identity(2)),
        Err(StorageGeometryError::UnresolvedLayout(identity(2)))
    );
}

#[test]
fn schema_geometry_source_keeps_arrays_and_selected_realizations_exact() {
    use terminal_psi::{LayoutApplicationId, TargetLayoutFieldLayoutSource as Source};
    let application = LayoutApplicationId(1);
    let declaration = |value, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        scalar_restriction: None,
        id: identity(value),
        identity: TerminalIdentityIndex(value as u32),
        shape,
    };
    let mut module = terminal_psi::TerminalModule::declarations();
    module.types.structural_types = vec![
        declaration(1, StructuralTypeShape::PrimitiveScalar(scalar(32))),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: identity(1),
                length: terminal_psi::ArrayExtent::Literal(32),
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                fields: Vec::new(),
                storage: StructuralRecordStorage::Packed,
            },
        ),
        declaration(
            4,
            StructuralTypeShape::Record {
                fields: Vec::new(),
                storage: StructuralRecordStorage::TargetLayout(application),
            },
        ),
        declaration(
            5,
            StructuralTypeShape::FixedArray {
                element: identity(4),
                length: terminal_psi::ArrayExtent::Literal(64),
            },
        ),
        declaration(
            6,
            StructuralTypeShape::FixedArray {
                element: identity(6),
                length: terminal_psi::ArrayExtent::Literal(1),
            },
        ),
    ];
    let source = |value| {
        schema_field_layout_source(
            &module,
            &StructuralFieldType::Structural(identity(value)),
            &UnselectedRecordStorage,
        )
    };
    assert_eq!(
        reference_field_representation_width(
            module.types.structural_types.iter(),
            &StructuralFieldType::Scalar(ScalarType::Boolean)
        ),
        Some(1),
    );
    assert_eq!(
        reference_field_representation_width(
            module.types.structural_types.iter(),
            &StructuralFieldType::Structural(identity(1))
        ),
        Some(32),
    );
    assert_eq!(
        reference_field_representation_width(
            module.types.structural_types.iter(),
            &StructuralFieldType::Structural(identity(2))
        ),
        None,
    );
    let bounded = semantic_vocabulary::BoundedIntegerType::new(
        semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            32,
        )
        .expect("u32"),
        semantic_vocabulary::IntegerValue::Unsigned(0),
        semantic_vocabulary::IntegerValue::Unsigned(7),
    )
    .expect("checked bounded integer");
    assert_eq!(
        reference_field_representation_width(
            module.types.structural_types.iter(),
            &StructuralFieldType::BoundedInteger(bounded)
        ),
        Some(3),
    );
    assert_eq!(source(2), Ok(Source::Scalar));
    assert_eq!(source(3), Ok(Source::Default));
    assert_eq!(
        source(4),
        Err(StorageGeometryError::UnresolvedLayout(identity(4)))
    );
    assert_eq!(
        source(6),
        Err(StorageGeometryError::RecursiveType(identity(6)))
    );
    assert_eq!(
        source(99),
        Err(StorageGeometryError::MissingType(identity(99)))
    );

    struct Selected {
        application: LayoutApplicationId,
        realization: TargetLayoutRealization,
    }
    impl RecordStorageResolver for Selected {
        fn realization(
            &self,
            record: StructuralTypeId,
            application: LayoutApplicationId,
        ) -> Result<&TargetLayoutRealization, StorageGeometryError> {
            if record != identity(4) || application != self.application {
                return Err(StorageGeometryError::UnresolvedLayout(record));
            }
            Ok(&self.realization)
        }
    }
    let mut selected = Selected {
        application,
        realization: TargetLayoutRealization {
            schema_argument: terminal_psi::TargetLayoutSchemaArgument {
                abi_version: 4,
                fields: Vec::new(),
                retired_field_identities: Vec::new(),
            },
            target: "actual-target".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
            storage: AuthoredRecordStorage {
                byte_size: 0,
                byte_alignment: 1,
            },
            fields: Vec::new(),
        },
    };
    let field = StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: StructuralFieldId::new(1).expect("field"),
        identity: TerminalIdentityIndex(1),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(identity(1)),
        storage: None,
    };
    let direct = AuthoredFieldStorage::Direct { byte_offset: 0 };
    let extent = |storage: &AuthoredFieldStorage, size| {
        authored_field_storage_extent(
            module.types.structural_types.iter(),
            &field,
            storage,
            size,
            |child| (child == identity(1)).then_some((4, 4)),
        )
    };
    assert_eq!(extent(&direct, 4), Ok(Some((0, 4))));
    assert_eq!(
        extent(&direct, 3),
        Err("authored field extent exceeds its record storage")
    );
    let recipe = |width| {
        AuthoredFieldStorage::Fragmented(semantic_vocabulary::AuthoredBitFragments::new([
            semantic_vocabulary::AuthoredBitFragment {
                container_byte_offset: 0,
                container_width_bits: 32,
                destination_lsb: 0,
                source_lsb: 0,
                width,
            },
        ]))
    };
    assert_eq!(extent(&recipe(32), 4), Ok(None));
    assert_eq!(
        extent(&recipe(31), 4),
        Err("fragment source ranges do not tile the declaration representation")
    );
    assert_eq!(
        validate_storage_destination_intervals(vec![(0, 8), (8, 16)]),
        Ok(())
    );
    assert_eq!(
        validate_storage_destination_intervals(vec![(0, 8), (7, 16)]),
        Err("authored field destination bits overlap")
    );
    let array = StructuralFieldType::Structural(identity(5));
    assert_eq!(
        schema_field_layout_source(&module, &array, &selected),
        Ok(Source::Explicit)
    );
    selected.application = LayoutApplicationId(2);
    assert_eq!(
        schema_field_layout_source(&module, &array, &selected),
        Err(StorageGeometryError::UnresolvedLayout(identity(4)))
    );
}

#[test]
fn sum_image_offsets_and_array_extent_share_the_selected_tag_geometry() {
    let declaration = |value, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        scalar_restriction: None,
        id: identity(value),
        identity: TerminalIdentityIndex(value as u32),
        shape,
    };
    let types = [
        declaration(
            1,
            StructuralTypeShape::Mixed {
                fields: vec![field(1, StructuralFieldType::Scalar(scalar(16)), None)],
                cases: vec![
                    terminal_psi::StructuralCaseDeclaration {
                        declaration_identity: Default::default(),
                        id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                        identity: TerminalIdentityIndex(10),
                        fields: vec![],
                    },
                    terminal_psi::StructuralCaseDeclaration {
                        declaration_identity: Default::default(),
                        id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                        identity: TerminalIdentityIndex(11),
                        fields: vec![
                            field(1, StructuralFieldType::Scalar(scalar(8)), None),
                            StructuralFieldDeclaration {
                                identity: TerminalIdentityIndex(5),
                                ..field(2, StructuralFieldType::Scalar(scalar(64)), None)
                            },
                        ],
                    },
                ],
            },
        ),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: identity(1),
                length: terminal_psi::ArrayExtent::Literal(3),
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![
                    field(1, StructuralFieldType::Scalar(scalar(32)), None),
                    StructuralFieldDeclaration {
                        identity: TerminalIdentityIndex(5),
                        ..field(2, StructuralFieldType::Structural(identity(1)), None)
                    },
                ],
            },
        ),
        declaration(
            4,
            StructuralTypeShape::Sum {
                cases: vec![terminal_psi::StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                    identity: TerminalIdentityIndex(11),
                    fields: vec![StructuralFieldDeclaration {
                        identity: TerminalIdentityIndex(5),
                        ..field(
                            2,
                            StructuralFieldType::Scalar(scalar(64)),
                            Some(AuthoredFieldStorage::Scalar {
                                byte_offset: 0,
                                stored_scalar: Some(scalar(32)),
                            }),
                        )
                    }],
                }],
            },
        ),
    ];
    let unresolved = |application| Err(StorageGeometryError::UnresolvedExtent(application));
    assert_eq!(
        ReferenceStorageGeometry::new(types.iter(), unresolved).sum_layout(identity(1)),
        Err(StorageGeometryError::MissingCarrierLayout)
    );
    for (tag_size, common, payload, extent) in [(4, 4, 8, 24), (8, 8, 16, 32)] {
        let mut geometry = ReferenceStorageGeometry::new(types.iter(), unresolved)
            .with_target_carriers(Some(TargetCarrierLayout {
                pointer_byte_size: 8,
                pointer_byte_alignment: 8,
                descriptor_byte_size: 16,
                descriptor_byte_alignment: 8,
                case_tag_byte_size: tag_size,
                case_tag_byte_alignment: tag_size,
            }));
        let sum = geometry.sum_layout(identity(1)).unwrap();
        assert_eq!(sum.tag_byte_offset, 0);
        assert_eq!(sum.tag.byte_size, tag_size);
        assert_eq!(sum.tag.byte_alignment, tag_size);
        assert_eq!(sum.common_field_offsets, vec![common]);
        assert_eq!(
            sum.case_field_offsets,
            vec![vec![], vec![payload, payload + 8]]
        );
        assert_eq!(geometry.layout(identity(1)), Ok((extent, 8)));
        assert_eq!(sum.storage.byte_size, extent);
        assert_eq!(geometry.layout(identity(2)), Ok((3 * extent, 8)));
        let common_region = geometry
            .region(
                identity(1),
                &[StructuralPathSegment::Field(TerminalIdentityIndex(4))],
            )
            .unwrap();
        assert_eq!(
            (common_region.byte_offset, common_region.byte_length),
            (common, 2)
        );
        let selected = [
            StructuralPathSegment::Case(
                semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
            ),
            StructuralPathSegment::Field(TerminalIdentityIndex(5)),
        ];
        let region = geometry.region(identity(1), &selected).unwrap();
        assert_eq!((region.byte_offset, region.byte_length), (payload + 8, 8));
        assert_eq!(region.scalar.unwrap().logical_scalar, scalar(64));
        let narrow = geometry.region(identity(4), &selected).unwrap();
        assert_eq!((narrow.byte_offset, narrow.byte_length), (tag_size, 4));
        assert_eq!(narrow.scalar.unwrap().logical_scalar, scalar(64));
        assert_eq!(narrow.scalar.unwrap().stored_scalar, scalar(32));
        let mut nested = vec![StructuralPathSegment::Field(TerminalIdentityIndex(5))];
        nested.extend_from_slice(&selected);
        let region = geometry.region(identity(3), &nested).unwrap();
        assert_eq!(
            (region.byte_offset, region.byte_length),
            (8 + payload + 8, 8)
        );
        let mut indexed = vec![StructuralPathSegment::FixedIndex(2)];
        indexed.extend_from_slice(&selected);
        let region = geometry.region(identity(2), &indexed).unwrap();
        assert_eq!(
            (region.byte_offset, region.byte_length),
            (2 * extent + payload + 8, 8)
        );
        assert!(geometry.region(identity(1), &selected[..1]).is_none());
        assert!(
            geometry
                .region(
                    identity(1),
                    &[
                        StructuralPathSegment::Case(
                            semantic_vocabulary::StructuralCaseId::new(1).unwrap()
                        ),
                        StructuralPathSegment::Field(TerminalIdentityIndex(5)),
                    ]
                )
                .is_none()
        );
    }
}

#[test]
fn extent_discovery_visits_unknown_siblings_but_respects_storage_and_erasure() {
    let application = |value| ScalarApplicationId::new(value).unwrap();
    let declaration = |value, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        scalar_restriction: None,
        id: identity(value),
        identity: TerminalIdentityIndex(value as u32),
        shape,
    };
    let mut types = [
        declaration(1, StructuralTypeShape::PrimitiveScalar(scalar(64))),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: identity(1),
                length: terminal_psi::ArrayExtent::Application(application(1)),
            },
        ),
        declaration(
            3,
            StructuralTypeShape::FixedArray {
                element: identity(1),
                length: terminal_psi::ArrayExtent::Application(application(2)),
            },
        ),
        declaration(
            4,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![
                    field(1, StructuralFieldType::Structural(identity(2)), None),
                    field(2, StructuralFieldType::Structural(identity(3)), None),
                ],
            },
        ),
    ];
    assert_eq!(
        reference_storage_dependencies(types.iter(), identity(4)),
        Ok(vec![application(1), application(2)])
    );
    assert_eq!(
        reference_storage_layout(types.iter(), identity(4)),
        None,
        "discovery cannot publish unknown geometry"
    );
    assert_eq!(
        ReferenceStorageGeometry::new(types.iter(), |dependency| Ok(dependency.get()))
            .layout(identity(4)),
        Ok((24, 8))
    );
    let StructuralTypeShape::Record { fields, .. } = &mut types[3].shape else {
        panic!("record");
    };
    fields[0].relevance = BindingRelevance::Erased;
    assert_eq!(
        reference_storage_dependencies(types.iter(), identity(4)),
        Ok(vec![application(2)])
    );
    let StructuralTypeShape::Record { storage, .. } = &mut types[3].shape else {
        panic!("record");
    };
    *storage = StructuralRecordStorage::Authored(AuthoredRecordStorage {
        byte_size: 3,
        byte_alignment: 1,
    });
    assert_eq!(
        reference_storage_dependencies(types.iter(), identity(4)),
        Ok(Vec::new())
    );
    assert_eq!(
        reference_storage_layout(types.iter(), identity(4)),
        Some((3, 1))
    );

    types[1].shape = StructuralTypeShape::FixedArray {
        element: identity(3),
        length: terminal_psi::ArrayExtent::Literal(0),
    };
    assert_eq!(
        reference_storage_dependencies(types.iter(), identity(2)),
        Ok(vec![application(2)])
    );
    types[2].shape = StructuralTypeShape::FixedArray {
        element: identity(2),
        length: terminal_psi::ArrayExtent::Literal(0),
    };
    assert_eq!(
        reference_storage_dependencies(types.iter(), identity(2)),
        Err(StorageGeometryError::RecursiveType(identity(2)))
    );
    types[2].shape = StructuralTypeShape::FixedArray {
        element: identity(99),
        length: terminal_psi::ArrayExtent::Literal(0),
    };
    assert_eq!(
        reference_storage_dependencies(types.iter(), identity(2)),
        Err(StorageGeometryError::MissingType(identity(99)))
    );
}

#[test]
fn resolved_array_extent_uses_the_same_geometry_and_reports_missing_or_overflow() {
    let dependency = ScalarApplicationId::new(1).unwrap();
    let mut types = [
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: identity(1),
            identity: TerminalIdentityIndex(1),
            shape: StructuralTypeShape::PrimitiveScalar(scalar(64)),
        },
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: identity(2),
            identity: TerminalIdentityIndex(2),
            shape: StructuralTypeShape::FixedArray {
                element: identity(1),
                length: terminal_psi::ArrayExtent::Application(dependency),
            },
        },
    ];
    assert_eq!(reference_storage_layout(types.iter(), identity(2)), None);
    assert_eq!(
        ReferenceStorageGeometry::new(types.iter(), |application| Err(
            StorageGeometryError::UnresolvedExtent(application)
        ))
        .layout(identity(2)),
        Err(StorageGeometryError::UnresolvedExtent(dependency))
    );
    assert_eq!(
        ReferenceStorageGeometry::new(types.iter(), |_| Ok(3)).layout(identity(2)),
        Ok((24, 8))
    );
    assert_eq!(
        ReferenceStorageGeometry::new(types.iter(), |_| Ok(0)).layout(identity(2)),
        Ok((0, 8))
    );
    assert_eq!(
        ReferenceStorageGeometry::new(types.iter(), |_| Ok(u64::MAX)).layout(identity(2)),
        Err(StorageGeometryError::Overflow)
    );
    types[1].shape = StructuralTypeShape::FixedArray {
        element: identity(2),
        length: terminal_psi::ArrayExtent::Application(dependency),
    };
    assert_eq!(
        ReferenceStorageGeometry::new(types.iter(), |_| Ok(3)).layout(identity(2)),
        Err(StorageGeometryError::RecursiveType(identity(2)))
    );
}

#[test]
fn canonical_parent_keeps_authored_child_width_and_codec() {
    let types = [
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: identity(1),
            identity: TerminalIdentityIndex(3),
            shape: StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                    byte_size: 1,
                    byte_alignment: 1,
                }),
                fields: vec![field(
                    1,
                    StructuralFieldType::Scalar(scalar(64)),
                    Some(AuthoredFieldStorage::Scalar {
                        byte_offset: 0,
                        stored_scalar: Some(scalar(8)),
                    }),
                )],
            },
        },
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: identity(2),
            identity: TerminalIdentityIndex(2),
            shape: StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![field(2, StructuralFieldType::Structural(identity(1)), None)],
            },
        },
    ];
    assert_eq!(reference_storage_size(types.iter(), identity(2)), Some(1));
    assert_eq!(
        reference_storage_projection(
            types.iter(),
            identity(2),
            &[
                StructuralPathSegment::Field(TerminalIdentityIndex(4)),
                StructuralPathSegment::Field(TerminalIdentityIndex(4))
            ]
        ),
        Some(ReferenceStorageProjection {
            byte_offset: 0,
            logical_scalar: scalar(64),
            stored_scalar: scalar(8)
        })
    );
}

#[test]
fn fragmented_fields_do_not_hide_the_record_extent_or_invent_a_scalar_address() {
    let types = [StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        scalar_restriction: None,
        id: identity(1),
        identity: TerminalIdentityIndex(1),
        shape: StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                byte_size: 1,
                byte_alignment: 1,
            }),
            fields: vec![field(
                1,
                StructuralFieldType::Scalar(scalar(8)),
                Some(AuthoredFieldStorage::Fragmented(
                    semantic_vocabulary::AuthoredBitFragments::new([
                        semantic_vocabulary::AuthoredBitFragment {
                            container_byte_offset: 0,
                            container_width_bits: 8,
                            destination_lsb: 4,
                            source_lsb: 0,
                            width: 4,
                        },
                        semantic_vocabulary::AuthoredBitFragment {
                            container_byte_offset: 0,
                            container_width_bits: 8,
                            destination_lsb: 0,
                            source_lsb: 4,
                            width: 4,
                        },
                    ]),
                )),
            )],
        },
    }];
    assert_eq!(reference_storage_size(types.iter(), identity(1)), Some(1));
    assert_eq!(
        reference_storage_projection(
            types.iter(),
            identity(1),
            &[StructuralPathSegment::Field(TerminalIdentityIndex(4))]
        ),
        None
    );
}
