use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId, Proposition,
    ScalarType, ServiceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer, CompletionReceipt,
    ContentEntryClaim, ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket,
    CrashRouteGuard, EntryClaim, MachineContract, NominalAffineCleanup, Operation, OperationKind,
    OperationResult, ServiceDeclaration, StructuralAffineDiscard, StructuralArgument,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{
    ModuleError, ProofBundle, ServiceCeilingOwner, reconstruct_operation_obligations,
    validate_module, verify_module,
};

#[test]
fn exact_empty_nominal_affine_cleanup_validates() {
    validate_module(&nominal_affine_module()).expect("exact empty nominal cleanup should validate");
}

#[test]
fn exact_one_call_nominal_affine_cleanup_validates_and_verifies() {
    let module = executable_nominal_affine_module();
    validate_module(&module).expect("exact one-call nominal cleanup should validate");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("exact one-call nominal cleanup requires no proof evidence");
}

#[test]
fn one_call_nominal_affine_cleanup_rejects_nonexact_closures() {
    let mut recursive_target = executable_nominal_affine_module();
    let cleanup_id = recursive_target.machines[1].id;
    let OperationKind::CallUnit { callee, .. } =
        &mut recursive_target.machines[1].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *callee = cleanup_id;
    assert!(matches!(
        validate_module(&recursive_target),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut nonempty_helper = executable_nominal_affine_module();
    nonempty_helper.machines[2].blocks[0]
        .operations
        .push(Operation {
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: machine_id(3),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
    assert!(matches!(
        validate_module(&nonempty_helper),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut extra_machine = executable_nominal_affine_module();
    let mut fourth = extra_machine.machines[2].clone();
    fourth.id = machine_id(4);
    fourth.entry = block_id(4);
    fourth.blocks[0].id = block_id(4);
    fourth.contract.id = contract_id(4);
    extra_machine.machines.push(fourth);
    assert!(matches!(
        validate_module(&extra_machine),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));
}

#[test]
fn exact_one_primitive_field_nominal_affine_cleanup_validates() {
    let mut module = nominal_affine_module();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            identity: "payload".into(),
            id: psi_core::StructuralFieldId::new(1).unwrap(),
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap(),
            )),
            relevance: psi_terminal::BindingRelevance::Relevant,
        }],
    };
    validate_module(&module).expect("one primitive-field nominal cleanup should validate");
}

#[test]
fn exact_two_primitive_fields_nominal_affine_cleanup_validates() {
    let mut module = nominal_affine_module();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![
            StructuralFieldDeclaration {
                identity: "tag".into(),
                id: psi_core::StructuralFieldId::new(1).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                relevance: psi_terminal::BindingRelevance::Relevant,
            },
            StructuralFieldDeclaration {
                identity: "payload".into(),
                id: psi_core::StructuralFieldId::new(2).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
                )),
                relevance: psi_terminal::BindingRelevance::Relevant,
            },
        ],
    };
    validate_module(&module).expect("two primitive-field nominal cleanup should validate");
}

#[test]
fn wide_flat_primitive_nominal_affine_cleanup_validates() {
    let mut module = nominal_affine_module();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: (1..=5)
            .map(|index| StructuralFieldDeclaration {
                identity: format!("payload_{index}"),
                id: psi_core::StructuralFieldId::new(index).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
                )),
                relevance: psi_terminal::BindingRelevance::Relevant,
            })
            .collect(),
    };
    validate_module(&module).expect("wide flat primitive nominal cleanup should validate");
}

#[test]
fn nominal_affine_cleanup_rejects_forged_target_and_unsupported_field_type() {
    let mut wrong_attachment = nominal_affine_module();
    wrong_attachment.machines[1].attachment = None;
    assert!(matches!(
        validate_module(&wrong_attachment),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut target_parameter = nominal_affine_module();
    target_parameter.machines[1]
        .parameters
        .push(ValueDeclaration {
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        });
    assert!(matches!(
        validate_module(&target_parameter),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut unsupported_scalar = nominal_affine_module();
    unsupported_scalar.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            identity: "payload".into(),
            id: psi_core::StructuralFieldId::new(1).unwrap(),
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 128).unwrap(),
            )),
            relevance: psi_terminal::BindingRelevance::Relevant,
        }],
    };
    assert!(matches!(
        validate_module(&unsupported_scalar),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));
}

#[test]
fn hard_root_unit_slice_validates_and_verifies() {
    let module = hard_root_module();

    validate_module(&module).expect("structural Unit call/boundary/effect slice validates");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("structural Unit operations require no producer-authored structural proof evidence");
}

