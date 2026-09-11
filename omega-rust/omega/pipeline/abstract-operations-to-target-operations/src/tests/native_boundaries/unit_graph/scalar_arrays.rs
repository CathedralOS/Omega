//! Owned array actuals retain their exact producer through repeated calls.
use super::*;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
};

fn fixture() -> AbstractOperationPlan {
    let mut plan = super::super::returning_byte_parameter::fixture();
    plan.boundary_machines.clear();
    let primitive = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let scalar_type = plan.functions[0].parameters[0].scalar_type;
    plan.structural_types = vec![
        StructuralTypeDeclaration {
            id: primitive,
            identity: "i32".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "[i32; 2]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: primitive,
                length: 2,
            },
        },
    ]
    .into();
    let declaration = terminal_psi::StructuralResultDeclaration {
        place: PlaceId::new(99).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let result = |identity| terminal_psi::StructuralOperationResult {
        place: PlaceId::new(identity).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let caller = &mut plan.functions[0];
    caller.parameters.clear();
    caller.entry = block(1);
    caller.block_entries[0].block = block(1);
    caller.result = AbstractFunctionResult::Structural(declaration.clone());
    let return_operation = |source| AbstractOperation::ReturnStructural {
        psi_edge: edge(1),
        source: PlaceId::new(source).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let call = |identity, source| AbstractOperation::CallStructural {
        psi_operation: operation(identity),
        result: result(identity),
        callee: MachineId::new(902).unwrap(),
        arguments: Vec::new(),
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place: PlaceId::new(source).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    caller.operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: operation(1),
            result: value(1),
            scalar_type,
            value: IntegerValue::Signed(7),
        },
        AbstractOperation::EstablishScalarArray {
            psi_operation: operation(2),
            result: result(2),
            elements: vec![value(1), value(1)],
        },
        call(3, 2),
        call(4, 2),
        call(5, 3),
        return_operation(5),
    ];
    let mut identity = caller.clone();
    identity.machine = MachineId::new(902).unwrap();
    identity.structural_parameters = vec![terminal_psi::StructuralParameterDeclaration {
        place: PlaceId::new(10).unwrap(),
        position: 0,
        is_self: false,
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    identity.operations = vec![return_operation(10)];
    plan.functions.push(identity);
    plan
}

#[test]
fn owned_array_calls_preserve_constructed_and_returned_home_identity() {
    let plan = fixture();
    for target in [NativeTarget::macos_arm64(), NativeTarget::linux_x64()] {
        let lowered = crate::lower_to_target_operations(&plan, target).unwrap();
        let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
            panic!("graph")
        };
        let producers = graph.blocks[0]
            .operations
            .iter()
            .filter_map(|operation| {
                let target_operations::TargetUnitOperation::StructuralResultCall {
                    arguments, ..
                } = operation
                else {
                    return None;
                };
                let target_operations::TargetStructuralArgumentSource::StructuralHome {
                    psi_operation,
                } = arguments[0].source
                else {
                    panic!("home")
                };
                Some(psi_operation)
            })
            .collect::<Vec<_>>();
        assert_eq!(producers, vec![operation(2), operation(2), operation(3)]);
        let TargetOperation::ControlGraph(identity) = &lowered.functions[1].operation else {
            panic!("identity graph")
        };
        assert!(
            matches!(&identity.blocks[0].terminator, target_operations::TargetControlTerminator::ReturnStructural {
            source: target_operations::TargetStructuralReturnSource::Parameter(parameter), ..
        } if parameter.place == PlaceId::new(10).unwrap())
        );
    }
}

#[test]
fn owned_array_calls_reject_unavailable_places_and_borrowed_actuals() {
    for mutation in 0..3 {
        let mut plan = fixture();
        let AbstractOperation::CallStructural {
            structural_arguments,
            ..
        } = &mut plan.functions[0].operations[2]
        else {
            panic!("call")
        };
        match mutation {
            0 => structural_arguments[0].place = PlaceId::new(4).unwrap(),
            1 => structural_arguments[0].access = StructuralAccess::SharedBorrow,
            _ => {
                plan.functions[1].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Affine
            }
        }
        assert!(crate::lower_to_target_operations(&plan, NativeTarget::macos_arm64()).is_err());
    }
}
