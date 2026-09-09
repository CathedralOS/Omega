use super::*;

fn fixture(projected: bool) -> TerminalModule {
    let mut module = ordinary_buffer_module();
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "FixedBytes".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(4),
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(4),
            identity: "Octet".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        },
    ]);
    if projected {
        buffer_field(&mut module).field_type =
            StructuralFieldType::Structural(structural_type_id(3));
    } else {
        module.machines[0].structural_parameters[0].structural_type = structural_type_id(3);
        call_argument(&mut module).path.clear();
    }
    module
}

fn call_argument(module: &mut TerminalModule) -> &mut StructuralArgument {
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    &mut structural_arguments[0]
}

fn extent(module: &TerminalModule) -> Option<u64> {
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    terminal_semantics::mutable_fixed_byte_array_extent(
        module,
        &module.machines[0].structural_parameters[0],
        &structural_arguments[0],
        &module.machines[1].structural_parameters[0],
    )
}

fn boundary_fixture(mut module: TerminalModule) -> TerminalModule {
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let structural_arguments = structural_arguments.clone();
    let mut boundary = buffer_module().boundary_machines.remove(0);
    boundary.structural_parameters = module.machines[1].structural_parameters.clone();
    module.boundary_machines.push(boundary);
    module.machines.pop();
    module.machines[0].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
        boundary: boundary_id(1),
        arguments: Vec::new(),
        structural_arguments,
        completion_receipts: Vec::new(),
    };
    module
}

#[test]
fn fixed_byte_array_unit_view_retains_true_type_extent_and_field_path() {
    for projected in [false, true] {
        for length in [1, 3, u64::MAX] {
            let mut module = fixture(projected);
            let StructuralTypeShape::FixedArray {
                length: declared, ..
            } = &mut module.structural_types[2].shape
            else {
                unreachable!()
            };
            *declared = length;
            assert_eq!(extent(&module), Some(length));
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .unwrap();
            assert_eq!(
                module.structural_types[2].shape,
                StructuralTypeShape::FixedArray {
                    element: structural_type_id(4),
                    length
                }
            );
            assert_eq!(
                call_argument(&mut module).path,
                if projected {
                    vec![StructuralPathSegment::Field("left".into())]
                } else {
                    Vec::new()
                }
            );
        }
    }
}

#[test]
fn fixed_byte_array_unit_view_rejects_wrong_primitive_array_and_access() {
    for mutation in 0..11 {
        let mut module = fixture(false);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
        match mutation {
            0 => {
                module.structural_types[3].shape = StructuralTypeShape::PrimitiveScalar(
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap()),
                )
            }
            1 => {
                module.structural_types[3].shape = StructuralTypeShape::PrimitiveScalar(
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
                )
            }
            2 => {
                module.structural_types[3].shape =
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
            }
            3 => {
                module.structural_types[2].shape =
                    StructuralTypeShape::Record { fields: Vec::new() }
            }
            4 => {
                module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow
            }
            5 => {
                module.machines[0].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow
            }
            6 => {
                module.machines[0].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Affine
            }
            7 => {
                module.machines[1].structural_parameters[0].access = StructuralAccess::SharedBorrow
            }
            8 => call_argument(&mut module).access = StructuralAccess::WriteOnlyBorrow,
            9 => {
                module.structural_types[3].shape = StructuralTypeShape::PrimitiveScalar(
                    ScalarType::Integer(IntegerType::address(64).unwrap()),
                )
            }
            _ => module.machines[0].structural_parameters[0].access = StructuralAccess::Owned,
        }
        assert_eq!(extent(&module), None, "shape mutation {mutation}");
        assert!(
            validate_module(&module).is_err(),
            "validation mutation {mutation}"
        );
        assert!(
            validate_module(&boundary_fixture(module)).is_err(),
            "boundary shape mutation {mutation}"
        );
    }
}

