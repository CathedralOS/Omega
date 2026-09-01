use psi_core::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentDomainId, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, ContentTerm, ContractId, EdgeId, EvidenceIdentity, IeeeFloatFormat,
    IntegerSign, IntegerType, MachineId, ObligationId, OperationId, PlaceId, Proposition,
    ScalarTerm, ScalarType, ServiceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer, CompletionReceipt,
    ContentEntryClaim, ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket,
    CrashRouteGuard, EntryClaim, MachineContract, NominalAffineCleanup, Operation, OperationKind,
    OperationResult, ServiceDeclaration, StructuralAccess, StructuralAffineDiscard,
    StructuralArgument, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, ServiceCeilingOwner,
    reconstruct_operation_obligations, reconstruct_structural_ownership_frontiers, validate_module,
    verify_module,
};

#[test]
fn direct_write_only_primitive_store_is_total_and_preserves_custody() {
    let module = write_only_primitive_store_module();
    validate_module(&module).expect("exact whole primitive write-only stores should validate");
    assert!(
        reconstruct_operation_obligations(&module)
            .expect("total stores reconstruct")
            .is_empty()
    );

    let frontiers = reconstruct_structural_ownership_frontiers(&module)
        .expect("write-only stores have verifier-owned frontier snapshots");
    let machine = frontiers.machine(machine_id(1)).expect("store machine");
    for operation in [operation_id(1), operation_id(2)] {
        assert_eq!(
            machine.operation_entry(operation),
            machine.operation_exit(operation),
            "a store keeps structural custody unchanged",
        );
    }
}

#[test]
fn direct_mutable_primitive_store_is_total_and_preserves_custody() {
    let mut module = write_only_primitive_store_module();
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    validate_module(&module).expect("a non-observing store may use readable mutable authority");
    assert!(
        reconstruct_operation_obligations(&module)
            .expect("total stores reconstruct")
            .is_empty()
    );
}

#[test]
fn direct_write_only_primitive_store_rejects_custody_shape_and_value_mutations() {
    let mut wrong_access = write_only_primitive_store_module();
    wrong_access.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        validate_module(&wrong_access),
        Err(ModuleError::WriteOnlyPrimitiveStoreDestinationMismatch {
            operation,
            place,
        }) if operation == operation_id(1) && place == place_id(1)
    ));

    let mut wrong_multiplicity = write_only_primitive_store_module();
    wrong_multiplicity.machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    assert!(matches!(
        validate_module(&wrong_multiplicity),
        Err(ModuleError::WriteOnlyPrimitiveStoreDestinationMismatch {
            operation,
            place,
        }) if operation == operation_id(1) && place == place_id(1)
    ));

    let mut qualified = write_only_primitive_store_module();
    qualified
        .structural_domains
        .push(StructuralDomainDeclaration {
            id: domain_id(1),
            semantic_domain: psi_core::DomainSemanticId::new(1).expect("domain semantic identity"),
            identity: "QualifiedPrimitive".into(),
            carrier: structural_type_id(1),
            content_projection: None,
        });
    qualified.machines[0].structural_parameters[0]
        .qualifications
        .push(domain_id(1));
    assert!(matches!(
        validate_module(&qualified),
        Err(ModuleError::WriteOnlyPrimitiveStoreDestinationMismatch {
            operation,
            place,
        }) if operation == operation_id(1) && place == place_id(1)
    ));

    let mut unknown_destination = write_only_primitive_store_module();
    let OperationKind::WriteOnlyPrimitiveStore { destination, .. } =
        &mut unknown_destination.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *destination = place_id(9);
    assert!(matches!(
        validate_module(&unknown_destination),
        Err(ModuleError::WriteOnlyPrimitiveStoreDestinationMismatch {
            operation,
            place,
        }) if operation == operation_id(1) && place == place_id(9)
    ));

    let mut wrong_shape = write_only_primitive_store_module();
    wrong_shape.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
    assert!(matches!(
        validate_module(&wrong_shape),
        Err(ModuleError::WriteOnlyPrimitiveStoreRequiresPrimitiveScalar {
            operation,
            structural_type,
        }) if operation == operation_id(1) && structural_type == structural_type_id(1)
    ));

    let mut wrong_type = write_only_primitive_store_module();
    wrong_type.machines[0].parameters[0].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&wrong_type),
        Err(ModuleError::WriteOnlyPrimitiveStoreValueTypeMismatch {
            operation,
            expected,
            actual,
        }) if operation == operation_id(1)
            && expected == signed_i8()
            && actual == ScalarType::Boolean
    ));

    let mut late_value = write_only_primitive_store_module();
    late_value.machines[0].parameters.clear();
    late_value.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(3),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(1),
            scalar_type: signed_i8(),
        }),
        kind: OperationKind::IntegerConstant {
            value: psi_core::IntegerValue::Signed(7),
        },
    });
    assert_eq!(
        validate_module(&late_value).unwrap_err(),
        ModuleError::ValueUsedBeforeDefinition(value_id(1)),
    );

    let mut forged_result = write_only_primitive_store_module();
    forged_result.machines[0].blocks[0].operations[0].result =
        OperationResult::Scalar(ValueDeclaration {
            id: value_id(2),
            scalar_type: signed_i8(),
        });
    assert_eq!(
        validate_module(&forged_result).unwrap_err(),
        ModuleError::UnitOperationHasScalarResult(operation_id(1)),
    );
}

