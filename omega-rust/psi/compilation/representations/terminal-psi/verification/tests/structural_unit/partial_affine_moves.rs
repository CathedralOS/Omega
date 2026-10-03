use super::{
    boundary_call_mut, content_entry_claim, content_owner_projection, hard_root_module,
    mixed_direct_nested_partial_affine_field_module, multiple_move_partial_affine_field_module,
    multiple_nested_partial_affine_field_module, nested_partial_affine_field_module,
    partial_affine_field_module, projected_boundary_qualification_module, structural_parameter,
    unit_call_mut,
};
use crate::structural_unit::{
    block_id, boundary_id, claim_id, contract_id, domain_id, edge_id, machine_id, obligation_id,
    operation_id, place_id, service_id, structural_type_id, value_id,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentPlaceSegment, ContentPlaceVersion,
    ContentStructuralPlace, ContentTerm, Proposition, ScalarType, StructuralPlaceKind, ValueId,
};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, ContractClause, EntryClaim, Operation, OperationKind,
    OperationResult, ServiceDeclaration, StructuralAccess, StructuralArgument,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralMultiplicity,
    StructuralPathSegment, StructuralPlaceDeclaration, TerminalAffineCleanupAction,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
};
use terminal_verifier::{
    ModuleError, ProofBundle, ServiceCeilingOwner, validate_module, verify_module,
};

#[test]
fn predicate_byte_siblings_do_not_change_the_partial_affine_complement() {
    let mut module = multiple_move_partial_affine_field_module();
    let mut predicates = terminal_psi::ByteSequencePredicateSet::EMPTY;
    predicates.insert(terminal_psi::ByteSequencePredicate::ValidUtf8);
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut module.structural_types[1].shape
    else {
        panic!("fixture root is a record")
    };
    fields.insert(
        1,
        terminal_psi::StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(4).unwrap(),
            identity: "label".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: terminal_psi::StructuralFieldType::PredicateByteSequence {
                carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 3 },
                predicates,
            },
        },
    );
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("bounded byte predicates add no cleanup obligation");

    let mut borrowed = module.clone();
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut borrowed.structural_types[1].shape
    else {
        unreachable!()
    };
    fields[1].field_type = terminal_psi::StructuralFieldType::PredicateByteSequence {
        carrier: terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        },
        predicates,
    };
    assert!(
        verify_module(
            &borrowed,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "a predicate must not let borrowed storage masquerade as a bounded owner"
    );

    let mut missing_residual = module.clone();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut missing_residual.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.clear();
    assert!(
        verify_module(
            &missing_residual,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "cleanup-free bytes do not erase a structural sibling's residual obligation"
    );
}

#[test]
fn multiple_direct_field_partial_affine_moves_validate_and_verify() {
    let module = multiple_move_partial_affine_field_module();
    let caller = &module.machines[0];
    assert_eq!(
        caller.blocks[0]
            .operations
            .iter()
            .map(|operation| {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &operation.kind
                else {
                    panic!("expected direct-field Unit call")
                };
                let [StructuralArgument { path, .. }] = structural_arguments.as_slice() else {
                    panic!("expected one structural argument")
                };
                let [StructuralPathSegment::Field(identity)] = path.as_slice() else {
                    panic!("expected one direct field")
                };
                identity.as_str()
            })
            .collect::<Vec<_>>(),
        vec!["right", "middle"]
    );
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &caller.blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| match discard.path.as_slice() {
                [StructuralPathSegment::Field(identity)] => identity.as_str(),
                _ => panic!("expected one direct field residual"),
            })
            .collect::<Vec<_>>(),
        vec!["left"]
    );
    validate_module(&module)
        .expect("distinct moved fields plus their exact complement exhaust root");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("multiple partial affine moves introduce no producer-authored proposition");
}

#[test]
fn multiple_direct_field_partial_affine_moves_reject_duplicates_and_exhaustion() {
    let expected = |module: &TerminalModule| ModuleError::InvalidPartialAffineCleanup {
        machine: module.machines[0].id,
        block: module.machines[0].blocks[0].id,
    };

    let mut duplicate = multiple_move_partial_affine_field_module();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut duplicate.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("right".into())];
    assert_eq!(
        validate_module(&duplicate).unwrap_err(),
        expected(&duplicate)
    );

    let mut exhaustive = multiple_move_partial_affine_field_module();
    let mut final_call = exhaustive.machines[0].blocks[0].operations[1].clone();
    final_call.id = operation_id(3);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut final_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("left".into())];
    exhaustive.machines[0].blocks[0].operations.push(final_call);
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut exhaustive.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.clear();
    assert_eq!(
        validate_module(&exhaustive).unwrap_err(),
        expected(&exhaustive)
    );
}

