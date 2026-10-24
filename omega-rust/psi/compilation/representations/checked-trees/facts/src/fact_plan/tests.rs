use crate::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment, ProgramPoint,
    QualificationCorrespondence, QualificationEvidence, QualificationPayloadIdentity,
};
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{
    ExpressionNode, TableIndexedExpression, TableMemberExpression, TableNamePath,
};
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceHandle;

#[test]
fn invocation_referents_match_only_the_same_typed_storage_identity() {
    let program = TypedTrees::default();
    let mut facts = FactPlan::default();
    let parameter = SymbolHandle::from_arena_index(10);
    let reference = TypeReferenceHandle::from_arena_index(20);
    let root = PlaceRoot::InvocationReferent {
        parameter,
        type_reference: reference,
    };
    let first = facts.places.append(crate::Place {
        root,
        segments: HandleSpan::empty(),
    });
    let same = facts.places.append(crate::Place {
        root,
        segments: HandleSpan::empty(),
    });
    let other_parameter = facts.places.append(crate::Place {
        root: PlaceRoot::InvocationReferent {
            parameter: SymbolHandle::from_arena_index(11),
            type_reference: reference,
        },
        segments: HandleSpan::empty(),
    });
    let other_type = facts.places.append(crate::Place {
        root: PlaceRoot::InvocationReferent {
            parameter,
            type_reference: TypeReferenceHandle::from_arena_index(21),
        },
        segments: HandleSpan::empty(),
    });
    let executable = facts.append_symbol_place(parameter);
    assert!(facts.places_match(&program, first, same));
    assert!(!facts.places_match(&program, first, other_parameter));
    assert!(!facts.places_match(&program, first, other_type));
    assert!(!facts.places_match(&program, first, executable));
    assert!(root.as_symbol().is_none());
    assert!(root.as_expression().is_none());
}

#[test]
fn unique_context_facts_keeps_complete_identity_and_order() {
    let mut plan = FactPlan::default();
    // This collection operation retains opaque identities; it does not
    // dereference or admit their payloads, including a different generation.
    let first = crate::FactHandle::from_parts(7, 1);
    let next_generation = crate::FactHandle::from_parts(7, 2);
    let other = crate::FactHandle::from_parts(8, 1);
    let mut first_references = HandleSpan::empty();
    for fact in [first, next_generation, first] {
        plan.append_ref(&mut first_references, fact);
    }
    let first_context = plan.append_context(ProgramPoint::Global, first_references);
    let mut other_references = HandleSpan::empty();
    for fact in [other, first, next_generation] {
        plan.append_ref(&mut other_references, fact);
    }
    let other_context = plan.append_context(ProgramPoint::Global, other_references);
    assert_eq!(
        plan.unique_context_facts([other_context, first_context, other_context]),
        [other, first, next_generation],
    );
}

#[test]
fn clone_from_replaces_every_fact_root_with_the_source_snapshot() {
    let mut source = FactPlan::default();
    source.places.append(Default::default());
    source.place_segments.append(Default::default());
    source.facts.append(Default::default());
    source.scalar_values.append(Default::default());
    source.integer_ranges.append(Default::default());
    source
        .qualification_correspondences
        .append(Default::default());
    source.domain_definition_facts.append(Default::default());
    source.data_definition_facts.append(Default::default());
    source.instantiated_expressions.append(Default::default());
    source.refs.append(Default::default());
    source.contexts.append(Default::default());
    source.symbol_sets.append(Default::default());
    let mut destination = FactPlan::with_capacity(8, 8);
    destination.clone_from(&source);
    assert_eq!(destination, source.clone());
    destination.clone_from(&FactPlan::default());
    assert_eq!(destination, FactPlan::default());
    destination.clone_from(&source);
    assert_eq!(destination, source);
}

