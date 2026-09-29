//! Custody-free values inside an unranked cycle. A boundary call's scalar
//! result is an ordinary value, as an in-module call's is, and an
//! unrestricted scalar case is a copy payload. Re-entering either producer on
//! every traversal moves no custody, so neither closes the cycle fence. An
//! affine scalar case keeps its confinement rule.

use super::{
    AdmissionProfile, ControlCycleIneligibility, IntegerSign, IntegerType, ModuleError, Operation,
    OperationId, OperationKind, OperationResult, PlaceId, ProofBundle, ScalarType,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, TerminalModule,
    ValueDeclaration, ValueId, id, unranked_scalar_cycle, validate_module,
    verify_module_for_interpretation,
};
use semantic_vocabulary::{BoundaryMachineId, StructuralCaseId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, BoundaryMachineDeclaration, BoundaryMachineResult, ScalarCaseField,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralOperationResult,
};

fn scalar_boundary_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let boundary = id(1, BoundaryMachineId::new);
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    module.boundary_machines.push(BoundaryMachineDeclaration {
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_requires: Vec::new(),
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary,
        identity: "Console::write_status".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Scalar(scalar_type),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(30, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(30, ValueId::new),
            scalar_type,
        }),
        kind: OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    module
}

#[test]
fn cyclic_boundary_scalar_result_is_custody_free() {
    let module = scalar_boundary_cycle();
    validate_module(&module).expect("a scalar boundary result does not close the cycle fence");
    verify_module_for_interpretation(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the cyclic scalar boundary call verifies");
}

fn scalar_case_cycle(multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let sum_type = id(1, StructuralTypeId::new);
    let set_case = id(1, StructuralCaseId::new);
    let value_field = id(1, StructuralFieldId::new);
    let place = id(1, PlaceId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: sum_type,
        identity: "test::Flag".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: set_case,
                    identity: "Set".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: value_field,
                        identity: "value".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    }],
                },
                StructuralCaseDeclaration {
                    id: id(2, StructuralCaseId::new),
                    identity: "Clear".into(),
                    fields: Vec::new(),
                },
            ],
        },
    });
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id(31, OperationId::new),
            structural_type: sum_type,
        },
    });
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(31, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: sum_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarCase {
                result_case: set_case,
                fields: vec![ScalarCaseField {
                    field: value_field,
                    value: id(10, ValueId::new),
                    range_obligation: None,
                }],
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(32, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(32, ValueId::new),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::StructuralCaseMembership {
                source: place,
                path: Vec::new(),
                case: set_case,
            },
        },
    ];
    module
}

#[test]
fn cyclic_unrestricted_scalar_case_is_a_copy_payload() {
    let module = scalar_case_cycle(StructuralMultiplicity::Unrestricted);
    validate_module(&module).expect("an unrestricted scalar case does not close the cycle fence");
    verify_module_for_interpretation(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the cyclic unrestricted scalar case verifies");
}

#[test]
fn cyclic_affine_scalar_case_stays_confined() {
    let module = scalar_case_cycle(StructuralMultiplicity::Affine);
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::ControlCycleIneligible {
                reason: ControlCycleIneligibility::BlockOperation { .. },
                ..
            })
        ),
        "an unconfined affine scalar case keeps the fence: {:?}",
        validate_module(&module)
    );
}