#[test]
fn one_nested_partial_affine_move_validates_recursive_residual_order() {
    let module = nested_partial_affine_field_module();
    let caller = &module.machines[0];
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &caller.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    assert_eq!(
        structural_arguments[0].path,
        vec![
            StructuralPathSegment::Field("nested".into()),
            StructuralPathSegment::Field("middle".into()),
        ]
    );
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &caller.blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![StructuralPathSegment::Field("right".into())],
            vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("right".into()),
            ],
            vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("left".into()),
            ],
            vec![StructuralPathSegment::Field("left".into())],
        ]
    );
    validate_module(&module).expect("one nested move has an exact maximal residual partition");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("nested partial affine cleanup introduces no authored proposition");
}

#[test]
fn multiple_nested_partial_affine_moves_share_prefixes_and_mix_with_direct_moves() {
    let shared_prefix = multiple_nested_partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &shared_prefix.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![StructuralPathSegment::Field("right".into())],
            vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("right".into()),
            ],
            vec![StructuralPathSegment::Field("left".into())],
        ]
    );
    validate_module(&shared_prefix)
        .expect("two leaves under one nested subtree retain its maximal complement");
    verify_module(
        &shared_prefix,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("shared-prefix nested moves verify independently of source facts");

    let mixed = mixed_direct_nested_partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mixed.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("right".into()),
            ],
            vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("left".into()),
            ],
            vec![StructuralPathSegment::Field("left".into())],
        ]
    );
    validate_module(&mixed).expect("a direct and nested move have one exact residual forest");
    verify_module(
        &mixed,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("mixed-depth moves verify independently of source facts");
}

#[test]
fn nested_partial_affine_move_rejects_forged_residual_subtrees() {
    let expected = |module: &TerminalModule| ModuleError::InvalidPartialAffineCleanup {
        machine: module.machines[0].id,
        block: module.machines[0].blocks[0].id,
    };

    let mut missing = nested_partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut missing.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.remove(1);
    assert_eq!(validate_module(&missing).unwrap_err(), expected(&missing));

    let mut ancestor = nested_partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut ancestor.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[1].path = vec![StructuralPathSegment::Field("nested".into())];
    residual_affine_discards[1].structural_type = structural_type_id(4);
    assert_eq!(validate_module(&ancestor).unwrap_err(), expected(&ancestor));

    let mut descendant = nested_partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut descendant.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[0]
        .path
        .push(StructuralPathSegment::Field("missing".into()));
    assert_eq!(
        validate_module(&descendant).unwrap_err(),
        expected(&descendant)
    );

    let mut reordered = nested_partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.swap(1, 2);
    assert_eq!(
        validate_module(&reordered).unwrap_err(),
        expected(&reordered)
    );

    let mut wrong_type = nested_partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut wrong_type.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[1].structural_type = structural_type_id(4);
    assert_eq!(
        validate_module(&wrong_type).unwrap_err(),
        expected(&wrong_type)
    );

    let mut extra = nested_partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut extra.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.push(residual_affine_discards[0].clone());
    assert_eq!(validate_module(&extra).unwrap_err(), expected(&extra));
}

#[test]
fn nested_partial_affine_move_rejects_path_overlap_in_either_order() {
    let expected = |module: &TerminalModule| ModuleError::InvalidPartialAffineCleanup {
        machine: module.machines[0].id,
        block: module.machines[0].blocks[0].id,
    };

    let mut overlapping = nested_partial_affine_field_module();
    let inner_type = structural_type_id(4);
    let mut ancestor_callee = overlapping.machines[1].clone();
    ancestor_callee.id = machine_id(3);
    ancestor_callee.structural_parameters[0].structural_type = inner_type;
    ancestor_callee.entry = block_id(3);
    ancestor_callee.blocks[0].id = block_id(3);
    ancestor_callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: vec![place_id(2)],
    };
    ancestor_callee.contract.id = contract_id(3);
    let mut ancestor_call = overlapping.machines[0].blocks[0].operations[0].clone();
    ancestor_call.id = operation_id(2);
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        ..
    } = &mut ancestor_call.kind
    else {
        unreachable!()
    };
    *callee = ancestor_callee.id;
    structural_arguments[0].path = vec![StructuralPathSegment::Field("nested".into())];
    overlapping.machines[0].blocks[0]
        .operations
        .insert(0, ancestor_call);
    overlapping.machines.push(ancestor_callee);
    assert_eq!(
        validate_module(&overlapping).unwrap_err(),
        expected(&overlapping)
    );

    let mut reverse_overlap = overlapping.clone();
    reverse_overlap.machines[0].blocks[0].operations.swap(0, 1);
    assert_eq!(
        validate_module(&reverse_overlap).unwrap_err(),
        expected(&reverse_overlap)
    );
}

