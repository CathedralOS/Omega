//! Mutations are replayed against aliases reconstructed from real Terminal
//! operations, while predicate equality keeps the exact observed descriptor.
use super::*;
use semantic_vocabulary::{
    BlockId, ByteSequencePredicate, ContractId, EdgeId, IntegerSign, IntegerType, MachineId,
    ObligationId, OperationId, ScalarTerm, ScalarType, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, Block, ByteSequenceCarrier, MachineContract, Operation, OperationKind,
    OperationResult, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalMachineResult, Terminator,
};

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}

#[test]
fn shared_mutation_observations_cover_atomic_aliases_not_plain_siblings_or_ssa() {
    let mut module = fixture();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![
            StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: "atomic".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Atomic(ScalarType::Boolean),
            },
            StructuralFieldDeclaration {
                id: StructuralFieldId::new(2).unwrap(),
                identity: "ordinary".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            },
        ],
    };
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    let mut alias = machine.structural_parameters[0].clone();
    alias.place = place(2);
    alias.position = 1;
    machine.structural_parameters.push(alias);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let storage = StorageObservations::new(&module, &module.machines[0]);
    for root in [place(1), place(2)] {
        for (field, mutable) in [(1, true), (2, false)] {
            let observation = Proposition::Equal(
                ScalarTerm::BooleanField {
                    root,
                    path: vec![CanonicalStructuralPathSegment::Field(
                        StructuralFieldId::new(field).unwrap(),
                    )],
                },
                ScalarTerm::Boolean(true),
            );
            assert_eq!(storage.observes_shared_mutation(&observation), mutable);
        }
    }
    let snapshot = Proposition::Equal(
        ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    );
    assert!(!storage.observes_shared_mutation(&snapshot));
}
fn machine(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}
fn structure(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}
fn scalar(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}
fn parameter(
    access: StructuralAccess,
    structural_type: StructuralTypeId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(1),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}
