use super::*;

#[path = "block_parameters/returns.rs"]
mod returns;
#[path = "block_parameters/stores.rs"]
mod stores;

fn block_reader(scalar_type: ScalarType, multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = owned_reader(multiplicity, scalar_type);
    let machine = &mut module.machines[0];
    let mut body = machine.blocks[0].clone();
    body.id = id::<BlockId>(3);
    let mut parameter = machine.structural_parameters[0].clone();
    parameter.place = id::<PlaceId>(3);
    body.structural_parameters.push(parameter.clone());
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: parameter.place,
        kind: StructuralPlaceKind::BlockParameter {
            block: body.id,
            position: 0,
        },
    });
    match &mut body.operations[0].kind {
        OperationKind::IntegerStructuralField { source, .. }
        | OperationKind::BooleanStructuralField { source, .. } => *source = parameter.place,
        _ => unreachable!(),
    }
    if multiplicity == StructuralMultiplicity::Affine {
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut body.terminator
        else {
            unreachable!();
        };
        *cleanup_actions = vec![TerminalAffineCleanupAction::DiscardRoot(parameter.place)];
    }
    machine.blocks[0].operations.clear();
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id::<EdgeId>(3),
        target: body.id,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: machine.structural_parameters[0].place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(body);
    module
}

#[test]
fn integer_and_boolean_reads_accept_exact_owned_block_parameters() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        for multiplicity in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Unrestricted,
        ] {
            validate_module(&block_reader(scalar_type, multiplicity))
                .expect("read from the exact transferred block descriptor");
        }
    }
}

fn bounded_block_reader(multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = block_reader(integer_type(), multiplicity);
    let ScalarType::Integer(integer) = integer_type() else {
        panic!("integer carrier");
    };
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        panic!("record carrier");
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            integer,
            IntegerValue::Signed(12),
            IntegerValue::Signed(100),
        )
        .unwrap(),
    );
    module.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        id: id(5),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(5),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        }),
        kind: OperationKind::IntegerExactCast {
            operand: id(4),
            obligation: id(5),
        },
    });
    module
}

fn read_bounds(minimum: i128, maximum: i128) -> [Proposition; 2] {
    let ScalarType::Integer(integer) = integer_type() else {
        panic!("integer carrier");
    };
    let value = ScalarTerm::value(id(4), integer_type());
    let endpoint = |value| ScalarTerm::Integer {
        scalar_type: integer,
        value: IntegerValue::Signed(value),
    };
    [
        Proposition::LessOrEqual(endpoint(minimum), value.clone()),
        Proposition::LessOrEqual(value, endpoint(maximum)),
    ]
}

fn nest_block_read(module: &mut TerminalModule) {
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].structural_type = id(1);
    machine.blocks[1].structural_parameters[0].structural_type = id(1);
    match &mut machine.blocks[1].operations[0].kind {
        OperationKind::IntegerStructuralField { path, .. }
        | OperationKind::BooleanStructuralField { path, .. } => {
            *path = vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                id(1),
            )];
        }
        _ => unreachable!(),
    }
}

#[test]
fn nested_block_reads_validate_exact_carriers_and_preserve_bounded_snapshots() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        for scalar_type in [integer_type(), ScalarType::Boolean] {
            let mut module = block_reader(scalar_type, multiplicity);
            nest_block_read(&mut module);
            validate_module(&module).expect("nested field under transferred whole record");
        }
        let mut module = bounded_block_reader(multiplicity);
        nest_block_read(&mut module);
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        for bound in read_bounds(12, 100) {
            assert!(obligations[0].semantic_axioms.contains(&bound));
        }
        let ScalarType::Integer(integer) = integer_type() else {
            unreachable!();
        };
        let full_path = vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(id(1)); 2];
        let equation = Proposition::Equal(
            ScalarTerm::value(id(4), integer_type()),
            ScalarTerm::integer_field_path(id(3), full_path, integer),
        );
        assert!(obligations[0].semantic_axioms.contains(&equation));
    }
}

