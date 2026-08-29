//! General unit, dominance, affine-local, and custody tests.

use super::*;

#[test]
fn independently_accepts_builder_output() {
    validate_psi_optimization_unit(&unit()).unwrap();
    validate_psi_optimization_unit(&scalar_call_unit()).unwrap();
    validate_psi_optimization_unit(&scalar_boundary_call_unit()).unwrap();
}

#[test]
fn operation_result_return_accepts_cross_block_dominance_independent_of_storage_order() {
    let candidate = operation_result_cfg_unit(OperationResultCfgShape::DominatingNonTopological);
    assert_ne!(
        candidate.functions[0].blocks[0].id, candidate.functions[0].entry,
        "fixture stores the return block before its dominating producer block"
    );
    validate_psi_optimization_unit(&candidate)
        .expect("CallStructural result dominates the structural return through the CFG");
}

#[test]
fn byte_literal_catalog_and_exact_establishment_correspondence_validate() {
    let baseline = byte_literal_boundary_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("one exact borrowed-view literal establishment validates");

    let mut ordinal_gap = baseline.clone();
    let StructuralPlaceKind::ByteSequenceLiteral {
        declaration_ordinal,
        ..
    } = &mut ordinal_gap.functions[0].structural_places[0].kind
    else {
        panic!("fixture retains its byte-literal place")
    };
    *declaration_ordinal = 1;
    let AbstractOperation::EstablishByteSequenceLiteral { place, .. } =
        &mut ordinal_gap.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with its literal establishment")
    };
    let StructuralPlaceKind::ByteSequenceLiteral {
        declaration_ordinal,
        ..
    } = &mut place.kind
    else {
        panic!("establishment retains its literal declaration")
    };
    *declaration_ordinal = 1;
    refresh_function_derivatives(&mut ordinal_gap, 0);
    assert_eq!(
        validate_psi_optimization_unit(&ordinal_gap),
        Err(
            OptimizationUnitValidationError::NonCanonicalByteSequenceLiterals(id(
                4_600,
                MachineId::new
            ))
        )
    );

    let mut wrong_carrier = baseline.clone();
    wrong_carrier.structural_types[0].shape =
        psi_terminal::StructuralTypeShape::Record { fields: Vec::new() };
    refresh_identity(&mut wrong_carrier);
    assert_eq!(
        validate_psi_optimization_unit(&wrong_carrier),
        Err(
            OptimizationUnitValidationError::ByteSequenceLiteralDeclarationRequiresBorrowedView {
                machine: id(4_600, MachineId::new),
                place: id(4_604, PlaceId::new),
            }
        )
    );

    let expected = OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(id(
        4_600,
        MachineId::new,
    ));
    let mut missing = baseline.clone();
    missing.functions[0].blocks[0].nodes.remove(0);
    refresh_function_derivatives(&mut missing, 0);
    assert_eq!(
        validate_psi_optimization_unit(&missing),
        Err(expected.clone())
    );

    let mut forged_type = baseline.clone();
    let AbstractOperation::EstablishByteSequenceLiteral {
        structural_type, ..
    } = &mut forged_type.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with its literal establishment")
    };
    structural_type.identity.push_str("::forged");
    refresh_function_derivatives(&mut forged_type, 0);
    assert_eq!(
        validate_psi_optimization_unit(&forged_type),
        Err(expected.clone())
    );

    let mut duplicate = baseline;
    let mut second = duplicate.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. } =
        &mut second.operation
    else {
        panic!("fixture begins with its literal establishment")
    };
    *psi_operation = id(4_619, OperationId::new);
    duplicate.functions[0].blocks[0].nodes.insert(1, second);
    refresh_function_derivatives(&mut duplicate, 0);
    assert_eq!(
        validate_psi_optimization_unit(&duplicate),
        Err(
            OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(id(
                4_600,
                MachineId::new
            ))
        )
    );

    let mut two_literals = byte_literal_boundary_unit();
    let second_place = id(4_645, PlaceId::new);
    let second_declaration = psi_terminal::StructuralPlaceDeclaration {
        id: second_place,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 1,
            structural_type: id(4_603, StructuralTypeId::new),
        },
    };
    let mut second = two_literals.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::EstablishByteSequenceLiteral {
        psi_operation,
        place,
        bytes,
        ..
    } = &mut second.operation
    else {
        panic!("fixture begins with its literal establishment")
    };
    *psi_operation = id(4_646, OperationId::new);
    *place = second_declaration;
    *bytes = vec![1, 2, 3];
    two_literals.functions[0]
        .structural_places
        .push(second_declaration);
    two_literals.functions[0]
        .declared_places
        .insert(second_place);
    two_literals.functions[0].blocks[0].nodes.insert(1, second);
    refresh_function_derivatives(&mut two_literals, 0);
    validate_psi_optimization_unit(&two_literals)
        .expect("two dense exact literal witnesses validate independent of use count");
}