#[test]
fn direct_field_partial_affine_return_rejects_forged_conservation_shapes() {
    let expected = |module: &TerminalModule| ModuleError::InvalidPartialAffineCleanup {
        machine: module.machines[0].id,
        block: module.machines[0].blocks[0].id,
    };

    let mut missing = partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut missing.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.clear();
    assert_eq!(validate_module(&missing).unwrap_err(), expected(&missing));

    let mut extra = partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut extra.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.push(residual_affine_discards[0].clone());
    assert_eq!(validate_module(&extra).unwrap_err(), expected(&extra));

    let mut wrong_path = partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut wrong_path.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[0].path = vec![StructuralPathSegment::Field("missing".into())];
    assert_eq!(
        validate_module(&wrong_path).unwrap_err(),
        expected(&wrong_path)
    );

    let mut wrong_type = partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut wrong_type.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[0].structural_type = structural_type_id(3);
    assert_eq!(
        validate_module(&wrong_type).unwrap_err(),
        expected(&wrong_type)
    );

    let mut same_field = partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut same_field.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[0].path = vec![StructuralPathSegment::Field("right".into())];
    assert_eq!(
        validate_module(&same_field).unwrap_err(),
        expected(&same_field)
    );

    let mut wrong_order = partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut wrong_order.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.swap(0, 1);
    assert_eq!(
        validate_module(&wrong_order).unwrap_err(),
        expected(&wrong_order)
    );

    let mut reordered_root_cleanup = partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        trivial_affine_discards,
        ..
    } = &mut reordered_root_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.push(place_id(1));
    assert_eq!(
        validate_module(&reordered_root_cleanup).unwrap_err(),
        expected(&reordered_root_cleanup)
    );

    let mut claim_overlap = partial_affine_field_module();
    claim_overlap.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(1),
        input: place_id(1),
        path: vec![StructuralPathSegment::Field("left".into())],
    });
    assert!(matches!(
        validate_module(&claim_overlap),
        Err(ModuleError::UnitCallClaimPresenceMismatch {
            operation,
            argument_index: 0,
        }) if operation == operation_id(1)
    ));
}

#[test]
fn content_only_affine_claims_require_explicit_transfer_and_settlement() {
    let mut module = hard_root_module();
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims.clear();
    }
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    module.machines[0].content_entry_claims = vec![content_entry_claim(place_id(1))];
    module.machines[1].content_entry_claims = vec![content_entry_claim(place_id(2))];
    validate_module(&module).expect("content-only affine custody transfers and settles explicitly");

    let mut untransferred = module.clone();
    unit_call_mut(&mut untransferred).clear();
    assert_eq!(
        validate_module(&untransferred).unwrap_err(),
        ModuleError::UnitCallClaimTransferCountMismatch {
            operation: operation_id(1),
            expected: 1,
            actual: 0,
        }
    );

    let mut unsettled = module.clone();
    boundary_call_mut(&mut unsettled).clear();
    assert_eq!(
        validate_module(&unsettled).unwrap_err(),
        ModuleError::BoundaryCompletionReceiptMismatch(operation_id(3))
    );

    for machine in &mut module.machines {
        machine.content_entry_claims[0].input.segments =
            vec![ContentPlaceSegment::Field("left".to_owned())];
        let mut second = content_entry_claim(machine.structural_parameters[0].place);
        second.claim = claim_id(2);
        second.input.segments = vec![ContentPlaceSegment::Field("right".to_owned())];
        machine.content_entry_claims.push(second);
    }
    unit_call_mut(&mut module).push(ClaimTransfer {
        claim: claim_id(2),
        argument_index: 0,
    });
    boundary_call_mut(&mut module).push(CompletionReceipt {
        claim: claim_id(2),
        argument_index: 0,
    });
    validate_module(&module)
        .expect("disjoint content claims may share one structural argument and transfer exactly");

    let mut reordered_transfers = module.clone();
    unit_call_mut(&mut reordered_transfers).swap(0, 1);
    assert_eq!(
        validate_module(&reordered_transfers).unwrap_err(),
        ModuleError::NonCanonicalUnitCallClaimTransfers(operation_id(1))
    );

    let mut reordered_settlements = module;
    boundary_call_mut(&mut reordered_settlements).swap(0, 1);
    assert_eq!(
        validate_module(&reordered_settlements).unwrap_err(),
        ModuleError::NonCanonicalBoundaryCompletionReceipts(operation_id(3))
    );
}

#[test]
fn content_claim_identity_cannot_refine_a_different_structural_root() {
    let mut module = hard_root_module();
    let mut second_parameter = structural_parameter(place_id(10));
    second_parameter.position = 1;
    second_parameter.multiplicity = StructuralMultiplicity::Affine;
    module.machines[0]
        .structural_parameters
        .push(second_parameter);
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(10),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    module.machines[0].content_entry_claims = vec![content_entry_claim(place_id(10))];
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::ContentEntryClaimStructuralBindingMismatch(claim_id(1))
    );
}

