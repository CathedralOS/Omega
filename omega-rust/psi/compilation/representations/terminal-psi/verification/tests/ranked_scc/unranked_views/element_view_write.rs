//! Element-view writes mirror the byte-view mutable-write fixture: a mutable
//! element view is forwarded to a block whose write op targets it, with the
//! element count as the exact-length witness and the element scalar as the
//! value type. Shared destinations, stale witnesses and wrong scalar types
//! are refused at validation.

use super::super::{
    Proposition, ScalarTerm, reconstruct_interpretable_operation_obligations,
    validate_module_for_interpretation,
};
use super::{
    AdmissionProfile, IntegerSign, IntegerType, ModuleError, ObligationId, Operation, OperationId,
    OperationKind, OperationResult, PlaceId, ProofBundle, ScalarType, StructuralAccess,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, TerminalModule,
    ValueDeclaration, ValueId, VerificationError, id, validate_module,
    verify_module_for_interpretation, view_cycle,
};

fn fixture() -> TerminalModule {
    let mut module = view_cycle();
    let element = id(9, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: element,
        identity: "test::Element".into(),
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        )),
    });
    module.structural_types[0].identity = "test::MutableElements".into();
    module.structural_types[0].shape = StructuralTypeShape::ElementView { element };
    let machine = &mut module.machines[0];
    let terminator = machine.blocks[3].terminator.clone();
    machine.blocks.truncate(2);
    machine.structural_places.truncate(2);
    machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(30, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    });
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(20, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(21, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::ElementViewLength {
            source: id(1, PlaceId::new),
        },
    });
    let super::Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    machine.blocks[1].operations[0].kind = OperationKind::ElementViewLength {
        source: id(2, PlaceId::new),
    };
    machine.blocks[1].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(30, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::ElementViewWrite {
            path: Vec::new(),
            destination: id(2, PlaceId::new),
            index: id(1, ValueId::new),
            value: id(30, ValueId::new),
            length: id(10, ValueId::new),
            obligation: id(30, ObligationId::new),
        },
    });
    machine.blocks[1].terminator = terminator;
    module
}

#[test]
fn mutable_element_write_validates_and_requires_bounds_evidence() {
    let module = fixture();
    validate_module(&module).unwrap();
    let obligations = reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(&module).unwrap(),
    )
    .unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    assert!(obligations[0].semantic_axioms.contains(&Proposition::Equal(
        ScalarTerm::value(id(10, ValueId::new), scalar_type),
        ScalarTerm::value(id(21, ValueId::new), scalar_type),
    )));
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation)) if obligation == id(30, ObligationId::new)
    ));
}

#[test]
fn element_write_rejects_shared_destination_wrong_witness_and_value_type() {
    for mutation in 0..4 {
        let mut module = fixture();
        let machine = &mut module.machines[0];
        match mutation {
            0 => {
                machine.blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
            }
            1 => {
                let OperationKind::ElementViewWrite { length, .. } =
                    &mut machine.blocks[1].operations[1].kind
                else {
                    unreachable!()
                };
                *length = id(21, ValueId::new);
            }
            2 => {
                let OperationKind::ElementViewWrite { length, .. } =
                    &mut machine.blocks[1].operations[1].kind
                else {
                    unreachable!()
                };
                *length = id(1, ValueId::new);
            }
            _ => machine.parameters.last_mut().unwrap().scalar_type = ScalarType::Boolean,
        }
        assert!(validate_module(&module).is_err(), "mutation {mutation}");
    }
}