#[test]
fn explicit_structural_roots_require_current_cfg_availability() {
    let dominating = byte_literal_dominating_non_topological_unit();
    assert_ne!(
        dominating.functions[0].blocks[0].id, dominating.functions[0].entry,
        "fixture stores the literal use block before its producer block"
    );
    validate_psi_optimization_unit(&dominating)
        .expect("a dominating byte-literal producer is available independent of storage order");

    let local_dominating = explicit_local_dominating_non_topological_unit();
    assert_ne!(
        local_dominating.functions[0].blocks[0].id, local_dominating.functions[0].entry,
        "fixture stores the local cleanup block before its producer block"
    );
    validate_psi_optimization_unit(&local_dominating)
        .expect("a dominating explicit local establishment reaches cleanup");

    let mut same_block = byte_literal_boundary_unit();
    same_block.functions[0].blocks[0].nodes.swap(0, 1);
    refresh_function_derivatives(&mut same_block, 0);
    assert_eq!(
        validate_psi_optimization_unit(&same_block),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(4_600, MachineId::new),
                block: id(4_601, BlockId::new),
                node: 0,
                place: id(4_604, PlaceId::new),
            }
        )
    );

    let sibling = byte_literal_sibling_use_unit();
    assert_eq!(
        validate_psi_optimization_unit(&sibling),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(4_600, MachineId::new),
                block: id(4_613, BlockId::new),
                node: 0,
                place: id(4_604, PlaceId::new),
            }
        )
    );

    let partial = byte_literal_partial_predecessor_unit();
    assert_eq!(
        validate_psi_optimization_unit(&partial),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(4_600, MachineId::new),
                block: id(4_633, BlockId::new),
                node: 0,
                place: id(4_604, PlaceId::new),
            }
        )
    );

    let local_same_block = explicit_local_same_block_use_before_definition_unit();
    assert_eq!(
        validate_psi_optimization_unit(&local_same_block),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(390, MachineId::new),
                block: id(391, BlockId::new),
                node: 0,
                place: id(393, PlaceId::new),
            }
        )
    );

    let local_cleanup = explicit_local_sibling_cleanup_unit();
    assert_eq!(
        validate_psi_optimization_unit(&local_cleanup),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(390, MachineId::new),
                block: id(4_622, BlockId::new),
                node: 0,
                place: id(393, PlaceId::new),
            }
        )
    );
}