#[test]
fn unit_call_requirements_remain_callee_contract_obligations() {
    let mut module = hard_root_module();
    module.machines[1].contract.requires = vec![Proposition::Truth];
    let OperationKind::CallUnit {
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    requirement_obligations.push(obligation_id(1));

    let obligations = reconstruct_operation_obligations(&module).expect("Unit call obligations");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, obligation_id(1));
    assert_eq!(obligations[0].obligation.proposition, Proposition::Truth);
}

#[test]
fn unit_call_checks_structural_type_qualification_and_transfer_shape() {
    let mut wrong_type = hard_root_module();
    wrong_type.machines[0].structural_parameters[0].structural_type = structural_type_id(2);
    wrong_type.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    assert_eq!(
        validate_module(&wrong_type).unwrap_err(),
        ModuleError::StructuralArgumentTypeMismatch {
            operation: operation_id(1),
            argument_index: 0,
            expected: structural_type_id(1),
            actual: structural_type_id(2),
        }
    );

    let mut missing_qualification = hard_root_module();
    missing_qualification.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    assert_eq!(
        validate_module(&missing_qualification).unwrap_err(),
        ModuleError::StructuralArgumentMissingQualification {
            operation: operation_id(1),
            argument_index: 0,
            domain: domain_id(1),
        }
    );

    let mut missing_transfer = hard_root_module();
    unit_call_mut(&mut missing_transfer).clear();
    assert_eq!(
        validate_module(&missing_transfer).unwrap_err(),
        ModuleError::UnitCallClaimTransferCountMismatch {
            operation: operation_id(1),
            expected: 1,
            actual: 0,
        }
    );
}

#[test]
fn unit_call_requires_exact_aggregate_claim_path() {
    let mut module = hard_root_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("expected record shape")
    };
    fields.push(StructuralFieldDeclaration {
        id: psi_core::StructuralFieldId::new(1).expect("field identity"),
        identity: "#7".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    module.machines[0].entry_claims[0].path = vec!["#7".into()];
    module.machines[1].entry_claims[0].path = vec!["#7".into()];
    validate_module(&module).expect("matching aggregate custody paths validate");

    let mut abandoned = module.clone();
    abandoned.machines[1].blocks[0].operations.pop();
    assert_eq!(
        validate_module(&abandoned).unwrap_err(),
        ModuleError::LiveLinearClaimAtUnitReturn {
            machine: machine_id(2),
            block: block_id(2),
            claim: claim_id(1),
        }
    );

    module.machines[1].entry_claims[0].path.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::UnitCallClaimPresenceMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );
}

#[test]
fn nested_record_claim_path_is_walked_and_matched_exactly() {
    let mut module = hard_root_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("expected record shape")
    };
    fields.push(StructuralFieldDeclaration {
        id: psi_core::StructuralFieldId::new(1).expect("field identity"),
        identity: "#7".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        panic!("expected record shape")
    };
    fields.push(StructuralFieldDeclaration {
        id: psi_core::StructuralFieldId::new(1).expect("field identity"),
        identity: "#9".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(3)),
    });
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims[0].path = vec!["#7".into(), "#9".into()];
    }
    validate_module(&module).expect("a complete nested record path should validate");

    let mut unknown_inner = module.clone();
    unknown_inner.machines[0].entry_claims[0].path[1] = "#8".into();
    assert_eq!(
        validate_module(&unknown_inner).unwrap_err(),
        ModuleError::InvalidEntryClaimFieldPath(claim_id(1))
    );

    let mut truncated_at_call = module.clone();
    truncated_at_call.machines[1].entry_claims[0].path.pop();
    assert_eq!(
        validate_module(&truncated_at_call).unwrap_err(),
        ModuleError::UnitCallClaimPresenceMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut overlapping = module;
    let input = overlapping.machines[0].entry_claims[0].input;
    overlapping.machines[0].entry_claims = vec![
        EntryClaim {
            claim: claim_id(1),
            input,
            path: vec!["#7".into()],
        },
        EntryClaim {
            claim: claim_id(2),
            input,
            path: vec!["#7".into(), "#9".into()],
        },
    ];
    assert_eq!(
        validate_module(&overlapping).unwrap_err(),
        ModuleError::OverlappingEntryClaimInput {
            first: claim_id(1),
            second: claim_id(2),
        }
    );
}

