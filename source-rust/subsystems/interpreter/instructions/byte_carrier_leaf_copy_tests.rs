// A scalar record read out of a byte array takes whatever the bytes hold, so
// the copy itself must check each field's declared range and refuse a result
// that claims a domain the bytes cannot establish.
use super::*;
use crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage;
use crate::engine::calls::frames::tests::empty_program;
use crate::storage::scalar_storage::ScalarCell;
use semantic_vocabulary::{BoundedIntegerType, PlaceId, StructuralFieldId};
use std::sync::Arc;
use terminal_psi::{
    ArrayExtent, BindingRelevance, StructuralFieldDeclaration, StructuralOperationResult,
    StructuralRecordStorage, StructuralTypeDeclaration,
};

const BYTE: u64 = 1;
const ARRAY: u64 = 2;
const RECORD: u64 = 3;

fn ty(id: u64) -> StructuralTypeId {
    StructuralTypeId::new(id).unwrap()
}

fn byte_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
}

fn declaration(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: ty(id),
        identity: terminal_psi::TerminalIdentityIndex(0),
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    }
}

fn field(id: u64, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: StructuralFieldId::new(id).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(id as u32),
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    }
}

/// Executes the copy of `record { digit: u8 in 0..=9, raw: u8 }` from
/// `bytes[0..2]` and returns the published fields.
fn copy(
    bytes: &[u8],
    qualifications: Vec<StructuralDomainId>,
) -> Result<(ScalarCell, ScalarCell), TerminalInterpretError> {
    let digit = StructuralFieldType::BoundedInteger(
        BoundedIntegerType::new(
            byte_type(),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(9),
        )
        .unwrap(),
    );
    let mut program = empty_program();
    Arc::get_mut(&mut program)
        .unwrap()
        .fixture_module_mut()
        .types
        .structural_types = vec![
        declaration(
            BYTE,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(byte_type())),
        ),
        declaration(
            ARRAY,
            StructuralTypeShape::FixedArray {
                element: ty(BYTE),
                length: ArrayExtent::Literal(4),
            },
        ),
        declaration(
            RECORD,
            StructuralTypeShape::Record {
                fields: vec![
                    field(1, digit),
                    field(
                        2,
                        StructuralFieldType::Scalar(ScalarType::Integer(byte_type())),
                    ),
                ],
                storage: StructuralRecordStorage::Packed,
            },
        ),
    ];
    let mut execution = TerminalExecution::start_with_program(
        program,
        &[],
        crate::TerminalStructuralInputs::default(),
        None,
        None,
    )
    .unwrap();
    let array = execution.local_structural_identities.allocate().unwrap();
    execution
        .structural_byte_storage
        .insert(
            StructuralRuntimePlace {
                opaque_identity: array,
                path: Vec::new(),
            },
            StructuralByteArrayStorage::new(
                ty(ARRAY),
                ByteSequenceView::new(ByteSequenceStorage::copy(bytes, None).unwrap()),
                None,
            ),
        )
        .unwrap();
    let leaf = TerminalStructuralValue {
        path: vec![StructuralPathSegment::FixedIndex(0)],
        ..TerminalStructuralValue::minted(array, ty(BYTE), Vec::new())
    };
    let result = StructuralOperationResult {
        place: PlaceId::new(9).unwrap(),
        structural_type: ty(RECORD),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications,
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    execution.byte_carrier_leaf_copy(&leaf, &result)?;
    let copied = StructuralRuntimePlace::from(execution.structural_value(result.place)?);
    let read = |id| {
        execution
            .structural_scalar_fields
            .get(&StructuralScalarRuntimeField::member(
                copied.clone(),
                StructuralFieldId::new(id).unwrap(),
            ))
            .cloned()
            .unwrap()
    };
    Ok((read(1), read(2)))
}

fn byte(value: u128) -> ScalarCell {
    ScalarCell::Local(TerminalScalarValue::Integer {
        scalar_type: byte_type(),
        value: IntegerValue::Unsigned(value),
    })
}

#[test]
fn byte_carrier_copy_reads_fields_within_their_declared_range() {
    assert_eq!(
        copy(&[9, 200, 0, 0], Vec::new()).unwrap(),
        (byte(9), byte(200))
    );
}

#[test]
fn byte_carrier_copy_refuses_a_byte_outside_a_bounded_field() {
    // Only the bounded field changes: 10 is outside `0..=9`.
    assert!(copy(&[10, 200, 0, 0], Vec::new()).is_err());
}

#[test]
fn byte_carrier_copy_refuses_a_domain_the_bytes_cannot_establish() {
    let domain = StructuralDomainId::new(1).unwrap();
    assert!(copy(&[9, 200, 0, 0], vec![domain]).is_err());
}
