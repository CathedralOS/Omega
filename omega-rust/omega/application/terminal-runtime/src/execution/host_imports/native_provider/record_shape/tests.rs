use super::*;
use crate::execution::host_imports::native_provider::values::ScalarField;
use calling_conventions::{BoundaryValueField, BoundaryValueShape};
use semantic_vocabulary::{IntegerSign, IntegerType, StructuralFieldId};
use terminal_psi::{BindingRelevance, TerminalIdentityIndex};

fn scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
}

fn type_id(value: u32) -> StructuralTypeId {
    StructuralTypeId::new(value.into()).unwrap()
}

fn field_id(value: u32) -> StructuralFieldId {
    StructuralFieldId::new(value.into()).unwrap()
}

fn member(value: u32) -> Path {
    Path::Field(field_id(value))
}

fn field(id: u32, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: field_id(id),
        identity: TerminalIdentityIndex(id),
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    }
}

fn record(id: u32, fields: Vec<StructuralFieldDeclaration>) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        quotient_carrier: None,
        id: type_id(id),
        identity: TerminalIdentityIndex(id),
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape: StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        },
    }
}

struct Fixture {
    signature: BoundarySignatureGeometry,
    codec: StorageCodec,
    declarations: Vec<StructuralTypeDeclaration>,
}

impl Fixture {
    /// The two semantic occurrences share one child type and one public node;
    /// their containing paths and physical slots remain different.
    fn nested() -> Self {
        Self {
            signature: BoundarySignatureGeometry {
                shapes: vec![
                    BoundaryValueShape {
                        class: BoundaryValueClass::Record {
                            first_field: 0,
                            field_count: 2,
                        },
                        byte_size: 16,
                        alignment: 8,
                    },
                    BoundaryValueShape {
                        class: BoundaryValueClass::Record {
                            first_field: 2,
                            field_count: 1,
                        },
                        byte_size: 8,
                        alignment: 8,
                    },
                    BoundaryValueShape {
                        class: BoundaryValueClass::Integer,
                        byte_size: 8,
                        alignment: 8,
                    },
                ],
                fields: vec![
                    BoundaryValueField {
                        shape: 1,
                        byte_offset: 0,
                    },
                    BoundaryValueField {
                        shape: 1,
                        byte_offset: 8,
                    },
                    BoundaryValueField {
                        shape: 2,
                        byte_offset: 0,
                    },
                ],
                parameters: Vec::new(),
                result: Some(0),
                native_parameters: Default::default(),
                native_parameter_rows: Default::default(),
                callback_layouts: Default::default(),
                callback_layout_rows: Default::default(),
            },
            codec: StorageCodec {
                structural_type: type_id(1),
                extent: 16,
                alignment: 8,
                fields: vec![
                    ScalarField {
                        path: vec![member(1), member(1)],
                        offset: 0,
                        logical: scalar(),
                        stored: scalar(),
                    },
                    ScalarField {
                        path: vec![member(2), member(1)],
                        offset: 8,
                        logical: scalar(),
                        stored: scalar(),
                    },
                ],
            },
            declarations: vec![
                record(
                    1,
                    vec![
                        field(1, StructuralFieldType::Structural(type_id(2))),
                        field(2, StructuralFieldType::Structural(type_id(2))),
                    ],
                ),
                record(2, vec![field(1, StructuralFieldType::Scalar(scalar()))]),
            ],
        }
    }

    fn validate(&self) -> Result<(), TerminalEffectRejection> {
        validate(&self.signature, 0, &self.codec, |identity| {
            self.declarations
                .iter()
                .find(|declaration| declaration.id == identity)
        })
    }

    fn root_fields(&mut self) -> &mut Vec<StructuralFieldDeclaration> {
        let StructuralTypeShape::Record { fields, .. } = &mut self.declarations[0].shape else {
            unreachable!()
        };
        fields
    }
}