#[test]
fn exact_empty_nominal_affine_cleanup_validates() {
    validate_module(&nominal_affine_module()).expect("exact empty nominal cleanup should validate");
}

#[test]
fn contextual_nominal_affine_cleanup_reconstructs_and_discharges_receiver_requirement() {
    let module = contextual_nominal_affine_module();
    validate_module(&module).expect("contextual nominal cleanup shape should validate");
    let expected = Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field(
            place_id(1),
            psi_core::StructuralFieldId::new(1).expect("field"),
        ),
    );
    let obligations = reconstruct_operation_obligations(&module).expect("cleanup obligation");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, obligation_id(1));
    assert_eq!(obligations[0].obligation.proposition, expected);

    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == obligation_id(1)
    ));
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: expected,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("caller requirement discharges contextual cleanup premise");

    let wrong_bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(2).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Falsehood,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    assert!(matches!(
        verify_module(&module, &wrong_bundle, &AdmissionProfile::default()),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == obligation_id(1)
    ));
}

#[test]
fn scalar_return_nominal_cleanup_reconstructs_target_requirement() {
    let mut module = contextual_nominal_affine_module();
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = std::mem::replace(
        &mut caller.blocks[0].terminator,
        Terminator::ReturnUnit {
            edge: edge_id(99),
            trivial_affine_discards: Vec::new(),
        },
    ) else {
        unreachable!()
    };
    caller.blocks[0].terminator = Terminator::Return {
        edge,
        value: value_id(10),
        cleanup_actions: cleanups
            .into_iter()
            .map(TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };
    validate_module(&module).expect("scalar contextual cleanup shape validates");
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, obligation_id(1));
    assert_eq!(
        obligations[0].obligation.proposition,
        module.machines[0].contract.requires[0]
    );
}

#[test]
fn scalar_return_contextual_cleanups_require_reverse_root_order() {
    let mut module = two_root_shared_contextual_nominal_affine_module();
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = std::mem::replace(
        &mut caller.blocks[0].terminator,
        Terminator::ReturnUnit {
            edge: edge_id(99),
            trivial_affine_discards: Vec::new(),
        },
    ) else {
        unreachable!()
    };
    caller.blocks[0].terminator = Terminator::Return {
        edge,
        value: value_id(10),
        cleanup_actions: cleanups
            .into_iter()
            .map(TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };
    validate_module(&module).expect("ordered scalar contextual cleanups validate");
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    let first = psi_core::StructuralFieldId::new(1).expect("first field");
    let second = psi_core::StructuralFieldId::new(2).expect("second field");
    let expected = [
        (3, place_id(2), first),
        (4, place_id(2), second),
        (1, place_id(1), first),
        (2, place_id(1), second),
    ];
    assert_eq!(obligations.len(), expected.len());
    for (reconstructed, (identity, root, field)) in obligations.iter().zip(expected) {
        assert_eq!(reconstructed.obligation.id, obligation_id(identity));
        assert_eq!(
            reconstructed.obligation.proposition,
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, field),
            )
        );
    }
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: obligations
            .into_iter()
            .enumerate()
            .map(|(index, reconstructed)| ObligationEvidence {
                obligation: reconstructed.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(index as u64 + 1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: reconstructed.obligation.proposition.clone(),
                        rule: ProofRule::Assumption {
                            index: module.machines[0]
                                .contract
                                .requires
                                .iter()
                                .position(|requirement| {
                                    requirement == &reconstructed.obligation.proposition
                                })
                                .expect("scalar cleanup goal is a caller premise"),
                        },
                    },
                }),
            })
            .collect(),
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("reverse-ordered scalar cleanup premises discharge per owned root");

    let Terminator::Return {
        cleanup_actions, ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanup_actions.reverse();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::ScalarReturnAffineDiscardsMismatch {
            machine: machine_id(1),
            block: block_id(1),
        }
    );
}

#[test]
fn contextual_nominal_affine_cleanup_reconstructs_and_discharges_false_receiver_requirement() {
    let mut module = contextual_nominal_affine_module();
    let field = psi_core::StructuralFieldId::new(1).expect("field");
    let caller_requirement = Proposition::Equal(
        ScalarTerm::boolean(false),
        ScalarTerm::boolean_field(place_id(1), field),
    );
    let target_requirement = Proposition::Equal(
        ScalarTerm::boolean(false),
        ScalarTerm::boolean_field(place_id(99), field),
    );
    module.machines[0].contract.requires = vec![caller_requirement.clone()];
    module.machines[1].contract.requires = vec![target_requirement];

    validate_module(&module).expect("a false-polarity cleanup requirement should validate");
    let obligations = reconstruct_operation_obligations(&module).expect("cleanup obligation");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, obligation_id(1));
    assert_eq!(obligations[0].obligation.proposition, caller_requirement);

    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: caller_requirement,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the matching false caller fact discharges the cleanup premise");
}

