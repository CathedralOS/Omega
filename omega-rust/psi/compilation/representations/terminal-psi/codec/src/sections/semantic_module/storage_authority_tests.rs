//! Byte geometry must not manufacture the operations of a receiving type.
use super::module_wire::encode_raw;
use super::target_family_tests::{identity_table, unit_machine};
use crate::decode_module;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarQualificationSetId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, ReferenceStorageView, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex, TerminalModule, ValueDeclaration,
};

fn unsigned(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}

fn byte_view(field_type: StructuralFieldType) -> TerminalModule {
    let byte = StructuralTypeId::new(1).unwrap();
    let record = StructuralTypeId::new(2).unwrap();
    let byte_reference = StructuralTypeId::new(3).unwrap();
    let record_reference = StructuralTypeId::new(4).unwrap();
    let source = PlaceId::new(1).unwrap();
    let result = PlaceId::new(2).unwrap();
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    module.identities = identity_table(7);
    for (identity, shape) in [
        (byte, StructuralTypeShape::PrimitiveScalar(unsigned(8))),
        (
            record,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(7),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type,
                    storage: None,
                }],
            },
        ),
        (
            byte_reference,
            StructuralTypeShape::Reference {
                referent: byte,
                access: StructuralAccess::SharedBorrow,
            },
        ),
        (
            record_reference,
            StructuralTypeShape::Reference {
                referent: record,
                access: StructuralAccess::SharedBorrow,
            },
        ),
    ] {
        module.structural_types.push(StructuralTypeDeclaration {
            id: identity,
            identity: TerminalIdentityIndex(identity.get() as u32 + 2),
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            quotient_carrier: None,
            shape,
        });
    }
    let mut machine = unit_machine(MachineId::new(1).unwrap());
    machine.declaration = None;
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 0,
            structural_type: byte_reference,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::Parameter { position: 0 },
        },
        StructuralPlaceDeclaration {
            id: result,
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(2).unwrap(),
                structural_type: record_reference,
            },
        },
    ];
    let operation = |identity, result, kind| Operation {
        id: OperationId::new(identity).unwrap(),
        result,
        kind,
        static_reach_binding: None,
        suspension_crossing: None,
    };
    machine.blocks[0].operations = vec![
        operation(
            1,
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(1).unwrap(),
                scalar_type: unsigned(64),
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        operation(
            2,
            OperationResult::Structural(StructuralOperationResult {
                place: result,
                structural_type: record_reference,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::EstablishReference {
                source: StructuralArgument {
                    place: source,
                    access: StructuralAccess::SharedBorrow,
                    path: vec![StructuralPathSegment::Referent],
                },
                storage: Some(ReferenceStorageView {
                    byte_offset: ValueId::new(1).unwrap(),
                    stored_scalar: None,
                }),
            },
        ),
        operation(
            3,
            OperationResult::Unit,
            OperationKind::ReleaseReference {
                source: result,
                path: Vec::new(),
            },
        ),
        operation(
            4,
            OperationResult::Unit,
            OperationKind::ReleaseReference {
                source,
                path: Vec::new(),
            },
        ),
    ];
    module.machines.push(machine);
    module
}

#[test]
fn byte_endpoint_keeps_plain_shared_record_views() {
    let module = byte_view(StructuralFieldType::Scalar(unsigned(8)));
    assert_eq!(
        decode_module(&encode_raw(&module).unwrap()).unwrap(),
        module
    );
}

#[test]
fn byte_endpoint_cannot_introduce_atomic_mutation() {
    let module = byte_view(StructuralFieldType::Atomic(unsigned(8)));
    let error = decode_module(&encode_raw(&module).unwrap()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reference establishment has inconsistent custody"),
        "{error}"
    );
}

#[test]
fn same_atomic_reference_keeps_ordinary_projection_authority() {
    let mut module = byte_view(StructuralFieldType::Atomic(unsigned(8)));
    module.machines[0].structural_parameters[0].structural_type = StructuralTypeId::new(4).unwrap();
    let OperationKind::EstablishReference { storage, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *storage = None;
    assert_eq!(
        decode_module(&encode_raw(&module).unwrap()).unwrap(),
        module
    );
}