#[test]
fn nested_block_reads_reject_truncated_erased_and_nonrecord_carrier_paths() {
    use semantic_vocabulary::CanonicalStructuralPathSegment;
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        let mut original = block_reader(scalar_type, StructuralMultiplicity::Affine);
        nest_block_read(&mut original);
        for replacement in [
            Vec::new(),
            vec![CanonicalStructuralPathSegment::Field(id(99))],
            vec![CanonicalStructuralPathSegment::FixedIndex(0)],
            vec![CanonicalStructuralPathSegment::Case(id(1))],
            vec![CanonicalStructuralPathSegment::Field(id(1)); 2],
        ] {
            let mut module = original.clone();
            match &mut module.machines[0].blocks[1].operations[0].kind {
                OperationKind::IntegerStructuralField { path, .. }
                | OperationKind::BooleanStructuralField { path, .. } => *path = replacement,
                _ => unreachable!(),
            }
            assert!(validate_module(&module).is_err());
        }
        let StructuralTypeShape::Record { fields } = &mut original.structural_types[0].shape else {
            unreachable!();
        };
        fields[0].relevance = BindingRelevance::Erased;
        assert!(validate_module(&original).is_err());
    }
}

#[test]
fn bounded_block_reads_capture_declared_range_on_the_fresh_scalar() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        let module = bounded_block_reader(multiplicity);
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        let [cast] = obligations.as_slice() else {
            panic!("one conversion obligation");
        };
        for bound in read_bounds(12, 100) {
            assert!(
                cast.semantic_axioms.contains(&bound),
                "{multiplicity:?}: {bound:?}"
            );
        }
        assert!(
            cast.semantic_axioms
                .iter()
                .filter(|fact| matches!(fact, Proposition::LessOrEqual(..)))
                .all(|fact| {
                    !matches!(
                        fact,
                        Proposition::LessOrEqual(ScalarTerm::IntegerField { .. }, _)
                            | Proposition::LessOrEqual(_, ScalarTerm::IntegerField { .. })
                    )
                }),
            "range facts describe the captured SSA value, not a mutable-place alias"
        );
    }
}

#[test]
fn nested_block_read_redirection_cannot_reuse_sibling_range_facts() {
    let mut module = bounded_block_reader(StructuralMultiplicity::Unrestricted);
    nest_block_read(&mut module);
    let mut sibling = module.structural_types[1].clone();
    sibling.id = id(3);
    sibling.identity = "test::OtherItem".into();
    let StructuralTypeShape::Record { fields } = &mut sibling.shape else {
        unreachable!();
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            IntegerValue::Signed(13),
            IntegerValue::Signed(99),
        )
        .unwrap(),
    );
    module.structural_types.push(sibling);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!();
    };
    let mut redirected = fields[0].clone();
    redirected.id = id(2);
    redirected.identity = "other".into();
    redirected.field_type = StructuralFieldType::Structural(id(3));
    fields.push(redirected);
    let OperationKind::IntegerStructuralField { path, .. } =
        &mut module.machines[0].blocks[1].operations[0].kind
    else {
        unreachable!();
    };
    *path = vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
        id(2),
    )];
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    for stale in read_bounds(12, 100) {
        assert!(!obligations[0].semantic_axioms.contains(&stale));
    }
    for actual in read_bounds(13, 99) {
        assert!(obligations[0].semantic_axioms.contains(&actual));
    }
}

#[test]
fn bounded_block_reads_cannot_reuse_another_declarations_range() {
    let original = bounded_block_reader(StructuralMultiplicity::Unrestricted);
    for replacement in [
        StructuralFieldType::Scalar(integer_type()),
        StructuralFieldType::BoundedInteger(
            semantic_vocabulary::BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                IntegerValue::Signed(13),
                IntegerValue::Signed(99),
            )
            .unwrap(),
        ),
    ] {
        let mut module = original.clone();
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
            panic!("record carrier");
        };
        fields[0].field_type = replacement;
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        for stale in read_bounds(12, 100) {
            assert!(!obligations[0].semantic_axioms.contains(&stale));
        }
    }
    let mut wrong_field = original.clone();
    let OperationKind::IntegerStructuralField { field, .. } =
        &mut wrong_field.machines[0].blocks[1].operations[0].kind
    else {
        panic!("integer read");
    };
    *field = id(99);
    assert!(reconstruct_operation_obligations(&wrong_field).is_err());

    let mut wrong_carrier = original;
    let StructuralTypeShape::Record { fields } = &mut wrong_carrier.structural_types[1].shape
    else {
        panic!("record carrier");
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            IntegerValue::Unsigned(12),
            IntegerValue::Unsigned(100),
        )
        .unwrap(),
    );
    assert!(reconstruct_operation_obligations(&wrong_carrier).is_err());
}

