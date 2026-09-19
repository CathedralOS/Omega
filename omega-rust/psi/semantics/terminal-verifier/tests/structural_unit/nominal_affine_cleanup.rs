use super::{
    boundary_call_mut, content_entry_claim, content_predicate, executable_nominal_affine_module,
    five_call_executable_nominal_affine_module, hard_root_module, nominal_affine_module,
    partial_affine_field_module, projected_unit_call_module, structural_parameter,
    three_call_executable_nominal_affine_module, two_call_executable_nominal_affine_module,
    two_element_projected_unit_call_module, unit_call_mut,
};
use crate::structural_unit::{
    block_id, claim_id, contract_id, domain_id, edge_id, machine_id, obligation_id, operation_id,
    place_id, structural_type_id, value_id,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceSegment, IeeeFloatFormat, Proposition, ScalarTerm,
    ScalarType, StructuralPlaceKind,
};
use terminal_psi::{
    Block, ClaimTransfer, CompletionReceipt, ContractClause, CrashCause, CrashPredicateTerm,
    CrashRouteBucket, CrashRouteGuard, EntryClaim, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralArgument, StructuralDomainDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalMachineResult, Terminator, ValueDeclaration,
};
use terminal_verifier::{
    ModuleError, ProofBundle, reconstruct_operation_obligations,
    reconstruct_structural_ownership_frontiers, validate_module, verify_module,
};

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
fn exact_two_call_nominal_affine_cleanup_validates_and_verifies_in_order() {
    let module = two_call_executable_nominal_affine_module();
    let [first, second] = module.machines[1].blocks[0].operations.as_slice() else {
        panic!("cleanup target has two ordered operations")
    };
    let OperationKind::CallUnit {
        callee: first_callee,
        ..
    } = first.kind
    else {
        panic!("first cleanup operation is a Unit call")
    };
    let OperationKind::CallUnit {
        callee: second_callee,
        ..
    } = second.kind
    else {
        panic!("second cleanup operation is a Unit call")
    };
    assert_eq!(
        (first_callee, second_callee),
        (machine_id(3), machine_id(4))
    );
    validate_module(&module).expect("exact two-call nominal cleanup should validate");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("exact two-call nominal cleanup requires no proof evidence");
}

#[test]
fn exact_three_call_nominal_affine_cleanup_validates_and_verifies_in_order() {
    let module = three_call_executable_nominal_affine_module();
    let [first, second, third] = module.machines[1].blocks[0].operations.as_slice() else {
        panic!("cleanup target has three ordered operations")
    };
    let callees = [first, second, third].map(|operation| match operation.kind {
        OperationKind::CallUnit { callee, .. } => callee,
        _ => panic!("cleanup operation is a Unit call"),
    });
    assert_eq!(callees, [machine_id(3), machine_id(4), machine_id(5)]);
    validate_module(&module).expect("exact three-call nominal cleanup should validate");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("exact three-call nominal cleanup requires no proof evidence");
}