#[test]
fn unit_call_transfers_complete_canonical_sibling_claim_set() {
    let mut module = hard_root_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("expected record shape")
    };
    fields.extend([
        StructuralFieldDeclaration {
            id: psi_core::StructuralFieldId::new(1).expect("field identity"),
            identity: "#7".into(),
            relevance: psi_terminal::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(2)),
        },
        StructuralFieldDeclaration {
            id: psi_core::StructuralFieldId::new(2).expect("field identity"),
            identity: "#9".into(),
            relevance: psi_terminal::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(2)),
        },
    ]);
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims[0].path = vec!["#7".into()];
        machine.entry_claims.push(EntryClaim {
            claim: claim_id(2),
            input: machine.structural_parameters[0].place,
            path: vec!["#9".into()],
        });
    }
    unit_call_mut(&mut module).push(ClaimTransfer {
        claim: claim_id(2),
        argument_index: 0,
    });
    boundary_call_mut(&mut module).0.push(CompletionReceipt {
        claim: claim_id(2),
        argument_index: 0,
    });
    validate_module(&module).expect("both disjoint sibling claims should transfer and settle");

    let mut incomplete = module.clone();
    unit_call_mut(&mut incomplete).pop();
    assert_eq!(
        validate_module(&incomplete).unwrap_err(),
        ModuleError::UnitCallClaimTransferCountMismatch {
            operation: operation_id(1),
            expected: 2,
            actual: 1,
        }
    );

    let mut noncanonical = module;
    noncanonical.machines[0].entry_claims[0].path = vec!["#9".into()];
    noncanonical.machines[0].entry_claims[1].path = vec!["#7".into()];
    assert_eq!(
        validate_module(&noncanonical).unwrap_err(),
        ModuleError::NonCanonicalEntryClaimOrder(machine_id(1))
    );
}

#[test]
fn structural_calls_preserve_optional_affine_claim_custody() {
    let mut dropped_at_call = hard_root_module();
    for machine in &mut dropped_at_call.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    }
    dropped_at_call.machines[1].entry_claims.clear();
    unit_call_mut(&mut dropped_at_call).clear();
    assert_eq!(
        validate_module(&dropped_at_call).unwrap_err(),
        ModuleError::UnitCallClaimPresenceMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut minted_at_call = hard_root_module();
    for machine in &mut minted_at_call.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    }
    minted_at_call.machines[0].entry_claims.clear();
    unit_call_mut(&mut minted_at_call).clear();
    assert_eq!(
        validate_module(&minted_at_call).unwrap_err(),
        ModuleError::UnitCallClaimPresenceMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut settled_at_boundary = hard_root_module();
    for machine in &mut settled_at_boundary.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    }
    settled_at_boundary.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    validate_module(&settled_at_boundary)
        .expect("a proof-visible affine claim is settled with its consumed owned place");

    boundary_call_mut(&mut settled_at_boundary).0.clear();
    assert_eq!(
        validate_module(&settled_at_boundary).unwrap_err(),
        ModuleError::BoundaryCompletionReceiptMismatch(operation_id(3))
    );
}

