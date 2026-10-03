use super::*;
use crate::execution::ExecutableMachine;
use crate::values::{TerminalScalarValue, TerminalStructuralPrimitiveValue};
use semantic_vocabulary::{
    BlockId, EdgeId, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, StructuralPlaceKind,
};
use std::sync::Arc;
use terminal_psi::{
    Block, OperationKind, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, TerminalMachineResult, Terminator,
};

fn scalar_view_runtime(
    stored: TerminalScalarValue,
    target: ScalarType,
    access: StructuralAccess,
) -> (TerminalExecution, Operation, StructuralArgument) {
    let source = PlaceId::new(1).unwrap();
    let descriptor = PlaceId::new(2).unwrap();
    let source_type = StructuralTypeId::new(1).unwrap();
    let target_type = StructuralTypeId::new(2).unwrap();
    let reference_type = StructuralTypeId::new(3).unwrap();
    let backing = StructuralArgument {
        place: source,
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    };
    let operation = Operation {
        id: OperationId::new(1).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: descriptor,
            structural_type: reference_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishReference {
            source: backing.clone(),
        },
    };
    let machine = |number, structural_type| {
        let block = BlockId::new(number).unwrap();
        ExecutableMachine {
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            structural_places: vec![StructuralPlaceDeclaration {
                id: source,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
            entry_claims: Vec::new(),
            content_entry_claims: Vec::new(),
            result: TerminalMachineResult::Unit,
            entry: block,
            blocks: BTreeMap::from([(
                block,
                Block {
                    id: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(number).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            )]),
        }
    };
    let mut caller = machine(1, source_type);
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: descriptor,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation.id,
            structural_type: reference_type,
        },
    });
    caller
        .blocks
        .values_mut()
        .next()
        .unwrap()
        .operations
        .push(operation.clone());
    let types = [
        StructuralTypeDeclaration {
            id: source_type,
            identity: "Backing".into(),
            shape: StructuralTypeShape::PrimitiveScalar(stored.scalar_type()),
        },
        StructuralTypeDeclaration {
            id: target_type,
            identity: "Observed".into(),
            shape: StructuralTypeShape::PrimitiveScalar(target),
        },
        StructuralTypeDeclaration {
            id: reference_type,
            identity: "Reference".into(),
            shape: StructuralTypeShape::Reference {
                referent: target_type,
                access,
            },
        },
    ];
    let program = crate::program::AdmittedTerminalProgram::for_test(
        BTreeMap::from([
            (MachineId::new(1).unwrap(), caller),
            (MachineId::new(2).unwrap(), machine(2, target_type)),
        ]),
        types.into_iter().map(|row| (row.id, row)).collect(),
    );
    let execution = TerminalExecution::start_with_program(
        Arc::new(program),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 100,
            structural_type: source_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
        &[],
        &[TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: stored,
        }],
        &[],
        None,
    )
    .unwrap();
    (execution, operation, backing)
}

#[test]
fn shared_scalar_reference_call_observes_original_float_bits() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    for bits in [0x8000_0000, 0x7fc1_2345] {
        let stored = TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(bits));
        let (mut execution, operation, source) = scalar_view_runtime(
            stored,
            ScalarType::Integer(unsigned),
            StructuralAccess::SharedBorrow,
        );
        let original = execution.structural_values[&source.place].clone();
        execution.establish_reference(&operation, &source).unwrap();
        let descriptor = operation.result.structural().unwrap().place;
        let carrier = execution.structural_values[&descriptor].clone();
        assert_eq!(
            execution.reference_referents[&StructuralRuntimePlace::from(&carrier)],
            original
        );
        let argument = StructuralArgument {
            place: descriptor,
            path: vec![StructuralPathSegment::Referent],
            access: StructuralAccess::SharedBorrow,
        };
        let prepared = execution
            .prepare_structural_call_arguments(
                MachineId::new(2).unwrap(),
                std::slice::from_ref(&argument),
            )
            .unwrap();
        execution
            .begin_unit_call(
                MachineId::new(2).unwrap(),
                &[],
                &[argument],
                prepared,
                &[],
                BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(
            execution.structural_values[&source.place].opaque_identity,
            original.opaque_identity
        );
        assert_eq!(
            execution
                .read_primitive_place(source.place, &[], ScalarType::Integer(unsigned))
                .unwrap(),
            TerminalScalarValue::Integer {
                scalar_type: unsigned,
                value: IntegerValue::Unsigned(u128::from(bits))
            }
        );
        assert_eq!(
            execution.structural_primitive_storage[&StructuralRuntimePlace::from(&original)],
            stored
        );
        assert!(
            execution
                .store_primitive_place(
                    source.place,
                    &[],
                    TerminalScalarValue::Integer {
                        scalar_type: unsigned,
                        value: IntegerValue::Unsigned(0)
                    }
                )
                .is_err()
        );
    }
}

#[test]
fn shared_scalar_reference_rejects_non_total_width_and_access_changes() {
    let unsigned =
        |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
    let float = TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0));
    let byte = TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        value: IntegerValue::Unsigned(1),
    };
    for (stored, target, access) in [
        (float, unsigned(64), StructuralAccess::SharedBorrow),
        (byte, ScalarType::Boolean, StructuralAccess::SharedBorrow),
        (float, unsigned(32), StructuralAccess::MutableBorrow),
    ] {
        let (mut execution, operation, source) = scalar_view_runtime(stored, target, access);
        let before = execution.structural_primitive_storage.clone();
        assert!(matches!(
            execution.establish_reference(&operation, &source),
            Err(TerminalInterpretError::VerifiedOperationMalformed)
        ));
        assert!(execution.reference_referents.is_empty());
        assert_eq!(execution.structural_primitive_storage, before);
    }
}

#[test]
fn shared_scalar_reference_projection_requires_live_carrier_and_shared_access() {
    let (mut execution, operation, source) = scalar_view_runtime(
        TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0)),
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
        StructuralAccess::SharedBorrow,
    );
    execution.establish_reference(&operation, &source).unwrap();
    let descriptor = operation.result.structural().unwrap().place;
    let mut argument = StructuralArgument {
        place: descriptor,
        path: vec![StructuralPathSegment::Referent],
        access: StructuralAccess::MutableBorrow,
    };
    assert!(
        execution
            .resolve_reference_call_arguments(std::slice::from_ref(&argument))
            .is_err()
    );
    argument.access = StructuralAccess::SharedBorrow;
    assert!(
        execution
            .resolve_reference_call_arguments(std::slice::from_ref(&argument))
            .is_ok()
    );
    execution.live_affine_frontier.clear();
    assert!(
        execution
            .resolve_reference_call_arguments(&[argument])
            .is_err()
    );
}