#[test]
fn unit_call_content_custody_must_name_a_structural_argument() {
    let mut module = hard_root_module();
    module.machines[1].structural_parameters.clear();
    module.machines[1].entry_claims.clear();
    module.machines[1].content_entry_claims = vec![content_entry_claim(place_id(2))];
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments.clear();
    claim_transfers.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::UnitCallClaimHasNoStructuralArgument {
            operation: operation_id(1),
            claim: claim_id(1),
        }
    );
}

#[test]
fn unit_call_contract_content_must_name_a_structural_argument() {
    let mut module = hard_root_module();
    module.machines[1].structural_parameters.clear();
    module.machines[1].entry_claims.clear();
    let subject = ContentTerm::Projection {
        projection: content_owner_projection().identity,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: place_id(2),
            segments: Vec::new(),
        },
    };
    module.machines[1].contract.ensures = vec![ContractClause {
        obligation: obligation_id(1),
        proposition: Proposition::ContentConservation(
            semantic_vocabulary::ContentConservation::new(
                ContentAlgebra {
                    kind: ContentAlgebraKind::CountedQuantity,
                    parameter: "Acknowledgement".to_owned(),
                },
                subject.clone(),
                subject,
            ),
        ),
    }];
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments.clear();
    claim_transfers.clear();

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::UnitCallContractPlaceHasNoArgument {
            operation: operation_id(1),
            callee: machine_id(2),
            place: place_id(2),
        }
    );
}

#[test]
fn boundary_call_checks_qualification_and_settlement() {
    let mut missing_qualification = hard_root_module();
    missing_qualification.machines[1].structural_parameters[0]
        .qualifications
        .clear();
    assert_eq!(
        validate_module(&missing_qualification).unwrap_err(),
        ModuleError::BoundaryArgumentMissingQualification {
            operation: operation_id(3),
            argument_index: 0,
            domain: domain_id(1),
        }
    );

    let mut missing_settlement = hard_root_module();
    boundary_call_mut(&mut missing_settlement).clear();
    assert_eq!(
        validate_module(&missing_settlement).unwrap_err(),
        ModuleError::BoundaryCompletionReceiptMismatch(operation_id(3))
    );
}

#[test]
fn boundary_requirement_consumes_only_the_exact_projected_qualification() {
    let module = projected_boundary_qualification_module();
    validate_module(&module).expect("exact parameter-rooted field qualification");

    let mut missing = module.clone();
    missing.machines[0].structural_parameters[0]
        .projected_qualifications
        .clear();
    assert_eq!(
        validate_module(&missing).unwrap_err(),
        ModuleError::BoundaryArgumentMissingQualification {
            operation: operation_id(1),
            argument_index: 0,
            domain: domain_id(1),
        }
    );

    let mut sibling = module.clone();
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut sibling.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("right".into())];
    assert_eq!(
        validate_module(&sibling).unwrap_err(),
        ModuleError::BoundaryArgumentMissingQualification {
            operation: operation_id(1),
            argument_index: 0,
            domain: domain_id(1),
        }
    );

    let mut empty = module.clone();
    empty.machines[0].structural_parameters[0].projected_qualifications[0]
        .path
        .clear();
    assert!(matches!(
        validate_module(&empty),
        Err(ModuleError::InvalidProjectedStructuralQualificationPath { .. })
    ));

    let mut duplicate = module;
    let row = duplicate.machines[0].structural_parameters[0].projected_qualifications[0].clone();
    duplicate.machines[0].structural_parameters[0]
        .projected_qualifications
        .push(row);
    assert_eq!(
        validate_module(&duplicate).unwrap_err(),
        ModuleError::NonCanonicalProjectedStructuralQualifications(place_id(1)),
    );
}

#[test]
fn claims_are_linear_across_unit_operations_and_return() {
    let mut reused = hard_root_module();
    reused.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(4),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                access: StructuralAccess::Owned,
                path: Vec::new(),
            }],
            completion_receipts: vec![CompletionReceipt {
                claim: claim_id(1),
                argument_index: 0,
            }],
        },
    });
    assert_eq!(
        validate_module(&reused).unwrap_err(),
        ModuleError::ClaimNotLiveAtOperation {
            operation: operation_id(4),
            claim: claim_id(1),
        }
    );

    let mut leaked = hard_root_module();
    leaked.machines[1].blocks[0].operations.truncate(1);
    assert_eq!(
        validate_module(&leaked).unwrap_err(),
        ModuleError::LiveLinearClaimAtUnitReturn {
            machine: machine_id(2),
            block: block_id(2),
            claim: claim_id(1),
        }
    );
}

