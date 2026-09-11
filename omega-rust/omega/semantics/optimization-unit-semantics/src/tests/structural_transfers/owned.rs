use super::*;
use terminal_psi::TerminalAffineCleanupAction;

pub(super) fn owned_unit() -> PsiOptimizationUnit {
    let mut candidate = transfer_unit();
    candidate.structural_types.make_mut()[0].shape =
        StructuralTypeShape::Record { fields: Vec::new() };
    let function = &mut candidate.functions[0];
    for parameter in function.structural_parameters.iter_mut().chain(
        function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.structural_parameters),
    ) {
        parameter.access = StructuralAccess::Owned;
        parameter.multiplicity = StructuralMultiplicity::Affine;
    }
    let mut source = function.structural_parameters[0].clone();
    source.place = id(30, PlaceId::new);
    source.position = 1;
    function.structural_parameters.push(source.clone());
    let mut destination = source;
    destination.place = id(31, PlaceId::new);
    function.blocks[1].structural_parameters.push(destination);
    function.structural_places.extend([
        terminal_psi::StructuralPlaceDeclaration {
            id: id(30, PlaceId::new),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: id(31, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(3, BlockId::new),
                position: 1,
            },
        },
    ]);
    function
        .declared_places
        .extend([id(30, PlaceId::new), id(31, PlaceId::new)]);
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut function.blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[0].argument.access = StructuralAccess::Owned;
    structural_bindings.push(AbstractStructuralBinding {
        parameter: id(31, PlaceId::new),
        argument: StructuralArgument {
            place: id(30, PlaceId::new),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
    });
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    function.blocks[1].nodes[0].operation = AbstractOperation::IntegerConstant {
        psi_operation: id(21, OperationId::new),
        result: id(13, ValueId::new),
        scalar_type,
        value: IntegerValue::Unsigned(7),
    };
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut function.blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    *cleanup_actions = vec![
        TerminalAffineCleanupAction::DiscardRoot(id(31, PlaceId::new)),
        TerminalAffineCleanupAction::DiscardRoot(id(11, PlaceId::new)),
    ];
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    refresh_node_derivatives(&mut candidate, 0, 1, 0);
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    candidate
}

#[test]
fn whole_owned_arrivals_replace_the_consumed_roots() {
    validate_psi_optimization_unit(&owned_unit()).unwrap();
}

#[test]
fn owned_transfer_and_discard_cannot_consume_the_same_root() {
    let mut candidate = owned_unit();
    let AbstractOperation::Jump {
        trivial_affine_discards,
        ..
    } = &mut candidate.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    trivial_affine_discards.push(id(10, PlaceId::new));
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn owned_arrival_cannot_duplicate_one_source() {
    let mut candidate = owned_unit();
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut candidate.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[1].argument.place = structural_bindings[0].argument.place;
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn owned_arrival_cleanup_is_exact_and_ordered() {
    for mutation in 0..4 {
        let mut candidate = owned_unit();
        let AbstractOperation::Return {
            cleanup_actions, ..
        } = &mut candidate.functions[0].blocks[1].nodes[1].operation
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                cleanup_actions.pop();
            }
            1 => cleanup_actions.swap(0, 1),
            2 => {
                cleanup_actions[0] = TerminalAffineCleanupAction::DiscardRoot(id(30, PlaceId::new))
            }
            3 => cleanup_actions.push(cleanup_actions[0].clone()),
            _ => unreachable!(),
        }
        refresh_node_derivatives(&mut candidate, 0, 1, 1);
        assert!(
            matches!(
                validate_psi_optimization_unit(&candidate),
                Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn owned_arrival_signature_cannot_change_multiplicity_or_access() {
    for mutation in 0..3 {
        let mut candidate = owned_unit();
        let parameter = &mut candidate.functions[0].blocks[1].structural_parameters[0];
        match mutation {
            0 => parameter.multiplicity = StructuralMultiplicity::Unrestricted,
            1 => parameter.multiplicity = StructuralMultiplicity::Linear,
            2 => parameter.access = StructuralAccess::SharedBorrow,
            _ => unreachable!(),
        }
        refresh_identity(&mut candidate);
        assert!(
            validate_psi_optimization_unit(&candidate).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn nested_owned_arrivals_dispose_by_dominance_not_serialized_block_order() {
    let mut candidate = owned_unit();
    let function = &mut candidate.functions[0];
    let mut continuation = function.blocks[1].clone();
    continuation.id = id(1, BlockId::new);
    let mut parameter = continuation.structural_parameters[1].clone();
    parameter.place = id(41, PlaceId::new);
    parameter.position = 0;
    continuation.structural_parameters = vec![parameter];
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut continuation.nodes[1].operation
    else {
        unreachable!()
    };
    cleanup_actions[0] = TerminalAffineCleanupAction::DiscardRoot(id(41, PlaceId::new));
    function.blocks[1].nodes.truncate(1);
    function.blocks[1].nodes[0].operation = AbstractOperation::Jump {
        psi_edge: id(42, EdgeId::new),
        target: continuation.id,
        bindings: Vec::new(),
        structural_bindings: vec![AbstractStructuralBinding {
            parameter: id(41, PlaceId::new),
            argument: StructuralArgument {
                place: id(31, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: id(41, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter {
                block: continuation.id,
                position: 0,
            },
        });
    function.declared_places.insert(id(41, PlaceId::new));
    function.blocks.push(continuation);
    refresh_node_derivatives(&mut candidate, 0, 1, 0);
    refresh_node_derivatives(&mut candidate, 0, 2, 0);
    refresh_node_derivatives(&mut candidate, 0, 2, 1);
    candidate.functions[0].blocks[1].nodes[0].fuel.clear();
    refresh_effects(&mut candidate);
    validate_psi_optimization_unit(&candidate).unwrap();
    candidate.functions[0].blocks.swap(1, 2);
    refresh_effects(&mut candidate);
    validate_psi_optimization_unit(&candidate).unwrap();
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut candidate.functions[0].blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    cleanup_actions.swap(0, 1);
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

fn refresh_effects(candidate: &mut PsiOptimizationUnit) {
    for (ordinal, node) in candidate.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .enumerate()
    {
        node.effect = optimization_unit::EffectLink {
            input: ordinal as u64,
            output: ordinal as u64 + 1,
        };
    }
    refresh_identity(candidate);
}