/// The guard makes both the parent write and the nonempty `[index..length]`
/// children provable. Reordering child uses changes custody, never these bounds.
fn mutable_child_fixture() -> TerminalModule {
    use semantic_vocabulary::{BlockId, EdgeId, StructuralPlaceKind};
    use terminal_psi::{
        StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
        SuccessorEdge, Terminator,
    };
    let mut module = fixture();
    let machine = &mut module.machines[0];
    // Keep this local-loan regression independent of descriptor handoff facts.
    // The entry view and its exact dominating length survive the guard.
    machine.structural_places.truncate(1);
    machine.blocks[1].structural_parameters.clear();
    machine.blocks[1].operations[0].kind = OperationKind::ElementViewLength {
        source: id(1, PlaceId::new),
    };
    if let OperationKind::ElementViewWrite {
        destination,
        length,
        ..
    } = &mut machine.blocks[1].operations[1].kind
    {
        *destination = id(1, PlaceId::new);
        *length = id(21, ValueId::new);
    }
    let mut exit = machine.blocks[1].clone();
    exit.id = id(60, BlockId::new);
    exit.structural_parameters.clear();
    exit.operations.clear();
    let Terminator::ReturnUnit { edge, .. } = &mut exit.terminator else {
        unreachable!()
    };
    *edge = id(60, EdgeId::new);
    machine.blocks.push(exit);
    machine.blocks[0].operations.push(Operation {
        id: id(50, OperationId::new),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(50, ValueId::new),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::IntegerLessThan {
            left: id(1, ValueId::new),
            right: id(21, ValueId::new),
        },
    });
    let Terminator::Jump { edge, target, .. } = machine.blocks[0].terminator.clone() else {
        unreachable!()
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(50, ValueId::new),
        when_true: SuccessorEdge {
            edge,
            target,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: id(61, EdgeId::new),
            target: id(60, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    for ordinal in [40, 42] {
        let place = id(ordinal, PlaceId::new);
        let producer = id(ordinal, OperationId::new);
        let structural_type = machine.structural_parameters[0].structural_type;
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        });
        machine.blocks[1].operations.push(Operation {
            id: producer,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::ElementViewSubslice {
                source: id(1, PlaceId::new),
                start: id(1, ValueId::new),
                end: id(21, ValueId::new),
                length: id(21, ValueId::new),
                obligation: id(ordinal, ObligationId::new),
            },
        });
        machine.blocks[1].operations.push(Operation {
            id: id(ordinal + 1, OperationId::new),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(ordinal + 1, ValueId::new),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                qualifications: Default::default(),
            }),
            kind: OperationKind::ElementViewLength { source: place },
        });
    }
    // Close the first affine carrier before establishing the next exclusive
    // loan. Last payload observation alone does not publish its disposal.
    let mut next = machine.blocks[1].clone();
    next.id = id(70, BlockId::new);
    next.operations = machine.blocks[1].operations.split_off(4);
    if let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards,
        ..
    } = &mut next.terminator
    {
        *edge = id(70, EdgeId::new);
        *trivial_affine_discards = vec![id(42, PlaceId::new)];
    } else {
        unreachable!();
    }
    machine.blocks[1].terminator = Terminator::Jump {
        edge: id(71, EdgeId::new),
        target: next.id,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: vec![id(40, PlaceId::new)],
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(next);
    module
}

fn mutable_child_bounds(module: &TerminalModule) -> ProofBundle {
    use proof_admission::{CertificateEnvelope, EvidenceRoute, ProofSystemMarker};
    use semantic_vocabulary::EvidenceIdentity;
    let obligations = reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(module).unwrap(),
    )
    .unwrap();
    assert_eq!(obligations.len(), 3);
    ProofBundle {
        evidence: obligations
            .into_iter()
            .enumerate()
            .map(|(position, reconstructed)| {
                let proof = mutable_child_bound_certificate(
                    &reconstructed.obligation.proposition,
                    &reconstructed.semantic_axioms,
                );
                terminal_verifier::ObligationEvidence {
                    obligation: reconstructed.obligation.id,
                    route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                        identity: id(position as u64 + 70, EvidenceIdentity::new),
                        proof_system_marker: ProofSystemMarker::CURRENT,
                        proof,
                    }),
                }
            })
            .collect(),
        ..ProofBundle::default()
    }
}

/// The fixture needs only the exact selected guard, weakening, and reflexivity.
/// Full verification below checks every citation against the reconstructed site.
fn mutable_child_bound_certificate(
    goal: &Proposition,
    axioms: &[Proposition],
) -> proof_admission::ProofNode {
    use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
    let rule = if let Some(index) = axioms.iter().position(|axiom| axiom == goal) {
        ProofRule::SemanticAxiom { index }
    } else {
        match goal {
            Proposition::Conjunction(children) => ProofRule::ConjunctionIntroduction(
                children
                    .iter()
                    .map(|child| mutable_child_bound_certificate(child, axioms))
                    .collect(),
            ),
            Proposition::LessOrEqual(left, right) => {
                let relation = if left == right {
                    ProofNode {
                        conclusion: Proposition::Equal(left.clone(), right.clone()),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                    }
                } else {
                    mutable_child_bound_certificate(
                        &Proposition::LessThan(left.clone(), right.clone()),
                        axioms,
                    )
                };
                ProofRule::IntegerOrderWeakening {
                    relation: Box::new(relation),
                }
            }
            _ => panic!("missing exact guard for {goal:?}; axioms={axioms:#?}"),
        }
    };
    ProofNode {
        conclusion: goal.clone(),
        rule,
    }
}