#[test]
fn scalar_return_cannot_abandon_linear_structural_custody() {
    let mut module = hard_root_module();
    let scalar = module.machines.remove(1);
    module.machines = vec![scalar];
    module.entry = machine_id(2);
    module.root_service_reach.concrete.clear();
    let value = ValueId::new(1).expect("scalar value");
    let result = ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(2).expect("result value"),
        scalar_type: ScalarType::Boolean,
    };
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(result);
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(2),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value,
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    }];
    machine.blocks[0].terminator = Terminator::Return {
        cleanup_actions: Vec::new(),
        edge: edge_id(2),
        value,
    };

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::LiveLinearClaimAtScalarReturn {
            machine: machine_id(2),
            block: block_id(2),
            claim: claim_id(1),
        }
    );

    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    module.machines[0].entry_claims.clear();
    module.machines[0].blocks[0].terminator = Terminator::Return {
        cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(2))],
        edge: edge_id(2),
        value,
    };
    validate_module(&module).expect("affine custody has an explicit scalar-return cleanup");
}

#[test]
fn entry_claims_are_dense_in_each_machine_local_namespace() {
    let mut module = hard_root_module();
    assert_eq!(module.machines[0].entry_claims[0].claim, claim_id(1));
    assert_eq!(module.machines[1].entry_claims[0].claim, claim_id(1));
    validate_module(&module).expect("each machine starts its claim namespace at one");

    module.machines[1].entry_claims[0].claim = claim_id(2);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::NonDenseStructuralEntryClaim {
            machine: machine_id(2),
            expected: claim_id(1),
            actual: claim_id(2),
        }
    );
}

#[test]
fn structural_semantic_sets_have_one_canonical_order() {
    let second_domain = StructuralDomainDeclaration {
        byte_predicate: None,
        establishment_routes: Vec::new(),
        id: domain_id(2),
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(2).unwrap(),
        identity: "Ready".into(),
        carrier: structural_type_id(1),
        content_projection: None,
    };

    let mut qualifications = hard_root_module();
    qualifications
        .structural_domains
        .push(second_domain.clone());
    qualifications.machines[0].structural_parameters[0].qualifications =
        vec![domain_id(2), domain_id(1)];
    assert_eq!(
        validate_module(&qualifications).unwrap_err(),
        ModuleError::NonCanonicalStructuralQualifications(place_id(1))
    );

    let mut requirements = hard_root_module();
    requirements.structural_domains.push(second_domain);
    requirements.boundary_machines[0].requires = vec![
        StructuralDomainRequirement {
            argument_index: 0,
            domain: domain_id(2),
        },
        StructuralDomainRequirement {
            argument_index: 0,
            domain: domain_id(1),
        },
    ];
    assert_eq!(
        validate_module(&requirements).unwrap_err(),
        ModuleError::NonCanonicalBoundaryRequirements(boundary_id(1))
    );

    let second_service = ServiceDeclaration {
        id: service_id(2),
        identity: "DebugIo".into(),
        parents: Vec::new(),
    };
    let mut ceiling = hard_root_module();
    ceiling.services.push(second_service.clone());
    ceiling.machines[0].published_service_ceiling = vec![service_id(2), service_id(1)];
    assert_eq!(
        validate_module(&ceiling).unwrap_err(),
        ModuleError::NonCanonicalPublishedServiceCeiling(ServiceCeilingOwner::Machine(machine_id(
            1
        )))
    );

    let mut parents = hard_root_module();
    parents.services.push(second_service);
    parents.services.push(ServiceDeclaration {
        id: service_id(3),
        identity: "RootIo".into(),
        parents: vec![service_id(2), service_id(1)],
    });
    assert_eq!(
        validate_module(&parents).unwrap_err(),
        ModuleError::NonCanonicalServiceParents(service_id(3))
    );
}

#[test]
fn unit_return_requires_exact_reverse_order_affine_discards() {
    let mut module = hard_root_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.blocks[0].operations.clear();
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    let mut second_parameter = structural_parameter(place_id(4));
    second_parameter.position = 1;
    second_parameter.multiplicity = StructuralMultiplicity::Affine;
    machine.structural_parameters.push(second_parameter);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(4),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![place_id(4), place_id(2)];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach.concrete.clear();
    validate_module(&module).expect("complete reverse-order affine cleanup should validate");

    let mut missing = module.clone();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut missing.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.pop();
    assert_eq!(
        validate_module(&missing).unwrap_err(),
        ModuleError::UnitReturnAffineDiscardsMismatch {
            machine: machine_id(2),
            block: block_id(2),
        }
    );

    let mut reordered = module.clone();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.reverse();
    assert_eq!(
        validate_module(&reordered).unwrap_err(),
        ModuleError::UnitReturnAffineDiscardsMismatch {
            machine: machine_id(2),
            block: block_id(2),
        }
    );

    let mut unknown = module;
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut unknown.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards[0] = place_id(99);
    assert_eq!(
        validate_module(&unknown).unwrap_err(),
        ModuleError::UnitReturnAffineDiscardsMismatch {
            machine: machine_id(2),
            block: block_id(2),
        }
    );
}

