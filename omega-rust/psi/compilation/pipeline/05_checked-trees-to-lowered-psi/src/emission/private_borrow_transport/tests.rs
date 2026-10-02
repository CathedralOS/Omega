use semantic_vocabulary::{
    BlockId, EdgeId, OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment, SuccessorEdge,
    Terminator, ValueDeclaration,
};

fn forward_private_state_borrows(
    root: BlockId,
    blocks: &mut [Block],
    next_place: &mut u64,
    places: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
) -> Result<(), crate::LoweringError> {
    super::forward_private_state_borrows(
        root,
        blocks,
        &[terminal_psi::StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: "Record".into(),
            shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        }],
        next_place,
        places,
    )
}

fn place(identity: u64) -> PlaceId {
    PlaceId::new(identity).unwrap()
}

fn parameter(identity: u64, position: u32) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(identity),
        position,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn argument(identity: u64) -> StructuralArgument {
    StructuralArgument {
        place: place(identity),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    }
}

fn edge(identity: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: EdgeId::new(identity).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn block(identity: u64, target: Option<u64>) -> Block {
    Block {
        id: BlockId::new(identity).unwrap(),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: match target {
            Some(target) => Terminator::Jump {
                edge: EdgeId::new(identity).unwrap(),
                target: BlockId::new(target).unwrap(),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
            None => Terminator::ReturnUnit {
                edge: EdgeId::new(identity).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        },
    }
}

fn observe(block: &mut Block, source: u64) {
    block.operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(block.id.get()).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(block.id.get()).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::BooleanStructuralField {
            source: place(source),
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
        },
    });
}

fn jump_arguments(block: &Block) -> &[StructuralArgument] {
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &block.terminator
    else {
        panic!("jump");
    };
    structural_arguments
}

#[test]
fn private_state_borrow_demand_is_specific_to_each_diamond_branch() {
    let mut blocks = vec![
        block(1, None),
        block(2, Some(4)),
        block(3, Some(4)),
        block(4, None),
    ];
    blocks[0].structural_parameters = vec![parameter(1, 0), parameter(2, 1)];
    blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(1).unwrap(),
        when_true: edge(5, 2),
        when_false: edge(6, 3),
    };
    observe(&mut blocks[1], 1);
    observe(&mut blocks[2], 2);
    let mut places = Vec::new();
    forward_private_state_borrows(blocks[0].id, &mut blocks, &mut 10, &mut places).unwrap();
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &blocks[0].terminator
    else {
        panic!("conditional");
    };
    assert_eq!(when_true.structural_arguments, vec![argument(1)]);
    assert_eq!(when_false.structural_arguments, vec![argument(2)]);
    for branch in &blocks[1..3] {
        assert_eq!(branch.structural_parameters.len(), 1);
        let OperationKind::BooleanStructuralField { source, .. } = branch.operations[0].kind else {
            panic!("read");
        };
        assert_eq!(source, branch.structural_parameters[0].place);
        assert!(jump_arguments(branch).is_empty());
    }
    assert!(blocks[3].structural_parameters.is_empty());
    assert_eq!(places.len(), 2);
}

#[test]
fn private_state_borrow_keeps_existing_shared_projection_without_root_duplication() {
    let mut blocks = vec![block(1, Some(2)), block(2, None)];
    blocks[0].structural_parameters.push(parameter(1, 0));
    let mut shared = parameter(3, 0);
    shared.access = StructuralAccess::SharedBorrow;
    blocks[1].structural_parameters.push(shared);
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut blocks[0].terminator
    else {
        panic!("jump");
    };
    structural_arguments.push(StructuralArgument {
        place: place(1),
        path: vec![StructuralPathSegment::Field("member".to_owned())],
        access: StructuralAccess::SharedBorrow,
    });
    observe(&mut blocks[1], 3);
    let expected = blocks.clone();
    let mut places = Vec::new();
    let mut next_place = 10;
    forward_private_state_borrows(blocks[0].id, &mut blocks, &mut next_place, &mut places).unwrap();
    assert_eq!(blocks, expected);
    assert!(places.is_empty());
    assert_eq!(next_place, 10);
}