#[test]
fn repeated_record_child_retains_each_complete_path_and_physical_slot() {
    let mut fixture = Fixture::nested();
    assert!(fixture.validate().is_ok());
    fixture.codec.fields[1].path = fixture.codec.fields[0].path.clone();
    assert!(fixture.validate().is_err());
    fixture.codec.fields[1].path = vec![member(2), member(1)];
    fixture.codec.fields.swap(0, 1);
    assert!(fixture.validate().is_err());
}

#[test]
fn complete_record_roster_includes_children_with_no_scalar_leaves() {
    let mut fixture = Fixture::nested();
    fixture
        .root_fields()
        .push(field(3, StructuralFieldType::Structural(type_id(3))));
    fixture.declarations.push(record(3, Vec::new()));
    // The scalar codec alone cannot expose the missing empty child.
    assert!(fixture.validate().is_err());
    fixture.signature.shapes[0].class = BoundaryValueClass::Record {
        first_field: 0,
        field_count: 3,
    };
    fixture.signature.shapes[1].class = BoundaryValueClass::Record {
        first_field: 3,
        field_count: 1,
    };
    fixture.signature.shapes.push(BoundaryValueShape {
        class: BoundaryValueClass::Record {
            first_field: 4,
            field_count: 0,
        },
        byte_size: 0,
        alignment: 1,
    });
    fixture.signature.fields.insert(
        2,
        BoundaryValueField {
            shape: 3,
            byte_offset: 16,
        },
    );
    assert!(fixture.validate().is_ok());
    fixture.signature.shapes[3].class = BoundaryValueClass::Integer;
    assert!(fixture.validate().is_err());
}

#[test]
fn equal_extent_does_not_allow_changed_class_width_offset_or_child_padding() {
    let mut fixture = Fixture::nested();
    fixture.signature.shapes[2].class = BoundaryValueClass::Float;
    assert!(fixture.validate().is_err());
    fixture.signature.shapes[2].class = BoundaryValueClass::Integer;
    fixture.signature.shapes[2].byte_size = 4;
    assert!(fixture.validate().is_err());
    fixture.signature.shapes[2].byte_size = 8;
    fixture.signature.fields[1].byte_offset = 0;
    assert!(fixture.validate().is_err());
    fixture.signature.fields[1].byte_offset = 1;
    assert!(fixture.validate().is_err());
    fixture.signature.fields[1].byte_offset = 16;
    assert!(fixture.validate().is_err());
    fixture.signature.fields[1].byte_offset = 8;
    fixture.signature.shapes[0].byte_size = 24;
    fixture.codec.extent = 24;
    fixture.signature.shapes[1].byte_size = 16;
    // Scalar leaves themselves do not overlap; the complete child records do.
    assert!(fixture.validate().is_err());
    fixture.signature.shapes[1].byte_size = 8;
    fixture.signature.shapes[1].alignment = 3;
    assert!(fixture.validate().is_err());
}

#[test]
fn missing_extra_or_reordered_scalar_fields_cannot_cover_the_same_record() {
    let mut fixture = Fixture::nested();
    fixture.codec.fields.pop();
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::nested();
    fixture.codec.fields.push(ScalarField {
        path: vec![member(3)],
        offset: 0,
        logical: scalar(),
        stored: scalar(),
    });
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::nested();
    fixture.signature.fields.swap(0, 1);
    assert!(fixture.validate().is_err());
    fixture.signature.shapes[0].class = BoundaryValueClass::Record {
        first_field: 3,
        field_count: 2,
    };
    assert!(fixture.validate().is_err());
}

#[test]
fn erased_fields_contribute_no_native_child_but_cannot_hide_a_relevant_one() {
    let mut fixture = Fixture::nested();
    let mut erased = field(3, StructuralFieldType::Scalar(scalar()));
    erased.relevance = BindingRelevance::Erased;
    fixture.root_fields().push(erased);
    assert!(fixture.validate().is_ok());
    fixture.signature.shapes[0].class = BoundaryValueClass::Record {
        first_field: 0,
        field_count: 3,
    };
    assert!(fixture.validate().is_err());
}

