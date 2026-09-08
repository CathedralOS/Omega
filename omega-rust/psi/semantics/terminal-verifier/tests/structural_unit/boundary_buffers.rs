//! Boundary buffer presentation preserves the owning field, not a view alias.

use super::*;
use terminal_psi::ByteSequenceCarrier;

#[path = "boundary_buffers/fixed_array_views.rs"]
mod fixed_array_views;

fn buffer_module() -> TerminalModule {
    let mut module = projected_boundary_qualification_module();
    module.structural_domains.clear();
    module.structural_types[0].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        unreachable!()
    };
    for field in fields {
        field.field_type =
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 16 });
    }
    let boundary = &mut module.boundary_machines[0];
    boundary.requires.clear();
    boundary.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    boundary.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    machine.structural_parameters[0]
        .projected_qualifications
        .clear();
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: Vec::new(),
    };
    arguments(&mut module)[0].access = StructuralAccess::MutableBorrow;
    module
}

fn arguments(module: &mut TerminalModule) -> &mut Vec<StructuralArgument> {
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments
}

fn buffer_field(module: &mut TerminalModule) -> &mut StructuralFieldDeclaration {
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        unreachable!()
    };
    &mut fields[0]
}

#[test]
fn boundary_buffer_keeps_inline_capacity_and_exact_path() {
    for capacity in [0, 1, 256, u64::MAX] {
        let mut module = buffer_module();
        buffer_field(&mut module).field_type =
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity });
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("semantic presentation does not choose a native capacity limit");
        assert_eq!(
            arguments(&mut module)[0].path,
            vec![StructuralPathSegment::Field("left".into())]
        );
        assert_eq!(
            buffer_field(&mut module).field_type,
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity })
        );
    }
}

#[test]
fn boundary_buffer_rejects_wrong_leaf_erasure_access_and_type() {
    for field_type in [
        StructuralFieldType::Scalar(ScalarType::Boolean),
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView),
    ] {
        let mut module = buffer_module();
        buffer_field(&mut module).field_type = field_type;
        assert!(matches!(
            validate_module(&module),
            Err(ModuleError::InvalidStructuralArgumentPath { .. })
        ));
    }
    let mut module = buffer_module();
    buffer_field(&mut module).relevance = terminal_psi::BindingRelevance::Erased;
    assert!(validate_module(&module).is_err());

    let mut module = buffer_module();
    module.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));

    let mut module = buffer_module();
    module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentAccessExceedsSource { .. })
    ));

    for access in [
        StructuralAccess::Owned,
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = buffer_module();
        arguments(&mut module)[0].access = access;
        module.boundary_machines[0].structural_parameters[0].access = access;
        assert!(validate_module(&module).is_err());
    }
}

#[test]
fn boundary_buffer_resolves_each_index_and_rejects_overlapping_loans() {
    let mut module = buffer_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "BufferArray".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(2),
            length: 2,
        },
    });
    module.machines[0].structural_parameters[0].structural_type = structural_type_id(3);
    arguments(&mut module)[0]
        .path
        .insert(0, StructuralPathSegment::FixedIndex(1));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the array prefix retains the exact buffer owner");
    let mut invalid = module.clone();
    arguments(&mut invalid)[0].path[0] = StructuralPathSegment::FixedIndex(2);
    assert!(matches!(
        validate_module(&invalid),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));
    arguments(&mut invalid)[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    assert!(validate_module(&invalid).is_err());

    let mut second_parameter = module.boundary_machines[0].structural_parameters[0].clone();
    second_parameter.position = 1;
    second_parameter.place = place_id(3);
    module.boundary_machines[0]
        .structural_parameters
        .push(second_parameter);
    let mut second_argument = arguments(&mut module)[0].clone();
    second_argument.path[0] = StructuralPathSegment::FixedIndex(0);
    arguments(&mut module).push(second_argument);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("disjoint buffers remain independent mutable loans");
    arguments(&mut module)[1].path[0] = StructuralPathSegment::FixedIndex(1);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::OverlappingExclusiveStructuralArguments { .. })
    ));
}