#[test]
fn unit_calls_preserve_exact_content_claim_shape() {
    let mut matching = hard_root_module();
    matching.machines[0].content_entry_claims = vec![content_entry_claim(place_id(1))];
    matching.machines[1].content_entry_claims = vec![content_entry_claim(place_id(2))];
    validate_module(&matching).expect("an ordinary structural transfer preserves exact content");

    let mut dropped = matching.clone();
    dropped.machines[1].content_entry_claims.clear();
    assert_eq!(
        validate_module(&dropped).unwrap_err(),
        ModuleError::UnitCallContentClaimMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut minted = matching.clone();
    minted.machines[0].content_entry_claims.clear();
    assert_eq!(
        validate_module(&minted).unwrap_err(),
        ModuleError::UnitCallContentClaimMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut redirected = matching.clone();
    redirected.machines[1].content_entry_claims[0]
        .input
        .segments
        .push(ContentPlaceSegment::Field("payload".to_owned()));
    assert_eq!(
        validate_module(&redirected).unwrap_err(),
        ModuleError::UnitCallContentClaimMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut reinterpreted = matching;
    reinterpreted.machines[1].content_entry_claims[0].projections[0]
        .projection
        .projection_fingerprint ^= 1;
    assert_eq!(
        validate_module(&reinterpreted).unwrap_err(),
        ModuleError::UnitCallContentClaimMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );
}

#[test]
fn projected_unit_calls_accept_only_the_exact_unqualified_whole_claim_slice() {
    let module = projected_unit_call_module();
    validate_module(&module).expect("one literal fixed index may transfer one whole callee claim");

    let mut nested = module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut nested.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(psi_terminal::StructuralPathSegment::FixedIndex(0));
    assert_eq!(
        validate_module(&nested).unwrap_err(),
        ModuleError::InvalidStructuralArgumentPath {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut out_of_bounds = module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut out_of_bounds.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![psi_terminal::StructuralPathSegment::FixedIndex(1)];
    assert_eq!(
        validate_module(&out_of_bounds).unwrap_err(),
        ModuleError::InvalidStructuralArgumentPath {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut qualified_caller = module.clone();
    qualified_caller
        .structural_domains
        .push(StructuralDomainDeclaration {
            id: domain_id(2),
            identity: "ArrayPending".into(),
            carrier: structural_type_id(3),
        });
    qualified_caller.machines[0].structural_parameters[0]
        .qualifications
        .push(domain_id(2));
    assert_eq!(
        validate_module(&qualified_caller).unwrap_err(),
        ModuleError::InvalidStructuralArgumentPath {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut qualified_callee = module.clone();
    qualified_callee.machines[1].structural_parameters[0]
        .qualifications
        .push(domain_id(1));
    assert_eq!(
        validate_module(&qualified_callee).unwrap_err(),
        ModuleError::StructuralArgumentMissingQualification {
            operation: operation_id(1),
            argument_index: 0,
            domain: domain_id(1),
        }
    );

    let mut nested_callee_claim = module.clone();
    let StructuralTypeShape::Record { fields } = &mut nested_callee_claim.structural_types[0].shape
    else {
        unreachable!()
    };
    fields.push(StructuralFieldDeclaration {
        id: psi_core::StructuralFieldId::new(1).expect("field identity"),
        identity: "payload".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    nested_callee_claim.machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    nested_callee_claim.machines[0].entry_claims[0]
        .path
        .push(psi_terminal::StructuralPathSegment::Field("payload".into()));
    nested_callee_claim.machines[1].entry_claims[0].path =
        vec![psi_terminal::StructuralPathSegment::Field("payload".into())];
    assert_eq!(
        validate_module(&nested_callee_claim).unwrap_err(),
        ModuleError::UnitCallClaimPresenceMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut content_bearing = module.clone();
    content_bearing.machines[0].content_entry_claims = vec![content_entry_claim(place_id(1))];
    content_bearing.machines[1].content_entry_claims = vec![content_entry_claim(place_id(2))];
    assert!(validate_module(&content_bearing).is_err());

    let mut missing_transfer = module.clone();
    unit_call_mut(&mut missing_transfer).clear();
    assert!(validate_module(&missing_transfer).is_err());

    let mut duplicate_transfer = module.clone();
    unit_call_mut(&mut duplicate_transfer).push(ClaimTransfer {
        claim: claim_id(1),
        argument_index: 0,
    });
    assert!(validate_module(&duplicate_transfer).is_err());

    let mut wrong_transfer = module;
    unit_call_mut(&mut wrong_transfer)[0].claim = claim_id(2);
    assert!(validate_module(&wrong_transfer).is_err());
}

#[test]
fn projected_unit_calls_reject_signatures_outside_the_bounded_slice() {
    let mut scalar_caller = projected_unit_call_module();
    scalar_caller.machines[0].parameters.push(ValueDeclaration {
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    });
    assert_eq!(
        validate_module(&scalar_caller).unwrap_err(),
        ModuleError::ProjectedUnitCallOutsideBoundedSlice {
            operation: operation_id(1),
        }
    );

    let mut extra_caller_parameter = projected_unit_call_module();
    let mut parameter = structural_parameter(place_id(10));
    parameter.position = 1;
    parameter.structural_type = structural_type_id(2);
    parameter.multiplicity = StructuralMultiplicity::Unrestricted;
    parameter.qualifications.clear();
    extra_caller_parameter.machines[0]
        .structural_parameters
        .push(parameter);
    extra_caller_parameter.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(10),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    assert_eq!(
        validate_module(&extra_caller_parameter).unwrap_err(),
        ModuleError::ProjectedUnitCallOutsideBoundedSlice {
            operation: operation_id(1),
        }
    );

    let mut extra_callee_parameter = projected_unit_call_module();
    let mut parameter = structural_parameter(place_id(10));
    parameter.position = 1;
    parameter.structural_type = structural_type_id(2);
    parameter.multiplicity = StructuralMultiplicity::Unrestricted;
    parameter.qualifications.clear();
    extra_callee_parameter.machines[1]
        .structural_parameters
        .push(parameter);
    extra_callee_parameter.machines[1]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(10),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    assert_eq!(
        validate_module(&extra_callee_parameter).unwrap_err(),
        ModuleError::ProjectedUnitCallOutsideBoundedSlice {
            operation: operation_id(1),
        }
    );

    let mut extra_argument = projected_unit_call_module();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut extra_argument.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments.push(structural_arguments[0].clone());
    assert_eq!(
        validate_module(&extra_argument).unwrap_err(),
        ModuleError::ProjectedUnitCallOutsideBoundedSlice {
            operation: operation_id(1),
        }
    );
}

#[test]
fn projected_unit_calls_reject_contracts_over_the_projected_parameter() {
    let callee_predicate = content_predicate(place_id(2));
    let expected = ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
        operation: operation_id(1),
        callee: machine_id(2),
        place: place_id(2),
    };

    let mut required = projected_unit_call_module();
    required.machines[1]
        .contract
        .requires
        .push(callee_predicate.clone());
    let OperationKind::CallUnit {
        requirement_obligations,
        ..
    } = &mut required.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    requirement_obligations.push(obligation_id(1));
    assert_eq!(validate_module(&required).unwrap_err(), expected);

    let mut ensured = projected_unit_call_module();
    ensured.machines[1].contract.ensures.push(ContractClause {
        obligation: obligation_id(1),
        proposition: callee_predicate.clone(),
    });
    assert_eq!(validate_module(&ensured).unwrap_err(), expected);

    let mut crashing = projected_unit_call_module();
    let callee_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            callee_predicate,
        ))],
    };
    let caller_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            content_predicate(place_id(1)),
        ))],
    };
    crashing.machines[0].contract.crash_routes = vec![caller_route.clone()];
    crashing.machines[1].contract.crash_routes = vec![callee_route];
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut crashing.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![caller_route];
    assert_eq!(validate_module(&crashing).unwrap_err(), expected);
}

#[test]
fn direct_field_partial_affine_return_validates_and_verifies() {
    let module = partial_affine_field_module();
    validate_module(&module).expect("direct moved field plus residual cleanup exhausts the root");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("partial affine cleanup introduces no producer-authored proposition");
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
    boundary_call_mut(&mut unsettled).0.clear();
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
    boundary_call_mut(&mut module).0.push(CompletionReceipt {
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
    boundary_call_mut(&mut reordered_settlements).0.swap(0, 1);
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
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(1).expect("content domain"),
            projection_fingerprint: 0xfeed,
        },
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: place_id(2),
            segments: Vec::new(),
        },
    };
    module.machines[1].contract.ensures = vec![ContractClause {
        obligation: obligation_id(1),
        proposition: Proposition::ContentConservation(psi_core::ContentConservation::new(
            ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Acknowledgement".to_owned(),
            },
            subject.clone(),
            subject,
        )),
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
fn boundary_call_checks_qualification_settlement_and_obligation_absence() {
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
    boundary_call_mut(&mut missing_settlement).0.clear();
    assert_eq!(
        validate_module(&missing_settlement).unwrap_err(),
        ModuleError::BoundaryCompletionReceiptMismatch(operation_id(3))
    );

    let mut minted_obligation = hard_root_module();
    boundary_call_mut(&mut minted_obligation)
        .1
        .push(obligation_id(1));
    assert_eq!(
        validate_module(&minted_obligation).unwrap_err(),
        ModuleError::BoundaryStructuralRequirementsMintObligations(operation_id(3))
    );
}

#[test]
fn claims_are_linear_across_unit_operations_and_return() {
    let mut reused = hard_root_module();
    reused.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(4),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCallUnit {
            boundary: boundary_id(1),
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
            }],
            completion_receipts: vec![CompletionReceipt {
                claim: claim_id(1),
                argument_index: 0,
            }],
            requirement_obligations: Vec::new(),
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
    let value = ValueId::new(1).expect("scalar value");
    let result = ValueDeclaration {
        id: ValueId::new(2).expect("result value"),
        scalar_type: ScalarType::Boolean,
    };
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(result);
    machine.blocks[0].operations = vec![Operation {
        id: operation_id(2),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value,
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    }];
    machine.blocks[0].terminator = Terminator::Return {
        trivial_affine_discards: Vec::new(),
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
    validate_module(&module).expect("affine custody may be abandoned at scalar return");
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
        id: domain_id(2),
        identity: "Ready".into(),
        carrier: structural_type_id(1),
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
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(10),
        trivial_affine_discards: vec![place_id(4), place_id(2)],
    };
    module.entry = machine.id;
    module.machines = vec![machine];
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
        trivial_affine_discards: Vec::new(),
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
        trivial_affine_discards,
        ..
    } = &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.reverse();
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

#[test]
fn jump_applies_a_canonical_subset_of_affine_discards() {
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
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks = vec![
        Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: edge_id(2),
                target: block_id(3),
                arguments: vec![value_id(10)],
                trivial_affine_discards: vec![place_id(4)],
            },
        },
        Block {
            id: block_id(3),
            parameters: vec![ValueDeclaration {
                id: value_id(12),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(3),
                value: value_id(12),
                trivial_affine_discards: vec![place_id(2)],
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
    validate_module(&module).expect("jump may discard a canonical eligible subset");

    let mut reordered = module.clone();
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![place_id(2), place_id(4)];
    assert_eq!(
        validate_module(&reordered).unwrap_err(),
        ModuleError::EdgeAffineDiscardsInvalid { edge: edge_id(2) }
    );

    let mut claim_bearing = module;
    claim_bearing.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(1),
        input: place_id(4),
        path: Vec::new(),
    });
    assert_eq!(
        validate_module(&claim_bearing).unwrap_err(),
        ModuleError::EdgeAffineDiscardsInvalid { edge: edge_id(2) }
    );
}

#[test]
fn conditional_applies_affine_discards_only_to_each_selected_successor() {
    let mut module = hard_root_module();
    let mut machine = module.machines.pop().expect("callee machine");
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
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks = vec![
        Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(10),
                when_true: SuccessorEdge {
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: vec![value_id(10)],
                    trivial_affine_discards: vec![place_id(4)],
                },
                when_false: SuccessorEdge {
                    edge: edge_id(3),
                    target: block_id(4),
                    arguments: vec![value_id(10)],
                    trivial_affine_discards: vec![place_id(2)],
                },
            },
        },
        Block {
            id: block_id(3),
            parameters: vec![ValueDeclaration {
                id: value_id(12),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(4),
                value: value_id(12),
                trivial_affine_discards: vec![place_id(2)],
            },
        },
        Block {
            id: block_id(4),
            parameters: vec![ValueDeclaration {
                id: value_id(13),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(5),
                value: value_id(13),
                trivial_affine_discards: vec![place_id(4)],
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
    validate_module(&module).expect("each conditional successor owns its cleanup subset");

    let mut reordered = module;
    let Terminator::Conditional { when_true, .. } = &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_true.trivial_affine_discards = vec![place_id(2), place_id(4)];
    assert_eq!(
        validate_module(&reordered).unwrap_err(),
        ModuleError::EdgeAffineDiscardsInvalid { edge: edge_id(2) }
    );
}

#[test]
fn affine_structural_arguments_transfer_at_most_once() {
    let mut repeated = hard_root_module();
    for machine in &mut repeated.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims.clear();
    }
    unit_call_mut(&mut repeated).clear();
    repeated.machines[1].blocks[0].operations.truncate(1);
    let mut second_call = repeated.machines[0].blocks[0].operations[0].clone();
    second_call.id = operation_id(4);
    repeated.machines[0].blocks[0].operations.push(second_call);
    assert_eq!(
        validate_module(&repeated).unwrap_err(),
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: operation_id(4),
            place: place_id(1),
        }
    );

    for machine in &mut repeated.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    }
    validate_module(&repeated).expect("unrestricted structural arguments remain reusable");

    let mut repeated_boundary = hard_root_module();
    repeated_boundary.machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    repeated_boundary.machines[0].entry_claims.clear();
    repeated_boundary.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    let boundary_call = Operation {
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCallUnit {
            boundary: boundary_id(1),
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
            }],
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        },
    };
    let mut second_boundary_call = boundary_call.clone();
    second_boundary_call.id = operation_id(4);
    repeated_boundary.machines[0].blocks[0].operations = vec![boundary_call, second_boundary_call];
    assert_eq!(
        validate_module(&repeated_boundary).unwrap_err(),
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: operation_id(4),
            place: place_id(1),
        }
    );
}