#[test]
fn semantic_and_public_cycles_are_refused_without_recursive_execution() {
    let mut fixture = Fixture::nested();
    fixture.declarations[1] = record(
        2,
        vec![field(1, StructuralFieldType::Structural(type_id(1)))],
    );
    assert!(fixture.validate().is_err());
    fixture.declarations[1] = record(
        2,
        vec![field(1, StructuralFieldType::Structural(type_id(3)))],
    );
    fixture.declarations.push(record(
        3,
        vec![field(1, StructuralFieldType::Scalar(scalar()))],
    ));
    fixture.signature.fields[2].shape = 1;
    assert!(fixture.validate().is_err());
}

#[test]
fn unsupported_descendants_do_not_gain_owned_value_admission() {
    for shape in [
        StructuralTypeShape::Reference {
            referent: type_id(1),
            access: terminal_psi::StructuralAccess::SharedBorrow,
        },
        StructuralTypeShape::Sum { cases: Vec::new() },
        StructuralTypeShape::BindingCarrier,
    ] {
        let mut fixture = Fixture::nested();
        fixture.declarations[1].shape = shape;
        assert!(fixture.validate().is_err());
    }
}

fn byte() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
}

fn array(id: u32, element: u32, length: u64) -> StructuralTypeDeclaration {
    let mut declaration = record(id, Vec::new());
    declaration.shape = StructuralTypeShape::FixedArray {
        element: type_id(element),
        length,
    };
    declaration
}

impl Fixture {
    fn record_arrays() -> Self {
        let mut fixture = Self::nested();
        fixture.signature.shapes = vec![
            BoundaryValueShape {
                class: BoundaryValueClass::Record {
                    first_field: 0,
                    field_count: 1,
                },
                byte_size: 32,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::FixedArray {
                    element: 2,
                    length: 2,
                },
                byte_size: 32,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::Record {
                    first_field: 1,
                    field_count: 2,
                },
                byte_size: 16,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::Integer,
                byte_size: 8,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::FixedArray {
                    element: 5,
                    length: 4,
                },
                byte_size: 4,
                alignment: 1,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::Integer,
                byte_size: 1,
                alignment: 1,
            },
        ];
        fixture.signature.fields = vec![
            BoundaryValueField {
                shape: 1,
                byte_offset: 0,
            },
            BoundaryValueField {
                shape: 3,
                byte_offset: 0,
            },
            BoundaryValueField {
                shape: 4,
                byte_offset: 8,
            },
        ];
        let mut primitive = record(5, Vec::new());
        primitive.shape = StructuralTypeShape::PrimitiveScalar(byte());
        fixture.declarations = vec![
            record(
                1,
                vec![field(1, StructuralFieldType::Structural(type_id(2)))],
            ),
            array(2, 3, 2),
            record(
                3,
                vec![
                    field(1, StructuralFieldType::Scalar(scalar())),
                    field(2, StructuralFieldType::Structural(type_id(4))),
                ],
            ),
            array(4, 5, 4),
            primitive,
        ];
        fixture.codec.extent = 32;
        fixture.codec.fields.clear();
        for index in 0..2 {
            fixture.codec.fields.push(ScalarField {
                path: vec![member(1), Path::FixedIndex(index), member(1)],
                offset: index as usize * 16,
                logical: scalar(),
                stored: scalar(),
            });
            for guard in 0..4 {
                fixture.codec.fields.push(ScalarField {
                    path: vec![
                        member(1),
                        Path::FixedIndex(index),
                        member(2),
                        Path::FixedIndex(guard),
                    ],
                    offset: index as usize * 16 + 8 + guard as usize,
                    logical: byte(),
                    stored: byte(),
                });
            }
        }
        fixture
    }

