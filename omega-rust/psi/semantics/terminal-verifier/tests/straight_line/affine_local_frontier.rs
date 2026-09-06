//! Entry-established locals survive evaluator branches without early cleanup.

use super::*;

fn branched_locals() -> TerminalModule {
    let mut module = unit_module();
    let structural_type = StructuralTypeId::new(901).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "Scratch".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let machine = &mut module.machines[0];
    let condition = ValueId::new(901).unwrap();
    machine.parameters.push(ValueDeclaration {
        id: condition,
        scalar_type: ScalarType::Boolean,
    });
    for ordinal in 0..2 {
        let destination = PlaceId::new(901 + ordinal).unwrap();
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: destination,
            kind: StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: ordinal as u32,
                structural_type,
                construction: None,
            },
        });
        machine.blocks[0].operations.push(Operation {
            id: OperationId::new(901 + ordinal).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal { destination },
        });
    }
    machine.blocks[0].terminator = Terminator::Conditional {
        condition,
        when_true: SuccessorEdge {
            edge: EdgeId::new(901).unwrap(),
            target: BlockId::new(901).unwrap(),
            arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: EdgeId::new(902).unwrap(),
            target: BlockId::new(902).unwrap(),
            arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    for index in 0..2 {
        machine.blocks.push(Block {
            id: BlockId::new(901 + index).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(903 + index).unwrap(),
                trivial_affine_discards: vec![
                    PlaceId::new(902).unwrap(),
                    PlaceId::new(901).unwrap(),
                ],
            },
        });
    }
    module
}

#[test]
fn entry_local_prefix_has_pathwise_normal_cleanup_and_no_crash_cleanup() {
    let mut module = branched_locals();
    validate_module(&module).expect("both normal paths retain reverse local cleanup");
    module.machines[0].contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
    }];
    module.machines[0].blocks[2].terminator = Terminator::Crash {
        edge: EdgeId::new(904).unwrap(),
        cause: CrashCause::Abort,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    validate_module(&module).expect("live affine locals need no cleanup on crash");
}

#[test]
fn branched_local_prefix_rejects_missing_duplicate_reordered_or_nonentry_establishment() {
    for mutation in 0..4 {
        let mut module = branched_locals();
        let machine = &mut module.machines[0];
        match mutation {
            0 => {
                machine.blocks[0].operations.pop();
            }
            1 => {
                let mut duplicate = machine.blocks[0].operations[0].clone();
                duplicate.id = OperationId::new(903).unwrap();
                machine.blocks[0].operations.push(duplicate);
            }
            2 => machine.blocks[0].operations.reverse(),
            _ => {
                let moved = machine.blocks[0].operations.pop().unwrap();
                machine.blocks[1].operations.push(moved);
            }
        }
        assert!(
            matches!(
                validate_module(&module),
                Err(ModuleError::TrivialAffineLocalEstablishmentMismatch(_))
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn branched_local_cleanup_rejects_missing_reordered_and_double_discard() {
    for mutation in 0..3 {
        let mut module = branched_locals();
        if mutation == 2 {
            let Terminator::Conditional { when_true, .. } =
                &mut module.machines[0].blocks[0].terminator
            else {
                unreachable!()
            };
            when_true
                .trivial_affine_discards
                .push(PlaceId::new(902).unwrap());
        } else {
            let Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } = &mut module.machines[0].blocks[1].terminator
            else {
                unreachable!()
            };
            if mutation == 0 {
                trivial_affine_discards.clear();
            } else {
                trivial_affine_discards.reverse();
            }
        }
        assert!(
            matches!(
                validate_module(&module),
                Err(ModuleError::UnitReturnAffineDiscardsMismatch { .. })
            ),
            "mutation {mutation}"
        );
    }
}