fn body(
    identity: u64,
    access: StructuralAccess,
    structural_type: StructuralTypeId,
) -> TerminalMachine {
    let entry = BlockId::new(1).unwrap();
    TerminalMachine {
        id: machine(identity),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![parameter(access, structural_type)],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            id: entry,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(identity).unwrap(),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn fixture() -> TerminalModule {
    let mut module = TerminalModule::for_entry(machine(1));
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structure(1),
            identity: "Bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            }),
        },
        StructuralTypeDeclaration {
            id: structure(2),
            identity: "Array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structure(3),
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: structure(3),
            identity: "Byte".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar(8)),
        },
    ];
    module.machines = vec![body(1, StructuralAccess::MutableBorrow, structure(1))];
    module
}
fn add_view(machine: &mut TerminalMachine, source: PlaceId, result: PlaceId, array: bool) {
    let id = OperationId::new(result.get()).unwrap();
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: result,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type: structure(1),
        },
    });
    machine.blocks[0].operations.push(Operation {
        id,
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: result,
            structural_type: structure(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
            qualification_establishments: vec![],
            claims: vec![],
        }),
        kind: if array {
            OperationKind::EstablishElementView {
                region: terminal_psi::ElementViewRegion::WholeCollection,
                destination: result,
                source: StructuralArgument {
                    place: source,
                    path: vec![],
                    access: StructuralAccess::SharedBorrow,
                },
                element: structure(3),
            }
        } else {
            OperationKind::ByteSequenceSubslice {
                source,
                start: ValueId::new(1).unwrap(),
                end: ValueId::new(2).unwrap(),
                length: ValueId::new(3).unwrap(),
                obligation: ObligationId::new(result.get()).unwrap(),
            }
        },
    });
}
fn predicate(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> ScalarTerm {
    ScalarTerm::ByteSequencePredicate {
        predicate: ByteSequencePredicate::ValidUtf8,
        root,
        path,
    }
}
fn verdict(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> Proposition {
    Proposition::Equal(predicate(root, path), ScalarTerm::Boolean(true))
}

#[test]
fn byte_predicate_alias_writes_retire_contents_without_snapshotting_verdicts() {
    let mut module = fixture();
    add_view(&mut module.machines[0], place(1), place(2), false);
    add_view(&mut module.machines[0], place(2), place(3), false);
    let storage = StorageObservations::new(&module, &module.machines[0]);
    assert!(storage.observes_payload_write(&verdict(place(1), vec![]), place(3), &[]));
    assert!(storage.observes_payload_write(&verdict(place(3), vec![]), place(1), &[]));
    assert!(!storage.observes_payload_write(&verdict(place(1), vec![]), place(9), &[]));
    let captured = ScalarTerm::value(ValueId::new(7).unwrap(), ScalarType::Boolean);
    let remembered = Proposition::Equal(captured.clone(), ScalarTerm::Boolean(true));
    let extent = Proposition::Equal(
        ScalarTerm::ViewExtent {
            root: place(3),
            path: vec![],
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        },
        ScalarTerm::value(ValueId::new(8).unwrap(), scalar(64)),
    );
    let mut facts = vec![
        Proposition::Equal(captured, predicate(place(3), vec![])),
        verdict(place(3), vec![]),
        remembered.clone(),
        extent.clone(),
    ];
    super::super::field_snapshots::retain(&mut facts, true, |fact| {
        !storage.observes_payload_write(fact, place(1), &[])
    });
    assert_eq!(facts, vec![remembered, extent]);
    let mut observed = vec![verdict(place(3), vec![])];
    storage.append_invocation_facts(&mut observed);
    assert_eq!(
        observed,
        vec![verdict(place(3), vec![])],
        "interference never equates a subslice with its parent contents"
    );
}

#[test]
fn byte_predicate_local_array_backing_and_unknown_cycles_are_not_independent_storage() {
    let mut module = fixture();
    module.machines[0].structural_parameters[0].structural_type = structure(2);
    // A completed local array has no nonlocal returned-origin summary.
    module.machines[0].structural_parameters.clear();
    module.machines[0].structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: OperationId::new(1).unwrap(),
        structural_type: structure(2),
    };
    module.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: place(1),
            structural_type: structure(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
            qualification_establishments: vec![],
            claims: vec![],
        }),
        kind: OperationKind::EstablishZeroedStructural {},
    });
    add_view(&mut module.machines[0], place(1), place(2), true);
    let storage = StorageObservations::new(&module, &module.machines[0]);
    assert!(storage.observes_write(
        &verdict(place(2), vec![]),
        place(1),
        &[CanonicalStructuralPathSegment::FixedIndex(1)]
    ));
    let mut cyclic = storage;
    cyclic.aliases.insert(
        place(4),
        vec![(place(5), borrowed_storage::Projection::new(&[]))],
    );
    cyclic.aliases.insert(
        place(5),
        vec![(place(4), borrowed_storage::Projection::new(&[]))],
    );
    assert!(cyclic.observes_payload_write(&verdict(place(4), vec![]), place(9), &[]));
    assert!(cyclic.observes_payload_write(&verdict(place(9), vec![]), place(4), &[]));
}

#[test]
fn byte_predicate_bounded_siblings_are_disjoint_but_nested_borrowed_backing_is_not() {
    let mut module = fixture();
    let field = |number, borrowed| StructuralFieldDeclaration {
        id: StructuralFieldId::new(number).unwrap(),
        identity: format!("bytes{number}"),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::PredicateByteSequence {
            carrier: if borrowed {
                ByteSequenceCarrier::BorrowedView { access: None }
            } else {
                ByteSequenceCarrier::BoundedOwned { capacity: 4 }
            },
            predicates: terminal_psi::ByteSequencePredicateSet::EMPTY,
        },
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: structure(4),
        identity: "Record".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![field(1, false), field(2, false), field(3, true)],
        },
    });
    module.machines[0].structural_parameters[0].structural_type = structure(4);
    let path = |number| {
        vec![CanonicalStructuralPathSegment::Field(
            StructuralFieldId::new(number).unwrap(),
        )]
    };
    let storage = StorageObservations::new(&module, &module.machines[0]);
    assert!(!storage.observes_write(&verdict(place(1), path(1)), place(1), &path(2)));
    assert!(storage.observes_write(&verdict(place(1), path(1)), place(1), &path(1)));
    assert!(storage.observes_payload_write(&verdict(place(1), path(3)), place(9), &[]));
    assert!(storage.observes_payload_write(&verdict(place(9), vec![]), place(1), &path(3)));
    assert!(!storage.observes_payload_write(&verdict(place(1), path(1)), place(1), &path(2)));
}

