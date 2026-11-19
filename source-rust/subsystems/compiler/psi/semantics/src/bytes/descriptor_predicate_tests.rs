//! Raw source-graph queries, not an admitted terminal product. The graph is
//! derived by the production adapter; deliberate graph damage below tests
//! whether incoming evidence remains conservative at the observation cut.

use super::byte_sequence_predicates::{
    byte_sequence_source_predicates, incoming_byte_sequence_argument_predicates,
};
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorContentsQuery, DescriptorLocation,
    DescriptorMachine, DescriptorPayloadTarget, DescriptorSourceOrigin,
    require_unchanged_entry_storage, terminal_borrowed_storage_graph_from,
};
use semantic_vocabulary::{
    BlockId, ByteSequencePredicate, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarTerm, ScalarType,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, BoundaryParameterKind, ByteSequenceCarrier, ByteSequencePredicateSet, MachineContract,
    Operation, OperationKind, OperationResult, RegisterTargetApplicability, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalIdentityIndex, TerminalMachine, TerminalModule,
    Terminator, ValueDeclaration,
};

fn place(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}
fn block(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}
fn parameter(raw: u64, position: u32, access: StructuralAccess) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(raw),
        position,
        structural_type: StructuralTypeId::new(1).unwrap(),
        access,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }
}
fn argument(raw: u64) -> StructuralArgument {
    StructuralArgument::whole(place(raw), StructuralAccess::SharedBorrow)
}
fn jump(edge: u64, source: u64) -> Terminator {
    Terminator::Jump {
        edge: EdgeId::new(edge).unwrap(),
        target: block(4),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument::whole(
            place(source),
            StructuralAccess::Owned,
        )],
        cleanup_actions: Vec::new(),
    }
}
fn required(root: u64) -> Vec<Proposition> {
    [
        ByteSequencePredicate::NoNul,
        ByteSequencePredicate::ValidUtf8,
        ByteSequencePredicate::NonEmpty,
    ]
    .into_iter()
    .map(|predicate| {
        Proposition::Equal(
            ScalarTerm::byte_sequence_predicate(predicate, place(root), Vec::new()),
            ScalarTerm::Boolean(true),
        )
    })
    .collect()
}

fn fixture(second_arrival: u64) -> TerminalModule {
    let byte_type = StructuralTypeId::new(1).unwrap();
    let mut module = TerminalModule::declarations();
    module.structural_types.push(StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id: byte_type,
        identity: TerminalIdentityIndex::ABSENT,
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        scalar_restriction: None,
        requires: Vec::new(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }),
    });
    let mut contract = MachineContract::empty(ContractId::new(1).unwrap());
    contract.parameters.push(ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    contract.structural_parameters = vec![
        parameter(1, 0, StructuralAccess::SharedBorrow),
        parameter(2, 1, StructuralAccess::SharedBorrow),
    ];
    contract.parameter_order = vec![
        BoundaryParameterKind::Scalar,
        BoundaryParameterKind::Structural,
        BoundaryParameterKind::Structural,
    ];
    contract.requires = required(1);
    let successor = |edge, target| SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: block(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let entry = Block::plain(
        block(1),
        Vec::new(),
        Terminator::Conditional {
            condition: ValueId::new(1).unwrap(),
            when_true: successor(1, 2),
            when_false: successor(2, 3),
        },
    );
    let left = Block::plain(block(2), Vec::new(), jump(3, 1));
    let right = Block::plain(block(3), Vec::new(), jump(4, second_arrival));
    let mut join = Block::plain(
        block(4),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(5).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    join.structural_parameters
        .push(parameter(3, 0, StructuralAccess::Owned));
    let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    join.operations = vec![
        Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(2).unwrap(),
                scalar_type: count,
                qualifications: Default::default(),
            }),
            OperationKind::ByteSequenceLength {
                source: argument(3),
            },
        ),
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(3).unwrap(),
                scalar_type: count,
                qualifications: Default::default(),
            }),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        Operation::unsuspended(
            OperationId::new(3).unwrap(),
            OperationResult::Structural(StructuralOperationResult {
                place: place(4),
                structural_type: byte_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::ByteSequenceSubslice {
                source: argument(3),
                start: ValueId::new(3).unwrap(),
                end: ValueId::new(3).unwrap(),
                length: ValueId::new(2).unwrap(),
                obligation: ObligationId::new(1).unwrap(),
            },
        ),
    ];
    module.machines.push(TerminalMachine {
        call_formations: Vec::new(),
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: MachineId::new(1).unwrap(),
        attachment: None,
        ranked_scc: None,
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: place(1),
                kind: StructuralPlaceKind::Parameter { position: 0 },
            },
            StructuralPlaceDeclaration {
                id: place(2),
                kind: StructuralPlaceKind::Parameter { position: 1 },
            },
            StructuralPlaceDeclaration {
                id: place(3),
                kind: StructuralPlaceKind::BlockParameter {
                    block: block(4),
                    position: 0,
                },
            },
            StructuralPlaceDeclaration {
                id: place(4),
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(3).unwrap(),
                    structural_type: byte_type,
                },
            },
        ],
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block(1),
        blocks: vec![entry, left, right, join],
        contract,
    });
    module
}