#[test]
fn contextual_nominal_affine_cleanup_orders_mixed_polarities_before_field_bytes() {
    let mut module = two_requirement_contextual_nominal_affine_module();
    let first = psi_core::StructuralFieldId::new(1).expect("first field");
    let second = psi_core::StructuralFieldId::new(2).expect("second field");
    let caller_requirements = [
        Proposition::Equal(
            ScalarTerm::boolean(false),
            ScalarTerm::boolean_field(place_id(1), second),
        ),
        Proposition::Equal(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_field(place_id(1), first),
        ),
    ];
    let target_requirements = [
        Proposition::Equal(
            ScalarTerm::boolean(false),
            ScalarTerm::boolean_field(place_id(99), second),
        ),
        Proposition::Equal(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_field(place_id(99), first),
        ),
    ];
    module.machines[0].contract.requires = caller_requirements.to_vec();
    module.machines[1].contract.requires = target_requirements.to_vec();

    validate_module(&module)
        .expect("encoded false polarity sorts before true independently of field identity");
    let obligations = reconstruct_operation_obligations(&module).expect("cleanup obligations");
    assert_eq!(obligations.len(), 2);
    for (index, expected) in caller_requirements.into_iter().enumerate() {
        assert_eq!(obligations[index].obligation.proposition, expected);
    }

    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: obligations
            .into_iter()
            .enumerate()
            .map(|(index, reconstructed)| ObligationEvidence {
                obligation: reconstructed.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(index as u64 + 1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: reconstructed.obligation.proposition,
                        rule: ProofRule::Assumption { index },
                    },
                }),
            })
            .collect(),
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("mixed-polarity caller facts discharge in canonical order");

    let mut reversed = module.clone();
    reversed.machines[1].contract.requires.reverse();
    assert!(matches!(
        validate_module(&reversed),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut duplicate_key = module;
    duplicate_key.machines[1].contract.requires[1] =
        duplicate_key.machines[1].contract.requires[0].clone();
    assert!(matches!(
        validate_module(&duplicate_key),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));
}

#[test]
fn contextual_nominal_affine_cleanup_reconstructs_finite_ordered_requirements() {
    let module = two_requirement_contextual_nominal_affine_module();
    validate_module(&module).expect("two contextual cleanup requirements should validate");

    let expected = [
        Proposition::Equal(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_field(
                place_id(1),
                psi_core::StructuralFieldId::new(1).expect("first field"),
            ),
        ),
        Proposition::Equal(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_field(
                place_id(1),
                psi_core::StructuralFieldId::new(2).expect("second field"),
            ),
        ),
    ];
    let obligations = reconstruct_operation_obligations(&module).expect("cleanup obligations");
    assert_eq!(obligations.len(), 2);
    for (index, obligation) in obligations.iter().enumerate() {
        assert_eq!(obligation.obligation.id, obligation_id(index as u64 + 1));
        assert_eq!(obligation.obligation.proposition, expected[index]);
    }

    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: expected
            .into_iter()
            .enumerate()
            .map(|(index, conclusion)| ObligationEvidence {
                obligation: obligation_id(index as u64 + 1),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(index as u64 + 1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion,
                        rule: ProofRule::Assumption { index },
                    },
                }),
            })
            .collect(),
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("both ordered caller facts discharge the cleanup requirements");
}

#[test]
fn contextual_nominal_affine_cleanup_rejects_malformed_finite_requirements() {
    let invalid = |module: &TerminalModule| ModuleError::InvalidNominalAffineCleanup {
        machine: module.machines[0].id,
        block: module.machines[0].blocks[0].id,
    };

    let mut reordered = two_requirement_contextual_nominal_affine_module();
    reordered.machines[1].contract.requires.reverse();
    assert_eq!(
        validate_module(&reordered).unwrap_err(),
        invalid(&reordered)
    );

    let mut duplicate = two_requirement_contextual_nominal_affine_module();
    duplicate.machines[1].contract.requires[1] = duplicate.machines[1].contract.requires[0].clone();
    assert_eq!(
        validate_module(&duplicate).unwrap_err(),
        invalid(&duplicate)
    );

    let mut mixed_receiver = two_requirement_contextual_nominal_affine_module();
    let Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) =
        &mut mixed_receiver.machines[1].contract.requires[1]
    else {
        unreachable!()
    };
    *root = place_id(98);
    assert_eq!(
        validate_module(&mixed_receiver).unwrap_err(),
        invalid(&mixed_receiver)
    );

    let mut wrong_type = two_requirement_contextual_nominal_affine_module();
    let StructuralTypeShape::Record { fields } = &mut wrong_type.structural_types[0].shape else {
        unreachable!()
    };
    fields[1].field_type = StructuralFieldType::Scalar(ScalarType::Integer(
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8"),
    ));
    assert_eq!(
        validate_module(&wrong_type).unwrap_err(),
        invalid(&wrong_type)
    );

    let mut missing_obligation = two_requirement_contextual_nominal_affine_module();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut missing_obligation.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].requirement_obligations.pop();
    assert_eq!(
        validate_module(&missing_obligation).unwrap_err(),
        invalid(&missing_obligation)
    );
}

