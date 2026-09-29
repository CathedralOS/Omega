//! Element-view writes mirror the byte-view mutable-write fixture: a mutable
//! element view is forwarded to a block whose write op targets it, with the
//! element count as the exact-length witness and the element scalar as the
//! value type. Shared destinations, stale witnesses and wrong scalar types
//! are refused at validation.

use super::super::{
    Proposition, ScalarTerm, reconstruct_interpretable_operation_obligations,
    validate_module_for_interpretation,
};
use super::{
    AdmissionProfile, IntegerSign, IntegerType, ObligationId, Operation, OperationId,
    OperationKind, OperationResult, PlaceId, ProofBundle, ScalarType, StructuralAccess,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, TerminalModule,
    ValueDeclaration, ValueId, VerificationError, id, validate_module,
    verify_module_for_interpretation, view_cycle,
};

fn fixture() -> TerminalModule {
    let mut module = view_cycle();
    let element = id(9, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: element,
        identity: "test::Element".into(),
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        )),
    });
    module.structural_types[0].identity = "test::MutableElements".into();
    module.structural_types[0].shape = StructuralTypeShape::ElementView { element };
    let machine = &mut module.machines[0];
    let terminator = machine.blocks[3].terminator.clone();
    machine.blocks.truncate(2);
    machine.structural_places.truncate(2);
    machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(30, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    });
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(20, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(21, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::ElementViewLength {
            source: id(1, PlaceId::new),
        },
    });
    let super::Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    machine.blocks[1].operations[0].kind = OperationKind::ElementViewLength {
        source: id(2, PlaceId::new),
    };
    machine.blocks[1].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(30, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::ElementViewWrite {
            destination: id(2, PlaceId::new),
            index: id(1, ValueId::new),
            value: id(30, ValueId::new),
            length: id(10, ValueId::new),
            obligation: id(30, ObligationId::new),
        },
    });
    machine.blocks[1].terminator = terminator;
    module
}

#[test]
fn mutable_element_write_validates_and_requires_bounds_evidence() {
    let module = fixture();
    validate_module(&module).unwrap();
    let obligations = reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(&module).unwrap(),
    )
    .unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    assert!(obligations[0].semantic_axioms.contains(&Proposition::Equal(
        ScalarTerm::value(id(10, ValueId::new), scalar_type),
        ScalarTerm::value(id(21, ValueId::new), scalar_type),
    )));
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation)) if obligation == id(30, ObligationId::new)
    ));
}

#[test]
fn element_write_rejects_shared_destination_wrong_witness_and_value_type() {
    for mutation in 0..4 {
        let mut module = fixture();
        let machine = &mut module.machines[0];
        match mutation {
            0 => {
                machine.blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
            }
            1 => {
                let OperationKind::ElementViewWrite { length, .. } =
                    &mut machine.blocks[1].operations[1].kind
                else {
                    unreachable!()
                };
                *length = id(21, ValueId::new);
            }
            2 => {
                let OperationKind::ElementViewWrite { length, .. } =
                    &mut machine.blocks[1].operations[1].kind
                else {
                    unreachable!()
                };
                *length = id(1, ValueId::new);
            }
            _ => machine.parameters.last_mut().unwrap().scalar_type = ScalarType::Boolean,
        }
        assert!(validate_module(&module).is_err(), "mutation {mutation}");
    }
}