#[test]
fn exact_five_call_nominal_affine_cleanup_validates_and_verifies_in_order() {
    let module = five_call_executable_nominal_affine_module();
    let callees = module.machines[1].blocks[0]
        .operations
        .iter()
        .map(|operation| match operation.kind {
            OperationKind::CallUnit { callee, .. } => callee,
            _ => panic!("cleanup operation is a Unit call"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        callees,
        vec![
            machine_id(3),
            machine_id(4),
            machine_id(5),
            machine_id(6),
            machine_id(7)
        ]
    );
    validate_module(&module).expect("exact five-call nominal cleanup should validate");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("exact five-call nominal cleanup requires no proof evidence");
}

#[test]
fn two_call_nominal_affine_cleanup_rejects_repeated_or_nonempty_helpers() {
    let mut repeated = two_call_executable_nominal_affine_module();
    let first_callee = match repeated.machines[1].blocks[0].operations[0].kind {
        OperationKind::CallUnit { callee, .. } => callee,
        _ => unreachable!(),
    };
    let OperationKind::CallUnit { callee, .. } =
        &mut repeated.machines[1].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *callee = first_callee;
    assert!(matches!(
        validate_module(&repeated),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut nonempty_second = two_call_executable_nominal_affine_module();
    nonempty_second.machines[3].blocks[0]
        .operations
        .push(Operation {
            static_reach_binding: None,
            id: operation_id(3),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                arguments: Vec::new(),
                callee: machine_id(3),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
    assert!(matches!(
        validate_module(&nonempty_second),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut third_call = two_call_executable_nominal_affine_module();
    third_call.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(3),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    assert!(matches!(
        validate_module(&third_call),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut extra_helper = two_call_executable_nominal_affine_module();
    let mut unused = extra_helper.machines[3].clone();
    unused.id = machine_id(5);
    unused.entry = block_id(5);
    unused.blocks[0].id = block_id(5);
    unused.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    unused.contract.id = contract_id(5);
    extra_helper.machines.push(unused);
    assert!(matches!(
        validate_module(&extra_helper),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));
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
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                arguments: Vec::new(),
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
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    32,
                )
                .unwrap(),
            )),
            relevance: terminal_psi::BindingRelevance::Relevant,
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
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                relevance: terminal_psi::BindingRelevance::Relevant,
            },
            StructuralFieldDeclaration {
                identity: "payload".into(),
                id: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                )),
                relevance: terminal_psi::BindingRelevance::Relevant,
            },
        ],
    };
    validate_module(&module).expect("two primitive-field nominal cleanup should validate");
}

#[test]
fn erased_ieee_float_structural_field_requires_an_opaque_semantic_type() {
    let mut module = nominal_affine_module();
    let structural_type = module.structural_types[0].id;
    let field = semantic_vocabulary::StructuralFieldId::new(1).expect("field identity");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            identity: "proof_float".into(),
            id: field,
            field_type: StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
            relevance: terminal_psi::BindingRelevance::Erased,
        }],
    };

    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidErasedStructuralField {
            structural_type: actual_type,
            field: actual_field,
        }) if actual_type == structural_type && actual_field == field
    ));
}

#[test]
fn wide_flat_primitive_nominal_affine_cleanup_validates() {
    let mut module = nominal_affine_module();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: (1..=5)
            .map(|index| StructuralFieldDeclaration {
                identity: format!("payload_{index}"),
                id: semantic_vocabulary::StructuralFieldId::new(index).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                )),
                relevance: terminal_psi::BindingRelevance::Relevant,
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
            qualifications: Default::default(),
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
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    128,
                )
                .unwrap(),
            )),
            relevance: terminal_psi::BindingRelevance::Relevant,
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
fn structural_call_access_is_exact_and_cannot_widen() {
    let mut attenuation = hard_root_module();
    attenuation.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    attenuation.machines[1].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    attenuation.boundary_machines[0].structural_parameters[0].access =
        StructuralAccess::WriteOnlyBorrow;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut attenuation.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::WriteOnlyBorrow;
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut attenuation.machines[1].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::WriteOnlyBorrow;
    validate_module(&attenuation).expect("mutable access may attenuate to write-only");

    let mut mismatched = attenuation.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut mismatched.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert_eq!(
        validate_module(&mismatched).unwrap_err(),
        ModuleError::StructuralArgumentAccessMismatch {
            operation: operation_id(1),
            argument_index: 0,
            expected: StructuralAccess::WriteOnlyBorrow,
            actual: StructuralAccess::SharedBorrow,
        }
    );

    let mut widened = attenuation;
    widened.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert_eq!(
        validate_module(&widened).unwrap_err(),
        ModuleError::StructuralArgumentAccessExceedsSource {
            operation: operation_id(1),
            argument_index: 0,
            source: StructuralAccess::SharedBorrow,
            presented: StructuralAccess::WriteOnlyBorrow,
        }
    );
}

