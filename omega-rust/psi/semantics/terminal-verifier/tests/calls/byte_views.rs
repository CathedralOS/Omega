//! Scalar-result calls consume only established, exact shared byte-view sources.
use super::*;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralPathSegment,
};

fn module() -> TerminalModule {
    let mut module = call_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "test::Bytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    });
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: PlaceId::new(9).unwrap(),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: structural_type_id(1),
        },
    });
    caller.blocks[0].operations[0] = Operation {
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination: PlaceId::new(9).unwrap(),
            bytes: vec![0, 0x80, 0xff],
        },
    };
    caller.blocks[0].operations[1].kind = OperationKind::CallStructuralScalar {
        callee: machine_id(2),
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(9).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let callee = &mut module.machines[1];
    callee.parameters.clear();
    callee.contract.requires.clear();
    callee.contract.ensures.clear();
    callee
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: PlaceId::new(10).unwrap(),
            position: 0,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: PlaceId::new(10).unwrap(),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    callee.blocks[0].operations.push(Operation {
        id: operation_id(4),
        result: OperationResult::Scalar(boolean_declaration(value_id(4))),
        kind: OperationKind::BooleanConstant { value: true },
    });
    module
}

#[test]
fn scalar_byte_call_accepts_established_literal_and_rejects_forged_source_contracts() {
    let base = module();
    verify_module(&base, &ProofBundle::default(), &AdmissionProfile::default()).unwrap();
    for corruption in 0..8 {
        let mut changed = base.clone();
        match corruption {
            0 => {
                changed.machines[0].blocks[0].operations.remove(0);
            }
            1 => changed.machines[0].blocks[0].operations.swap(0, 1),
            2 => {
                changed.machines[1].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Affine
            }
            3 => {
                let mut other = changed.structural_types[0].clone();
                other.id = structural_type_id(2);
                other.identity = "test::OtherBytes".into();
                changed.structural_types.push(other);
                changed.machines[1].structural_parameters[0].structural_type =
                    structural_type_id(2);
            }
            _ => {
                let OperationKind::CallStructuralScalar {
                    structural_arguments,
                    claim_transfers,
                    ..
                } = &mut changed.machines[0].blocks[0].operations[1].kind
                else {
                    panic!("scalar byte call")
                };
                match corruption {
                    4 => structural_arguments[0]
                        .path
                        .push(StructuralPathSegment::Field("bytes".into())),
                    5 => structural_arguments[0].access = StructuralAccess::Owned,
                    6 => structural_arguments[0].access = StructuralAccess::MutableBorrow,
                    7 => claim_transfers.push(terminal_psi::ClaimTransfer {
                        claim: semantic_vocabulary::ClaimId::new(1).unwrap(),
                        argument_index: 0,
                    }),
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            verify_module(
                &changed,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn scalar_byte_call_cannot_import_a_sibling_branch_literal() {
    let mut module = module();
    let caller = &mut module.machines[0];
    let mut operations = std::mem::take(&mut caller.blocks[0].operations);
    let establish = operations.remove(0);
    let returned = caller.blocks[0].terminator.clone();
    caller.parameters.push(boolean_declaration(value_id(6)));
    let successor = |edge, block| terminal_psi::SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(block),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(6),
        when_true: successor(3, 3),
        when_false: successor(4, 4),
    };
    for (block, operations) in [(3, vec![establish]), (4, Vec::new())] {
        caller.blocks.push(Block {
            id: block_id(block),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations,
            terminator: Terminator::Jump {
                edge: edge_id(block + 2),
                target: block_id(5),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        });
    }
    caller.blocks.push(Block {
        id: block_id(5),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator: returned,
    });
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}