#[test]
fn port_write_requires_a_declared_reachable_service_and_preserves_claims() {
    let mut outside_ceiling = hard_root_module();
    outside_ceiling.services.push(ServiceDeclaration {
        id: service_id(2),
        identity: "DebugIo".into(),
        parents: Vec::new(),
    });
    let OperationKind::PortWrite { service, .. } =
        &mut outside_ceiling.machines[1].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *service = service_id(2);
    assert_eq!(
        validate_module(&outside_ceiling).unwrap_err(),
        ModuleError::OperationServiceOutsidePublishedCeiling {
            operation: operation_id(2),
            service: service_id(2),
        }
    );
}

#[test]
fn crash_frontier_is_the_exact_live_frontier_after_transfer() {
    let mut module = hard_root_module();
    module.machines[0].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    module.machines[0].blocks[0].terminator = Terminator::Crash {
        edge: edge_id(1),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    validate_module(&module).expect("the transferred claim is absent from the crash frontier");

    let Terminator::Crash {
        frontier_lower_bound,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    frontier_lower_bound.push(claim_id(1));
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::CrashFrontierMismatch { block: block_id(1) }
    );
}

#[test]
fn unit_calls_preserve_exact_crash_routes_and_remain_acyclic() {
    let mut crash_erasing = hard_root_module();
    crash_erasing.machines[1].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    assert_eq!(
        validate_module(&crash_erasing).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(1),
            callee: machine_id(2),
        }
    );

    let mut recursive = hard_root_module();
    recursive.machines[1].blocks[0].operations = vec![Operation {
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(2),
            structural_arguments: vec![StructuralArgument {
                place: place_id(2),
                path: Vec::new(),
            }],
            claim_transfers: vec![ClaimTransfer {
                claim: claim_id(1),
                argument_index: 0,
            }],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    assert_eq!(
        validate_module(&recursive).unwrap_err(),
        ModuleError::RecursiveCallSliceNotYetSupported(machine_id(2))
    );
}

#[test]
fn unit_call_crash_routes_substitute_structural_parameters() {
    let mut module = hard_root_module();
    let callee_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            content_predicate(place_id(2)),
        ))],
    };
    let caller_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            content_predicate(place_id(1)),
        ))],
    };
    module.machines[0].contract.crash_routes = vec![caller_route.clone()];
    module.machines[1].contract.crash_routes = vec![callee_route.clone()];
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![caller_route];
    validate_module(&module).expect("callee structural crash places substitute to caller places");

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![callee_route];
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(1),
            callee: machine_id(2),
        }
    );
}