#[test]
fn boundary_buffer_checks_nested_field_identity_and_relevance() {
    let mut module = buffer_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "BufferContainer".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "owner".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(structural_type_id(2)),
            }],
        },
    });
    module.machines[0].structural_parameters[0].structural_type = structural_type_id(3);
    arguments(&mut module)[0]
        .path
        .insert(0, StructuralPathSegment::Field("owner".into()));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("nested buffer retains both exact fields");
    let mut missing = module.clone();
    arguments(&mut missing)[0].path[0] = StructuralPathSegment::Field("other".into());
    assert!(matches!(
        validate_module(&missing),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        unreachable!()
    };
    fields[0].relevance = terminal_psi::BindingRelevance::Erased;
    let argument = arguments(&mut module)[0].clone();
    assert_eq!(
        terminal_semantics::boundary_buffer_capacity(
            &module,
            structural_type_id(3),
            &argument,
            &module.boundary_machines[0].structural_parameters[0],
        ),
        None
    );
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidErasedStructuralField { .. })
    ));
}

#[test]
fn boundary_buffer_does_not_infer_view_qualifications() {
    let mut module = buffer_module();
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain_id(1),
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "View::Ready".into(),
        carrier: structural_type_id(1),
        content_projection: None,
    });
    module.boundary_machines[0].structural_parameters[0]
        .qualifications
        .push(domain_id(1));
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));
}

#[test]
fn byte_field_presentation_does_not_widen_scalar_result_calls() {
    let mut module = ordinary_buffer_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let structural_arguments = structural_arguments.clone();
    let result = ValueDeclaration {
        id: value_id(101),
        scalar_type: ScalarType::Boolean,
    };
    module.machines[0].blocks[0].operations[0].result = OperationResult::Scalar(result);
    module.machines[0].blocks[0].operations[0].kind = OperationKind::CallStructuralScalar {
        callee: machine_id(2),
        arguments: Vec::new(),
        structural_arguments,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let callee = &mut module.machines[1];
    callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(102),
        scalar_type: ScalarType::Boolean,
    });
    callee.blocks[0].operations = vec![Operation {
        id: operation_id(100),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(100),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    }];
    callee.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(100),
        cleanup_actions: Vec::new(),
    };
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));
}

fn ordinary_buffer_module() -> TerminalModule {
    let mut module = buffer_module();
    let structural_arguments = arguments(&mut module).clone();
    let mut callee = module.machines[0].clone();
    callee.id = machine_id(2);
    callee.parameters.clear();
    callee.structural_parameters = module.boundary_machines[0].structural_parameters.clone();
    callee.structural_places = callee
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect();
    callee.contract = MachineContract {
        id: contract_id(2),
        requires: Vec::new(),
        ensures: Vec::new(),
        crash_routes: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    callee.entry_claims.clear();
    callee.content_entry_claims.clear();
    callee.entry = block_id(2);
    callee.blocks[0].id = block_id(2);
    callee.blocks[0].operations.clear();
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(2),
        trivial_affine_discards: Vec::new(),
    };
    module.machines[0].blocks[0].operations[0].kind = OperationKind::CallUnit {
        callee: callee.id,
        arguments: Vec::new(),
        structural_arguments,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    module.machines.push(callee);
    module.boundary_machines.clear();
    module
}

#[test]
fn ordinary_unit_byte_subloan_keeps_exact_inline_field_and_capacity() {
    for capacity in [0, 1, 256, u64::MAX] {
        let mut module = ordinary_buffer_module();
        buffer_field(&mut module).field_type =
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity });
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        assert_eq!(
            structural_arguments[0].path,
            vec![StructuralPathSegment::Field("left".into())]
        );
    }
}

#[test]
fn ordinary_unit_byte_subloan_rejects_wrong_leaf_type_access_and_path() {
    for mutation in 0..6 {
        let mut module = ordinary_buffer_module();
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
        match mutation {
            0 => {
                buffer_field(&mut module).field_type =
                    StructuralFieldType::Scalar(ScalarType::Boolean)
            }
            1 => {
                buffer_field(&mut module).field_type =
                    StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView)
            }
            2 => buffer_field(&mut module).relevance = terminal_psi::BindingRelevance::Erased,
            3 => {
                module.structural_types[0].shape =
                    StructuralTypeShape::Record { fields: Vec::new() }
            }
            4 => {
                module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow
            }
            _ => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut module.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                structural_arguments[0].path = vec![StructuralPathSegment::Field("missing".into())];
            }
        }
        assert!(validate_module(&module).is_err(), "mutation {mutation}");
    }
}