#[test]
fn mutable_child_sequential_windows_have_valid_bounds_and_restoration() {
    let module = mutable_child_fixture();
    let proof = mutable_child_bounds(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default()).unwrap();
}

#[test]
fn mutable_child_rejects_parent_write_before_last_child_use() {
    let mut module = mutable_child_fixture();
    let proof = mutable_child_bounds(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default()).unwrap();
    // Same write, source, index and witness; only its position moves inside
    // the first child's interval, before that child's final observation.
    module.machines[0].blocks[1].operations.swap(1, 2);
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "a live mutable child must suspend an overlapping parent write"
    );
}

#[test]
fn mutable_child_rejects_overlapping_windows_with_both_children_live() {
    let mut module = mutable_child_fixture();
    let proof = mutable_child_bounds(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default()).unwrap();
    // Move the second child's birth before the first child's observation and
    // explicit closure. The endpoints still dominate both births.
    let second_birth = module.machines[0].blocks[3].operations.remove(0);
    module.machines[0].blocks[1]
        .operations
        .insert(3, second_birth);
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "two simultaneously live mutable children cannot capture overlapping ranges"
    );
}

#[test]
fn mutable_child_write_uses_its_own_live_authority() {
    let mut module = mutable_child_fixture();
    let mut write = module.machines[0].blocks[1].operations[1].clone();
    write.id = id(70, OperationId::new);
    if let OperationKind::ElementViewWrite {
        destination,
        length,
        obligation,
        ..
    } = &mut write.kind
    {
        *destination = id(40, PlaceId::new);
        *length = id(41, ValueId::new);
        *obligation = id(70, ObligationId::new);
    } else {
        unreachable!()
    }
    // The receiving structural check authenticates the child access; the
    // independent bounds obligation remains required by proof admission.
    module.machines[0].blocks[1].operations.insert(4, write);
    validate_module(&module).unwrap();
}

#[test]
fn mutable_child_requires_exact_affine_carrier_cleanup() {
    let mut module = mutable_child_fixture();
    let terminal_psi::Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[3].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.clear();
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::UnitReturnAffineDiscardsMismatch { .. })
    ));
}

/// The call observes the first child, then the parent writes before either
/// descriptor's final disposal. Neither descriptor carries owned elements.
fn mutable_child_call_fixture() -> TerminalModule {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, StructuralPlaceKind};
    use terminal_psi::{MachineContract, StructuralArgument, SuccessorEdge, Terminator};

    let mut module = mutable_child_fixture();
    let mut helper = module.machines[0].clone();
    helper.id = id(1000, MachineId::new);
    helper.parameters.clear();
    helper.structural_places.truncate(1);
    helper.structural_parameters[0].place = id(1000, PlaceId::new);
    helper.structural_places[0].id = id(1000, PlaceId::new);
    helper.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: false,
    };
    helper.contract = MachineContract {
        id: id(1000, ContractId::new),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        crash_routes: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    let mut body = helper.blocks[2].clone();
    body.id = id(1000, BlockId::new);
    body.operations = vec![Operation {
        id: id(1000, OperationId::new),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(1000, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            qualifications: Default::default(),
        }),
        kind: OperationKind::ElementViewLength {
            source: id(1000, PlaceId::new),
        },
    }];
    body.terminator = Terminator::ReturnUnit {
        edge: id(1000, EdgeId::new),
        trivial_affine_discards: Vec::new(),
    };
    helper.entry = body.id;
    helper.blocks = vec![body];
    module.machines.push(helper);

    let machine = &mut module.machines[0];
    let mut parent_write = machine.blocks[1].operations.remove(1);
    machine.blocks[1].operations.push(Operation {
        id: id(80, OperationId::new),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: id(1000, MachineId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(40, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[1].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.clear();
    // Mutable calls invalidate the old extent witness independently of the
    // descriptor loan's lifetime. Measure and guard the parent again so this
    // regression asks only whether the finished child still suspends it.
    let mut length = machine.blocks[1].operations[0].clone();
    length.id = id(82, OperationId::new);
    let OperationResult::Scalar(result) = &mut length.result else {
        unreachable!()
    };
    result.id = id(82, ValueId::new);
    machine.blocks[1].operations.push(length);
    machine.blocks[1].operations.push(Operation {
        id: id(83, OperationId::new),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(83, ValueId::new),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::IntegerLessThan {
            left: id(1, ValueId::new),
            right: id(82, ValueId::new),
        },
    });
    let OperationKind::ElementViewWrite { length, .. } = &mut parent_write.kind else {
        unreachable!()
    };
    *length = id(82, ValueId::new);
    let mut write = machine.blocks[2].clone();
    write.id = id(80, BlockId::new);
    write.operations = vec![parent_write];
    write.terminator = machine.blocks[1].terminator.clone();
    let mut empty = machine.blocks[2].clone();
    empty.id = id(81, BlockId::new);
    empty.terminator = Terminator::ReturnUnit {
        edge: id(82, EdgeId::new),
        trivial_affine_discards: vec![id(40, PlaceId::new)],
    };
    let successor = |identity| SuccessorEdge {
        edge: id(identity, EdgeId::new),
        target: id(identity, BlockId::new),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[1].terminator = Terminator::Conditional {
        condition: id(83, ValueId::new),
        when_true: successor(80),
        when_false: successor(81),
    };
    machine.blocks.extend([write, empty]);
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[3].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![id(42, PlaceId::new), id(40, PlaceId::new)];
    module
}

#[test]
fn mutable_child_call_restores_parent_before_exact_final_descriptor_cleanup() {
    let module = mutable_child_call_fixture();
    let proof = mutable_child_bounds(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default()).unwrap();
}

#[test]
fn mutable_child_call_cannot_restore_parent_before_a_later_child_observation() {
    let mut module = mutable_child_call_fixture();
    let proof = mutable_child_bounds(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default()).unwrap();
    let mut later_read = module.machines[0].blocks[1].operations[2].clone();
    later_read.id = id(81, OperationId::new);
    let OperationResult::Scalar(result) = &mut later_read.result else {
        unreachable!()
    };
    result.id = id(81, ValueId::new);
    module.machines[0].blocks[4].operations.push(later_read);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
    ));
}

#[test]
fn mutable_child_call_restoration_does_not_discharge_descriptor_cleanup() {
    let mut module = mutable_child_call_fixture();
    let proof = mutable_child_bounds(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default()).unwrap();
    let terminal_psi::Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[3].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.retain(|place| *place != id(40, PlaceId::new));
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::UnitReturnAffineDiscardsMismatch { .. })
    ));
}

