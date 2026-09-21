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