fn hard_root_module() -> TerminalModule {
    let resource = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "PortResource".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let other = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "OtherResource".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let pending = StructuralDomainDeclaration {
        id: domain_id(1),
        identity: "Pending".into(),
        carrier: resource.id,
    };
    let port_io = ServiceDeclaration {
        id: service_id(1),
        identity: "PortIo".into(),
        parents: Vec::new(),
    };
    let mut boundary_parameter = structural_parameter(place_id(9));
    boundary_parameter.qualifications.clear();
    let boundary = BoundaryMachineDeclaration {
        id: boundary_id(1),
        identity: "settle_port".into(),
        attachment: None,
        structural_parameters: vec![boundary_parameter],
        requires: vec![StructuralDomainRequirement {
            argument_index: 0,
            domain: pending.id,
        }],
        published_service_ceiling: vec![port_io.id],
    };

    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(1))],
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(1))],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: place_id(1),
            path: Vec::new(),
        }],
        published_service_ceiling: vec![port_io.id],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(1),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: machine_id(2),
                    structural_arguments: vec![StructuralArgument {
                        place: place_id(1),
                        path: Vec::new(),
                    }],
                    claim_transfers: vec![ClaimTransfer {
                        claim: claim_id(1),
                        argument_index: 0,
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(1)),
    };

    let callee = TerminalMachine {
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(2))],
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(2))],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: place_id(2),
            path: Vec::new(),
        }],
        published_service_ceiling: vec![port_io.id],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(2),
                    result: OperationResult::Unit,
                    kind: OperationKind::PortWrite {
                        service: port_io.id,
                        port: 0x3f8,
                        value: b'X',
                    },
                },
                Operation {
                    id: operation_id(3),
                    result: OperationResult::Unit,
                    kind: OperationKind::BoundaryCallUnit {
                        boundary: boundary.id,
                        structural_arguments: vec![StructuralArgument {
                            place: place_id(2),
                            path: Vec::new(),
                        }],
                        completion_receipts: vec![CompletionReceipt {
                            claim: claim_id(1),
                            argument_index: 0,
                        }],
                        requirement_obligations: Vec::new(),
                    },
                },
            ],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(2),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(2)),
    };

    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![resource, other],
        structural_domains: vec![pending],
        services: vec![port_io],
        boundary_machines: vec![boundary],
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn projected_unit_call_module() -> TerminalModule {
    let mut module = hard_root_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "[PortResource;1]".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(1),
            length: 1,
        },
    });
    module.boundary_machines[0].requires.clear();
    module.machines[0].structural_parameters[0].structural_type = structural_type_id(3);
    module.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    module.machines[0].entry_claims[0].path =
        vec![psi_terminal::StructuralPathSegment::FixedIndex(0)];
    module.machines[1].structural_parameters[0]
        .qualifications
        .clear();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![psi_terminal::StructuralPathSegment::FixedIndex(0)];
    module
}