#[test]
fn trivial_affine_locals_accept_explicit_and_exact_compressed_witnesses() {
    let compressed = compressed_trivial_affine_return_unit();
    let local_places = compressed.functions[0]
        .structural_places
        .iter()
        .filter_map(|place| {
            matches!(place.kind, StructuralPlaceKind::TrivialAffineLocal { .. }).then_some(place.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(local_places.len(), 2);
    assert!(
        local_places
            .iter()
            .all(|place| !compressed.functions[0].declared_places.contains(place)),
        "compressed no-ABI locals are not executable place roots"
    );
    let node = &compressed.functions[0].blocks[0].nodes[0];
    let O::ReturnStructural {
        psi_edge,
        trivial_affine_locals,
        ..
    } = &node.operation
    else {
        panic!("compressed fixture returns structurally")
    };
    let expected_custody = std::iter::once(PsiProvenance::Edge(*psi_edge))
        .chain(
            trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
        )
        .collect::<Vec<_>>();
    assert_eq!(node.provenance, expected_custody);
    assert_eq!(
        node.fuel
            .iter()
            .map(|settlement| (settlement.site, settlement.units))
            .collect::<Vec<_>>(),
        expected_custody
            .iter()
            .copied()
            .map(|site| (site, 1))
            .collect::<Vec<_>>()
    );
    validate_psi_optimization_unit(&compressed)
        .expect("exact compressed local declarations and reverse cleanup validate");
    validate_psi_optimization_unit(&explicit_trivial_affine_return_unit())
        .expect("an exact executable establishment remains a valid local witness");
}

#[test]
fn trivial_affine_local_catalog_requires_dense_empty_record_declarations() {
    let machine = id(360, MachineId::new);
    let second_local = id(368, PlaceId::new);

    let mut ordinal_gap = compressed_trivial_affine_return_unit();
    let second = ordinal_gap.functions[0]
        .structural_places
        .iter_mut()
        .find(|place| place.id == second_local)
        .expect("second local catalog row");
    let StructuralPlaceKind::TrivialAffineLocal {
        declaration_ordinal,
        ..
    } = &mut second.kind
    else {
        panic!("fixture local has a local kind")
    };
    *declaration_ordinal = 2;
    refresh_identity(&mut ordinal_gap);
    assert_eq!(
        validate_psi_optimization_unit(&ordinal_gap),
        Err(OptimizationUnitValidationError::NonCanonicalTrivialAffineLocals(machine))
    );

    let mut nonempty_carrier = compressed_trivial_affine_return_unit();
    nonempty_carrier.structural_types[0].shape = psi_terminal::StructuralTypeShape::ByteSequence(
        psi_terminal::ByteSequenceCarrier::BorrowedView,
    );
    refresh_identity(&mut nonempty_carrier);
    assert_eq!(
        validate_psi_optimization_unit(&nonempty_carrier),
        Err(
            OptimizationUnitValidationError::TrivialAffineLocalDeclarationRequiresEmptyRecord {
                machine,
                place: id(367, PlaceId::new),
            }
        )
    );

    let mut forged_explicit = explicit_trivial_affine_return_unit();
    let AbstractOperation::EstablishTrivialAffineLocal {
        structural_type, ..
    } = &mut forged_explicit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with an explicit local establishment")
    };
    structural_type.identity.push_str("::forged");
    refresh_node_derivatives(&mut forged_explicit, 0, 0, 0);
    assert_eq!(
        validate_psi_optimization_unit(&forged_explicit),
        Err(
            OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(id(
                390,
                MachineId::new
            ))
        )
    );
}

#[test]
fn compressed_trivial_affine_tuple_is_exact_and_hidden_operations_are_unique() {
    let expected = OptimizationUnitValidationError::StructuralReturnTrivialAffineLocalsMismatch {
        machine: id(360, MachineId::new),
        block: id(361, BlockId::new),
        node: 0,
    };

    let mut missing = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_locals,
        ..
    } = &mut missing.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_locals.pop();
    refresh_node_derivatives(&mut missing, 0, 0, 0);
    assert_eq!(
        validate_psi_optimization_unit(&missing),
        Err(expected.clone())
    );

    let mut extra = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_locals,
        ..
    } = &mut extra.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_locals.push(trivial_affine_locals[0].clone());
    refresh_node_derivatives(&mut extra, 0, 0, 0);
    assert_eq!(
        validate_psi_optimization_unit(&extra),
        Err(expected.clone())
    );

    let mut reordered = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_locals,
        ..
    } = &mut reordered.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_locals.swap(0, 1);
    refresh_node_derivatives(&mut reordered, 0, 0, 0);
    assert_eq!(
        validate_psi_optimization_unit(&reordered),
        Err(expected.clone())
    );

    let mut forged_place = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_locals,
        ..
    } = &mut forged_place.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_locals[0].1.id = id(389, PlaceId::new);
    refresh_node_derivatives(&mut forged_place, 0, 0, 0);
    assert_eq!(
        validate_psi_optimization_unit(&forged_place),
        Err(expected.clone())
    );

    let mut forged_type = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_locals,
        ..
    } = &mut forged_type.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_locals[0].2.identity.push_str("::forged");
    refresh_node_derivatives(&mut forged_type, 0, 0, 0);
    assert_eq!(
        validate_psi_optimization_unit(&forged_type),
        Err(expected.clone())
    );

    let mut duplicate_operation = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_locals,
        ..
    } = &mut duplicate_operation.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_locals[1].0 = trivial_affine_locals[0].0;
    refresh_node_derivatives(&mut duplicate_operation, 0, 0, 0);
    assert_eq!(
        validate_psi_optimization_unit(&duplicate_operation),
        Err(expected.clone())
    );

    let executable_collision = compressed_trivial_affine_return_unit_with_prefix(true, false);
    assert_eq!(
        validate_psi_optimization_unit(&executable_collision),
        Err(
            OptimizationUnitValidationError::StructuralReturnTrivialAffineLocalsMismatch {
                machine: id(360, MachineId::new),
                block: id(361, BlockId::new),
                node: 1,
            }
        )
    );

    let mixed_witnesses = compressed_trivial_affine_return_unit_with_prefix(false, true);
    assert_eq!(
        validate_psi_optimization_unit(&mixed_witnesses),
        Err(
            OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(id(
                360,
                MachineId::new
            ))
        )
    );
}

