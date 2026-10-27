use super::*;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{
    BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralFieldId,
};
use terminal_psi::{BindingRelevance, TerminalIdentityIndex};

fn identity(value: u32) -> StructuralTypeId {
    StructuralTypeId::new(value.into()).unwrap()
}

fn member(value: u32) -> Path {
    Path::Field(StructuralFieldId::new(value.into()).unwrap())
}

fn integer(bits: u16, sign: IntegerSign) -> IntegerType {
    IntegerType::new(sign, bits).unwrap()
}

fn byte() -> ScalarType {
    ScalarType::Integer(integer(8, IntegerSign::Unsigned))
}

fn signed() -> ScalarType {
    ScalarType::Integer(integer(64, IntegerSign::Signed))
}

fn byte_value(value: u64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: integer(8, IntegerSign::Unsigned),
        value: IntegerValue::Unsigned(value.into()),
    }
}

fn signed_value(value: i64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: integer(64, IntegerSign::Signed),
        value: IntegerValue::Signed(value.into()),
    }
}

fn field(value: u32, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(value.into()).unwrap(),
        identity: TerminalIdentityIndex(value),
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    }
}

fn declaration(value: u32, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        quotient_carrier: None,
        id: identity(value),
        identity: TerminalIdentityIndex(value),
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape,
    }
}

fn record(value: u32, fields: Vec<StructuralFieldDeclaration>) -> StructuralTypeDeclaration {
    declaration(
        value,
        StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        },
    )
}

fn array(value: u32, element: u32, length: u64) -> StructuralTypeDeclaration {
    declaration(
        value,
        StructuralTypeShape::FixedArray {
            element: identity(element),
            length,
        },
    )
}

struct Fixture {
    declarations: Vec<StructuralTypeDeclaration>,
    payload: TerminalScalarAggregateValue,
}

impl Fixture {
    /// Division.parts[2] owns Word { value: i64, guard: [u8; 4] }.
    /// Both Word occurrences share declarations but have distinct typed paths.
    fn mixed_arrays() -> Self {
        let declarations = vec![
            record(
                1,
                vec![field(1, StructuralFieldType::Structural(identity(2)))],
            ),
            array(2, 3, 2),
            record(
                3,
                vec![
                    field(1, StructuralFieldType::Scalar(signed())),
                    field(2, StructuralFieldType::Structural(identity(4))),
                ],
            ),
            array(4, 5, 4),
            declaration(5, StructuralTypeShape::PrimitiveScalar(byte())),
        ];
        let mut fields = Vec::new();
        for index in 0..2 {
            fields.push((
                vec![member(1), Path::FixedIndex(index), member(1)],
                signed_value(if index == 0 { -23_333_333_333 } else { -1 }),
            ));
            for guard in 0..4 {
                fields.push((
                    vec![
                        member(1),
                        Path::FixedIndex(index),
                        member(2),
                        Path::FixedIndex(guard),
                    ],
                    byte_value(index * 4 + guard),
                ));
            }
        }
        Self {
            declarations,
            payload: TerminalScalarAggregateValue {
                structural_type: identity(1),
                result_case: None,
                fields,
            },
        }
    }

    fn lookup(&self, value: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
        self.declarations.iter().find(|row| row.id == value)
    }

    fn plan(&self) -> Result<ScalarAggregatePlan<'_>, TerminalInterpretError> {
        ScalarAggregatePlan::new(&self.declarations[0], |value| self.lookup(value))
    }

    fn validate(&self) -> Result<(), TerminalInterpretError> {
        self.payload
            .validate_against(&self.declarations[0], |value| self.lookup(value))
    }
}

#[test]
fn records_arrays_and_primitives_retain_each_complete_ordered_index_path() {
    let fixture = Fixture::mixed_arrays();
    let plan = fixture.plan().unwrap();
    assert_eq!(plan.count().unwrap(), 10);
    let mut observed = Vec::new();
    plan.scalar_fields(|path, field| {
        observed.push((path.to_vec(), field.scalar_type().unwrap()));
        Ok(())
    })
    .unwrap();
    let expected = fixture
        .payload
        .fields
        .iter()
        .map(|(path, value)| (path.clone(), value.scalar_type()))
        .collect::<Vec<_>>();
    assert_eq!(observed, expected);
    assert!(fixture.validate().is_ok());
}