#[test]
fn scalar_return_requires_exact_affine_discards() {
    let mut module = hard_root_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.blocks[0].operations.clear();
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    let mut second_parameter = structural_parameter(place_id(4));
    second_parameter.position = 1;
    second_parameter.multiplicity = StructuralMultiplicity::Affine;
    machine.structural_parameters.push(second_parameter);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(4),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    machine.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(10),
        cleanup_actions: vec![
            TerminalAffineCleanupAction::DiscardRoot(place_id(4)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
        ],
    };
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach.concrete.clear();
    validate_module(&module).expect("scalar return should validate exact affine cleanup");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("no-code cleanup adds no proof-bundle obligation");

    let mut omitted = module.clone();
    omitted.machines[0].blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(10),
        cleanup_actions: Vec::new(),
    };
    assert_eq!(
        validate_module(&omitted).unwrap_err(),
        ModuleError::ScalarReturnAffineDiscardsMismatch {
            machine: machine_id(2),
            block: block_id(2),
        }
    );

    let mut reordered = module.clone();
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanup_actions.reverse();
    assert_eq!(
        validate_module(&reordered).unwrap_err(),
        ModuleError::ScalarReturnAffineDiscardsMismatch {
            machine: machine_id(2),
            block: block_id(2),
        }
    );

    let mut claim_bearing = module;
    claim_bearing.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(1),
        input: place_id(2),
        path: Vec::new(),
    });
    assert_eq!(
        validate_module(&claim_bearing).unwrap_err(),
        ModuleError::ScalarReturnAffineDiscardsMismatch {
            machine: machine_id(2),
            block: block_id(2),
        }
    );
}

fn projected_constructor_module(case: bool, descriptor_child: bool) -> TerminalModule {
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, RecordFieldInitializer, RecordFieldValue,
        StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
        StructuralOperationResult, StructuralTypeDeclaration, StructuralTypeShape,
    };
    let mut module = multiple_move_partial_affine_field_module();
    let child_type = if descriptor_child {
        let child = structural_type_id(5);
        module.structural_types.push(StructuralTypeDeclaration {
            id: child,
            identity: "RetainedBytes".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(5).unwrap(),
                    identity: "bytes".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::SharedBorrow),
                        },
                    ),
                }],
            },
        });
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
            unreachable!()
        };
        fields[2].field_type = StructuralFieldType::Structural(child);
        child
    } else {
        structural_type_id(1)
    };
    let field = StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(4).unwrap(),
        identity: "kept".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(child_type),
    };
    let case_id = semantic_vocabulary::StructuralCaseId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "Replacement".into(),
        shape: if case {
            StructuralTypeShape::Sum {
                cases: vec![StructuralCaseDeclaration {
                    id: case_id,
                    identity: "Kept".into(),
                    fields: vec![field.clone()],
                }],
            }
        } else {
            StructuralTypeShape::Record {
                fields: vec![field.clone()],
            }
        },
    });
    let caller = &mut module.machines[0];
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &caller.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let fields = vec![RecordFieldInitializer {
        field: field.id,
        value: RecordFieldValue::Structural(structural_arguments[0].clone()),
    }];
    caller.blocks[0].operations[0].kind = if case {
        OperationKind::EstablishStructuralCase {
            result_case: case_id,
            fields,
        }
    } else {
        OperationKind::EstablishRecord { fields }
    };
    caller.blocks[0].operations[0].result =
        OperationResult::Structural(StructuralOperationResult {
            place: place_id(4),
            structural_type: structural_type_id(4),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(4),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(1),
            structural_type: structural_type_id(4),
        },
    });
    let Terminator::ReturnUnitPartialAffine {
        trivial_affine_discards,
        ..
    } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.push(place_id(4));
    module
}

#[test]
fn projected_constructor_operands_preserve_siblings_and_exact_residuals() {
    for case in [false, true] {
        for descriptor_child in [false, true] {
            let module = projected_constructor_module(case, descriptor_child);
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .expect("constructor moves one child, then an ordinary call moves its live sibling");

            let mut retargeted = module.clone();
            let (OperationKind::EstablishRecord { fields }
            | OperationKind::EstablishStructuralCase { fields, .. }) =
                &mut retargeted.machines[0].blocks[0].operations[0].kind
            else {
                unreachable!()
            };
            let terminal_psi::RecordFieldValue::Structural(argument) = &mut fields[0].value else {
                unreachable!()
            };
            argument.path = vec![StructuralPathSegment::Field("left".into())];
            assert!(
                validate_module(&retargeted).is_err(),
                "the retained complement cannot describe a different selected child"
            );

            let mut repeated = module.clone();
            let OperationKind::CallUnit {
                structural_arguments,
                ..
            } = &mut repeated.machines[0].blocks[0].operations[1].kind
            else {
                unreachable!()
            };
            structural_arguments[0].path = vec![StructuralPathSegment::Field("right".into())];
            assert!(
                validate_module(&repeated).is_err(),
                "constructor transfer makes the selected child unavailable"
            );

            let mut missing = module.clone();
            let Terminator::ReturnUnitPartialAffine {
                residual_affine_discards,
                ..
            } = &mut missing.machines[0].blocks[0].terminator
            else {
                unreachable!()
            };
            residual_affine_discards.clear();
            assert!(
                validate_module(&missing).is_err(),
                "constructing a result does not dispose the source complement"
            );

            let mut borrowed = module;
            borrowed.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
            assert!(
                validate_module(&borrowed).is_err(),
                "a shared root cannot donate owned constructor storage"
            );
        }
    }
}

