use super::*;
use terminal_psi::{
    BoundaryMachineResult, BoundaryParameterKind, BoundaryStructuralResultDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
};

fn boundary_array_call(dimensions: &[u64], copied: bool) -> TerminalModule {
    let mut module = array_call(dimensions);
    let mut boundary = super::super::boundary_call_module()
        .boundary_machines
        .remove(0);
    boundary.scalar_parameters.clear();
    boundary.parameter_order = vec![BoundaryParameterKind::Structural; 2];
    boundary.structural_parameters = module.machines[1].structural_parameters.clone();
    boundary.result = BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
    });
    module.machines.truncate(1);
    let caller = &mut module.machines[0];
    caller.blocks[0].operations[2].kind = OperationKind::BoundaryCall {
        boundary: boundary.id,
        arguments: Vec::new(),
        structural_arguments: vec![
            StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            };
            2
        ],
        completion_receipts: Vec::new(),
        requirement_obligations: Vec::new(),
    };
    module.boundary_machines.push(boundary);
    if copied {
        let holder = structural_type_id(dimensions.len() as u64 + 2);
        module.structural_types.push(StructuralTypeDeclaration {
            id: holder,
            identity: "holder".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: "items".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                }],
            },
        });
        caller
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: place_id(20),
                position: 0,
                is_self: false,
                structural_type: holder,
                access: StructuralAccess::MutableBorrow,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(20),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
        caller.blocks[0].operations[1].kind = OperationKind::StructuralLeafCopy {
            source: place_id(20),
            path: vec![StructuralPathSegment::Field("items".into())],
        };
    }
    module
}

#[test]
fn boundary_arrays_copy_borrowed_fields_and_repeat_owned_payloads() {
    for dimensions in [&[2][..], &[2, 2], &[2, 0], &[0, 2, 0]] {
        for copied in [false, true] {
            verify_module(
                &boundary_array_call(dimensions, copied),
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .expect("array snapshots and boundary results replay independently");
        }
    }
}

#[test]
fn boundary_array_copies_require_exact_shape_readable_custody_and_dominance() {
    let valid = boundary_array_call(&[2], true);
    validate_module(&valid).expect("unmutated copied array boundary");
    let mut early = valid.clone();
    early.machines[0].blocks[0].operations.swap(1, 2);
    assert!(matches!(
        validate_module(&early),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
    ));

    let mut wrong_path = valid.clone();
    let OperationKind::StructuralLeafCopy { path, .. } =
        &mut wrong_path.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *path = Vec::new();
    assert!(validate_module(&wrong_path).is_err());

    let mut unreadable = valid.clone();
    unreadable.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    assert!(validate_module(&unreadable).is_err());

    let mut borrowed_as_owned = valid.clone();
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut borrowed_as_owned.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = place_id(20);
    structural_arguments[0].path = vec![StructuralPathSegment::Field("items".into())];
    assert!(validate_module(&borrowed_as_owned).is_err());

    let mut wrong_formal = valid.clone();
    wrong_formal.boundary_machines[0].structural_parameters[0].structural_type =
        structural_type_id(2);
    assert!(validate_module(&wrong_formal).is_err());

    let mut wrong_result = valid;
    let BoundaryMachineResult::Structural(result) = &mut wrong_result.boundary_machines[0].result
    else {
        unreachable!()
    };
    result.structural_type = structural_type_id(2);
    assert!(validate_module(&wrong_result).is_err());
}

fn boundary_array_loan(access: StructuralAccess, view: bool) -> TerminalModule {
    let mut module = boundary_array_call(&[2], false);
    let formal_type = if view {
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "bool_view".into(),
            shape: StructuralTypeShape::ElementView {
                element: structural_type_id(2),
            },
        });
        structural_type_id(3)
    } else {
        structural_type_id(1)
    };
    let boundary = &mut module.boundary_machines[0];
    boundary.parameter_order.truncate(1);
    boundary.structural_parameters.truncate(1);
    boundary.structural_parameters[0].structural_type = formal_type;
    boundary.structural_parameters[0].access = access;
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    structural_arguments.truncate(1);
    structural_arguments[0].access = access;
    module
}

#[test]
fn boundary_array_result_loans_preserve_type_aliasing_and_availability() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        for view in [false, true] {
            let valid = boundary_array_loan(access, view);
            verify_module(
                &valid,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .unwrap_or_else(|error| panic!("{access:?}, view={view}: {error:?}"));
            let mut early = valid.clone();
            early.machines[0].blocks[0].operations.swap(1, 2);
            assert!(matches!(
                validate_module(&early),
                Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
            ));

            let mut overlap = valid.clone();
            let mut second = overlap.boundary_machines[0].structural_parameters[0].clone();
            second.position = 1;
            second.place = place_id(4);
            overlap.boundary_machines[0]
                .structural_parameters
                .push(second);
            overlap.boundary_machines[0]
                .parameter_order
                .push(BoundaryParameterKind::Structural);
            let OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } = &mut overlap.machines[0].blocks[0].operations[2].kind
            else {
                unreachable!()
            };
            structural_arguments.push(structural_arguments[0].clone());
            if access == StructuralAccess::MutableBorrow {
                assert!(matches!(
                    validate_module(&overlap),
                    Err(ModuleError::OverlappingExclusiveStructuralArguments { .. })
                ));
            } else {
                validate_module(&overlap).expect("shared whole-array observations may coexist");
            }

            let mut wrong_type = valid.clone();
            wrong_type.boundary_machines[0].structural_parameters[0].structural_type =
                structural_type_id(2);
            assert!(validate_module(&wrong_type).is_err());
            let mut wrong_extent = valid;
            wrong_extent
                .structural_types
                .push(StructuralTypeDeclaration {
                    id: structural_type_id(4),
                    identity: "three_bools".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: structural_type_id(2),
                        length: 3,
                    },
                });
            wrong_extent.boundary_machines[0].structural_parameters[0].structural_type =
                structural_type_id(4);
            assert!(validate_module(&wrong_extent).is_err());
        }
    }
}

#[test]
fn zeroed_array_result_loans_require_establishment_and_plain_custody() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        for view in [false, true] {
            let mut valid = boundary_array_loan(access, view);
            valid.machines[0].blocks[0].operations[1].kind =
                OperationKind::EstablishZeroedStructural {};
            verify_module(
                &valid,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .unwrap_or_else(|error| panic!("zeroed {access:?}, view={view}: {error:?}"));
            let mut early = valid.clone();
            early.machines[0].blocks[0].operations.swap(1, 2);
            assert!(matches!(
                validate_module(&early),
                Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
            ));
            for mutation in ["qualification", "claim", "multiplicity"] {
                let mut changed = valid.clone();
                let OperationResult::Structural(result) =
                    &mut changed.machines[0].blocks[0].operations[1].result
                else {
                    unreachable!()
                };
                match mutation {
                    "qualification" => result
                        .qualifications
                        .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap()),
                    "claim" => result
                        .claims
                        .push(terminal_psi::StructuralResultClaimBinding {
                            claim: semantic_vocabulary::ClaimId::new(1).unwrap(),
                            path: Vec::new(),
                        }),
                    "multiplicity" => result.multiplicity = StructuralMultiplicity::Linear,
                    _ => unreachable!(),
                }
                assert!(
                    validate_module(&changed).is_err(),
                    "zeroed {access:?}, view={view}: {mutation}"
                );
            }
        }
    }
}