#[test]
fn byte_predicate_actual_substitution_preserves_descriptor_and_rejects_implicit_windows() {
    let mut module = fixture();
    module.machines[0].structural_parameters[0].structural_type = structure(2);
    add_view(&mut module.machines[0], place(1), place(2), true);
    let argument = |place, path| StructuralArgument {
        place,
        path,
        access: StructuralAccess::SharedBorrow,
    };
    assert_eq!(
        crate::validation::structural_argument_canonical_prefix(
            &module,
            &module.machines[0],
            &argument(place(2), vec![])
        ),
        Some(vec![])
    );
    assert_eq!(
        crate::validation::structural_argument_canonical_prefix(
            &module,
            &module.machines[0],
            &argument(
                place(1),
                vec![terminal_psi::StructuralPathSegment::FixedByteRange { start: 1, end: 3 }]
            )
        ),
        None
    );
    let mappings = BTreeMap::from([(place(7), (place(2), vec![]))]);
    assert_eq!(
        super::super::substitution::substitute_proposition_structural_places(
            &verdict(place(7), vec![]),
            &mappings
        ),
        verdict(place(2), vec![])
    );
}

/// The complete verifier reconstructs the call's proof question from a real
/// branch, not a hand-picked fact set. The guard value remains the same SSA
/// value, but writes expire both its content equation and the expanded branch
/// verdict. Ordinary reconstruction publishes the expanded verdict, not a
/// second raw-Boolean truth axiom.
#[test]
fn byte_predicate_reconstruction_rejects_stale_verdict_after_write_and_call() {
    use proof_admission::{ProofNode, ProofRule};
    use semantic_vocabulary::PropositionContext;
    use terminal_psi::{SuccessorEdge, control_flow::ByteSequenceOperand};

    let make_operation = |identity, result, kind| Operation {
        id: OperationId::new(identity).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result,
        kind,
    };
    let scalar_value = |identity, scalar_type| terminal_psi::ValueDeclaration {
        id: ValueId::new(identity).unwrap(),
        scalar_type,
        qualifications: Default::default(),
    };
    let call = |callee, access, requirements| OperationKind::CallUnit {
        callee: machine(callee),
        arguments: vec![],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![StructuralArgument {
            place: place(1),
            path: vec![],
            access,
        }],
        claim_transfers: vec![],
        requirement_obligations: requirements,
        crash_continuations: vec![],
    };
    let mut module = fixture();
    let field = StructuralFieldId::new(1).unwrap();
    let field_path = vec![CanonicalStructuralPathSegment::Field(field)];
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: field,
            identity: "bytes".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 4,
            }),
        }],
    };
    let mut callee = body(2, StructuralAccess::SharedBorrow, structure(1));
    callee.entry = BlockId::new(20).unwrap();
    callee.blocks[0].id = callee.entry;
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(20).unwrap(),
        trivial_affine_discards: vec![],
    };
    callee.structural_parameters[0].place = place(20);
    callee.structural_places[0].id = place(20);
    callee
        .contract
        .requires
        .push(verdict(place(20), field_path.clone()));
    let mut mutator = body(3, StructuralAccess::MutableBorrow, structure(1));
    mutator.entry = BlockId::new(30).unwrap();
    mutator.blocks[0].id = mutator.entry;
    mutator.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(30).unwrap(),
        trivial_affine_discards: vec![],
    };
    mutator.structural_parameters[0].place = place(30);
    mutator.structural_places[0].id = place(30);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structure(4),
        identity: "DescriptorOwner".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: "view".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                }),
            }],
        },
    });
    let mut nested_mutator = body(4, StructuralAccess::MutableBorrow, structure(4));
    nested_mutator.entry = BlockId::new(40).unwrap();
    nested_mutator.blocks[0].id = nested_mutator.entry;
    nested_mutator.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(40).unwrap(),
        trivial_affine_discards: vec![],
    };
    nested_mutator.structural_parameters[0].place = place(40);
    nested_mutator.structural_places[0].id = place(40);
    let caller = &mut module.machines[0];
    let mut nested_parameter = parameter(StructuralAccess::MutableBorrow, structure(4));
    nested_parameter.place = place(50);
    nested_parameter.position = 1;
    caller.structural_parameters.push(nested_parameter);
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place(50),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    caller.parameters = vec![scalar_value(10, scalar(64)), scalar_value(11, scalar(8))];
    caller.blocks[0].operations = vec![
        make_operation(
            1,
            OperationResult::Scalar(scalar_value(1, scalar(64))),
            OperationKind::StructuralByteSequenceFieldLength {
                source: place(1),
                path: vec![],
                field,
            },
        ),
        make_operation(
            2,
            OperationResult::Scalar(scalar_value(2, ScalarType::Boolean)),
            OperationKind::ByteSequencePredicate {
                operand: ByteSequenceOperand::Field {
                    source: StructuralArgument {
                        place: place(1),
                        path: vec![],
                        access: StructuralAccess::SharedBorrow,
                    },
                    field,
                    length: ValueId::new(1).unwrap(),
                },
                predicate: ByteSequencePredicate::ValidUtf8,
            },
        ),
    ];
    let successor = |identity| SuccessorEdge {
        edge: EdgeId::new(identity).unwrap(),
        target: BlockId::new(identity).unwrap(),
        arguments: vec![],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
    };
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(2).unwrap(),
        when_true: successor(2),
        when_false: successor(3),
    };
    for identity in [2, 3] {
        let mut block = body(1, StructuralAccess::MutableBorrow, structure(1))
            .blocks
            .remove(0);
        block.id = BlockId::new(identity).unwrap();
        block.terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(identity + 2).unwrap(),
            trivial_affine_discards: vec![],
        };
        caller.blocks.push(block);
    }
    caller.blocks[1].operations.push(make_operation(
        4,
        OperationResult::Unit,
        call(
            2,
            StructuralAccess::SharedBorrow,
            vec![ObligationId::new(4).unwrap()],
        ),
    ));
    module.machines.extend([callee, mutator, nested_mutator]);
    let context = PropositionContext::from_value_types_and_places(
        BTreeMap::from([
            (ValueId::new(1).unwrap(), scalar(64)),
            (ValueId::new(2).unwrap(), ScalarType::Boolean),
            (ValueId::new(10).unwrap(), scalar(64)),
            (ValueId::new(11).unwrap(), scalar(8)),
        ]),
        module.machines[0]
            .structural_places
            .iter()
            .map(|place| (place.id, place.kind)),
    )
    .unwrap();
    let boolean = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean);
    let observed = predicate(place(1), field_path.clone());
    let equation = Proposition::Equal(boolean.clone(), observed.clone());
    let selected = verdict(place(1), field_path.clone());
    for mutation in 0..10 {
        let mut candidate = module.clone();
        match mutation {
            0 => {}
            1 => candidate.machines[0].blocks[1].operations.insert(
                0,
                make_operation(
                    3,
                    OperationResult::Unit,
                    OperationKind::StructuralByteSequenceFieldByteStore {
                        destination: place(1),
                        path: vec![],
                        field,
                        index: ValueId::new(10).unwrap(),
                        value: ValueId::new(11).unwrap(),
                        length: ValueId::new(1).unwrap(),
                        obligation: ObligationId::new(3).unwrap(),
                    },
                ),
            ),
            2 => candidate.machines[0].blocks[1].operations.insert(
                0,
                make_operation(
                    3,
                    OperationResult::Unit,
                    call(3, StructuralAccess::MutableBorrow, vec![]),
                ),
            ),
            3 => {
                let mut nested_call = call(4, StructuralAccess::MutableBorrow, vec![]);
                if let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut nested_call
                {
                    structural_arguments[0].place = place(50);
                }
                candidate.machines[0].blocks[1]
                    .operations
                    .insert(0, make_operation(3, OperationResult::Unit, nested_call));
            }
            4..=9 => {
                install_predicate_dynamic_call(&mut candidate, mutation);
            }
            _ => unreachable!(),
        }
        let unchanged = matches!(mutation, 0 | 4 | 6 | 8);
        let questions = crate::reconstruct_terminal_obligations(&candidate)
            .expect("the same typed byte write/call passes structural custody before proof replay");
        let question = questions.obligations().iter().find(|row| matches!(row.owner,
            crate::ReconstructedTerminalObligationOwner::CallRequires { operation, .. } if operation == OperationId::new(4).unwrap()
        )).expect("exact successor call requirement");
        assert_eq!(
            question.obligation.proposition,
            verdict(place(1), field_path.clone())
        );
        let selected_index = question
            .semantic_axioms
            .iter()
            .position(|fact| fact == &selected);
        assert_eq!(
            selected_index.is_some(),
            unchanged,
            "only unchanged bytes retain the expanded branch verdict"
        );
        assert_eq!(
            question.semantic_axioms.contains(&equation),
            unchanged,
            "only unchanged bytes retain their runtime observation equation"
        );
        let proof = ProofNode {
            conclusion: question.obligation.proposition.clone(),
            rule: ProofRule::SemanticAxiom {
                index: selected_index.unwrap_or(question.semantic_axioms.len()),
            },
        };
        let check = |axioms: &[Proposition]| {
            proof_admission::check_certificate(
                &context,
                &question.obligation.proposition,
                &question.requirements,
                axioms,
                &proof,
            )
        };
        let accepted = check(&question.semantic_axioms);
        assert_eq!(
            accepted.is_ok(),
            unchanged,
            "fresh-content proof must reject the stale verdict, mutation {mutation}: {accepted:?}"
        );
        if !unchanged {
            let mut stale = question.semantic_axioms.clone();
            stale.push(selected.clone());
            assert!(
                check(&stale).is_ok(),
                "retaining the stale branch verdict would admit the exact invalid proof"
            );
        }
    }
}