#[test]
fn qualification_correspondence_ledger_is_exact_and_idempotent() {
    let mut facts = FactPlan::default();
    let source_place = facts.append_symbol_place(SymbolHandle::from_arena_index(10));
    let destination_place = facts.append_symbol_place(SymbolHandle::from_arena_index(11));
    let source_fact = facts.append_fact(Fact::default());
    let destination_fact = facts.append_fact(Fact::default());
    let row = QualificationCorrespondence {
        source_fact,
        destination_fact,
        source_occurrence_place: source_place,
        source_place,
        destination_place,
        formation: ProgramPoint::Statement {
            machine_symbol: SymbolHandle::from_arena_index(12),
            state_symbol: SymbolHandle::from_arena_index(13),
            statement_index: 2,
        },
        payload: QualificationPayloadIdentity::CarryOrigin,
        evidence: QualificationEvidence::from_origin(
            language_semantics::QualificationEvidenceOrigin::CheckedTransformation,
            SymbolHandle::from_arena_index(14),
        ),
    };

    let first = facts.append_qualification_correspondence(row);
    let second = facts.append_qualification_correspondence(row);
    assert_eq!(first, second);
    assert_eq!(facts.qualification_correspondences.len(), 1);
    assert_eq!(*facts.qualification_correspondences.get(first), row);
}

#[test]
fn expression_places_preserve_roots_and_segments() {
    let root_symbol = SymbolHandle::from_arena_index(30);
    let field_symbol = SymbolHandle::from_arena_index(31);
    let tail_symbol = SymbolHandle::from_arena_index(32);

    let mut program = TypedTrees::default();
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated("root"));
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated("field"));
    let mut member_symbols = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, root_symbol);
    program
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, field_symbol);
    let name = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            head_symbol: root_symbol,
            symbol: field_symbol,
        }));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(0),
    ));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection: name,
                index,
            }));
    let member = program
        .expression_table
        .insert(ExpressionNode::Member(TableMemberExpression {
            receiver: indexed,
            member_symbol: tail_symbol,
            member: Identifier::generated("tail"),
            case_variant: None,
        }));

    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(&program, member);
    let place = facts.places.get(place);
    let segments = facts.place_segments.span_or_empty(place.segments);

    assert_eq!(place.root, PlaceRoot::Symbol(root_symbol));
    assert_eq!(segments.len(), 3);
    assert_eq!(
        segments[0],
        PlaceSegment::Field {
            symbol: field_symbol
        }
    );
    assert_eq!(segments[1], PlaceSegment::FixedIndex { index: 0 });
    assert_eq!(
        segments[2],
        PlaceSegment::Field {
            symbol: tail_symbol
        }
    );
}

#[test]
fn proves_domain_membership_for_structurally_equal_places() {
    let domain_symbol = SymbolHandle::from_arena_index(40);
    let value_symbol = SymbolHandle::from_arena_index(41);
    let field_symbol = SymbolHandle::from_arena_index(42);
    let mut program = TypedTrees::default();
    program.push_domain_definition(typed_trees::domain::DomainDefinition {
        symbol: domain_symbol,
        ..Default::default()
    });

    let mut facts = FactPlan::default();
    let left = facts.append_symbol_place(value_symbol);
    facts.push_place_segment(
        left,
        PlaceSegment::Field {
            symbol: field_symbol,
        },
    );

    let right = facts.append_symbol_place(value_symbol);
    facts.push_place_segment(
        right,
        PlaceSegment::Field {
            symbol: field_symbol,
        },
    );

    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(left),
        point: ProgramPoint::Global,
        origin: FactOrigin::DomainDefinition { domain_symbol },
        evidence: Default::default(),
        payload: FactPayload::DomainMembership {
            value: typed_trees::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol,
            semantic_domain: Default::default(),
        },
    });
    let mut refs = HandleSpan::empty();
    facts.append_ref(&mut refs, fact);
    let context = facts.append_context(ProgramPoint::Global, refs);

    assert!(facts.places_equal(left, right));
    assert!(
        facts
            .context_view(facts.contexts.get(context))
            .proves_place_domain_membership(&program, right, domain_symbol)
    );
}