fn graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
    terminal_borrowed_storage_graph_from(module, [module.machines[0].id], &[]).unwrap()
}
fn observe(
    module: &TerminalModule,
    graph: &[DescriptorMachine],
    source: u64,
    before: u32,
) -> Option<ByteSequencePredicateSet> {
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, graph);
    incoming_byte_sequence_argument_predicates(
        module,
        &module.machines[0],
        &mut analysis,
        (block(4), before),
        &argument(source),
    )
}

fn established(
    module: &TerminalModule,
    graph: &[DescriptorMachine],
    source: u64,
    before: u32,
) -> ByteSequencePredicateSet {
    observe(module, graph, source, before).unwrap_or_else(|| {
        let identity = module.machines[0].id;
        let position = (block(4), before);
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, graph);
        let origins = analysis.try_symbolic_source_origins(identity, position, &DescriptorLocation::whole(place(source)));
        let guards = origins.as_ref().ok().map(|origins| origins.iter().map(|origin| {
            let query = match origin {
                DescriptorSourceOrigin::IncomingSlot(slot) => DescriptorContentsQuery::from_location(slot),
                DescriptorSourceOrigin::IncomingContents { query, .. } => query.clone(),
                        DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { source, .. }) => DescriptorContentsQuery::from_location(&DescriptorLocation::whole(*source)),
                _ => return Err("not incoming entry contents"),
            };
            require_unchanged_entry_storage(&module.structural_types, graph, &mut analysis, identity, position, &DescriptorPayloadTarget::Incoming(query))
        }).collect::<Vec<_>>());
        panic!("no incoming byte facts for {source} at {position:?}; origins={origins:?}; version guards={guards:?}")
    })
}

#[test]
fn current_payload_facts_lose_byte_membership_after_a_write() {
    let mut module = fixture(1);
    let carrier = module.machines[0].structural_parameters[0].structural_type;
    let ascii = semantic_vocabulary::StructuralDomainId::new(1).unwrap();
    module
        .structural_domains
        .push(terminal_psi::StructuralDomainDeclaration {
            id: ascii,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex::ABSENT,
            subject_identity: TerminalIdentityIndex::ABSENT,
            carrier,
            required_memberships: Some(Vec::new()),
            predicate: None,
            establishment_routes: None,
            access_policy: None,
            content_projection: None,
            byte_predicate: Some(ByteSequencePredicate::AsciiOnly),
            complete_predicate_theory: true,
        });
    let machine = &mut module.machines[0];
    machine.contract.requires.clear();
    machine.structural_parameters[0].qualifications.push(ascii);
    let mut graph = graph(&module);
    // The write on one branch has no proved contents for this storage.
    graph[0]
        .payload_invalidations
        .insert((block(2), 0), vec![None]);
    let machine = &module.machines[0];
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut history = analysis.payload_history(&module, machine);
    let entry = history
        .current_selected_payload_facts_in(&mut analysis, (block(1), 0), place(1), &[])
        .unwrap();
    assert_eq!(entry.qualifications, [ascii]);
    let after = history
        .current_selected_payload_facts_in(&mut analysis, (block(4), 0), place(1), &[])
        .unwrap();
    assert!(after.qualifications.is_empty());
}