#[test]
fn owned_successors_reject_same_arity_aliases_and_transfer_after_disposal() {
    let mut original = block_reader(integer_type(), StructuralMultiplicity::Affine);
    let machine = &mut original.machines[0];
    let mut input = machine.structural_parameters[0].clone();
    input.position = 1;
    input.place = id::<PlaceId>(4);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: input.place,
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    machine.structural_parameters.push(input.clone());
    let mut target = machine.blocks[1].structural_parameters[0].clone();
    target.position = 1;
    target.place = id::<PlaceId>(5);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: target.place,
        kind: StructuralPlaceKind::BlockParameter {
            block: machine.blocks[1].id,
            position: 1,
        },
    });
    machine.blocks[1].structural_parameters.push(target.clone());
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments.push(StructuralArgument {
        place: input.place,
        path: Vec::new(),
        access: StructuralAccess::Owned,
    });
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut machine.blocks[1].terminator
    else {
        unreachable!()
    };
    cleanup_actions.insert(0, TerminalAffineCleanupAction::DiscardRoot(target.place));
    validate_module(&original).expect("two independently owned descriptors transfer once each");
    for duplicate in [false, true] {
        let mut module = original.clone();
        let Terminator::Jump {
            edge,
            structural_arguments,
            trivial_affine_discards,
            ..
        } = &mut module.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        let source = structural_arguments[0].place;
        if duplicate {
            structural_arguments[1].place = source;
        } else {
            trivial_affine_discards.push(source);
        }
        let expected = ModuleError::InvalidStructuralSuccessorArgument {
            edge: *edge,
            place: source,
        };
        assert_eq!(validate_module(&module).map(|_| ()), Err(expected));
    }
}

#[test]
fn owned_backedge_rejoins_the_rebound_frontier_and_cannot_reuse_invocation_custody() {
    let mut module = block_reader(integer_type(), StructuralMultiplicity::Affine);
    let machine = &mut module.machines[0];
    let header = machine.blocks[1].id;
    let owned = machine.blocks[1].structural_parameters[0].place;
    let exit = Block {
        id: id::<BlockId>(4),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: machine.blocks[1].terminator.clone(),
    };
    machine.blocks[1].operations.push(Operation {
        static_reach_binding: None,
        id: id::<OperationId>(5),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id::<ValueId>(5),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: false },
    });
    machine.blocks[1].terminator = Terminator::Conditional {
        condition: id::<ValueId>(5),
        when_true: terminal_psi::SuccessorEdge {
            edge: id::<EdgeId>(5),
            target: header,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: owned,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            trivial_affine_discards: Vec::new(),
        },
        when_false: terminal_psi::SuccessorEdge {
            edge: id::<EdgeId>(6),
            target: exit.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    machine.blocks.push(exit);
    validate_module(&module).expect("all arrivals carry the same rebound owned frontier");
    let invocation = module.machines[0].structural_parameters[0].place;
    let Terminator::Conditional { when_true, .. } = &mut module.machines[0].blocks[1].terminator
    else {
        unreachable!()
    };
    when_true.structural_arguments[0].place = invocation;
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: id::<EdgeId>(5),
            place: invocation
        })
    );
}

#[test]
fn ordinary_calls_consume_the_block_owner_without_invalidating_prior_scalar_snapshots() {
    let mut module = block_reader(integer_type(), StructuralMultiplicity::Affine);
    let mut consumer = owned_reader(StructuralMultiplicity::Affine, integer_type())
        .machines
        .remove(0);
    consumer.id = id::<MachineId>(3);
    consumer.contract = contract(3);
    consumer.entry = id::<BlockId>(99);
    consumer.result = TerminalMachineResult::Unit;
    consumer.structural_parameters[0].place = id::<PlaceId>(99);
    consumer.structural_places[0].id = id::<PlaceId>(99);
    consumer.blocks[0].id = consumer.entry;
    consumer.blocks[0].operations.clear();
    consumer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id::<EdgeId>(99),
        trivial_affine_discards: vec![id::<PlaceId>(99)],
    };
    let body = &mut module.machines[0].blocks[1];
    let place = body.structural_parameters[0].place;
    body.operations.push(Operation {
        static_reach_binding: None,
        id: id::<OperationId>(5),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: consumer.id,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut body.terminator
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    module.machines.push(consumer);
    validate_module(&module).expect("a read snapshot survives the later owned transfer");
    module.machines[0].blocks[1].operations.swap(0, 1);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: id::<OperationId>(4),
            place
        })
    );
}