/// Exercise concrete dispatch, an incoming opaque descriptor, and forwarding
/// that opaque descriptor through an ordinary call. All controls pass the full
/// typed catalog/operation validator before the stale-content proof is checked.
fn install_predicate_dynamic_call(module: &mut TerminalModule, variant: u32) {
    use terminal_psi::{
        ClosedConformanceApplication, ClosedConformanceCallableResult,
        ClosedConformanceRealizationCallable, ClosedConformanceRow,
        TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
        TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
        TerminalDynamicRequirement, TerminalIndirectDynamicDispatch,
        TerminalParameterDynamicDispatch, TerminalReboundDynamicDescriptor, ValueDeclaration,
        closed_conformance_application_commitment,
        closed_conformance_application_report_fingerprint,
    };
    let access = if variant.is_multiple_of(2) {
        StructuralAccess::SharedBorrow
    } else {
        StructuralAccess::MutableBorrow
    };
    let operation = |id, result, kind| Operation {
        id: OperationId::new(id).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result,
        kind,
    };
    let value = |id, bits| {
        OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(id).unwrap(),
            scalar_type: scalar(bits),
            qualifications: Default::default(),
        })
    };
    let parameter_call = || OperationKind::CallDynamicParameterUnit {
        parameter_ordinal: 0,
        requirement_slot: 0,
        requirement_obligations: vec![],
        crash_continuations: vec![],
    };
    let dynamic_parameter = |owner| TerminalDynamicDescriptorParameter {
        owner,
        ordinal: 0,
        source_position: 4,
        trait_identity: "BytesAction".into(),
        access,
        requirements: vec![TerminalDynamicRequirement {
            slot: 0,
            declaring_trait_identity: "BytesAction".into(),
            public_requirement_identity: "BytesAction::apply()".into(),
            family_tuple: vec![],
            result: ClosedConformanceCallableResult::Unit,
        }],
    };
    let dispatch = |owner, id| TerminalParameterDynamicDispatch {
        owner,
        operation: OperationId::new(id).unwrap(),
        parameter_ordinal: 0,
        requirement_slot: 0,
    };
    let kind = match variant {
        4 | 5 => {
            module
                .dynamic_dispatch
                .parameters
                .push(dynamic_parameter(machine(1)));
            module
                .dynamic_dispatch
                .parameter_dispatches
                .push(dispatch(machine(1), 3));
            parameter_call()
        }
        6 | 7 => {
            let mut application = ClosedConformanceApplication {
                owner: machine(1),
                declaration_identity: "BytesImplementsAction".into(),
                telescope: vec![],
                subject_identity: Some(module.structural_types[0].identity.clone()),
                trait_identity: "BytesAction".into(),
                trait_lifetime_arguments: vec![],
                trait_arguments: vec![],
                realization_callables: vec![ClosedConformanceRealizationCallable {
                    source_callable_identity: "Bytes::apply#callable".into(),
                    machine: machine(3),
                    result: ClosedConformanceCallableResult::Unit,
                }],
                rows: vec![ClosedConformanceRow {
                    declaring_trait_identity: "BytesAction".into(),
                    public_requirement_identity: "BytesAction::apply()".into(),
                    family_tuple: vec![],
                    requirement_identity: "BytesAction::apply".into(),
                    realization_identity: "Bytes::apply".into(),
                    realization_callable_identity: Some("Bytes::apply#callable".into()),
                }],
                report_fingerprint: 0,
                commitment: Default::default(),
            };
            application.report_fingerprint =
                closed_conformance_application_report_fingerprint(&application);
            application.commitment = closed_conformance_application_commitment(&application);
            module
                .dynamic_dispatch
                .selections
                .push(TerminalDynamicConformanceSelection {
                    owner: machine(1),
                    ordinal: 0,
                    source: StructuralArgument {
                        place: place(1),
                        path: vec![],
                        access,
                    },
                    conformance_application_report_fingerprint: application.report_fingerprint,
                    conformance_application_commitment: application.commitment,
                });
            let mut rebound = module.dynamic_dispatch.selections[0].clone();
            rebound.ordinal = 1;
            module.dynamic_dispatch.selections.push(rebound);
            module.closed_conformance_applications.push(application);
            module
                .dynamic_dispatch
                .rebound_descriptors
                .push(TerminalReboundDynamicDescriptor {
                    owner: machine(1),
                    ordinal: 0,
                    initial_selection_ordinal: 0,
                    rebound_selection_ordinal: 1,
                });
            module
                .dynamic_dispatch
                .indirect_dispatches
                .push(TerminalIndirectDynamicDispatch {
                    owner: machine(1),
                    operation: OperationId::new(3).unwrap(),
                    descriptor_ordinal: 0,
                    declaring_trait_identity: "BytesAction".into(),
                    public_requirement_identity: "BytesAction::apply()".into(),
                    family_tuple: vec![],
                    requirement_identity: "BytesAction::apply".into(),
                    realization_identity: "Bytes::apply".into(),
                    realization_callable_identity: "Bytes::apply#callable".into(),
                    realization: machine(3),
                });
            let realization = &mut module.machines[2];
            realization.structural_parameters[0].access = access;
            if access == StructuralAccess::MutableBorrow {
                // This is a real typed write to the observed bytes, not merely
                // a write-shaped interface with an empty implementation.
                realization.blocks[0].operations = vec![
                    operation(
                        101,
                        value(101, 64),
                        OperationKind::StructuralByteSequenceFieldLength {
                            source: place(30),
                            path: vec![],
                            field: StructuralFieldId::new(1).unwrap(),
                        },
                    ),
                    operation(
                        102,
                        value(102, 64),
                        OperationKind::IntegerConstant {
                            value: semantic_vocabulary::IntegerValue::Unsigned(0),
                        },
                    ),
                    operation(
                        103,
                        value(103, 8),
                        OperationKind::IntegerConstant {
                            value: semantic_vocabulary::IntegerValue::Unsigned(255),
                        },
                    ),
                    operation(
                        104,
                        OperationResult::Unit,
                        OperationKind::StructuralByteSequenceFieldByteStore {
                            destination: place(30),
                            path: vec![],
                            field: StructuralFieldId::new(1).unwrap(),
                            index: ValueId::new(102).unwrap(),
                            value: ValueId::new(103).unwrap(),
                            length: ValueId::new(101).unwrap(),
                            obligation: ObligationId::new(104).unwrap(),
                        },
                    ),
                ];
            }
            OperationKind::CallDynamicUnit {
                descriptor_ordinal: 0,
                requirement_obligations: vec![],
                crash_continuations: vec![],
            }
        }
        8 | 9 => {
            let mut helper = body(5, access, structure(1));
            helper.structural_parameters.clear();
            helper.structural_places.clear();
            helper.entry = BlockId::new(50).unwrap();
            helper.blocks[0].id = helper.entry;
            helper.blocks[0].terminator = Terminator::ReturnUnit {
                edge: EdgeId::new(50).unwrap(),
                trivial_affine_discards: vec![],
            };
            helper.blocks[0].operations =
                vec![operation(50, OperationResult::Unit, parameter_call())];
            module.machines.push(helper);
            module
                .dynamic_dispatch
                .parameters
                .extend([dynamic_parameter(machine(1)), dynamic_parameter(machine(5))]);
            module
                .dynamic_dispatch
                .arguments
                .push(TerminalDynamicDescriptorArgument {
                    owner: machine(1),
                    operation: OperationId::new(3).unwrap(),
                    parameter_ordinal: 0,
                    source: TerminalDynamicDescriptorSource::Parameter { ordinal: 0 },
                });
            module
                .dynamic_dispatch
                .parameter_dispatches
                .push(dispatch(machine(5), 50));
            OperationKind::CallUnit {
                callee: machine(5),
                arguments: vec![],
                erased_arguments: vec![],
                erased_proof_arguments: vec![],
                structural_arguments: vec![],
                claim_transfers: vec![],
                requirement_obligations: vec![],
                crash_continuations: vec![],
            }
        }
        _ => unreachable!(),
    };
    module.machines[0].blocks[1]
        .operations
        .insert(0, operation(3, OperationResult::Unit, kind));
}