#[test]
fn expression_places_resolve_attached_data_members() {
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    // Place typing walks each symbol's retained parent, so the fixture's
    // parameter, state and machine are real table entries in that chain.
    let mut symbols = SymbolTableBuilder::default();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let top_level: Vec<_> = SymbolTableBuilder::child_handles(symbols.insert_children(
        root,
        [
            (SymbolKind::Data, SymbolNameRef::Static("Player")),
            (SymbolKind::Data, SymbolNameRef::Static("Main")),
            (SymbolKind::Machine, SymbolNameRef::Static("Main::main")),
        ],
    ))
    .collect();
    let (player_type_symbol, main_data_symbol, machine_symbol) =
        (top_level[0], top_level[1], top_level[2]);
    let player_field_symbol = symbols
        .insert_children(
            main_data_symbol,
            [(SymbolKind::Field, SymbolNameRef::Static("player"))],
        )
        .start();
    let state_symbol = symbols
        .insert_children(
            machine_symbol,
            [(SymbolKind::State, SymbolNameRef::Static("main"))],
        )
        .start();
    let main_parameter_symbol = symbols
        .insert_children(
            state_symbol,
            [(SymbolKind::Parameter, SymbolNameRef::Static("main"))],
        )
        .start();

    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..TypedTrees::default()
    };
    program.push_data_definition(typed_trees::data::DataDefinition {
        symbol: player_type_symbol,
        name: Identifier::generated("Player"),
        is_public: false,
        supply_mode: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        properties: typed_trees::data::DataProperties::default(),
        quotient: None,
        where_facts: Default::default(),
        zero_gated: false,
        retired_identities: Vec::new(),
        generic_instance: None,
        members: HandleSpan::empty(),
    });
    let mut main_data = typed_trees::data::DataDefinition {
        symbol: main_data_symbol,
        name: Identifier::generated("Main"),
        is_public: false,
        supply_mode: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        properties: typed_trees::data::DataProperties::default(),
        quotient: None,
        where_facts: Default::default(),
        zero_gated: false,
        retired_identities: Vec::new(),
        generic_instance: None,
        members: HandleSpan::empty(),
    };
    program.push_data_member(
        &mut main_data,
        typed_trees::data::DataMember::Field(typed_trees::data::DataField {
            identity: None,
            symbol: player_field_symbol,
            name: Identifier::generated("player"),
            relevance: Default::default(),
            type_reference: TypeReferenceHandle::invalid(),
        }),
    );
    program.push_data_definition(main_data);

    let mut machine = typed_trees::machine::Machine {
        assembly_target_requirement: language_core::inline_assembly::AsmTargetApplicability::Any,
        symbol: machine_symbol,
        name: Identifier::generated("Main::main"),
        target: None,
        supply_mode: Default::default(),
        target_family_interface: false,
        interface_exposure: Default::default(),
        purpose: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        admitted_same_stack_bytes: 0,
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        body_is_present: true,
        body_status: typed_trees::machine::MachineBodyStatus::Complete,
        structural_type_equations_pending: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        attached_data: Some(Identifier::generated("Main")),
        attached_data_symbol: main_data_symbol,
        attached_data_application: TypeReferenceHandle::invalid(),
        generic_data_template: SymbolHandle::invalid(),
        spelling: None,
        is_public: false,
        owned_data: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        invokes: HandleSpan::empty(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::empty(),
        states: HandleSpan::empty(),
        authored_target: None,
        entry_context_origin: None,
    };
    let mut state = typed_trees::state::State {
        symbol: state_symbol,
        name: Identifier::generated("main"),
        parameters: HandleSpan::empty(),
        return_type: TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        statement_nodes: HandleSpan::empty(),
    };
    let main_type =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: main_data_symbol,
                name: Identifier::generated("Main"),
            });
    program.push_state_parameter(
        &mut state,
        typed_trees::signature::StateParameter {
            symbol: main_parameter_symbol,
            name: Identifier::generated("main"),
            type_reference: main_type,
            is_const: false,
            is_mutable: true,
            relevance: Default::default(),
        },
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let main_expression = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members: HandleSpan::empty(),
            member_symbols: HandleSpan::empty(),
            head_symbol: main_parameter_symbol,
            symbol: main_parameter_symbol,
        }));
    let member_expression =
        program
            .expression_table
            .insert(ExpressionNode::Member(TableMemberExpression {
                receiver: main_expression,
                member_symbol: SymbolHandle::invalid(),
                member: Identifier::generated("player"),
                case_variant: None,
            }));

    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(&program, member_expression);
    let place = facts.places.get(place);
    let segments = facts.place_segments.span_or_empty(place.segments);

    assert_eq!(place.root, PlaceRoot::Symbol(main_parameter_symbol));
    assert_eq!(segments.len(), 1);
    assert_eq!(
        segments[0],
        PlaceSegment::Field {
            symbol: player_field_symbol
        }
    );
}