#[test]
fn fixed_byte_array_unit_view_rejects_qualified_and_claim_bearing_roots() {
    for mutation in 0..5 {
        let mut module = fixture(false);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
        match mutation {
            0 | 1 => {
                let carrier = if mutation == 0 {
                    structural_type_id(3)
                } else {
                    structural_type_id(1)
                };
                module.structural_domains.push(StructuralDomainDeclaration {
                    id: domain_id(1),
                    semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
                    identity: "QualifiedBytes".into(),
                    carrier,
                    content_projection: None,
                });
                module.machines[mutation].structural_parameters[0]
                    .qualifications
                    .push(domain_id(1));
                assert_eq!(extent(&module), None);
            }
            2 => module.machines[0].entry_claims.push(EntryClaim {
                claim: claim_id(1),
                input: place_id(1),
                path: Vec::new(),
            }),
            3 => module.machines[0]
                .content_entry_claims
                .push(content_entry_claim(place_id(1))),
            _ => module.machines[1].entry_claims.push(EntryClaim {
                claim: claim_id(1),
                input: place_id(2),
                path: Vec::new(),
            }),
        }
        assert!(
            validate_module(&module).is_err(),
            "custody mutation {mutation}"
        );
        // Callee entry claims have no boundary-declaration counterpart.
        if mutation != 4 {
            assert!(
                validate_module(&boundary_fixture(module)).is_err(),
                "boundary custody mutation {mutation}"
            );
        }
    }
}

#[test]
fn fixed_byte_array_unit_view_rejects_missing_erased_and_indexed_paths() {
    for mutation in 0..4 {
        let mut module = fixture(true);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
        match mutation {
            0 => {
                call_argument(&mut module).path =
                    vec![StructuralPathSegment::Field("missing".into())]
            }
            1 => buffer_field(&mut module).relevance = terminal_psi::BindingRelevance::Erased,
            2 => call_argument(&mut module)
                .path
                .push(StructuralPathSegment::FixedIndex(0)),
            _ => call_argument(&mut module).place = place_id(99),
        }
        assert_eq!(extent(&module), None);
        assert!(validate_module(&module).is_err());
        assert!(validate_module(&boundary_fixture(module)).is_err());
    }
}

#[test]
fn fixed_byte_array_unit_view_rejects_duplicate_exclusive_arguments() {
    let mut module = fixture(false);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let mut parameter = module.machines[1].structural_parameters[0].clone();
    parameter.place = place_id(3);
    parameter.position = 1;
    module.machines[1].structural_parameters.push(parameter);
    module.machines[1]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(3),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments.push(structural_arguments[0].clone());
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::OverlappingExclusiveStructuralArguments { .. })
    ));
    assert!(matches!(
        validate_module(&boundary_fixture(module)),
        Err(ModuleError::OverlappingExclusiveStructuralArguments { .. })
    ));
}

#[test]
fn fixed_byte_array_boundary_view_preserves_whole_and_field_storage() {
    for projected in [false, true] {
        for length in [1, 3, 256, u64::MAX] {
            let mut source = fixture(projected);
            let StructuralTypeShape::FixedArray {
                length: declared, ..
            } = &mut source.structural_types[2].shape
            else {
                unreachable!()
            };
            *declared = length;
            let source_type = source.machines[0].structural_parameters[0].structural_type;
            let source_argument = call_argument(&mut source).clone();
            let mut module = boundary_fixture(source);
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .expect("a boundary borrows the exact fixed range, without replacing its owner");
            assert_eq!(
                module.machines[0].structural_parameters[0].structural_type,
                source_type
            );
            assert_eq!(arguments(&mut module).as_slice(), &[source_argument]);
            assert_eq!(
                module.structural_types[2].shape,
                StructuralTypeShape::FixedArray {
                    element: structural_type_id(4),
                    length
                }
            );
        }
    }
}

#[test]
fn fixed_byte_array_unit_view_keeps_existing_zero_array_admission_fence() {
    let mut module = fixture(false);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let StructuralTypeShape::FixedArray { length, .. } = &mut module.structural_types[2].shape
    else {
        unreachable!()
    };
    *length = 0;
    assert_eq!(extent(&module), None);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralArrayLength(
            structural_type_id(3)
        ))
    );
    assert_eq!(
        validate_module(&boundary_fixture(module)).map(|_| ()),
        Err(ModuleError::InvalidStructuralArrayLength(
            structural_type_id(3)
        ))
    );
}