#[path = "element_view_write/contract_ordering.rs"]
mod contract_ordering;

#[test]
fn projected_element_write_reconstructs_declared_range_and_rejects_path_drift() {
    use semantic_vocabulary::{BoundedIntegerType, IntegerValue, StructuralFieldId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    };
    let mut module = fixture();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    module
        .structural_types
        .iter_mut()
        .find(|row| row.id == id(9, StructuralTypeId::new))
        .unwrap()
        .shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: id(1, StructuralFieldId::new),
            identity: "bounded".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    integer,
                    IntegerValue::Unsigned(0),
                    IntegerValue::Unsigned(100),
                )
                .unwrap(),
            ),
        }],
    };
    let OperationKind::ElementViewWrite { path, .. } =
        &mut module.machines[0].blocks[1].operations[1].kind
    else {
        unreachable!()
    };
    *path = vec![StructuralPathSegment::Field("bounded".into())];
    validate_module(&module).unwrap();
    let obligations = reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(&module).unwrap(),
    )
    .unwrap();
    let obligation = obligations
        .iter()
        .find(|row| row.obligation.id == id(30, ObligationId::new))
        .unwrap();
    let Proposition::Conjunction(legs) = &obligation.obligation.proposition else {
        panic!("range evidence cannot be erased")
    };
    assert_eq!(legs.len(), 3);
    assert_eq!(
        legs[2],
        Proposition::LessOrEqual(
            ScalarTerm::value(id(30, ValueId::new), ScalarType::Integer(integer)),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(100)).unwrap()
        )
    );
    assert!(
        matches!(verify_module_for_interpretation(&module,&ProofBundle::default(),&AdmissionProfile::default()),Err(VerificationError::MissingEvidence(id)) if id==ObligationId::new(30).unwrap())
    );
    for mutation in 0..3 {
        let mut wrong = module.clone();
        let OperationKind::ElementViewWrite { path, .. } =
            &mut wrong.machines[0].blocks[1].operations[1].kind
        else {
            unreachable!()
        };
        match mutation {
            0 => path.clear(),
            1 => path[0] = StructuralPathSegment::Field("other".into()),
            _ => path[0] = StructuralPathSegment::Referent,
        }
        assert!(validate_module(&wrong).is_err(), "mutation {mutation}");
    }
}