#[test]
fn expression_places_resolve_exact_local_and_collection_element_types() {
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::statement::{StatementNode, TableLocalData};
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    for (shape, indexed, malformed) in [
        ("record", false, "none"),
        ("array", true, "none"),
        ("slice", true, "none"),
        ("array", true, "stale"),
        ("record", false, "wrong_parent"),
    ] {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("Outer")),
                (SymbolKind::Machine, SymbolNameRef::Static("main")),
            ],
        );
        let children: Vec<_> = SymbolTableBuilder::child_handles(children).collect();
        let data_symbol = children[0];
        let machine_symbol = children[1];
        let field_symbol = symbols
            .insert_children(
                data_symbol,
                [(SymbolKind::Field, SymbolNameRef::Static("inner"))],
            )
            .start();
        let states = symbols.insert_children(
            machine_symbol,
            [
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::State, SymbolNameRef::Static("foreign")),
            ],
        );
        let states: Vec<_> = SymbolTableBuilder::child_handles(states).collect();
        let local_symbol = symbols
            .insert_children(
                if malformed == "wrong_parent" {
                    states[1]
                } else {
                    states[0]
                },
                [(SymbolKind::Local, SymbolNameRef::Static("value"))],
            )
            .start();
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        };
        let record = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                name: Identifier::generated("Outer"),
                symbol: data_symbol,
            });
        let local_type = match shape {
            "array" => program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: record,
                    length: FixedArrayLength::Literal(2),
                }),
            "slice" => program
                .type_reference_table
                .insert(TypeReferenceNode::Slice {
                    element_type: record,
                }),
            _ => record,
        };
        let mut data = DataDefinition {
            symbol: data_symbol,
            name: Identifier::generated("Outer"),
            ..DataDefinition::default()
        };
        program.push_data_member(
            &mut data,
            DataMember::Field(DataField {
                identity: None,
                symbol: field_symbol,
                name: Identifier::generated("inner"),
                relevance: Default::default(),
                type_reference: TypeReferenceHandle::invalid(),
            }),
        );
        program.push_data_definition(data);
        let mut state = typed_trees::state::State {
            symbol: states[0],
            name: Identifier::generated("entry"),
            ..Default::default()
        };
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: local_symbol,
                name: Identifier::generated("value"),
                type_reference: local_type,
                ..Default::default()
            }),
        );
        let mut machine = typed_trees::machine::Machine {
            symbol: machine_symbol,
            name: Identifier::generated("main"),
            ..Default::default()
        };
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        let root_symbol = if malformed == "stale" {
            SymbolHandle::from_parts(local_symbol.arena_index(), local_symbol.generation() + 1)
        } else {
            local_symbol
        };
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("value"));
        let mut member_symbols = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, root_symbol);
        let mut receiver = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols,
                head_symbol: root_symbol,
                symbol: root_symbol,
            }));
        if indexed {
            let index = program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(0),
            ));
            receiver =
                program
                    .expression_table
                    .insert(ExpressionNode::Indexed(TableIndexedExpression {
                        collection: receiver,
                        index,
                    }));
        }
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: SymbolHandle::invalid(),
                    member: Identifier::generated("inner"),
                    case_variant: None,
                }));
        let mut facts = FactPlan::default();
        let place = facts.append_place_from_expression(&program, expression);
        let place = facts.places.get(place);
        let segments = facts.place_segments.span_or_empty(place.segments);
        assert_eq!(place.root, PlaceRoot::Symbol(root_symbol));
        assert_eq!(segments.len(), if indexed { 2 } else { 1 });
        if indexed {
            assert_eq!(segments[0], PlaceSegment::FixedIndex { index: 0 });
        }
        assert_eq!(
            segments.last(),
            Some(&PlaceSegment::Field {
                symbol: if malformed == "none" {
                    field_symbol
                } else {
                    SymbolHandle::invalid()
                },
            }),
            "{shape} {malformed}"
        );
    }
}