#[test]
fn contextual_nominal_affine_cleanup_uses_canonical_field_bytes_across_id_rollover() {
    let mut module = two_requirement_contextual_nominal_affine_module();
    let first = psi_core::StructuralFieldId::new(1).expect("first field");
    let rollover = psi_core::StructuralFieldId::new(256).expect("rollover field");
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields.truncate(2);
    fields[1].id = rollover;
    let receiver = place_id(99);
    module.machines[1].contract.requires = [rollover, first]
        .into_iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(receiver, field),
            )
        })
        .collect();
    module.machines[0].contract.requires = [rollover, first]
        .into_iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(place_id(1), field),
            )
        })
        .collect();
    validate_module(&module).expect("little-endian proposition bytes define canonical order");

    module.machines[1].contract.requires.reverse();
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));
}

#[test]
fn contextual_nominal_affine_cleanup_rejects_forged_requirement_binding() {
    let expected_invalid = |module: &TerminalModule| ModuleError::InvalidNominalAffineCleanup {
        machine: module.machines[0].id,
        block: module.machines[0].blocks[0].id,
    };
    let mut missing_obligation = contextual_nominal_affine_module();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut missing_obligation.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].requirement_obligations.clear();
    assert_eq!(
        validate_module(&missing_obligation).unwrap_err(),
        expected_invalid(&missing_obligation)
    );

    let mut wrong_receiver = contextual_nominal_affine_module();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut wrong_receiver.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(place_id(98));
    assert_eq!(
        validate_module(&wrong_receiver).unwrap_err(),
        expected_invalid(&wrong_receiver)
    );

    let mut wrong_field = contextual_nominal_affine_module();
    let Proposition::Equal(_, ScalarTerm::BooleanField { path, .. }) =
        &mut wrong_field.machines[1].contract.requires[0]
    else {
        unreachable!()
    };
    path[0] =
        CanonicalStructuralPathSegment::Field(psi_core::StructuralFieldId::new(2).expect("field"));
    assert_eq!(
        validate_module(&wrong_field).unwrap_err(),
        expected_invalid(&wrong_field)
    );

    let mut executable_receiver = contextual_nominal_affine_module();
    executable_receiver.machines[1].contract.requires[0] = Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field(
            place_id(1),
            psi_core::StructuralFieldId::new(1).expect("field"),
        ),
    );
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut executable_receiver.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(place_id(1));
    assert!(validate_module(&executable_receiver).is_err());

    let mut reversed = contextual_nominal_affine_module();
    let Proposition::Equal(left, right) = &mut reversed.machines[1].contract.requires[0] else {
        unreachable!()
    };
    std::mem::swap(left, right);
    assert_eq!(
        validate_module(&reversed).unwrap_err(),
        expected_invalid(&reversed)
    );

    let mut malformed_expected_term = contextual_nominal_affine_module();
    let Proposition::Equal(left, _) = &mut malformed_expected_term.machines[1].contract.requires[0]
    else {
        unreachable!()
    };
    *left = ScalarTerm::BooleanNot {
        operand: Box::new(ScalarTerm::boolean(false)),
    };
    assert_eq!(
        validate_module(&malformed_expected_term).unwrap_err(),
        expected_invalid(&malformed_expected_term)
    );

    let mut extra_requirement = contextual_nominal_affine_module();
    extra_requirement.machines[1]
        .contract
        .requires
        .push(Proposition::Truth);
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut extra_requirement.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].requirement_obligations.push(obligation_id(2));
    assert_eq!(
        validate_module(&extra_requirement).unwrap_err(),
        expected_invalid(&extra_requirement)
    );
}

#[test]
fn shared_contextual_cleanup_target_reconstructs_each_goal_per_owned_root() {
    let module = two_root_shared_contextual_nominal_affine_module();
    let first = psi_core::StructuralFieldId::new(1).expect("first field");
    let second = psi_core::StructuralFieldId::new(2).expect("second field");

    validate_module(&module).expect("a shared contextual target retains place-specific custody");
    let obligations = reconstruct_operation_obligations(&module).expect("cleanup obligations");
    assert_eq!(obligations.len(), 4);
    let expected = [
        (3, place_id(2), first),
        (4, place_id(2), second),
        (1, place_id(1), first),
        (2, place_id(1), second),
    ];
    for (obligation, (identity, root, field)) in obligations.iter().zip(expected) {
        assert_eq!(obligation.obligation.id, obligation_id(identity));
        assert_eq!(
            obligation.obligation.proposition,
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, field),
            )
        );
    }

    let mut duplicate = module;
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut duplicate.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].requirement_obligations = vec![obligation_id(1), obligation_id(4)];
    assert_eq!(
        validate_module(&duplicate).unwrap_err(),
        ModuleError::DuplicateObligation(obligation_id(1))
    );
}