#[test]
fn incoming_byte_predicates_preserve_exact_descriptors_and_only_safe_window_facts() {
    let module = fixture(1);
    let graph = graph(&module);
    let whole = established(&module, &graph, 3, 0);
    let mut expected = ByteSequencePredicateSet::EMPTY;
    for predicate in [
        ByteSequencePredicate::NoNul,
        ByteSequencePredicate::ValidUtf8,
        ByteSequencePredicate::NonEmpty,
    ] {
        expected.insert(predicate);
    }
    assert_eq!(whole, expected);
    assert_eq!(
        byte_sequence_source_predicates(&module, &module.machines[0], place(3)),
        Some(expected),
        "both arms pass the same immutable entry view"
    );
    let window = observe(&module, &graph, 4, 3).unwrap();
    let mut no_nul = ByteSequencePredicateSet::EMPTY;
    no_nul.insert(ByteSequencePredicate::NoNul);
    assert_eq!(
        window, no_nul,
        "empty or split UTF-8 windows retain no nonempty/UTF-8 promise"
    );
    assert!(
        module.machines[0].blocks[3].structural_parameters[0]
            .qualifications
            .is_empty()
    );
}

#[test]
fn incoming_byte_predicates_require_every_arrival_and_real_shared_entry_evidence() {
    let mut module = fixture(2);
    assert!(observe(&module, &graph(&module), 3, 0).unwrap().is_empty());
    module.machines[0].contract.requires.extend(required(2));
    assert!(!observe(&module, &graph(&module), 3, 0).unwrap().is_empty());
    module.machines[0].contract.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    assert!(observe(&module, &graph(&module), 3, 0).is_none());
    let cyclic = fixture(3);
    let cyclic_graph = graph(&cyclic);
    assert!(observe(&cyclic, &cyclic_graph, 3, 0).is_none());
    assert!(observe(&cyclic, &cyclic_graph, 99, 0).is_none());
}

#[test]
fn incoming_byte_predicates_refuse_changed_contents_on_any_path() {
    let module = fixture(1);
    let mut graph = graph(&module);
    assert!(observe(&module, &graph, 3, 0).is_some());
    graph[0].payload_invalidations.insert(
        (block(2), 0),
        vec![Some(DescriptorContentsQuery::from_location(
            &DescriptorLocation::whole(place(1)),
        ))],
    );
    assert!(
        observe(&module, &graph, 3, 0).is_none(),
        "fixed descriptor cannot certify modified bytes"
    );
    graph[0].payload_invalidations.clear();
    graph[0].unresolved_contents_writes.insert((block(3), 0));
    assert!(observe(&module, &graph, 3, 0).is_none());
}

#[test]
fn incoming_byte_predicates_refuse_forgotten_or_erased_window_geometry() {
    let module = fixture(1);
    let mut graph = graph(&module);
    let key = graph[0].projections.keys().next().unwrap().clone();
    let original = graph[0].projections[&key].clone();
    graph[0]
        .projections
        .insert(key.clone(), original.clone().without_captured_address());
    assert!(observe(&module, &graph, 4, 3).is_none());
    graph[0]
        .projections
        .insert(key, original.without_runtime_coordinates());
    assert!(observe(&module, &graph, 4, 3).is_none());
}
