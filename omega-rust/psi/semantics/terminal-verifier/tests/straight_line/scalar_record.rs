use super::*;
use semantic_vocabulary::StructuralFieldId;

fn module() -> TerminalModule {
    let mut module = unit_module();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let producer = OperationId::new(1).unwrap();
    let types = [
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        ScalarType::Boolean,
    ];
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "RuntimeRecord".into(),
        shape: StructuralTypeShape::Record {
            fields: types
                .iter()
                .enumerate()
                .map(|(position, scalar_type)| StructuralFieldDeclaration {
                    id: StructuralFieldId::new(position as u64 + 1).unwrap(),
                    identity: format!("field{position}"),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(*scalar_type),
                })
                .collect(),
        },
    });
    let machine = &mut module.machines[0];
    machine.parameters = types
        .iter()
        .enumerate()
        .map(|(position, scalar_type)| ValueDeclaration {
            id: ValueId::new(position as u64 + 1).unwrap(),
            scalar_type: *scalar_type,
            qualifications: Default::default(),
        })
        .collect();
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    });
    machine.blocks[0].operations.push(Operation {
        id: producer,
        result: OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Default::default(),
            projected_qualifications: Default::default(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarRecord {
            fields: (1..=2)
                .map(|identity| terminal_psi::ScalarRecordFieldValue {
                    field: StructuralFieldId::new(identity).unwrap(),
                    value: ValueId::new(identity).unwrap(),
                })
                .collect(),
        },
    });
    module
}

fn fields(module: &mut TerminalModule) -> &mut Vec<terminal_psi::ScalarRecordFieldValue> {
    let OperationKind::EstablishScalarRecord { fields } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("constructor")
    };
    fields
}

#[test]
fn runtime_scalar_record_requires_complete_ordered_exact_defined_fields() {
    validate_module(&module()).expect("runtime integer and Boolean fields are supported");
    for mutation in 0..5 {
        let mut module = module();
        match mutation {
            0 => {
                fields(&mut module).pop();
            }
            1 => fields(&mut module).swap(0, 1),
            2 => fields(&mut module)[0].value = ValueId::new(2).unwrap(),
            3 => fields(&mut module)[0].value = ValueId::new(99).unwrap(),
            4 => fields(&mut module)[0].field = StructuralFieldId::new(99).unwrap(),
            _ => unreachable!(),
        }
        assert!(validate_module(&module).is_err(), "mutation {mutation}");
    }
}

#[test]
fn runtime_scalar_record_rejects_malformed_result_and_linear_custody() {
    for mutation in 0..3 {
        let mut module = module();
        let operation = &mut module.machines[0].blocks[0].operations[0];
        if mutation == 0 {
            operation.result = OperationResult::Unit;
        } else {
            let OperationResult::Structural(result) = &mut operation.result else {
                panic!("result")
            };
            if mutation == 1 {
                result.multiplicity = StructuralMultiplicity::Linear;
            } else {
                result.place = PlaceId::new(99).unwrap();
            }
        }
        assert!(validate_module(&module).is_err());
    }
}

#[test]
fn constructed_scalar_record_block_transport_retains_exact_source_contract() {
    let mut module = module();
    let machine = &mut module.machines[0];
    let mut target = machine.blocks[0].clone();
    target.id = BlockId::new(901).unwrap();
    target.operations.clear();
    target.structural_parameters = vec![StructuralParameterDeclaration {
        place: PlaceId::new(2).unwrap(),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        access: StructuralAccess::Owned,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: PlaceId::new(2).unwrap(),
        kind: StructuralPlaceKind::BlockParameter {
            block: target.id,
            position: 0,
        },
    });
    machine.blocks[0].terminator = Terminator::Jump {
        edge: EdgeId::new(901).unwrap(),
        target: target.id,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(target);
    validate_module(&module).expect("plain record uses ordinary whole-value transport");
    for mutation in 0..5 {
        let mut altered = module.clone();
        let mut other_type = altered.structural_types[0].clone();
        other_type.id = StructuralTypeId::new(2).unwrap();
        other_type.identity = "OtherRecord".into();
        altered.structural_types.push(other_type);
        let machine = &mut altered.machines[0];
        match mutation {
            0 => {
                machine.blocks[1].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Affine
            }
            1 => machine.blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow,
            2 => {
                let Terminator::Jump {
                    structural_arguments,
                    ..
                } = &mut machine.blocks[0].terminator
                else {
                    panic!("jump")
                };
                structural_arguments[0].place = PlaceId::new(99).unwrap();
            }
            3 => {
                let producer = machine.blocks[0].operations.remove(0);
                machine.blocks[1].operations.push(producer);
            }
            _ => {
                machine.blocks[1].structural_parameters[0].structural_type =
                    StructuralTypeId::new(2).unwrap()
            }
        }
        assert!(validate_module(&altered).is_err(), "mutation {mutation}");
    }
}

fn read() -> Operation {
    Operation {
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(3).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::BooleanStructuralField {
            source: PlaceId::new(1).unwrap(),
            field: StructuralFieldId::new(2).unwrap(),
        },
    }
}

#[test]
fn runtime_scalar_record_reads_require_dominating_establishment() {
    let mut module = module();
    module.machines[0].blocks[0].operations.push(read());
    validate_module(&module).expect("established local read");
    module.machines[0].blocks[0].operations.swap(0, 1);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
    ));
}