#[test]
fn retained_affine_authority_rejects_order_and_frontier_corruption() {
    let unit = compressed_trivial_affine_return_unit();
    let function = &unit.functions[0];
    let owned = |place, multiplicity| OwnershipFrontierOwnedPlace {
        place: id(place, PlaceId::new),
        multiplicity,
    };
    let entry = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: vec![
            owned(363, psi_terminal::StructuralMultiplicity::Linear),
            owned(364, psi_terminal::StructuralMultiplicity::Affine),
            owned(365, psi_terminal::StructuralMultiplicity::Affine),
            owned(367, psi_terminal::StructuralMultiplicity::Affine),
            owned(368, psi_terminal::StructuralMultiplicity::Affine),
        ],
        partial_custody: Vec::new(),
    };
    let exit = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: vec![owned(363, psi_terminal::StructuralMultiplicity::Linear)],
        partial_custody: Vec::new(),
    };
    let exact_discards = [
        id(368, PlaceId::new),
        id(367, PlaceId::new),
        id(365, PlaceId::new),
        id(364, PlaceId::new),
    ];
    assert!(valid_edge_affine_transition(
        function,
        &entry,
        &exit,
        &exact_discards,
    ));

    let mut reordered = exact_discards;
    reordered.swap(0, 1);
    assert!(!valid_edge_affine_transition(
        function, &entry, &exit, &reordered,
    ));
    assert!(!valid_edge_affine_transition(
        function,
        &entry,
        &entry,
        &exact_discards,
    ));

    let hidden_entry = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: vec![owned(363, psi_terminal::StructuralMultiplicity::Linear)],
        partial_custody: Vec::new(),
    };
    let hidden_exit = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: vec![
            owned(363, psi_terminal::StructuralMultiplicity::Linear),
            owned(367, psi_terminal::StructuralMultiplicity::Affine),
        ],
        partial_custody: Vec::new(),
    };
    assert!(valid_hidden_affine_establishment(
        &hidden_entry,
        &hidden_exit,
        id(367, PlaceId::new),
    ));
    let mut wrong_hidden_exit = hidden_exit;
    wrong_hidden_exit.owned_places[1].multiplicity =
        psi_terminal::StructuralMultiplicity::Unrestricted;
    assert!(!valid_hidden_affine_establishment(
        &hidden_entry,
        &wrong_hidden_exit,
        id(367, PlaceId::new),
    ));
}

#[test]
fn compressed_trivial_affine_return_requires_exact_shape_and_reverse_discards() {
    let mut wrong_shape = compressed_trivial_affine_return_unit();
    wrong_shape.functions[0].structural_parameters[1].multiplicity =
        psi_terminal::StructuralMultiplicity::Unrestricted;
    refresh_identity(&mut wrong_shape);
    assert_eq!(
        validate_psi_optimization_unit(&wrong_shape),
        Err(
            OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                machine: id(360, MachineId::new),
                block: id(361, BlockId::new),
                node: 0,
            }
        )
    );

    let mut wrong_discards = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut wrong_discards.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_discards.swap(0, 1);
    refresh_node_derivatives(&mut wrong_discards, 0, 0, 0);
    assert_eq!(
        validate_psi_optimization_unit(&wrong_discards),
        Err(
            OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch {
                machine: id(360, MachineId::new),
                block: id(361, BlockId::new),
                node: 0,
            }
        )
    );

    let mut missing_discard = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut missing_discard.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_discards.pop();
    refresh_node_derivatives(&mut missing_discard, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_discard),
        Err(OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch { .. })
    ));

    let mut extra_discard = compressed_trivial_affine_return_unit();
    let AbstractOperation::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut extra_discard.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture is a structural return")
    };
    trivial_affine_discards.push(id(388, PlaceId::new));
    refresh_node_derivatives(&mut extra_discard, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&extra_discard),
        Err(OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch { .. })
    ));
}

#[test]
fn operation_result_return_rejects_sibling_and_partial_predecessor_producers() {
    let call_result = id(381, PlaceId::new);

    let mut sibling = operation_result_cfg_unit(OperationResultCfgShape::SiblingReturn);
    refresh_node_derivatives(&mut sibling, 0, 2, 0);
    assert_eq!(
        validate_psi_optimization_unit(&sibling),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(370, MachineId::new),
                block: id(374, BlockId::new),
                node: 0,
                place: call_result,
            }
        )
    );

    let mut partial = operation_result_cfg_unit(OperationResultCfgShape::PartialPredecessor);
    refresh_node_derivatives(&mut partial, 0, 3, 0);
    assert_eq!(
        validate_psi_optimization_unit(&partial),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(370, MachineId::new),
                block: id(375, BlockId::new),
                node: 0,
                place: call_result,
            }
        )
    );
}