#[test]
fn projected_constructor_operands_keep_parameter_and_result_cleanup_separate() {
    use terminal_psi::{
        RecordFieldInitializer, RecordFieldValue, StructuralAffineDiscard,
        StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
        StructuralTypeShape,
    };
    let mut module = projected_constructor_module(false, false);
    let extra = StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(6).unwrap(),
        identity: "extra".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(1)),
    };
    let StructuralTypeShape::Record { fields } =
        &mut module.structural_types.last_mut().unwrap().shape
    else {
        unreachable!()
    };
    fields.push(extra.clone());
    let kept = StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(7).unwrap(),
        identity: "kept".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(1)),
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(6),
        identity: "FinalOwner".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![kept.clone()],
        },
    });
    let caller = &mut module.machines[0];
    let mut establish_extra = caller.blocks[0].operations[0].clone();
    establish_extra.id = operation_id(3);
    let OperationResult::Structural(result) = &mut establish_extra.result else {
        unreachable!()
    };
    result.place = place_id(5);
    result.structural_type = structural_type_id(1);
    establish_extra.kind = OperationKind::EstablishRecord { fields: Vec::new() };
    let mut establish_final = caller.blocks[0].operations[0].clone();
    establish_final.id = operation_id(4);
    let OperationResult::Structural(result) = &mut establish_final.result else {
        unreachable!()
    };
    result.place = place_id(6);
    result.structural_type = structural_type_id(6);
    establish_final.kind = OperationKind::EstablishRecord {
        fields: vec![RecordFieldInitializer {
            field: kept.id,
            value: RecordFieldValue::Structural(StructuralArgument {
                place: place_id(4),
                path: vec![StructuralPathSegment::Field("kept".into())],
                access: StructuralAccess::Owned,
            }),
        }],
    };
    let OperationKind::EstablishRecord { fields } = &mut caller.blocks[0].operations[0].kind else {
        unreachable!()
    };
    fields.push(RecordFieldInitializer {
        field: extra.id,
        value: RecordFieldValue::Structural(StructuralArgument {
            place: place_id(5),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }),
    });
    let mut continuation = caller.blocks[0].clone();
    continuation.id = block_id(3);
    continuation.operations = vec![caller.blocks[0].operations.pop().unwrap()];
    let Terminator::ReturnUnitPartialAffine {
        edge,
        trivial_affine_discards,
        ..
    } = &mut continuation.terminator
    else {
        unreachable!()
    };
    *edge = edge_id(3);
    *trivial_affine_discards = vec![place_id(6)];
    caller.blocks[0].operations.insert(0, establish_extra);
    caller.blocks[0].operations.push(establish_final);
    caller.blocks[0].terminator = Terminator::Jump {
        edge: edge_id(1),
        target: block_id(3),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: vec![StructuralAffineDiscard {
            place: place_id(4),
            path: vec![StructuralPathSegment::Field("extra".into())],
            structural_type: structural_type_id(1),
        }],
    };
    caller.blocks.push(continuation);
    for (place, producer, structural_type) in [(5, 3, 1), (6, 4, 6)] {
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(place),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(producer),
                structural_type: structural_type_id(structural_type),
            },
        });
    }
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect(
        "temporary complement closes at its edge; parameter sibling remains live for the next call",
    );
    let mut omitted = module.clone();
    let Terminator::Jump {
        residual_affine_discards,
        ..
    } = &mut omitted.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.clear();
    assert!(
        validate_module(&omitted).is_err(),
        "a parameter's later cleanup cannot settle temporary debt"
    );
    let mut substituted = module;
    let Terminator::Jump {
        residual_affine_discards,
        ..
    } = &mut substituted.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[0].place = place_id(1);
    residual_affine_discards[0].path = vec![StructuralPathSegment::Field("left".into())];
    assert!(
        validate_module(&substituted).is_err(),
        "same-typed residuals keep their exact root identity"
    );
}