#[test]
fn runtime_scalar_record_reconstructs_field_equations_from_ssa_initializers() {
    let mut module = module();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.push(read());
    let result = ValueId::new(4).unwrap();
    let initializer = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean);
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: result,
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(900).unwrap(),
        value: ValueId::new(3).unwrap(),
        cleanup_actions: Vec::new(),
    };
    machine.contract.ensures.push(ContractClause {
        obligation: ObligationId::new(1).unwrap(),
        proposition: Proposition::Equal(
            ScalarTerm::value(result, ScalarType::Boolean),
            initializer.clone(),
        ),
    });
    let reconstructed =
        reconstruct_terminal_obligations(&module).expect("record facts reconstruct independently");
    let field =
        ScalarTerm::boolean_field(PlaceId::new(1).unwrap(), StructuralFieldId::new(2).unwrap());
    assert!(
        reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&Proposition::Equal(field.clone(), initializer))
    );
    assert!(
        reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&Proposition::Equal(
                ScalarTerm::value(ValueId::new(3).unwrap(), ScalarType::Boolean),
                field,
            ))
    );
}

#[test]
fn runtime_scalar_record_can_return_with_exact_multiplicity() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        let mut module = module();
        let machine = &mut module.machines[0];
        let OperationResult::Structural(result) = &mut machine.blocks[0].operations[0].result
        else {
            panic!("result")
        };
        result.multiplicity = multiplicity;
        machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: PlaceId::new(2).unwrap(),
            structural_type: result.structural_type,
            multiplicity,
            qualifications: Default::default(),
            projected_qualifications: Default::default(),
        });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: PlaceId::new(2).unwrap(),
            kind: StructuralPlaceKind::Result,
        });
        machine.blocks[0].terminator = Terminator::ReturnStructural {
            edge: EdgeId::new(900).unwrap(),
            source: PlaceId::new(1).unwrap(),
            returned_claims: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        validate_module(&module).expect("exact runtime record return");
    }
}

#[test]
fn runtime_scalar_record_shared_getter_preserves_exclusive_overlap_rejection() {
    let mut module = module();
    let mut getter = unit_module().machines.remove(0);
    getter.id = MachineId::new(901).unwrap();
    getter.attachment = Some(StructuralTypeId::new(1).unwrap());
    getter.entry = BlockId::new(901).unwrap();
    getter.blocks[0].id = getter.entry;
    getter.contract.id = ContractId::new(901).unwrap();
    getter.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(14).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    getter.structural_parameters = (0..2)
        .map(|position| StructuralParameterDeclaration {
            place: PlaceId::new(position as u64 + 11).unwrap(),
            position,
            is_self: position == 0,
            structural_type: StructuralTypeId::new(1).unwrap(),
            access: StructuralAccess::SharedBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Default::default(),
            projected_qualifications: Default::default(),
        })
        .collect();
    getter.structural_places = getter
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
    getter.blocks[0].operations = vec![Operation {
        id: OperationId::new(12).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(13).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::BooleanStructuralField {
            source: PlaceId::new(11).unwrap(),
            field: StructuralFieldId::new(2).unwrap(),
        },
    }];
    getter.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(901).unwrap(),
        value: ValueId::new(13).unwrap(),
        cleanup_actions: Vec::new(),
    };
    let call = Operation {
        id: OperationId::new(2).unwrap(),
        result: read().result,
        kind: OperationKind::CallStructuralScalar {
            callee: getter.id,
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            structural_arguments: vec![
                StructuralArgument {
                    place: PlaceId::new(1).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                };
                2
            ],
        },
    };
    module.machines[0].blocks[0].operations.push(call);
    module.machines.push(getter);
    validate_module(&module)
        .expect("shared receiver and another shared argument alias original local");
    module.machines[1].structural_parameters[1].access = StructuralAccess::MutableBorrow;
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        panic!("call")
    };
    structural_arguments[1].access = StructuralAccess::MutableBorrow;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::OverlappingExclusiveStructuralArguments { .. })
    ));
}

#[test]
fn branch_local_record_cannot_be_read_on_a_bypass_but_unused_record_allows_join() {
    let mut module = module();
    let machine = &mut module.machines[0];
    let mut construction = machine.blocks[0].clone();
    construction.id = BlockId::new(901).unwrap();
    construction.terminator = Terminator::Jump {
        edge: EdgeId::new(3).unwrap(),
        target: BlockId::new(902).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let mut join = machine.blocks[0].clone();
    join.id = BlockId::new(902).unwrap();
    join.operations = vec![read()];
    machine.blocks[0].operations.clear();
    let successor = |identity, target| SuccessorEdge {
        edge: EdgeId::new(identity).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(2).unwrap(),
        when_true: successor(1, 901),
        when_false: successor(2, 902),
    };
    machine.blocks.extend([construction, join]);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
    ));
    module.machines[0].blocks[2].operations.clear();
    validate_module(&module).expect("unused unrestricted record imposes no ownership frontier");
}