#[test]
fn missing_duplicate_reordered_extra_and_out_of_range_leaves_reject() {
    let mut fixture = Fixture::mixed_arrays();
    fixture.payload.fields.pop();
    assert!(fixture.validate().is_err());

    let mut fixture = Fixture::mixed_arrays();
    fixture.payload.fields[6] = fixture.payload.fields[1].clone();
    assert!(fixture.validate().is_err());

    let mut fixture = Fixture::mixed_arrays();
    fixture.payload.fields.swap(1, 2);
    assert!(fixture.validate().is_err());

    let mut fixture = Fixture::mixed_arrays();
    fixture
        .payload
        .fields
        .push(fixture.payload.fields[0].clone());
    assert!(fixture.validate().is_err());

    let mut fixture = Fixture::mixed_arrays();
    fixture.payload.fields[5].0[1] = Path::FixedIndex(2);
    assert!(fixture.validate().is_err());

    let mut fixture = Fixture::mixed_arrays();
    fixture.payload.fields[4].0[3] = Path::FixedIndex(4);
    assert!(fixture.validate().is_err());

    let mut fixture = Fixture::mixed_arrays();
    fixture.payload.fields[1].1 = signed_value(0);
    assert!(fixture.validate().is_err());
}

#[test]
fn primitive_declaration_restriction_is_enforced_on_every_array_element() {
    let mut fixture = Fixture::mixed_arrays();
    fixture.declarations[4].scalar_restriction = Some(StructuralFieldType::BoundedInteger(
        BoundedIntegerType::new(
            integer(8, IntegerSign::Unsigned),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(127),
        )
        .unwrap(),
    ));
    assert!(fixture.validate().is_ok());
    // The carrier is still a valid u8. Only the declaration predicate fails.
    fixture.payload.fields[9].1 = byte_value(128);
    assert!(fixture.validate().is_err());
}