#[test]
fn projected_constructor_block_parameter_custody_survives_staging_edges() {
    let mut module = projected_constructor_module(false, false);
    let caller = &mut module.machines[0];
    let mut construction = caller.blocks[0].clone();
    construction.id = block_id(3);
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.place = place_id(7);
    construction.structural_parameters = vec![parameter];
    let mut disposal = caller.blocks[0].clone();
    disposal.id = block_id(4);
    disposal.operations = vec![construction.operations.pop().unwrap()];
    let mut exit = caller.blocks[0].clone();
    exit.id = block_id(5);
    exit.operations.clear();
    exit.terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: vec![place_id(4)],
    };
    let jump = |edge, target| Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    caller.blocks[0].operations.clear();
    caller.blocks[0].terminator = jump(1, 3);
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments.push(StructuralArgument {
        place: place_id(1),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    });
    let OperationKind::EstablishRecord { fields } = &mut construction.operations[0].kind else {
        unreachable!()
    };
    let terminal_psi::RecordFieldValue::Structural(argument) = &mut fields[0].value else {
        unreachable!()
    };
    argument.place = place_id(7);
    construction.terminator = jump(3, 4);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut disposal.operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = place_id(7);
    disposal.terminator = jump(4, 5);
    let Terminator::Jump {
        residual_affine_discards,
        ..
    } = &mut disposal.terminator
    else {
        unreachable!()
    };
    residual_affine_discards.push(terminal_psi::StructuralAffineDiscard {
        place: place_id(7),
        path: vec![StructuralPathSegment::Field("left".into())],
        structural_type: structural_type_id(1),
    });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(7),
        kind: StructuralPlaceKind::BlockParameter {
            block: block_id(3),
            position: 0,
        },
    });
    caller.blocks.extend([construction, disposal, exit]);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the live sibling survives staging and its later exact residual closes the root");

    let mut moved_again = module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut moved_again.machines[0].blocks[2].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("right".into())];
    assert!(
        validate_module(&moved_again).is_err(),
        "staging does not restore moved custody"
    );

    let mut wrong_complement = module.clone();
    let Terminator::Jump {
        residual_affine_discards,
        ..
    } = &mut wrong_complement.machines[0].blocks[2].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[0].path = vec![StructuralPathSegment::Field("middle".into())];
    assert!(
        validate_module(&wrong_complement).is_err(),
        "later cleanup uses all preceding transfers"
    );

    let mut missing = module;
    let Terminator::Jump {
        residual_affine_discards,
        ..
    } = &mut missing.machines[0].blocks[2].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.clear();
    assert!(
        validate_module(&missing).is_err(),
        "surviving partial custody cannot escape final return"
    );
}

#[test]
fn constructed_shared_descriptor_aggregate_keeps_custody_across_successor() {
    for case in [false, true] {
        let mut module = projected_constructor_module(case, true);
        let caller = &mut module.machines[0];
        let mut successor = caller.blocks[0].clone();
        successor.id = block_id(3);
        successor.operations.clear();
        let mut parameter = caller.structural_parameters[0].clone();
        parameter.place = place_id(7);
        parameter.structural_type = structural_type_id(4);
        successor.structural_parameters = vec![parameter];
        successor.terminator = Terminator::ReturnUnit {
            edge: edge_id(3),
            trivial_affine_discards: vec![place_id(7)],
        };
        let Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            ..
        } = &caller.blocks[0].terminator
        else {
            unreachable!()
        };
        let residuals = residual_affine_discards.clone();
        caller.blocks[0].terminator = Terminator::Jump {
            edge: edge_id(1),
            target: block_id(3),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(4),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: residuals,
        };
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(7),
            kind: StructuralPlaceKind::BlockParameter {
                block: block_id(3),
                position: 0,
            },
        });
        caller.blocks.push(successor);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("a constructed aggregate forwards its owned carrier and retained shared slots");

        let mut wrong_access = module.clone();
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut wrong_access.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        structural_arguments[0].access = StructuralAccess::SharedBorrow;
        assert!(
            validate_module(&wrong_access).is_err(),
            "a borrowed carrier cannot supply owned arrival custody"
        );

        let mut wrong_type = module.clone();
        wrong_type.machines[0].blocks[1].structural_parameters[0].structural_type =
            structural_type_id(1);
        assert!(
            validate_module(&wrong_type).is_err(),
            "carrier geometry does not replace exact result identity"
        );

        for access in [None, Some(StructuralAccess::MutableBorrow)] {
            let mut changed = module.clone();
            let child = changed
                .structural_types
                .iter_mut()
                .find(|row| row.id == structural_type_id(5))
                .unwrap();
            let terminal_psi::StructuralTypeShape::Record { fields } = &mut child.shape else {
                unreachable!()
            };
            fields[0].field_type = terminal_psi::StructuralFieldType::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access },
            );
            assert!(
                validate_module(&changed).is_err(),
                "shared carrier admission requires explicit shared slot access"
            );
        }
    }
}