#[test]
fn distinct_contextual_cleanup_targets_use_distinct_receivers_and_reconstruct_each_root() {
    let module = two_root_distinct_contextual_nominal_affine_module();
    validate_module(&module).expect("distinct contextual targets use independent proof receivers");

    let first = psi_core::StructuralFieldId::new(1).expect("first field");
    let second = psi_core::StructuralFieldId::new(2).expect("second field");
    let obligations = reconstruct_operation_obligations(&module).expect("cleanup obligations");
    let expected = [
        (3, place_id(2), first),
        (4, place_id(2), second),
        (1, place_id(1), first),
        (2, place_id(1), second),
    ];
    assert_eq!(obligations.len(), expected.len());
    for (obligation, (identity, root, field)) in obligations.iter().zip(expected) {
        assert_eq!(obligation.obligation.id, obligation_id(identity));
        assert_eq!(
            obligation.obligation.proposition,
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, field),
            )
        );
    }

    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: obligations
            .into_iter()
            .enumerate()
            .map(|(index, reconstructed)| ObligationEvidence {
                obligation: reconstructed.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(index as u64 + 1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: reconstructed.obligation.proposition.clone(),
                        rule: ProofRule::Assumption {
                            index: module.machines[0]
                                .contract
                                .requires
                                .iter()
                                .position(|requirement| {
                                    requirement == &reconstructed.obligation.proposition
                                })
                                .expect("reconstructed requirement is a caller premise"),
                        },
                    },
                }),
            })
            .collect(),
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("root-specific caller premises discharge both contextual targets");
}

#[test]
fn distinct_contextual_cleanup_targets_reject_reused_receiver() {
    let mut module = two_root_distinct_contextual_nominal_affine_module();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    let reused_receiver = cleanups[1].cleanup_receiver.expect("first target receiver");
    let second_target = cleanups[0].cleanup_machine;
    cleanups[0].cleanup_receiver = Some(reused_receiver);
    let target = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == second_target)
        .expect("second cleanup target");
    for requirement in &mut target.contract.requires {
        let Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) = requirement else {
            unreachable!()
        };
        *root = reused_receiver;
    }

    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));
}

#[test]
fn shared_contextual_cleanup_target_rejects_one_action_changing_receiver() {
    let mut module = two_root_shared_contextual_nominal_affine_module();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(place_id(98));

    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));
}

#[test]
fn two_nominal_affine_roots_validate_in_reverse_order_and_may_share_a_target() {
    let module = two_root_nominal_affine_module();
    validate_module(&module).expect("two ordered nominal cleanup roots should validate");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("two ordered nominal cleanup roots require no proof evidence");

    let mut reordered = module;
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups.reverse();
    assert!(matches!(
        validate_module(&reordered),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));
}

#[test]
fn five_nominal_affine_roots_validate_in_reverse_order() {
    let module = five_root_nominal_affine_module();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        cleanups
            .iter()
            .map(|cleanup| cleanup.place)
            .collect::<Vec<_>>(),
        vec![
            place_id(5),
            place_id(4),
            place_id(3),
            place_id(2),
            place_id(1)
        ]
    );
    validate_module(&module).expect("five ordered nominal cleanup roots should validate");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("five shared cleanup targets require no proof evidence");
}