    fn empty_array() -> Self {
        let mut fixture = Self::nested();
        fixture.declarations[0] = record(
            1,
            vec![
                field(1, StructuralFieldType::Structural(type_id(2))),
                field(2, StructuralFieldType::Structural(type_id(3))),
            ],
        );
        fixture.declarations.push(array(3, 2, 0));
        fixture.signature.shapes.push(BoundaryValueShape {
            class: BoundaryValueClass::FixedArray {
                element: 1,
                length: 0,
            },
            byte_size: 0,
            alignment: 8,
        });
        fixture.signature.fields[1].shape = 3;
        fixture.codec.fields.pop();
        fixture
    }
}

#[test]
fn record_arrays_and_primitive_byte_arrays_preserve_all_indexed_coordinates() {
    let fixture = Fixture::record_arrays();
    assert!(fixture.validate().is_ok());
    let mut fixture = Fixture::record_arrays();
    fixture.codec.fields[1].path.pop();
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::record_arrays();
    fixture.codec.fields[5].path[1] = Path::FixedIndex(0);
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::record_arrays();
    fixture.codec.fields[6].offset = 8;
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::record_arrays();
    fixture.codec.fields[1].logical = scalar();
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::record_arrays();
    fixture.codec.fields[1].stored = scalar();
    assert!(fixture.validate().is_err());
}

#[test]
fn array_count_stride_extent_alignment_and_scalar_class_are_exact() {
    let mut fixture = Fixture::record_arrays();
    fixture.declarations[1].shape = StructuralTypeShape::FixedArray {
        element: type_id(3),
        length: 1,
    };
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::record_arrays();
    fixture.signature.shapes[1].byte_size = 31;
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::record_arrays();
    fixture.signature.shapes[2].byte_size = 12;
    fixture.signature.shapes[1].byte_size = 24;
    // Self-consistent public stride still differs from the root codec's slots.
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::record_arrays();
    fixture.signature.shapes[1].alignment = 4;
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::record_arrays();
    fixture.signature.shapes[5].class = BoundaryValueClass::Float;
    assert!(fixture.validate().is_err());
}

#[test]
fn zero_array_validates_its_complete_element_schema_without_instantiating_leaves() {
    let fixture = Fixture::empty_array();
    assert!(fixture.validate().is_ok());
    let mut fixture = Fixture::empty_array();
    fixture.declarations[2].shape = StructuralTypeShape::FixedArray {
        element: type_id(99),
        length: 0,
    };
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::empty_array();
    fixture.declarations[2].shape = StructuralTypeShape::FixedArray {
        element: type_id(3),
        length: 0,
    };
    fixture.signature.shapes[3].class = BoundaryValueClass::FixedArray {
        element: 3,
        length: 0,
    };
    assert!(fixture.validate().is_err());
    let mut fixture = Fixture::empty_array();
    fixture.signature.shapes[3].class = BoundaryValueClass::FixedArray {
        element: 0,
        length: 0,
    };
    assert!(fixture.validate().is_err());
}

#[test]
fn nonempty_arrays_of_empty_records_have_no_invented_codec_leaves() {
    let mut fixture = Fixture::empty_array();
    fixture.declarations[2].shape = StructuralTypeShape::FixedArray {
        element: type_id(4),
        length: 65535,
    };
    fixture.declarations.push(record(4, Vec::new()));
    fixture.signature.shapes[3].class = BoundaryValueClass::FixedArray {
        element: 4,
        length: 65535,
    };
    fixture.signature.shapes[3].alignment = 1;
    fixture.signature.shapes.push(BoundaryValueShape {
        class: BoundaryValueClass::Record {
            first_field: 3,
            field_count: 0,
        },
        byte_size: 0,
        alignment: 1,
    });
    assert!(fixture.validate().is_ok());
    // A zero-size array does not hide an unsupported element family.
    fixture.declarations[3].shape = StructuralTypeShape::Sum { cases: Vec::new() };
    assert!(fixture.validate().is_err());
}