#[test]
fn storage_parts_group_each_byte_array_once_but_walk_record_array_elements() {
    let fixture = Fixture::mixed_arrays();
    let plan = fixture.plan().unwrap();
    let mut observed = Vec::new();
    plan.storage_parts(|path, part| {
        let description = match part {
            ScalarAggregatePart::Field(field) => {
                assert_eq!(field.scalar_type(), Some(signed()));
                None
            }
            ScalarAggregatePart::Array {
                structural_type,
                count,
            } => Some((structural_type, count)),
            ScalarAggregatePart::Primitive(_) => {
                panic!("guard bytes must be grouped for byte backing")
            }
        };
        observed.push((path.to_vec(), description));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        observed,
        vec![
            (vec![member(1), Path::FixedIndex(0), member(1)], None),
            (
                vec![member(1), Path::FixedIndex(0), member(2)],
                Some((identity(4), 4))
            ),
            (vec![member(1), Path::FixedIndex(1), member(1)], None),
            (
                vec![member(1), Path::FixedIndex(1), member(2)],
                Some((identity(4), 4))
            ),
        ]
    );
}

#[test]
fn multidimensional_scalar_array_has_full_leaf_paths_and_one_storage_part() {
    let declarations = [
        array(1, 2, 2),
        array(2, 3, 2),
        declaration(3, StructuralTypeShape::PrimitiveScalar(byte())),
    ];
    let lookup = |value| declarations.iter().find(|row| row.id == value);
    let plan = ScalarAggregatePlan::new(&declarations[0], lookup).unwrap();
    let mut paths = Vec::new();
    plan.scalar_fields(|path, _| {
        paths.push(path.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(
        paths,
        vec![
            vec![Path::FixedIndex(0), Path::FixedIndex(0)],
            vec![Path::FixedIndex(0), Path::FixedIndex(1)],
            vec![Path::FixedIndex(1), Path::FixedIndex(0)],
            vec![Path::FixedIndex(1), Path::FixedIndex(1)],
        ]
    );
    let mut groups = Vec::new();
    plan.storage_parts(|path, part| {
        let ScalarAggregatePart::Array {
            structural_type,
            count,
        } = part
        else {
            panic!("pure scalar array must use the existing array storage owner")
        };
        groups.push((path.to_vec(), structural_type, count));
        Ok(())
    })
    .unwrap();
    assert_eq!(groups, vec![(Vec::new(), identity(1), 4)]);
}

#[test]
fn zero_array_dimensions_do_not_hide_missing_cyclic_or_unsupported_elements() {
    let missing = array(1, 99, 0);
    assert!(ScalarAggregatePlan::new(&missing, |_| None).is_err());

    let cyclic = array(1, 1, 0);
    assert!(
        ScalarAggregatePlan::new(&cyclic, |value| (value == cyclic.id).then_some(&cyclic)).is_err()
    );

    for unsupported in [
        StructuralTypeShape::Reference {
            referent: identity(1),
            access: terminal_psi::StructuralAccess::SharedBorrow,
        },
        StructuralTypeShape::Sum { cases: Vec::new() },
        StructuralTypeShape::BindingCarrier,
    ] {
        let declarations = [array(1, 2, 0), declaration(2, unsupported)];
        assert!(
            ScalarAggregatePlan::new(&declarations[0], |value| declarations
                .iter()
                .find(|row| row.id == value))
            .is_err()
        );
    }
}

#[test]
fn repeated_empty_record_schema_has_no_materialized_leaves_or_storage_parts() {
    let declarations = [array(1, 2, u64::MAX), record(2, Vec::new())];
    let lookup = |value| declarations.iter().find(|row| row.id == value);
    let plan = ScalarAggregatePlan::new(&declarations[0], lookup).unwrap();
    assert_eq!(plan.count().unwrap(), 0);
    plan.scalar_fields(|_, _| panic!("empty records have no scalar payload"))
        .unwrap();
    plan.storage_parts(|_, _| panic!("empty records need no scalar storage"))
        .unwrap();
    let payload = TerminalScalarAggregateValue {
        structural_type: identity(1),
        result_case: None,
        fields: Vec::new(),
    };
    assert!(payload.validate_against(&declarations[0], lookup).is_ok());
}

#[test]
fn outer_zero_suppresses_inner_count_overflow_without_skipping_schema_validation() {
    let mut declarations = vec![
        array(1, 2, 0),
        array(2, 3, u64::MAX),
        array(3, 4, u64::MAX),
        declaration(4, StructuralTypeShape::PrimitiveScalar(byte())),
    ];
    let plan = ScalarAggregatePlan::new(&declarations[0], |value| {
        declarations.iter().find(|row| row.id == value)
    })
    .unwrap();
    assert_eq!(plan.count().unwrap(), 0);
    plan.scalar_fields(|_, _| panic!("zero outer dimension creates no scalar instances"))
        .unwrap();
    let mut groups = Vec::new();
    plan.storage_parts(|path, part| {
        let ScalarAggregatePart::Array {
            structural_type,
            count,
        } = part
        else {
            panic!("scalar array grouping lost")
        };
        groups.push((path.to_vec(), structural_type, count));
        Ok(())
    })
    .unwrap();
    assert_eq!(groups, vec![(Vec::new(), identity(1), 0)]);
    // The same overflowing schema has no representable nonempty root payload.
    assert!(
        ScalarAggregatePlan::new(&declarations[1], |value| declarations
            .iter()
            .find(|row| row.id == value))
        .is_err()
    );
    declarations[3].shape = StructuralTypeShape::Reference {
        referent: identity(4),
        access: terminal_psi::StructuralAccess::SharedBorrow,
    };
    assert!(
        ScalarAggregatePlan::new(&declarations[0], |value| declarations
            .iter()
            .find(|row| row.id == value))
        .is_err()
    );
}