#[test]
fn two_nominal_affine_roots_allow_distinct_and_shared_executable_cleanup_bodies() {
    let module = two_root_one_executable_nominal_affine_module();
    validate_module(&module).expect("one executable cleanup action should validate");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("one executable cleanup action requires no proof evidence");

    let mut two_executable = module.clone();
    let mut second_helper = two_executable.machines[3].clone();
    second_helper.id = machine_id(5);
    second_helper.entry = block_id(5);
    second_helper.blocks[0].id = block_id(5);
    second_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    second_helper.contract.id = contract_id(5);
    two_executable.machines[2].blocks[0]
        .operations
        .push(Operation {
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: second_helper.id,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
    two_executable.machines.push(second_helper);
    validate_module(&two_executable).expect("two distinct executable cleanup bodies validate");
    verify_module(
        &two_executable,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("two distinct executable cleanup bodies verify");

    let mut shared_executable = two_root_nominal_affine_module();
    let mut helper = shared_executable.machines[1].clone();
    helper.id = machine_id(3);
    helper.entry = block_id(3);
    helper.blocks[0].id = block_id(3);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(3);
    shared_executable.machines[1].blocks[0]
        .operations
        .push(Operation {
            id: operation_id(1),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: helper.id,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
    shared_executable.machines.push(helper);
    validate_module(&shared_executable).expect("shared executable cleanup target validates");
    verify_module(
        &shared_executable,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("shared executable cleanup target and helper verify once as a closure");
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
            id: operation_id(3),
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
        validate_module(&nonempty_second),
        Err(ModuleError::InvalidNominalAffineCleanup { .. })
    ));

    let mut third_call = two_call_executable_nominal_affine_module();
    third_call.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(3),
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
fn erased_ieee_float_structural_field_requires_an_opaque_semantic_type() {
    let mut module = nominal_affine_module();
    let structural_type = module.structural_types[0].id;
    let field = psi_core::StructuralFieldId::new(1).expect("field identity");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            identity: "proof_float".into(),
            id: field,
            field_type: StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
            relevance: psi_terminal::BindingRelevance::Erased,
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
    let mut second_place = module.machines[1].structural_places[0].clone();
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
            semantic_domain: psi_core::DomainSemanticId::new(2).unwrap(),
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
fn projected_linear_move_cannot_return_its_partial_ancestor() {
    let mut module = two_element_projected_unit_call_module();
    let result_place = place_id(4);
    module.machines[0].result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: result_place,
        structural_type: structural_type_id(3),
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
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
fn projected_unit_call_crash_routes_prepend_the_canonical_argument_field_path() {
    let mut module = partial_affine_field_module();
    let flag = psi_core::StructuralFieldId::new(4).expect("field identity");
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields.push(StructuralFieldDeclaration {
        id: flag,
        identity: "should_abort".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
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
    let right = psi_core::StructuralFieldId::new(3).expect("field identity");
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

    let left = psi_core::StructuralFieldId::new(1).expect("field identity");
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
        id: operation_id(6),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
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
        projection: content_owner_projection().identity,
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
    module.root_service_reach.concrete.clear();
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
        id: domain_id(2),
        semantic_domain: psi_core::DomainSemanticId::new(2).unwrap(),
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
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(2))],
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach.concrete.clear();
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
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(2))],
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
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(4))],
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach.concrete.clear();
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
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                access: StructuralAccess::Owned,
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
                access: StructuralAccess::Owned,
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

fn signed_i8() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("i8"))
}

fn write_only_primitive_store_module() -> TerminalModule {
    let structural_type = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "WriteOnlyI8".into(),
        shape: StructuralTypeShape::PrimitiveScalar(signed_i8()),
    };
    let parameter = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: structural_type.id,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
    };
    let destination = parameter.place;
    let store = |raw| Operation {
        id: operation_id(raw),
        result: OperationResult::Unit,
        kind: OperationKind::WriteOnlyPrimitiveStore {
            destination,
            value: value_id(1),
        },
    };
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: vec![ValueDeclaration {
            id: value_id(1),
            scalar_type: signed_i8(),
        }],
        structural_parameters: vec![parameter],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![store(1), store(2)],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        structural_types: vec![structural_type],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: psi_terminal::TerminalRootServiceReach {
            concrete: Vec::new(),
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine],
    }
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
        semantic_domain: psi_core::DomainSemanticId::new(1).unwrap(),
        identity: "Pending".into(),
        carrier: resource.id,
        content_projection: Some(content_owner_projection()),
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
        scalar_parameters: Vec::new(),
        structural_parameters: vec![boundary_parameter],
        result: None,
        requires: vec![StructuralDomainRequirement {
            argument_index: 0,
            domain: pending.id,
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: vec![port_io.id],
    };

    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(1))],
        ranked_scc: None,
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
                        access: StructuralAccess::Owned,
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
        ranked_scc: None,
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
                    kind: OperationKind::BoundaryCall {
                        boundary: boundary.id,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: place_id(2),
                            access: StructuralAccess::Owned,
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
        root_service_reach: psi_terminal::TerminalRootServiceReach {
            concrete: vec![service_id(1)],
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![boundary],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
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

fn two_element_projected_unit_call_module() -> TerminalModule {
    let mut module = projected_unit_call_module();
    let StructuralTypeShape::FixedArray { length, .. } = &mut module.structural_types[2].shape
    else {
        unreachable!()
    };
    *length = 2;
    module.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(2),
        input: place_id(1),
        path: vec![StructuralPathSegment::FixedIndex(1)],
    });
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
                    identity: "middle".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
                StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(3).expect("field identity"),
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
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let callee_parameter = StructuralParameterDeclaration {
        place: place_id(2),
        position: 0,
        is_self: false,
        structural_type: token.id,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![caller_parameter],
        ranked_scc: None,
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
                        access: StructuralAccess::Owned,
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
                residual_affine_discards: vec![
                    StructuralAffineDiscard {
                        place: place_id(1),
                        path: vec![StructuralPathSegment::Field("middle".into())],
                        structural_type: token.id,
                    },
                    StructuralAffineDiscard {
                        place: place_id(1),
                        path: vec![StructuralPathSegment::Field("left".into())],
                        structural_type: token.id,
                    },
                ],
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    let callee = TerminalMachine {
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![callee_parameter],
        ranked_scc: None,
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
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn multiple_move_partial_affine_field_module() -> TerminalModule {
    let mut module = partial_affine_field_module();
    let caller_block = &mut module.machines[0].blocks[0];
    let mut second_call = caller_block.operations[0].clone();
    second_call.id = operation_id(2);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut second_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("middle".into())];
    caller_block.operations.push(second_call);
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut caller_block.terminator
    else {
        unreachable!()
    };
    residual_affine_discards.remove(0);
    module
}

fn nested_partial_affine_field_module() -> TerminalModule {
    let mut module = partial_affine_field_module();
    let inner = StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "Nested".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(4).expect("field identity"),
                    identity: "left".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                },
                StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(5).expect("field identity"),
                    identity: "middle".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                },
                StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(6).expect("field identity"),
                    identity: "right".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                },
            ],
        },
    };
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        unreachable!()
    };
    fields[1].identity = "nested".into();
    fields[1].field_type = StructuralFieldType::Structural(inner.id);
    module.structural_types.push(inner);

    let caller_block = &mut module.machines[0].blocks[0];
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller_block.operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![
        StructuralPathSegment::Field("nested".into()),
        StructuralPathSegment::Field("middle".into()),
    ];
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut caller_block.terminator
    else {
        unreachable!()
    };
    *residual_affine_discards = vec![
        StructuralAffineDiscard {
            place: place_id(1),
            path: vec![StructuralPathSegment::Field("right".into())],
            structural_type: structural_type_id(1),
        },
        StructuralAffineDiscard {
            place: place_id(1),
            path: vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("right".into()),
            ],
            structural_type: structural_type_id(1),
        },
        StructuralAffineDiscard {
            place: place_id(1),
            path: vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("left".into()),
            ],
            structural_type: structural_type_id(1),
        },
        StructuralAffineDiscard {
            place: place_id(1),
            path: vec![StructuralPathSegment::Field("left".into())],
            structural_type: structural_type_id(1),
        },
    ];
    module
}

