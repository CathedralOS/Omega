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
fn nested_nonrecord_shapes_do_not_gain_owned_value_admission() {
    for shape in [
        StructuralTypeShape::Reference {
            referent: type_id(1),
            access: terminal_psi::StructuralAccess::SharedBorrow,
        },
        StructuralTypeShape::FixedArray {
            element: type_id(1),
            length: 1,
        },
        StructuralTypeShape::Sum { cases: Vec::new() },
        StructuralTypeShape::BindingCarrier,
    ] {
        let mut fixture = Fixture::nested();
        fixture.declarations[1].shape = shape;
        assert!(fixture.validate().is_err());
    }
}
