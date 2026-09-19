use super::{
    contextual_nominal_affine_module, empty_contract, five_root_nominal_affine_module,
    nominal_affine_module, provider_attachment_root, provider_boundary_call, provider_field_id,
    signed_i8, two_requirement_contextual_nominal_affine_module,
    two_root_distinct_contextual_nominal_affine_module, two_root_nominal_affine_module,
    two_root_one_executable_nominal_affine_module,
    two_root_shared_contextual_nominal_affine_module, unused_provider_attachment_module,
    write_only_primitive_store_module,
};
use crate::structural_unit::{
    block_id, boundary_id, contract_id, domain_id, edge_id, machine_id, obligation_id,
    operation_id, place_id, structural_type_id, value_id,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, EvidenceIdentity, Proposition, ScalarTerm, ScalarType,
    StructuralPlaceKind,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralDomainDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, reconstruct_operation_obligations,
    reconstruct_structural_ownership_frontiers, validate_module, verify_module,
};

#[test]
fn unused_provider_attachment_verifies_without_roots_calls_or_codec() {
    let mut module = unused_provider_attachment_module();
    module.boundary_machines.clear();
    assert!(module.machines[0].structural_places.is_empty());
    assert!(module.machines[0].blocks[0].operations.is_empty());
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("an unused relevant opaque attachment needs no specialization roots");
}

#[test]
fn provider_attachment_verifier_keeps_callee_requirements_independent() {
    let mut module = unused_provider_attachment_module();
    let mut callee = module.machines[0].clone();
    callee.id = machine_id(3);
    callee.entry = block_id(3);
    callee.contract = empty_contract(contract_id(3));
    callee.blocks[0].id = block_id(3);
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    callee.structural_places.push(provider_attachment_root());
    callee.blocks[0].operations.push(provider_boundary_call());
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            callee: callee.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(callee);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the callee owns its boundary requirement and the caller has no direct calls");

    let mut missing = module.clone();
    missing.machines[1].structural_places.clear();
    assert_eq!(
        validate_module(&missing).map(|_| ()),
        Err(ModuleError::InvalidProviderAttachmentSpecialization(
            machine_id(3)
        )),
    );

    let mut orphan = provider_attachment_root();
    orphan.id = place_id(2);
    module.machines[0].structural_places.push(orphan);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidProviderAttachmentSpecialization(
            machine_id(2)
        )),
        "a callee boundary call cannot justify a caller root",
    );
}

#[test]
fn provider_attachment_verifier_requires_exact_direct_call_roots() {
    let mut module = unused_provider_attachment_module();
    module.machines[0].blocks[0]
        .operations
        .push(provider_boundary_call());
    let invalid = Err(ModuleError::InvalidProviderAttachmentSpecialization(
        machine_id(2),
    ));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        invalid,
        "actual direct requirement is missing"
    );

    module.machines[0]
        .structural_places
        .push(provider_attachment_root());
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("one direct boundary call and its exact root verify independently");

    for kind in [
        StructuralPlaceKind::ProviderAttachment {
            attachment: structural_type_id(9),
            field: provider_field_id(),
            boundary: boundary_id(1),
        },
        StructuralPlaceKind::ProviderAttachment {
            attachment: structural_type_id(1),
            field: semantic_vocabulary::StructuralFieldId::new(9).unwrap(),
            boundary: boundary_id(1),
        },
        StructuralPlaceKind::ProviderAttachment {
            attachment: structural_type_id(1),
            field: provider_field_id(),
            boundary: boundary_id(9),
        },
    ] {
        let mut wrong = module.clone();
        wrong.machines[0].structural_places[0].kind = kind;
        assert_eq!(validate_module(&wrong).map(|_| ()), invalid);
    }

    let mut duplicate = module.clone();
    let mut root = provider_attachment_root();
    root.id = place_id(2);
    duplicate.machines[0].structural_places.push(root);
    assert_eq!(validate_module(&duplicate).map(|_| ()), invalid);

    let mut attached_boundary = module.clone();
    attached_boundary.boundary_machines[0].attachment = Some(structural_type_id(1));
    assert_eq!(validate_module(&attached_boundary).map(|_| ()), invalid);

    module.machines[0].blocks[0].operations.clear();
    assert_eq!(
        validate_module(&module).map(|_| ()),
        invalid,
        "orphan provider root rejects"
    );
}

#[test]
fn provider_attachment_verifier_rejects_runtime_provider_arguments() {
    let mut module = unused_provider_attachment_module();
    module.machines[0]
        .structural_places
        .push(provider_attachment_root());
    let mut call = provider_boundary_call();
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut call.kind
    else {
        unreachable!()
    };
    structural_arguments.push(StructuralArgument {
        place: place_id(1),
        access: StructuralAccess::SharedBorrow,
        path: Vec::new(),
    });
    module.machines[0].blocks[0].operations.push(call);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidProviderAttachmentSpecialization(
            machine_id(2)
        )),
    );
}

#[test]
fn unused_provider_attachment_verifier_rejects_runtime_scalar_field_projection() {
    let mut module = unused_provider_attachment_module();
    module.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 0,
            is_self: true,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(2),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanStructuralField {
            path: Vec::new(),
            source: place_id(2),
            field: provider_field_id(),
        },
    });
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidBooleanStructuralField {
            operation: operation_id(1),
            source: place_id(2),
            field: provider_field_id(),
        }),
    );

    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
    validate_module(&module).expect("the corresponding ordinary scalar field remains readable");
}