fn multiple_nested_partial_affine_field_module() -> TerminalModule {
    let mut module = nested_partial_affine_field_module();
    let caller_block = &mut module.machines[0].blocks[0];
    let mut second_call = caller_block.operations[0].clone();
    second_call.id = operation_id(2);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut second_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![
        StructuralPathSegment::Field("nested".into()),
        StructuralPathSegment::Field("left".into()),
    ];
    caller_block.operations.push(second_call);
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut caller_block.terminator
    else {
        unreachable!()
    };
    residual_affine_discards.remove(2);
    module
}

fn mixed_direct_nested_partial_affine_field_module() -> TerminalModule {
    let mut module = nested_partial_affine_field_module();
    let caller_block = &mut module.machines[0].blocks[0];
    let mut direct_call = caller_block.operations[0].clone();
    direct_call.id = operation_id(2);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut direct_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("right".into())];
    caller_block.operations.push(direct_call);
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut caller_block.terminator
    else {
        unreachable!()
    };
    residual_affine_discards.remove(0);
    module
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
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        }],
        ranked_scc: None,
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
                cleanups: vec![NominalAffineCleanup {
                    place: place_id(1),
                    structural_type: token.id,
                    cleanup_machine: machine_id(2),
                    cleanup_receiver: None,
                    requirement_obligations: Vec::new(),
                }],
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    let cleanup = TerminalMachine {
        id: machine_id(2),
        attachment: Some(token.id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
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
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![caller, cleanup],
    }
}

fn contextual_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let field = psi_core::StructuralFieldId::new(1).expect("field");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: field,
            identity: "ready".into(),
            relevance: psi_terminal::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
        }],
    };
    let caller_requirement = Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field(place_id(1), field),
    );
    module.machines[0]
        .contract
        .requires
        .push(caller_requirement);
    let receiver = place_id(99);
    module.machines[1]
        .contract
        .requires
        .push(Proposition::Equal(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_field(receiver, field),
        ));
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(receiver);
    cleanups[0].requirement_obligations = vec![obligation_id(1)];
    module
}

fn two_requirement_contextual_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let receiver = place_id(99);
    let fields = (1_u64..=3)
        .map(|identity| StructuralFieldDeclaration {
            id: psi_core::StructuralFieldId::new(identity).expect("field"),
            identity: format!("flag_{identity}"),
            relevance: psi_terminal::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
        })
        .collect::<Vec<_>>();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: fields.clone(),
    };
    module.machines[0].contract.requires = fields
        .iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(place_id(1), field.id),
            )
        })
        .collect();
    module.machines[1].contract.requires = fields[..2]
        .iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(receiver, field.id),
            )
        })
        .collect();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(receiver);
    cleanups[0].requirement_obligations = vec![obligation_id(1), obligation_id(2)];
    module
}

fn two_root_shared_contextual_nominal_affine_module() -> TerminalModule {
    let mut module = two_requirement_contextual_nominal_affine_module();
    let first = psi_core::StructuralFieldId::new(1).expect("first field");
    let second = psi_core::StructuralFieldId::new(2).expect("second field");
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    caller
        .contract
        .requires
        .extend([first, second].map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(place_id(2), field),
            )
        }));
    caller.contract.requires.sort();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    let mut second_cleanup = cleanups[0].clone();
    second_cleanup.place = place_id(2);
    second_cleanup.requirement_obligations = vec![obligation_id(3), obligation_id(4)];
    cleanups.insert(0, second_cleanup);
    module
}