fn partial_affine_field_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let pair = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Pair".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(1).expect("field identity"),
                    identity: "left".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
                StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(2).expect("field identity"),
                    identity: "right".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
            ],
        },
    };
    let other = StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "Other".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let caller_parameter = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: pair.id,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
    };
    let callee_parameter = StructuralParameterDeclaration {
        place: place_id(2),
        position: 0,
        is_self: false,
        structural_type: token.id,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
    };
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![caller_parameter],
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(1))],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(1),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: machine_id(2),
                    structural_arguments: vec![StructuralArgument {
                        place: place_id(1),
                        path: vec![StructuralPathSegment::Field("right".into())],
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnitPartialAffine {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: vec![StructuralAffineDiscard {
                    place: place_id(1),
                    path: vec![StructuralPathSegment::Field("left".into())],
                    structural_type: token.id,
                }],
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    let callee = TerminalMachine {
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![callee_parameter],
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(2))],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(2),
                trivial_affine_discards: vec![place_id(2)],
            },
        }],
        contract: empty_contract(contract_id(2)),
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![token, pair, other],
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn nominal_affine_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: token.id,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        }],
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(1))],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnitNominalAffine {
                edge: edge_id(1),
                cleanup: NominalAffineCleanup {
                    place: place_id(1),
                    structural_type: token.id,
                    cleanup_machine: machine_id(2),
                },
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    let cleanup = TerminalMachine {
        id: machine_id(2),
        attachment: Some(token.id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(2),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(2)),
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![token],
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![caller, cleanup],
    }
}