#[test]
fn structural_call_rejects_overlapping_exclusive_arguments() {
    let mut module = hard_root_module();
    module.machines[0].entry_claims.clear();
    module.machines[1].entry_claims.clear();
    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    module.machines[1].structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    module.machines[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let mut second_target = module.machines[1].structural_parameters[0].clone();
    second_target.place = place_id(99);
    second_target.position = 1;
    module.machines[1].structural_parameters.push(second_target);
    let mut second_place = module.machines[1].structural_places[0];
    second_place.id = place_id(99);
    second_place.kind = StructuralPlaceKind::Parameter {
        position: 1,
        is_self: false,
    };
    module.machines[1].structural_places.push(second_place);
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers.clear();
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    structural_arguments.push(structural_arguments[0].clone());

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OverlappingExclusiveStructuralArguments {
            operation: operation_id(1),
            first_argument: 0,
            second_argument: 1,
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
        id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
        identity: "#7".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
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
        id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
        identity: "#7".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        panic!("expected record shape")
    };
    fields.push(StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
        identity: "#9".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
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
            id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
            identity: "#7".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(2)),
        },
        StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(2).expect("field identity"),
            identity: "#9".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
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
    boundary_call_mut(&mut module).push(CompletionReceipt {
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

    boundary_call_mut(&mut settled_at_boundary).clear();
    assert_eq!(
        validate_module(&settled_at_boundary).unwrap_err(),
        ModuleError::BoundaryCompletionReceiptMismatch(operation_id(3))
    );
}

#[test]
fn ordinary_unit_calls_cannot_mint_or_drop_content_claims() {
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
        .projection_report_fingerprint ^= 1;
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
        .push(terminal_psi::StructuralPathSegment::FixedIndex(0));
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
    structural_arguments[0].path = vec![terminal_psi::StructuralPathSegment::FixedIndex(1)];
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
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(2).unwrap(),
            identity: "ArrayPending".into(),
            carrier: structural_type_id(3),
            content_projection: None,
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
        id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
        identity: "payload".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    nested_callee_claim.machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    nested_callee_claim.machines[0].entry_claims[0]
        .path
        .push(terminal_psi::StructuralPathSegment::Field("payload".into()));
    nested_callee_claim.machines[1].entry_claims[0].path =
        vec![terminal_psi::StructuralPathSegment::Field("payload".into())];
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
fn projected_linear_move_cannot_return_its_partial_ancestor() {
    let mut module = two_element_projected_unit_call_module();
    let result_place = place_id(4);
    module.machines[0].result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type: structural_type_id(3),
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::Result,
        });
    module.machines[0].blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(1),
        source: place_id(1),
        returned_claims: vec![claim_id(2)],
        trivial_affine_discards: Vec::new(),
    };

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::ProjectedUnitCallOutsideBoundedSlice {
            operation: operation_id(1),
        }
    );
}

#[test]
fn complete_dense_projected_linear_consumption_closes_the_partial_frontier() {
    let mut module = two_element_projected_unit_call_module();
    let mut second = module.machines[0].blocks[0].operations[0].clone();
    second.id = operation_id(4);
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut second.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    claim_transfers[0].claim = claim_id(2);
    module.machines[0].blocks[0].operations.push(second);

    validate_module(&module)
        .expect("moving the complete dense sibling set should exhaust the linear array root");
}

#[test]
fn linear_projected_custody_survives_an_empty_jump() {
    let mut module = two_element_projected_unit_call_module();
    let mut second = module.machines[0].blocks[0].operations[0].clone();
    second.id = operation_id(4);
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut second.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    claim_transfers[0].claim = claim_id(2);
    module.machines[0].blocks[0].terminator = Terminator::Jump {
        structural_arguments: Vec::new(),
        edge: edge_id(1),
        target: block_id(3),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    module.machines[0].blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: block_id(3),
        parameters: Vec::new(),
        operations: vec![second],
        terminator: Terminator::ReturnUnit {
            edge: edge_id(3),
            trivial_affine_discards: Vec::new(),
        },
    });
    validate_module(&module).expect("a linear partial root survives an ordinary empty edge");
    let frontiers = reconstruct_structural_ownership_frontiers(&module).unwrap();
    let caller = frontiers.machine(machine_id(1)).unwrap();
    let continuation = caller.edge_exit(edge_id(1)).unwrap();
    assert_eq!(continuation, caller.block_entry(block_id(3)).unwrap());
    assert_eq!(continuation.owned_places().len(), 1);
    assert_eq!(continuation.owned_places()[0].place, place_id(1));
    assert_eq!(continuation.claims().len(), 1);
    assert_eq!(continuation.claims()[0].claim, claim_id(2));
    assert_eq!(continuation.partial_custody().len(), 1);
    assert_eq!(continuation.partial_custody()[0].place, place_id(1));
    assert_eq!(
        continuation.partial_custody()[0].moved_paths,
        vec![vec![StructuralPathSegment::FixedIndex(0)]],
    );
    let completed = caller.operation_exit(operation_id(4)).unwrap();
    assert!(completed.owned_places().is_empty());
    assert!(completed.claims().is_empty());
    assert!(completed.partial_custody().is_empty());
}