fn two_root_distinct_contextual_nominal_affine_module() -> TerminalModule {
    let mut module = two_root_shared_contextual_nominal_affine_module();
    let second_type = structural_type_id(2);
    let mut type_declaration = module.structural_types[0].clone();
    type_declaration.id = second_type;
    type_declaration.identity = "SecondToken".into();
    module.structural_types.push(type_declaration);

    let second_target_id = machine_id(3);
    let second_receiver = place_id(100);
    let mut second_target = module.machines[1].clone();
    second_target.id = second_target_id;
    second_target.attachment = Some(second_type);
    second_target.entry = block_id(3);
    second_target.blocks[0].id = block_id(3);
    second_target.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    second_target.contract.id = contract_id(3);
    for requirement in &mut second_target.contract.requires {
        let Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) = requirement else {
            unreachable!()
        };
        *root = second_receiver;
    }
    module.machines.push(second_target);

    let caller = &mut module.machines[0];
    caller.structural_parameters[1].structural_type = second_type;
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].structural_type = second_type;
    cleanups[0].cleanup_machine = second_target_id;
    cleanups[0].cleanup_receiver = Some(second_receiver);
    module
}

fn two_root_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups.insert(
        0,
        NominalAffineCleanup {
            place: place_id(2),
            structural_type: structural_type_id(1),
            cleanup_machine: machine_id(2),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn five_root_nominal_affine_module() -> TerminalModule {
    let mut module = two_root_nominal_affine_module();
    let caller = &mut module.machines[0];
    for position in 2_u32..5 {
        let place = place_id(u64::from(position + 1));
        caller
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
            });
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position,
                is_self: false,
            },
        });
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
        else {
            unreachable!()
        };
        cleanups.insert(
            0,
            NominalAffineCleanup {
                place,
                structural_type: structural_type_id(1),
                cleanup_machine: machine_id(2),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        );
    }
    module
}

fn two_root_one_executable_nominal_affine_module() -> TerminalModule {
    let mut module = two_root_nominal_affine_module();
    let mut second_cleanup = module.machines[1].clone();
    second_cleanup.id = machine_id(3);
    second_cleanup.entry = block_id(3);
    second_cleanup.blocks[0].id = block_id(3);
    second_cleanup.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    second_cleanup.contract.id = contract_id(3);
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(4);
    helper.entry = block_id(4);
    helper.blocks[0].id = block_id(4);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(4);
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: helper.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_machine = second_cleanup.id;
    module.machines.push(second_cleanup);
    module.machines.push(helper);
    module
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
        ranked_scc: None,
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

fn two_call_executable_nominal_affine_module() -> TerminalModule {
    let mut module = executable_nominal_affine_module();
    let helper_type = StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(4),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let mut second_helper = module.machines[2].clone();
    second_helper.id = machine_id(4);
    second_helper.attachment = Some(helper_type.id);
    second_helper.entry = block_id(4);
    second_helper.blocks[0].id = block_id(4);
    second_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    second_helper.contract.id = contract_id(4);
    module.machines.push(second_helper);
    module
}

fn three_call_executable_nominal_affine_module() -> TerminalModule {
    let mut module = two_call_executable_nominal_affine_module();
    let helper_type = StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "ThirdHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(5),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let mut third_helper = module.machines[2].clone();
    third_helper.id = machine_id(5);
    third_helper.attachment = Some(helper_type.id);
    third_helper.entry = block_id(5);
    third_helper.blocks[0].id = block_id(5);
    third_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    third_helper.contract.id = contract_id(5);
    module.machines.push(third_helper);
    module
}

fn five_call_executable_nominal_affine_module() -> TerminalModule {
    let mut module = three_call_executable_nominal_affine_module();
    for raw in 6_u64..=7 {
        let mut helper = module.machines[2].clone();
        helper.id = machine_id(raw);
        helper.entry = block_id(raw);
        helper.blocks[0].id = block_id(raw);
        helper.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(raw),
            trivial_affine_discards: Vec::new(),
        };
        helper.contract.id = contract_id(raw);
        module.machines[1].blocks[0].operations.push(Operation {
            id: operation_id(raw - 2),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: helper.id,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        module.machines.push(helper);
    }
    module
}

fn structural_parameter(place: PlaceId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
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
            projection: content_owner_projection().identity,
            algebra: ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Acknowledgement".to_owned(),
            },
        }],
    }
}

fn content_owner_projection() -> StructuralContentProjection {
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Acknowledgement".to_owned(),
    };
    let expression = ContentProjectionExpression::CountedQuantity(
        ContentProjectionScalar::Natural("1".to_owned()),
    );
    StructuralContentProjection {
        identity: ContentProjectionIdentity {
            domain: ContentDomainId::new(1).expect("content domain"),
            projection_report_fingerprint:
                psi_language_semantics::content::terminal_projection_report_fingerprint(
                    &algebra,
                    &expression,
                ),
        },
        algebra,
        expression,
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
        outcome_specific_ensures: Vec::new(),
    }
}

fn content_predicate(root: PlaceId) -> Proposition {
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(1).expect("content domain"),
        projection_report_fingerprint: 1,
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
    let OperationKind::BoundaryCall {
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