fn executable_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let helper_type = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(3),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        id: machine_id(3),
        attachment: Some(helper_type.id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(3),
        blocks: vec![Block {
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(3),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(3)),
    });
    module
}

fn structural_parameter(place: PlaceId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: vec![domain_id(1)],
    }
}

fn content_entry_claim(root: PlaceId) -> ContentEntryClaim {
    ContentEntryClaim {
        claim: claim_id(1),
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root,
            segments: Vec::new(),
        },
        projections: vec![ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: ContentDomainId::new(1).expect("content domain"),
                projection_fingerprint: 0xfeed,
            },
            algebra: ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Acknowledgement".to_owned(),
            },
        }],
    }
}

fn structural_place(id: PlaceId) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }
}

fn empty_contract(id: ContractId) -> MachineContract {
    MachineContract {
        id,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
    }
}

fn content_predicate(root: PlaceId) -> Proposition {
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(1).expect("content domain"),
        projection_fingerprint: 1,
    };
    let projected = |field: &str| ContentTerm::Projection {
        projection,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root,
            segments: vec![ContentPlaceSegment::Field(field.into())],
        },
    };
    Proposition::ContentConservation(psi_core::ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Byte".into(),
        },
        projected("left"),
        projected("right"),
    ))
}

fn unit_call_mut(module: &mut TerminalModule) -> &mut Vec<ClaimTransfer> {
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers
}

fn boundary_call_mut(
    module: &mut TerminalModule,
) -> (&mut Vec<CompletionReceipt>, &mut Vec<ObligationId>) {
    let OperationKind::BoundaryCallUnit {
        completion_receipts,
        requirement_obligations,
        ..
    } = &mut module.machines[1].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    (completion_receipts, requirement_obligations)
}

macro_rules! id_fn {
    ($name:ident, $type:ty) => {
        fn $name(raw: u64) -> $type {
            <$type>::new(raw).expect("nonzero test identity")
        }
    };
}

id_fn!(block_id, BlockId);
id_fn!(boundary_id, BoundaryMachineId);
id_fn!(claim_id, ClaimId);
id_fn!(contract_id, ContractId);
id_fn!(edge_id, EdgeId);
id_fn!(machine_id, MachineId);
id_fn!(obligation_id, ObligationId);
id_fn!(operation_id, OperationId);
id_fn!(place_id, PlaceId);
id_fn!(value_id, ValueId);
id_fn!(service_id, ServiceId);
id_fn!(structural_type_id, StructuralTypeId);
id_fn!(domain_id, StructuralDomainId);
