use super::*;

fn produced_partial_module() -> TerminalModule {
    let mut module = partial_affine_field_module();
    let root_type = module.machines[0].structural_parameters[0].structural_type;
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary_id(1),
        identity: "produce_pair".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Structural(
            terminal_psi::BoundaryStructuralResultDeclaration {
                structural_type: root_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            },
        ),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let caller = &mut module.machines[0];
    caller.structural_parameters.clear();
    caller.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: operation_id(2),
        structural_type: root_type,
    };
    caller.blocks[0].operations.insert(
        0,
        Operation {
            id: operation_id(2),
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: place_id(1),
                structural_type: root_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::BoundaryCall {
                boundary: boundary_id(1),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        },
    );
    module
}

#[test]
fn call_result_partial_moves_retain_the_exact_residual_complement() {
    let module = produced_partial_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a produced affine root retains exactly its untransferred fields");
}

#[test]
fn result_residuals_reject_wrong_production_and_cleanup_custody() {
    let original = produced_partial_module();
    validate_module(&original).expect("valid result root before mutations");
    for mutation in 0..10 {
        let mut changed = original.clone();
        let caller = &mut changed.machines[0];
        match mutation {
            0 => caller.blocks[0].operations.swap(0, 1),
            1 => {
                caller.blocks[0].operations.remove(0);
            }
            2 => {
                let mut duplicate = caller.blocks[0].operations[1].clone();
                duplicate.id = operation_id(3);
                caller.blocks[0].operations.push(duplicate);
            }
            3 | 4 => {
                caller.blocks[0].terminator = Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: if mutation == 3 {
                        Vec::new()
                    } else {
                        vec![place_id(1)]
                    },
                }
            }
            _ => {
                let Terminator::ReturnUnitPartialAffine {
                    residual_affine_discards,
                    ..
                } = &mut caller.blocks[0].terminator
                else {
                    unreachable!()
                };
                match mutation {
                    5 => {
                        residual_affine_discards.pop();
                    }
                    6 => residual_affine_discards.reverse(),
                    7 => residual_affine_discards[0].structural_type = structural_type_id(3),
                    8 => {
                        residual_affine_discards[0].path =
                            vec![StructuralPathSegment::Field("right".into())]
                    }
                    9 => residual_affine_discards[0].place = place_id(2),
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            validate_module(&changed).is_err(),
            "custody mutation {mutation}"
        );
    }
}

#[test]
fn fixed_array_result_roots_use_reverse_index_residuals() {
    let mut module = produced_partial_module();
    module.structural_types[1].shape = StructuralTypeShape::FixedArray {
        element: structural_type_id(1),
        length: 3,
    };
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *residual_affine_discards = [2, 0]
        .into_iter()
        .map(|index| StructuralAffineDiscard {
            place: place_id(1),
            path: vec![StructuralPathSegment::FixedIndex(index)],
            structural_type: structural_type_id(1),
        })
        .collect();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("array-root result uses the same complement");
    let StructuralTypeShape::FixedArray { length, .. } = &mut module.structural_types[1].shape
    else {
        unreachable!()
    };
    *length = u64::MAX;
    assert!(
        validate_module(&module).is_err(),
        "short evidence bounds a forged huge residual"
    );
}