#[test]
fn private_state_borrow_crosses_an_intermediate_block_without_direct_uses() {
    let mut blocks = vec![block(1, Some(2)), block(2, Some(3)), block(3, None)];
    blocks[0].structural_parameters.push(parameter(1, 0));
    observe(&mut blocks[2], 1);
    let mut places = Vec::new();
    forward_private_state_borrows(blocks[0].id, &mut blocks, &mut 10, &mut places).unwrap();
    assert_eq!(jump_arguments(&blocks[0]), &[argument(1)]);
    assert!(blocks[1].operations.is_empty());
    assert_eq!(blocks[1].structural_parameters.len(), 1);
    assert_eq!(blocks[2].structural_parameters.len(), 1);
    let intermediate = blocks[1].structural_parameters[0].place;
    let final_place = blocks[2].structural_parameters[0].place;
    assert_ne!(intermediate, final_place);
    assert_eq!(jump_arguments(&blocks[1]), &[argument(intermediate.get())]);
    let OperationKind::BooleanStructuralField { source, .. } = blocks[2].operations[0].kind else {
        panic!("read");
    };
    assert_eq!(source, final_place);
    assert_eq!(places.len(), 2);
}

#[test]
fn private_state_borrow_keeps_exclusive_access_until_the_last_write() {
    let mut blocks = vec![
        block(1, Some(2)),
        block(2, Some(3)),
        block(3, Some(4)),
        block(4, None),
    ];
    blocks[0].structural_parameters.push(parameter(1, 0));
    observe(&mut blocks[1], 1);
    observe(&mut blocks[2], 1);
    blocks[2].operations[0].kind = OperationKind::StructuralScalarFieldStore {
        destination: place(1),
        path: Vec::new(),
        field: StructuralFieldId::new(1).unwrap(),
        value: ValueId::new(1).unwrap(),
        range_obligation: None,
    };
    observe(&mut blocks[3], 1);
    forward_private_state_borrows(blocks[0].id, &mut blocks, &mut 10, &mut Vec::new()).unwrap();
    assert_eq!(
        blocks[1].structural_parameters[0].access,
        StructuralAccess::MutableBorrow
    );
    assert_eq!(
        blocks[2].structural_parameters[0].access,
        StructuralAccess::MutableBorrow
    );
    assert_eq!(
        blocks[3].structural_parameters[0].access,
        StructuralAccess::SharedBorrow
    );
    assert_eq!(
        jump_arguments(&blocks[0])[0].access,
        StructuralAccess::MutableBorrow
    );
    assert_eq!(
        jump_arguments(&blocks[1])[0].access,
        StructuralAccess::MutableBorrow
    );
    assert_eq!(
        jump_arguments(&blocks[2])[0].access,
        StructuralAccess::SharedBorrow
    );
}

#[test]
fn private_state_borrow_join_inherits_later_exclusive_demand_on_both_arms() {
    let mut blocks = vec![
        block(1, None),
        block(2, Some(4)),
        block(3, Some(4)),
        block(4, None),
    ];
    blocks[0].structural_parameters.push(parameter(1, 0));
    blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(1).unwrap(),
        when_true: edge(5, 2),
        when_false: edge(6, 3),
    };
    observe(&mut blocks[1], 1);
    observe(&mut blocks[3], 1);
    blocks[3].operations[0].kind = OperationKind::WriteOnlyPrimitiveStore {
        destination: place(1),
        path: Vec::new(),
        value: ValueId::new(1).unwrap(),
    };
    forward_private_state_borrows(blocks[0].id, &mut blocks, &mut 10, &mut Vec::new()).unwrap();
    for block in &blocks[1..] {
        assert_eq!(
            block.structural_parameters[0].access,
            StructuralAccess::MutableBorrow
        );
    }
}

#[test]
fn private_state_borrow_keeps_shared_views_through_each_guard_position() {
    for guard in [
        "view.len == 4 && packet.items[0] == 70 && packet.items[3] == 5",
        "packet.items[0] == 70 && view.len == 4 && packet.items[3] == 5",
        "view.len == 4 && packet.items[0] == 70 && view.len == 4",
    ] {
        let source = format!(
            r#"
            boundary trait Host {{ machine exit(code: i32); }}
            data Packet {{ items: [u8; 4]; marker: i32; }}
            machine stamp(values: &mut [u8]) {{
                transition values.len >= 4 {{ true -> write(values) _ -> skip() }}
                state write(values: &mut [u8]) {{ values[0] = 70; values[3] = 5; }}
                state skip() {{}}
            }}
            machine main() reaches Host {{
                let mut packet: Packet;
                transition {{ _ -> check(&mut packet) }}
                state check(packet: &mut Packet) {{
                    stamp(&mut packet.items);
                    let view: &[u8] = packet.items.as_slice();
                    transition {guard} {{ true -> done() _ -> failed() }}
                }}
                state done() {{ Host::exit(70); }}
                state failed() {{ Host::exit(71); }}
            }}
        "#
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("main"))
            .unwrap_or_else(|error| panic!("{guard}: {error:?}"));
        terminal_verifier::validate_module(&lowered.semantic_module)
            .unwrap_or_else(|error| panic!("{guard}: {error:?}"));
    }
}