#[test]
fn unused_provider_attachment_verifier_rejects_nonattachment_and_multiple_fields() {
    let mut unattached = unused_provider_attachment_module();
    unattached.machines[0].attachment = None;
    let invalid_field = Err(ModuleError::InvalidErasedStructuralField {
        structural_type: structural_type_id(1),
        field: provider_field_id(),
    });
    assert_eq!(validate_module(&unattached).map(|_| ()), invalid_field);
    unattached.machines[0]
        .structural_places
        .push(provider_attachment_root());
    assert_eq!(
        validate_module(&unattached).map(|_| ()),
        invalid_field,
        "a forged root cannot authorize a type"
    );

    let mut multiple = unused_provider_attachment_module();
    let StructuralTypeShape::Record { fields } = &mut multiple.structural_types[0].shape else {
        unreachable!()
    };
    let mut second = fields[0].clone();
    second.id = semantic_vocabulary::StructuralFieldId::new(2).unwrap();
    second.identity = "second".into();
    fields.push(second);
    assert_eq!(
        validate_module(&multiple).map(|_| ()),
        Err(ModuleError::InvalidProviderAttachmentSpecialization(
            machine_id(2)
        )),
    );
}

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
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("domain semantic identity"),
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
        static_reach_binding: None,
        id: operation_id(3),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(1),
            scalar_type: signed_i8(),
        }),
        kind: OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Signed(7),
        },
    });
    assert_eq!(
        validate_module(&late_value).unwrap_err(),
        ModuleError::ValueUsedBeforeDefinition(value_id(1)),
    );

    let mut forged_result = write_only_primitive_store_module();
    forged_result.machines[0].blocks[0].operations[0].result =
        OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
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
            semantic_vocabulary::StructuralFieldId::new(1).expect("field"),
        ),
    );
    let obligations = reconstruct_operation_obligations(&module).expect("cleanup obligation");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, obligation_id(1));
    assert_eq!(obligations[0].obligation.proposition, expected);
    assert_eq!(obligations[0].semantic_axioms[0], expected);

    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == obligation_id(1)
    ));
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: expected,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("caller requirement discharges contextual cleanup premise");

    let wrong_bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(2).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Falsehood,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
    };
    assert!(matches!(
        verify_module(&module, &wrong_bundle, &AdmissionProfile::default()),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
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
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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
    let first = semantic_vocabulary::StructuralFieldId::new(1).expect("first field");
    let second = semantic_vocabulary::StructuralFieldId::new(2).expect("second field");
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
                        rule: ProofRule::SemanticAxiom {
                            index: reconstructed
                                .semantic_axioms
                                .iter()
                                .position(|requirement| {
                                    requirement == &reconstructed.obligation.proposition
                                })
                                .expect(
                                    "unchanged owned field remains available at scalar cleanup",
                                ),
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
    let field = semantic_vocabulary::StructuralFieldId::new(1).expect("field");
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
    assert_eq!(obligations[0].semantic_axioms[0], caller_requirement);

    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: caller_requirement,
                    rule: ProofRule::SemanticAxiom { index: 0 },
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
    let first = semantic_vocabulary::StructuralFieldId::new(1).expect("first field");
    let second = semantic_vocabulary::StructuralFieldId::new(2).expect("second field");
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
        assert_eq!(obligations[index].semantic_axioms[index], expected);
    }

    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
                        rule: ProofRule::SemanticAxiom { index },
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
                semantic_vocabulary::StructuralFieldId::new(1).expect("first field"),
            ),
        ),
        Proposition::Equal(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_field(
                place_id(1),
                semantic_vocabulary::StructuralFieldId::new(2).expect("second field"),
            ),
        ),
    ];
    let obligations = reconstruct_operation_obligations(&module).expect("cleanup obligations");
    assert_eq!(obligations.len(), 2);
    for (index, obligation) in obligations.iter().enumerate() {
        assert_eq!(obligation.obligation.id, obligation_id(index as u64 + 1));
        assert_eq!(obligation.obligation.proposition, expected[index]);
        assert_eq!(obligation.semantic_axioms[index], expected[index]);
    }

    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
                        rule: ProofRule::SemanticAxiom { index },
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
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .expect("u8"),
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
    let first = semantic_vocabulary::StructuralFieldId::new(1).expect("first field");
    let rollover = semantic_vocabulary::StructuralFieldId::new(256).expect("rollover field");
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
    path[0] = CanonicalStructuralPathSegment::Field(
        semantic_vocabulary::StructuralFieldId::new(2).expect("field"),
    );
    assert_eq!(
        validate_module(&wrong_field).unwrap_err(),
        expected_invalid(&wrong_field)
    );

    let mut executable_receiver = contextual_nominal_affine_module();
    executable_receiver.machines[1].contract.requires[0] = Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field(
            place_id(1),
            semantic_vocabulary::StructuralFieldId::new(1).expect("field"),
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
    let first = semantic_vocabulary::StructuralFieldId::new(1).expect("first field");
    let second = semantic_vocabulary::StructuralFieldId::new(2).expect("second field");

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

    let first = semantic_vocabulary::StructuralFieldId::new(1).expect("first field");
    let second = semantic_vocabulary::StructuralFieldId::new(2).expect("second field");
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
                        rule: ProofRule::SemanticAxiom {
                            index: reconstructed
                                .semantic_axioms
                                .iter()
                                .position(|requirement| {
                                    requirement == &reconstructed.obligation.proposition
                                })
                                .expect("unchanged owned field remains available at its cleanup"),
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
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                arguments: Vec::new(),
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
            static_reach_binding: None,
            id: operation_id(1),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                arguments: Vec::new(),
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