#[test]
fn projected_unit_calls_reject_signatures_outside_the_bounded_slice() {
    let mut scalar_caller = projected_unit_call_module();
    scalar_caller.machines[0].parameters.push(ValueDeclaration {
        qualifications: Default::default(),
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
fn projected_unit_call_crash_routes_prepend_the_canonical_argument_field_path() {
    let mut module = partial_affine_field_module();
    let flag = semantic_vocabulary::StructuralFieldId::new(4).expect("field identity");
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields.push(StructuralFieldDeclaration {
        id: flag,
        identity: "should_abort".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    let callee_route = CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(place_id(2), flag),
            ),
        ))],
    };
    let right = semantic_vocabulary::StructuralFieldId::new(3).expect("field identity");
    let caller_route = CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field_path(
                    place_id(1),
                    vec![
                        CanonicalStructuralPathSegment::Field(right),
                        CanonicalStructuralPathSegment::Field(flag),
                    ],
                ),
            ),
        ))],
    };
    module.machines[0].contract.crash_routes = vec![caller_route.clone()];
    module.machines[1].contract.crash_routes = vec![callee_route];
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![caller_route];
    validate_module(&module).expect("projected member route rebases through the right field");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("projected member route needs no producer-authored proof");

    let left = semantic_vocabulary::StructuralFieldId::new(1).expect("field identity");
    let redirected = CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field_path(
                    place_id(1),
                    vec![
                        CanonicalStructuralPathSegment::Field(left),
                        CanonicalStructuralPathSegment::Field(flag),
                    ],
                ),
            ),
        ))],
    };
    module.machines[0].contract.crash_routes = vec![redirected.clone()];
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![redirected];
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn direct_field_partial_affine_return_validates_and_verifies() {
    let module = partial_affine_field_module();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &module.machines[0].blocks[0].terminator
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
        vec!["middle", "left"]
    );
    validate_module(&module).expect("direct moved field plus residual cleanup exhausts the root");
    let frontiers = reconstruct_structural_ownership_frontiers(&module)
        .expect("verifier exposes its path-sensitive frontier walk");
    let caller = frontiers.machine(machine_id(1)).expect("caller frontier");
    let operation_entry = caller
        .operation_entry(operation_id(1))
        .expect("projected call entry frontier");
    assert_eq!(operation_entry.owned_places()[0].place, place_id(1));
    assert!(operation_entry.partial_custody().is_empty());
    let operation_exit = caller
        .operation_exit(operation_id(1))
        .expect("projected call exit frontier");
    assert_eq!(operation_exit.owned_places()[0].place, place_id(1));
    assert_eq!(operation_exit.partial_custody().len(), 1);
    assert_eq!(
        operation_exit.partial_custody()[0].moved_paths,
        vec![vec![StructuralPathSegment::Field("right".into())]]
    );
    assert_eq!(
        caller.edge_entry(edge_id(1)),
        Some(operation_exit),
        "return cleanup begins from the exact post-operation frontier"
    );
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("partial affine cleanup introduces no producer-authored proposition");
    assert_eq!(verified.structural_frontiers(), &frontiers);
}

#[test]
fn projected_move_blocks_later_whole_root_use() {
    let mut module = two_element_projected_unit_call_module();
    let mut whole_callee = module.machines[0].clone();
    whole_callee.id = machine_id(3);
    whole_callee.structural_parameters[0].place = place_id(4);
    whole_callee.structural_places[0].id = place_id(4);
    for claim in &mut whole_callee.entry_claims {
        claim.input = place_id(4);
    }
    whole_callee.entry = block_id(3);
    whole_callee.blocks[0].id = block_id(3);
    whole_callee.contract.id = contract_id(3);
    let mut first = whole_callee.blocks[0].operations[0].clone();
    first.id = operation_id(4);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut first.kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = place_id(4);
    let mut second = first.clone();
    second.id = operation_id(5);
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut second.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    claim_transfers[0].claim = claim_id(2);
    whole_callee.blocks[0].operations = vec![first, second];
    whole_callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    module.machines.push(whole_callee);
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(6),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(3),
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: vec![
                ClaimTransfer {
                    claim: claim_id(1),
                    argument_index: 0,
                },
                ClaimTransfer {
                    claim: claim_id(2),
                    argument_index: 0,
                },
            ],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::PartiallyMovedStructuralPlaceUsedWholeAtOperation {
            operation: operation_id(6),
            place: place_id(1),
        }
    );
}