#[test]
fn shared_checkpoint_detaches_only_edited_fact_roots() {
    let mut baseline = FactPlan::default();
    macro_rules! seed_roots {
        ($($root:ident),+ $(,)?) => {
            $(baseline.$root.append(Default::default());)+
        };
    }
    seed_roots!(
        places,
        place_segments,
        facts,
        scalar_values,
        boolean_predicates,
        boolean_predicate_bindings,
        integer_ranges,
        qualification_correspondences,
        domain_definition_facts,
        data_definition_facts,
        instantiated_expressions,
        refs,
        symbol_sets,
    );
    let context = baseline.contexts.append(Default::default());
    baseline.share_pages();
    let mut worker = baseline.clone();
    macro_rules! assert_shared_then_detached {
        ($($root:ident),+ $(,)?) => {
            $(
                let handle = baseline.$root.iter().next().unwrap().0;
                assert!(std::ptr::eq(baseline.$root.get(handle), worker.$root.get(handle)),
                    stringify!($root));
                let _ = worker.$root.get_mut(handle);
                assert!(!std::ptr::eq(baseline.$root.get(handle), worker.$root.get(handle)),
                    stringify!($root));
            )+
        };
    }
    assert_shared_then_detached!(
        places,
        place_segments,
        facts,
        scalar_values,
        boolean_predicates,
        boolean_predicate_bindings,
        integer_ranges,
        qualification_correspondences,
        domain_definition_facts,
        data_definition_facts,
        instantiated_expressions,
        refs,
        symbol_sets,
    );
    assert!(std::ptr::eq(
        baseline.contexts.get(context),
        worker.contexts.get(context)
    ));
    let fact = baseline.facts.iter().next().unwrap().0;
    let original = *baseline.facts.get(fact);
    worker.facts.get_mut(fact).origin = FactOrigin::StatementTransfer;
    assert_eq!(*baseline.facts.get(fact), original);
    assert_eq!(worker.facts.get(fact).origin, FactOrigin::StatementTransfer);
    assert!(worker.contexts.free(context));
    assert_eq!(
        baseline
            .context_handles_at_point(ProgramPoint::Global)
            .count(),
        1
    );
    assert_eq!(
        worker
            .context_handles_at_point(ProgramPoint::Global)
            .count(),
        0
    );
}

#[test]
fn shared_checkpoint_adopts_disjoint_worker_pages_in_publication_order() {
    fn extend(
        plan: &mut FactPlan,
        generation: u32,
    ) -> (crate::FactHandle, crate::FactContextHandle) {
        plan.begin_page(generation);
        let fact = plan.append_fact(Fact::default());
        let mut references = HandleSpan::empty();
        plan.append_ref(&mut references, fact);
        let context = plan.append_context(ProgramPoint::Global, references);
        (fact, context)
    }

    let mut baseline = FactPlan::default();
    let first = baseline.append_context(ProgramPoint::Global, HandleSpan::empty());
    let group = baseline.context_group_at_point(ProgramPoint::Global);
    baseline.share_pages();
    let mut first_worker = baseline.clone();
    let mut second_worker = baseline.clone();
    let first_rows = extend(&mut first_worker, 2);
    let second_rows = extend(&mut second_worker, 3);
    assert_eq!(
        baseline.context_handles_in_group(group).collect::<Vec<_>>(),
        [first]
    );
    assert_eq!(
        first_worker
            .context_handles_in_group(group)
            .collect::<Vec<_>>(),
        [first, first_rows.1]
    );
    assert_eq!(
        second_worker
            .context_handles_in_group(group)
            .collect::<Vec<_>>(),
        [first, second_rows.1]
    );
    let first_pages = first_worker.take_additional_pages();
    let second_pages = second_worker.take_additional_pages();
    first_worker.clone_from(&baseline);
    second_worker.clone_from(&baseline);
    assert_eq!(
        first_worker
            .context_handles_in_group(group)
            .collect::<Vec<_>>(),
        [first]
    );
    assert_eq!(
        second_worker
            .context_handles_in_group(group)
            .collect::<Vec<_>>(),
        [first]
    );

    // Arrival can differ from page order. Compare with ordinary publication
    // in that same arrival order; no sorting may replace context encounter order.
    let mut sequential = baseline.clone();
    assert_eq!(extend(&mut sequential, 3), second_rows);
    assert_eq!(extend(&mut sequential, 2), first_rows);
    baseline.adopt_additional_pages(second_pages);
    baseline.adopt_additional_pages(first_pages);
    assert_eq!(
        baseline.facts.iter().collect::<Vec<_>>(),
        sequential.facts.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        baseline.contexts.iter().collect::<Vec<_>>(),
        sequential.contexts.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        baseline.context_handles_in_group(group).collect::<Vec<_>>(),
        [first, second_rows.1, first_rows.1]
    );
    assert_eq!(
        baseline.context_handles_in_group(group).collect::<Vec<_>>(),
        sequential
            .context_handles_in_group(group)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        baseline
            .contexts
            .get(first_rows.1)
            .facts
            .start()
            .generation(),
        2
    );
    assert_eq!(
        baseline
            .contexts
            .get(second_rows.1)
            .facts
            .start()
            .generation(),
        3
    );
}