#[test]
fn array_of_array_retains_both_indices_and_checks_schema_below_zero_length() {
    let mut fixture = Fixture::record_arrays();
    fixture.declarations[3].shape = StructuralTypeShape::FixedArray {
        element: type_id(6),
        length: 2,
    };
    fixture.declarations.push(array(6, 5, 2));
    fixture.signature.shapes[4].class = BoundaryValueClass::FixedArray {
        element: 6,
        length: 2,
    };
    fixture.signature.shapes.push(BoundaryValueShape {
        class: BoundaryValueClass::FixedArray {
            element: 5,
            length: 2,
        },
        byte_size: 2,
        alignment: 1,
    });
    for leaf in &mut fixture.codec.fields {
        if leaf.logical == byte() {
            let Path::FixedIndex(index) = leaf.path.pop().unwrap() else {
                unreachable!()
            };
            leaf.path
                .extend([Path::FixedIndex(index / 2), Path::FixedIndex(index % 2)]);
        }
    }
    assert!(fixture.validate().is_ok());
    fixture.codec.fields[3].path[3] = Path::FixedIndex(0);
    assert!(fixture.validate().is_err());
}

#[test]
fn authored_stored_width_and_packing_remain_independent_of_logical_scalar_width() {
    let mut fixture = Fixture::nested();
    let StructuralTypeShape::Record { fields, .. } = &mut fixture.declarations[1].shape else {
        unreachable!()
    };
    fields[0].storage = Some(semantic_vocabulary::AuthoredFieldStorage::Scalar {
        byte_offset: 0,
        stored_scalar: Some(byte()),
    });
    fixture.signature.shapes[2].byte_size = 1;
    for leaf in &mut fixture.codec.fields {
        leaf.stored = byte();
    }
    assert!(fixture.validate().is_ok());
    fixture.codec.fields[1].stored = scalar();
    assert!(fixture.validate().is_err());
}

#[test]
fn primitive_array_restrictions_remain_payload_obligations_not_new_abi_classes() {
    let mut fixture = Fixture::record_arrays();
    fixture.declarations[4].scalar_restriction = Some(StructuralFieldType::Scalar(byte()));
    assert!(fixture.validate().is_ok());
    fixture.signature.shapes[5].byte_size = 2;
    fixture.signature.shapes[4].byte_size = 8;
    assert!(fixture.validate().is_err());
}

#[test]
fn standalone_scalar_and_array_roots_join_the_same_exact_codec_paths() {
    let mut fixture = Fixture::record_arrays();
    fixture.codec.structural_type = type_id(4);
    fixture.codec.extent = 4;
    fixture.codec.alignment = 1;
    fixture.codec.fields = (0..4)
        .map(|index| ScalarField {
            path: vec![Path::FixedIndex(index)],
            offset: index as usize,
            logical: byte(),
            stored: byte(),
        })
        .collect();
    assert!(
        validate(&fixture.signature, 4, &fixture.codec, |identity| fixture
            .declarations
            .iter()
            .find(|row| row.id == identity))
        .is_ok()
    );
    fixture.codec.structural_type = type_id(5);
    fixture.codec.extent = 1;
    fixture.codec.fields = vec![ScalarField {
        path: Vec::new(),
        offset: 0,
        logical: byte(),
        stored: byte(),
    }];
    assert!(
        validate(&fixture.signature, 5, &fixture.codec, |identity| fixture
            .declarations
            .iter()
            .find(|row| row.id == identity))
        .is_ok()
    );
    fixture.codec.fields[0].path.push(Path::FixedIndex(0));
    assert!(
        validate(&fixture.signature, 5, &fixture.codec, |identity| fixture
            .declarations
            .iter()
            .find(|row| row.id == identity))
        .is_err()
    );
}