#[test]
fn block_field_reads_reject_declaration_access_type_and_dominance_drift() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        for mutation in [
            "block",
            "position",
            "place",
            "type",
            "access",
            "field",
            "claim",
            "dominance",
        ] {
            let mut module = block_reader(scalar_type, StructuralMultiplicity::Affine);
            validate_module(&module).unwrap();
            let machine = &mut module.machines[0];
            match mutation {
                "block" => {
                    machine.structural_places[1].kind = StructuralPlaceKind::BlockParameter {
                        block: machine.entry,
                        position: 0,
                    }
                }
                "position" => machine.blocks[1].structural_parameters[0].position = 1,
                "place" => machine.blocks[1].structural_parameters[0].place = id::<PlaceId>(4),
                "type" => {
                    machine.blocks[1].structural_parameters[0].structural_type =
                        id::<StructuralTypeId>(1)
                }
                "access" => {
                    machine.blocks[1].structural_parameters[0].access =
                        StructuralAccess::WriteOnlyBorrow
                }
                "claim" => machine.entry_claims.push(terminal_psi::EntryClaim {
                    claim: id::<semantic_vocabulary::ClaimId>(1),
                    input: machine.blocks[1].structural_parameters[0].place,
                    path: Vec::new(),
                }),
                "field" => match &mut machine.blocks[1].operations[0].kind {
                    OperationKind::IntegerStructuralField { field, .. }
                    | OperationKind::BooleanStructuralField { field, .. } => {
                        *field = id::<StructuralFieldId>(99)
                    }
                    _ => unreachable!(),
                },
                "dominance" => {
                    let read = machine.blocks[1].operations.remove(0);
                    machine.blocks[0].operations.push(read);
                }
                _ => unreachable!(),
            }
            assert!(
                validate_module(&module).is_err(),
                "accepted {scalar_type:?} {mutation}"
            );
        }
    }
}

#[test]
fn owned_block_field_reads_reject_after_a_selected_discard() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        let mut module = block_reader(scalar_type, StructuralMultiplicity::Affine);
        validate_module(&module).unwrap();
        let machine = &mut module.machines[0];
        let place = machine.blocks[1].structural_parameters[0].place;
        let mut continuation = machine.blocks[1].clone();
        continuation.id = id::<BlockId>(4);
        continuation.structural_parameters.clear();
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut continuation.terminator
        else {
            unreachable!();
        };
        cleanup_actions.clear();
        machine.blocks[1].operations.clear();
        machine.blocks[1].terminator = Terminator::Jump {
            edge: id::<EdgeId>(4),
            target: continuation.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: vec![place],
            residual_affine_discards: Vec::new(),
        };
        machine.blocks.push(continuation);
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: id::<OperationId>(4),
                place
            })
        );
    }
}

#[test]
fn owned_block_field_reads_reject_the_old_descriptor_after_successor_transfer() {
    for scalar_type in [integer_type(), ScalarType::Boolean] {
        let mut module = block_reader(scalar_type, StructuralMultiplicity::Affine);
        let machine = &mut module.machines[0];
        let previous_place = machine.blocks[1].structural_parameters[0].place;
        let mut continuation = machine.blocks[1].clone();
        continuation.id = id::<BlockId>(4);
        let next_place = id::<PlaceId>(4);
        continuation.structural_parameters[0].place = next_place;
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: next_place,
            kind: StructuralPlaceKind::BlockParameter {
                block: continuation.id,
                position: 0,
            },
        });
        match &mut continuation.operations[0].kind {
            OperationKind::IntegerStructuralField { source, .. }
            | OperationKind::BooleanStructuralField { source, .. } => *source = next_place,
            _ => unreachable!(),
        }
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut continuation.terminator
        else {
            unreachable!();
        };
        *cleanup_actions = vec![TerminalAffineCleanupAction::DiscardRoot(next_place)];
        machine.blocks[1].operations.clear();
        machine.blocks[1].terminator = Terminator::Jump {
            edge: id::<EdgeId>(4),
            target: continuation.id,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: previous_place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        machine.blocks.push(continuation);
        validate_module(&module).expect("the new descriptor is readable after exact transfer");
        match &mut module.machines[0].blocks[2].operations[0].kind {
            OperationKind::IntegerStructuralField { source, .. }
            | OperationKind::BooleanStructuralField { source, .. } => *source = previous_place,
            _ => unreachable!(),
        }
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: id::<OperationId>(4),
                place: previous_place
            })
        );
    }
}
