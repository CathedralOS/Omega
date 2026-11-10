#![forbid(unsafe_code)]

/*
We own `FactPlan`, the target-neutral arena store of checked places, fact
contexts, qualification evidence, write frames, and boolean/integer predicates
established during `Psi 05` checking.
*/

/*
We keep `FactPlan` in a dedicated companion package (`facts`) so `typed-trees`
 typing (`Psi 03`), elaboration (`Psi 04`), and checking (`Psi 05`) can share
paged fact arenas (`PagedArenaSet`), place resolution, and canonical predicate
representations without circular crate dependencies.
*/

/*
- Produced by: `Psi 03–05` (`symbol-resolved-trees-to-typed-trees`, `typed-trees-to-typed-trees`, `typed-trees-to-checked-trees`).
- Consumed by: `checked-trees`, `Psi 06`, `proof`, `validation`, and `review-evidence`.
- Flat module layout:
  - `lib.rs`: `FactPlan`, `FactHandle`, `FactContextHandle`, `PlaceHandle`, and whole-plan paged arena operations.
  - `contexts_and_places.rs`: program-point fact contexts (`contexts`) and projected storage places/write frames (`places`).
  - `evidence.rs`: `FactTable`, `Fact`, `FactPayload`, qualification evidence, and float predicates (`evidence`).
  - `predicates.rs`: boolean predicates, integer math expressions, and byte-sequence comparison operands (`predicates`).
*/

mod contexts_and_places;
pub use contexts_and_places::{contexts, places};
pub mod evidence;
pub mod predicates;

macro_rules! fact_plan_root_roster {
    ($callback:ident $(, $argument:ident)*) => {
        $callback!(
            $($argument,)* places, place_segments, facts, scalar_values, boolean_predicates,
            boolean_predicate_bindings, integer_ranges, qualification_correspondences,
            domain_definition_facts, data_definition_facts, instantiated_expressions, refs,
            assigned_refs, symbol_sets, qualification_issuers, contract_member_projections,
        );
    };
}

#[cfg(test)]
pub(crate) mod tests {
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
    fn owned_qualification_batches_keep_prefix_and_complete_record_identity() {
        use crate::{ContractFactKind, FloatPredicate, QualificationProvenance};
        use numerics::literals::FloatFormat;
        let mut plan = FactPlan::default();
        let root = PlaceRoot::Symbol(SymbolHandle::from_parts(10, 1));
        let mut place = |root, segments: &[PlaceSegment]| {
            let mut span = HandleSpan::empty();
            for segment in segments {
                plan.place_segments.append_to_span(&mut span, *segment);
            }
            plan.places.append(crate::Place {
                root,
                segments: span,
            })
        };
        let path = [PlaceSegment::FixedIndex { index: 3 }];
        let first_place = place(root, &path);
        let same_place = place(root, &path);
        let sibling = place(root, &[PlaceSegment::FixedIndex { index: 4 }]);
        let next_generation = place(PlaceRoot::Symbol(SymbolHandle::from_parts(10, 2)), &path);
        let base = Fact {
            place: FactPlace::Place(first_place),
            payload: FactPayload::DomainMembership {
                value: Default::default(),
                domain: HandleSpan::empty(),
                domain_symbol: SymbolHandle::from_parts(20, 1),
                semantic_domain: Default::default(),
            },
            ..Default::default()
        };
        let mut same = base;
        same.place = FactPlace::Place(same_place);
        same.origin = FactOrigin::StatementTransfer;
        same.point = ProgramPoint::Definition {
            symbol: SymbolHandle::from_parts(30, 1),
        };
        // Different display/value fields and the contract tag are not part of
        // duplicate identity. The selected domain and its identity still are.
        let contract = Fact {
            payload: FactPayload::ContractDomainMembership {
                kind: ContractFactKind::default(),
                fact: Default::default(),
                value: typed_trees::expression::ExpressionHandle::from_parts(40, 1),
                domain: HandleSpan::from_parts(arena::Handle::from_parts(50, 2), 1),
                domain_symbol: SymbolHandle::from_parts(20, 1),
                semantic_domain: Default::default(),
            },
            ..same
        };
        let mut evidence = same;
        evidence.evidence.provenance = QualificationProvenance::Independent;
        let mut issuer = same;
        issuer.evidence.issuer = arena::Handle::from_parts(5, 2);
        let mut domain = same;
        domain.payload = FactPayload::DomainMembership {
            value: Default::default(),
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::from_parts(20, 2),
            semantic_domain: Default::default(),
        };
        let float = Fact {
            payload: FactPayload::FloatPredicate {
                predicate: FloatPredicate::non_nan(FloatFormat::F32),
            },
            ..same
        };
        let other_float = Fact {
            payload: FactPayload::FloatPredicate {
                predicate: FloatPredicate::non_nan(FloatFormat::F64),
            },
            ..same
        };
        let unsupported = Fact {
            payload: FactPayload::default(),
            ..same
        };
        let bound = Fact {
            payload: FactPayload::FloatPredicate {
                predicate: FloatPredicate::comparison(
                    FloatFormat::F32,
                    typed_trees::expression::BinaryOperator::Greater,
                    &numerics::bignum::ExactFloat::from_f32(1.0),
                )
                .unwrap(),
            },
            ..same
        };
        let mut origin = same;
        origin.evidence.origin = language_semantics::QualificationEvidenceOrigin::Propagated;
        let mut source = same;
        source.evidence.source_symbol = SymbolHandle::from_parts(30, 1);
        let mut requirement = same;
        requirement.evidence.requirement_symbol = SymbolHandle::from_parts(30, 1);
        let mut identity = same;
        identity.payload = FactPayload::DomainMembership {
            value: Default::default(),
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::from_parts(20, 1),
            semantic_domain: language_semantics::SemanticDomainTable::WRAPPING,
        };
        let rows = [
            base,
            base,
            same,
            contract,
            evidence,
            issuer,
            domain,
            Fact {
                place: FactPlace::Place(sibling),
                ..same
            },
            Fact {
                place: FactPlace::Place(next_generation),
                ..same
            },
            float,
            float,
            other_float,
            unsupported,
            unsupported,
            bound,
            origin,
            source,
            requirement,
            identity,
        ];
        assert_ne!(first_place, same_place);
        assert_eq!(
            plan.fact_place_bucket_hash(base.place),
            plan.fact_place_bucket_hash(same.place)
        );
        assert_eq!(
            plan.first_owned_qualification_positions(&rows, 2),
            [
                0, 1, 0, 0, 4, 5, 6, 7, 8, 9, 9, 11, 12, 13, 14, 15, 16, 17, 18
            ]
        );
        // A fresh batch must see changed input records rather than a retained
        // duplicate decision from the previous call.
        let mut changed = rows;
        changed[2].evidence = evidence.evidence;
        assert_eq!(plan.first_owned_qualification_positions(&changed, 2)[2], 2);
    }

    #[test]
    fn owned_qualification_buckets_avoid_cross_place_comparisons() {
        let mut plan = FactPlan::default();
        let root = PlaceRoot::Symbol(SymbolHandle::from_parts(10, 1));
        let mut prefix = Vec::new();
        for index in 0..512 {
            let mut segments = HandleSpan::empty();
            plan.place_segments
                .append_to_span(&mut segments, PlaceSegment::FixedIndex { index });
            let place = plan.places.append(crate::Place { root, segments });
            prefix.push(Fact {
                place: FactPlace::Place(place),
                payload: FactPayload::DomainMembership {
                    value: Default::default(),
                    domain: HandleSpan::empty(),
                    domain_symbol: SymbolHandle::from_parts(20, 1),
                    semantic_domain: Default::default(),
                },
                ..Default::default()
            });
        }
        let mut rows = prefix.clone();
        rows.extend_from_slice(&prefix);
        rows.extend(prefix.iter().rev().copied());
        let start = std::time::Instant::now();
        let buckets = crate::places::PlaceBucketIndex::new(
            rows.iter()
                .map(|fact| plan.fact_place_bucket_hash(fact.place)),
        );
        let preparation = start.elapsed();
        let collisions = crate::places::PlaceBucketIndex::new(rows.iter().map(|_| 0));
        let mut direct_comparisons = 0;
        let mut bucket_comparisons = 0;
        let mut collision_comparisons = 0;
        let mut expected: Vec<_> = (0..prefix.len()).collect();
        // Every new row in this batch duplicates the original prefix. Count
        // the actual first-match work, rather than all possible bucket pairs.
        for position in prefix.len()..rows.len() {
            let same_place =
                |prior: usize| plan.fact_places_equal(rows[prior].place, rows[position].place);
            let direct = (0..prefix.len())
                .find(|prior| {
                    direct_comparisons += 1;
                    same_place(*prior)
                })
                .unwrap();
            let prepared = buckets
                .candidates(plan.fact_place_bucket_hash(rows[position].place))
                .take_while(|prior| *prior < prefix.len())
                .find(|prior| {
                    bucket_comparisons += 1;
                    same_place(*prior)
                })
                .unwrap();
            let collided = collisions
                .candidates(0)
                .take_while(|prior| *prior < prefix.len())
                .find(|prior| {
                    collision_comparisons += 1;
                    same_place(*prior)
                })
                .unwrap();
            assert_eq!(direct, prepared);
            assert_eq!(direct, collided);
            expected.push(direct);
        }
        assert_eq!(
            plan.first_owned_qualification_positions(&rows, prefix.len()),
            expected
        );
        assert_eq!(bucket_comparisons, 1024);
        assert_eq!(direct_comparisons, 262656);
        assert_eq!(collision_comparisons, direct_comparisons);
        println!(
            "qualification batch: {} rows prepared in {:?}; direct {} comparisons; bucket {} comparisons; collision {} comparisons",
            rows.len(),
            preparation,
            direct_comparisons,
            bucket_comparisons,
            collision_comparisons
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
                reserved_result_of: SymbolHandle::invalid(),
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
        let member =
            program
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
    fn context_assignments_select_one_root_in_context_order() {
        let mut facts = FactPlan::default();
        let wanted = SymbolHandle::from_arena_index(50);
        let other = SymbolHandle::from_arena_index(3);
        let assignment = |facts: &mut FactPlan, symbol, value| {
            let place = facts.append_symbol_place(symbol);
            facts.append_fact(Fact {
                place: FactPlace::Place(place),
                payload: FactPayload::AssignedValue {
                    value: typed_trees::expression::ExpressionHandle::from_arena_index(value),
                },
                ..Default::default()
            })
        };
        let first = assignment(&mut facts, wanted, 1);
        let unrelated = assignment(&mut facts, other, 2);
        let membership_place = facts.append_symbol_place(wanted);
        let membership = facts.append_fact(Fact {
            place: FactPlace::Place(membership_place),
            payload: FactPayload::DomainMembership {
                value: typed_trees::expression::ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol: SymbolHandle::from_arena_index(9),
                semantic_domain: Default::default(),
            },
            ..Default::default()
        });
        let second = assignment(&mut facts, wanted, 4);
        let mut refs = HandleSpan::empty();
        for fact in [first, unrelated, membership, second] {
            facts.append_ref(&mut refs, fact);
        }
        let context = facts.append_context(ProgramPoint::Global, refs);
        let selected = facts
            .assignments_with_root(facts.contexts.get(context), PlaceRoot::Symbol(wanted))
            .map(|fact| fact.payload)
            .collect::<Vec<_>>();
        assert_eq!(
            selected,
            [first, second].map(|fact| facts.facts.get(fact).payload),
        );
    }

    #[test]
    fn expression_places_resolve_attached_data_members() {
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
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
            predicate_subject_symbol: SymbolHandle::invalid(),
            predicate_subject_type: TypeReferenceHandle::invalid(),
            is_public: false,
            supply_mode: Default::default(),
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            properties: typed_trees::data::DataProperties::default(),
            quotient: None,
            cleanup_selection: None,
            where_facts: Default::default(),
            zero_gated: false,
            retired_identities: Vec::new(),
            conformance_bounds: Vec::new(),
            generic_instance: None,
            members: HandleSpan::empty(),
        });
        let mut main_data = typed_trees::data::DataDefinition {
            symbol: main_data_symbol,
            name: Identifier::generated("Main"),
            predicate_subject_symbol: SymbolHandle::invalid(),
            predicate_subject_type: TypeReferenceHandle::invalid(),
            is_public: false,
            supply_mode: Default::default(),
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            properties: typed_trees::data::DataProperties::default(),
            quotient: None,
            cleanup_selection: None,
            where_facts: Default::default(),
            zero_gated: false,
            retired_identities: Vec::new(),
            conformance_bounds: Vec::new(),
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
            assembly_target_requirement:
                language_core::inline_assembly::AsmTargetApplicability::Any,
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
            declared_type_equations: HandleSpan::empty(),
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
            lexical_region_origin: None,
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

        let main_expression =
            program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: main_parameter_symbol,
                    symbol: main_parameter_symbol,
                    reserved_result_of: SymbolHandle::invalid(),
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
            let mut receiver =
                program
                    .expression_table
                    .insert(ExpressionNode::Name(TableNamePath {
                        members,
                        member_symbols,
                        head_symbol: root_symbol,
                        symbol: root_symbol,
                        reserved_result_of: SymbolHandle::invalid(),
                    }));
            if indexed {
                let index = program.expression_table.insert(ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(0),
                ));
                receiver = program.expression_table.insert(ExpressionNode::Indexed(
                    TableIndexedExpression {
                        collection: receiver,
                        index,
                    },
                ));
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
        ($plan:ident, $($root:ident),+ $(,)?) => {
            $($plan.$root.append(Default::default());)+
        };
    }
        fact_plan_root_roster!(seed_roots, baseline);
        let context = baseline.contexts.append(Default::default());
        baseline.share_pages();
        let mut worker = baseline.clone();
        macro_rules! assert_shared_then_detached {
        ($baseline:ident, $worker:ident, $($root:ident),+ $(,)?) => {
            $(
                let handle = $baseline.$root.iter().next().unwrap().0;
                assert!(std::ptr::eq($baseline.$root.get(handle), $worker.$root.get(handle)),
                    stringify!($root));
                let _ = $worker.$root.get_mut(handle);
                assert!(!std::ptr::eq($baseline.$root.get(handle), $worker.$root.get(handle)),
                    stringify!($root));
            )+
        };
    }
        fact_plan_root_roster!(assert_shared_then_detached, baseline, worker);
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

    #[test]
    fn sealed_declaration_baseline_is_shared_by_a_flow_fork() {
        let mut baseline = FactPlan::default();
        baseline.append_fact_context(Fact {
            place: FactPlace::Unknown,
            point: ProgramPoint::Global,
            origin: FactOrigin::ProofObligation,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::StorageDependency {
                dependent: crate::FactHandle::default(),
            },
        });
        baseline.share_pages();
        let mut worker = baseline.clone();
        worker.clone_from(&baseline);
        assert!(worker.shares_declaration_baseline(&baseline));
        let before = crate::declaration_page_fork_totals();
        crate::note_declaration_page_fork(&worker, &baseline);
        let after = crate::declaration_page_fork_totals();
        assert_eq!(after.0, before.0 + 1);
        assert_eq!(after.1, before.1);
        worker.begin_page(2);
        worker.append_fact_context(Fact {
            place: FactPlace::Unknown,
            point: ProgramPoint::Global,
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::StorageDependency {
                dependent: crate::FactHandle::default(),
            },
        });
        assert!(worker.shares_declaration_baseline(&baseline));
        assert_eq!(baseline.facts.len() + 1, worker.facts.len());
    }
}

use arena::{Handle, HandleSpan};
type Arena<T> = arena::PagedArenaSet<T>;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub type FactHandle = Handle<Fact>;
pub type FactContextHandle = Handle<FactContext>;
pub type PlaceHandle = Handle<Place>;
pub type QualificationCorrespondenceHandle = Handle<QualificationCorrespondence>;

#[derive(Debug, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FactPlan {
    pub contract_member_projections: Arena<crate::ContractMemberProjection>,
    pub qualification_issuers: Arena<crate::BoundaryQualificationIssuerRecord>,
    pub places: Arena<Place>,
    pub place_segments: Arena<PlaceSegment>,
    pub facts: evidence::FactTable,
    pub scalar_values: Arena<crate::ScalarValue>,
    pub boolean_predicates: Arena<crate::BooleanPredicate>,
    pub boolean_predicate_bindings: Arena<crate::BooleanPredicateBinding>,
    pub integer_ranges: Arena<crate::IntegerRange>,
    pub qualification_correspondences: Arena<QualificationCorrespondence>,
    pub domain_definition_facts: Arena<DomainDefinitionFactRecord>,
    pub data_definition_facts: Arena<DataDefinitionFactRecord>,
    pub instantiated_expressions: Arena<InstantiatedExpression>,
    pub refs: Arena<FactRef>,
    pub assigned_refs: Arena<crate::AssignedFactRef>,
    pub contexts: contexts::FactContexts,
    pub symbol_sets: Arena<SymbolFactSet>,
}

/// Read one context's assignment records for a root in their original order.
pub struct RootAssignmentFacts<'plan> {
    plan: &'plan FactPlan,
    root: PlaceRoot,
    assignments: std::slice::Iter<'plan, AssignedFactRef>,
    finished: bool,
}

impl<'plan> Iterator for RootAssignmentFacts<'plan> {
    type Item = &'plan Fact;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        for assignment in self.assignments.by_ref() {
            if assignment.root.group_key() != self.root.group_key() {
                self.finished = true;
                return None;
            }
            if assignment.root == self.root {
                return Some(self.plan.facts.get(assignment.fact));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            0,
            Some(if self.finished {
                0
            } else {
                self.assignments.len()
            }),
        )
    }
}

macro_rules! fact_plan_root_impls {
    ($($root:ident),* $(,)?) => {
        impl Clone for FactPlan {
            fn clone(&self) -> Self {
                Self {
                    $($root: self.$root.clone(),)*
                    contexts: self.contexts.clone(),
                }
            }

            fn clone_from(&mut self, source: &Self) {
                $(self.$root.clone_from(&source.$root);)*
                self.contexts.clone_from(&source.contexts);
            }
        }

        impl FactPlan {
            pub fn with_capacity(fact_capacity: usize, context_capacity: usize) -> Self {
                Self {
                    $($root: Arena::with_capacity(fact_capacity).into(),)*
                    contexts: contexts::FactContexts::with_capacity(context_capacity),
                }
            }

            pub fn shares_declaration_baseline(&self, baseline: &Self) -> bool {
                self.contexts.shares_sealed_index(&baseline.contexts)
                    && self.facts.shares_first_page(&baseline.facts)
            }

            pub fn share_pages(&mut self) {
                let Self { $($root,)* contexts } = self;
                $($root.share_pages();)*
                contexts.share_pages();
            }

            pub fn begin_page(&mut self, generation: u32) {
                let Self { $($root,)* contexts } = self;
                $($root.begin_page(generation);)*
                contexts.begin_page(generation);
            }

            pub fn take_additional_pages(&mut self) -> Self {
                let Self { $($root,)* contexts } = self;
                Self {
                    $($root: $root.take_additional_pages(),)*
                    contexts: contexts.take_additional_pages(),
                }
            }

            pub fn adopt_additional_pages(&mut self, other: Self) {
                let Self { $($root,)* contexts } = other;
                $(self.$root.adopt_additional_pages($root);)*
                self.contexts.adopt_additional_pages(contexts);
            }
        }
    };
}

fact_plan_root_roster!(fact_plan_root_impls);

static SHARED_DECLARATION_FORKS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
static COPIED_DECLARATION_FORKS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn declaration_page_fork_totals() -> (u64, u64) {
    (
        SHARED_DECLARATION_FORKS.load(std::sync::atomic::Ordering::Relaxed),
        COPIED_DECLARATION_FORKS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

pub fn note_declaration_page_fork(worker: &FactPlan, baseline: &FactPlan) {
    if baseline.facts.is_empty() {
        return;
    }
    let counter = if worker.shares_declaration_baseline(baseline) {
        &SHARED_DECLARATION_FORKS
    } else {
        &COPIED_DECLARATION_FORKS
    };
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

impl FactPlan {
    pub fn admitted_qualification_issuers(
        &self,
    ) -> Result<Vec<&language_semantics::BoundaryQualificationIssuer>, &'static str> {
        let mut issuers = Vec::new();
        for (_, fact) in self.facts.iter().filter(|(_, fact)| {
            fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
        }) {
            if !self.qualification_issuers.is_valid(fact.evidence.issuer) {
                return Err("admitted qualification evidence has no retained issuer");
            }
            let record = self.qualification_issuers.get(fact.evidence.issuer);
            if record.source_symbol != fact.evidence.source_symbol
                || record.requirement_symbol != fact.evidence.requirement_symbol
            {
                return Err("admitted qualification evidence does not name its retained issuer");
            }
            issuers.push(&record.issuer);
        }
        Ok(issuers)
    }

    pub fn append_fact(&mut self, fact: Fact) -> FactHandle {
        self.facts.append(fact)
    }

    pub fn append_statement_transfer(
        &mut self,
        place: FactPlace,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        payload: FactPayload,
    ) -> FactHandle {
        self.append_fact(Fact {
            place,
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload,
        })
    }

    pub fn append_transfer_at_point(
        &mut self,
        place: FactPlace,
        point: ProgramPoint,
        payload: FactPayload,
    ) -> FactHandle {
        self.append_fact(Fact {
            place,
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload,
        })
    }

    pub fn append_qualification_correspondence(
        &mut self,
        correspondence: QualificationCorrespondence,
    ) -> QualificationCorrespondenceHandle {
        if let Some((handle, _)) = self
            .qualification_correspondences
            .iter()
            .find(|(_, retained)| **retained == correspondence)
        {
            return handle;
        }
        self.qualification_correspondences.append(correspondence)
    }

    pub fn append_instantiated_expression(
        &mut self,
        label: String,
    ) -> Handle<InstantiatedExpression> {
        self.instantiated_expressions
            .append(InstantiatedExpression { label })
    }

    pub fn boolean_fact_label(&self, program: &TypedTrees, fact: &Fact) -> Option<String> {
        match fact.payload {
            FactPayload::BooleanExpression(expression) => {
                Some(program.expression_table.display_name(expression))
            }
            FactPayload::ContractBooleanExpression {
                expression,
                instantiated,
                ..
            } => {
                if instantiated.is_valid() {
                    Some(
                        self.instantiated_expressions
                            .get(instantiated)
                            .label
                            .clone(),
                    )
                } else {
                    Some(program.expression_table.display_name(expression))
                }
            }
            _ => None,
        }
    }

    pub fn proposition_fact_label(&self, program: &TypedTrees, fact: &Fact) -> Option<String> {
        let (proof_fact, instantiated) = match fact.payload {
            FactPayload::PropositionApplication { fact, .. } => (fact, Handle::invalid()),
            FactPayload::ContractPropositionApplication {
                fact, instantiated, ..
            } => (fact, instantiated),
            _ => return None,
        };
        if instantiated.is_valid() {
            return Some(
                self.instantiated_expressions
                    .get(instantiated)
                    .label
                    .clone(),
            );
        }
        let application = program.proof_facts.get(proof_fact).as_proposition()?;
        program
            .normalize_proposition_application(application, None)
            .map(|formula| formula.identity_label())
    }

    pub fn append_place(&mut self, place: Place) -> PlaceHandle {
        self.places.append(place)
    }

    pub fn append_symbol_place(&mut self, symbol: SymbolHandle) -> PlaceHandle {
        self.append_place(Place {
            root: PlaceRoot::Symbol(symbol),
            segments: HandleSpan::empty(),
        })
    }

    pub fn append_expression_place(&mut self, expression: ExpressionHandle) -> PlaceHandle {
        self.append_place(Place {
            root: PlaceRoot::Expression(expression),
            segments: HandleSpan::empty(),
        })
    }

    pub fn append_place_from_expression(
        &mut self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> PlaceHandle {
        if !expression.is_valid() {
            return self.append_place(Place {
                root: PlaceRoot::Unknown,
                segments: HandleSpan::empty(),
            });
        }

        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => {
                self.append_place_from_expression(program, inner.target)
            }
            ExpressionNode::Name(path) => {
                let root = if path.head_symbol.is_valid() {
                    path.head_symbol
                } else {
                    path.symbol
                };
                if !root.is_valid() {
                    return self.append_expression_place(expression);
                }
                let place = self.append_symbol_place(root);
                let member_symbols = program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols);
                let member_names = program.expression_table.name_path_members(path.members);
                for (offset, member_name) in member_names.iter().skip(1).enumerate() {
                    let member_symbol = member_symbols
                        .get(offset + 1)
                        .copied()
                        .filter(|symbol| symbol.is_valid())
                        .or_else(|| {
                            resolve_place_member_symbol(program, self, place, member_name.as_str())
                        })
                        .unwrap_or_else(SymbolHandle::invalid);
                    self.push_field_place_segment(program, place, member_symbol);
                }
                place
            }
            ExpressionNode::Member(member) => {
                let place = self.append_place_from_expression(program, member.receiver);
                let symbol = {
                    let symbol = effective_member_symbol(program, member.receiver, member);
                    if symbol.is_valid() {
                        symbol
                    } else {
                        resolve_place_member_symbol(program, self, place, member.member.as_str())
                            .unwrap_or_else(SymbolHandle::invalid)
                    }
                };
                self.push_field_place_segment(program, place, symbol);
                place
            }
            ExpressionNode::Indexed(indexed) => {
                let place = self.append_place_from_expression(program, indexed.collection);
                let segment = program
                    .expression_table
                    .constant_integer_value(indexed.index)
                    .and_then(|value| usize::try_from(value).ok())
                    .map(|index| PlaceSegment::FixedIndex { index })
                    .unwrap_or(PlaceSegment::Index {
                        expression: indexed.index,
                    });
                self.push_place_segment(place, segment);
                place
            }
            _ => self.append_expression_place(expression),
        }
    }

    pub fn push_place_segment(&mut self, place: PlaceHandle, segment: PlaceSegment) {
        let segment = self.place_segments.append(segment);
        self.places.get_mut(place).segments.push_contiguous(segment);
    }

    fn push_field_place_segment(
        &mut self,
        program: &typed_trees::TypedTrees,
        place: PlaceHandle,
        symbol: SymbolHandle,
    ) {
        if let Some(variant) = crate::payload_variant_for_field(program, symbol) {
            self.push_place_segment(place, PlaceSegment::Case { variant });
        }
        self.push_place_segment(place, PlaceSegment::Field { symbol });
    }

    pub fn append_fact_context(&mut self, fact: Fact) -> FactHandle {
        let point = fact.point;
        let fact = self.append_fact(fact);
        let mut refs = HandleSpan::empty();
        self.append_ref(&mut refs, fact);
        self.append_context(point, refs);
        fact
    }

    pub fn append_ref(&mut self, refs: &mut HandleSpan<FactRef>, fact: FactHandle) {
        self.refs.append_to_span(refs, FactRef { fact });
    }

    pub fn append_context(
        &mut self,
        point: ProgramPoint,
        facts: HandleSpan<FactRef>,
    ) -> FactContextHandle {
        let mut assignments = self
            .refs
            .span_or_empty(facts)
            .iter()
            .filter_map(|reference| {
                if !self.facts.is_valid(reference.fact) {
                    return None;
                }
                let fact = self.facts.get(reference.fact);
                if !matches!(
                    fact.payload,
                    FactPayload::AssignedValue { .. }
                        | FactPayload::AssignedScalarValue { .. }
                        | FactPayload::AssignedIntegerBounds { .. }
                ) {
                    return None;
                }
                Some(crate::AssignedFactRef {
                    root: self.places.get(fact.place.as_place()?).root,
                    fact: reference.fact,
                })
            })
            .collect::<Vec<_>>();
        assignments.sort_by_key(|assignment| assignment.root.group_key());
        let assignments = self.assigned_refs.insert_many(assignments);
        self.contexts.append(FactContext {
            point,
            facts,
            assignments,
        })
    }

    pub fn assignments_with_root(
        &self,
        context: &FactContext,
        root: PlaceRoot,
    ) -> RootAssignmentFacts<'_> {
        let assignments = self.assigned_refs.span_or_empty(context.assignments);
        let key = root.group_key();
        let start = assignments.partition_point(|assignment| assignment.root.group_key() < key);
        RootAssignmentFacts {
            plan: self,
            root,
            assignments: assignments[start..].iter(),
            finished: false,
        }
    }

    pub fn append_symbol_set(
        &mut self,
        symbol: SymbolHandle,
        facts: HandleSpan<FactRef>,
    ) -> Handle<SymbolFactSet> {
        self.symbol_sets.append(SymbolFactSet { symbol, facts })
    }

    pub fn facts_for_symbol(&self, symbol: SymbolHandle) -> impl Iterator<Item = &Fact> {
        self.symbol_sets
            .iter()
            .filter(move |(_, set)| set.symbol == symbol)
            .flat_map(move |(_, set)| {
                self.refs
                    .span_or_empty(set.facts)
                    .iter()
                    .map(move |reference| self.facts.get(reference.fact))
            })
    }

    pub fn context_view(&self, context: &FactContext) -> FactContextView<'_> {
        FactContextView {
            plan: self,
            point: context.point,
            facts: context.facts,
        }
    }

    pub fn unique_context_facts(
        &self,
        contexts: impl IntoIterator<Item = FactContextHandle>,
    ) -> Vec<FactHandle> {
        arena::unique_handles_in_order(self.context_fact_handles(contexts))
    }

    pub fn context_fact_handles(
        &self,
        contexts: impl IntoIterator<Item = FactContextHandle>,
    ) -> impl Iterator<Item = FactHandle> {
        contexts.into_iter().flat_map(move |context| {
            self.refs
                .span_or_empty(self.contexts.get(context).facts)
                .iter()
                .map(|reference| reference.fact)
        })
    }

    pub fn contexts_at_point(
        &self,
        point: ProgramPoint,
    ) -> impl Iterator<Item = FactContextView<'_>> {
        self.context_handles_at_point(point)
            .map(move |handle| self.context_view(self.contexts.get(handle)))
    }

    pub fn context_handles_at_point(
        &self,
        point: ProgramPoint,
    ) -> impl Iterator<Item = FactContextHandle> + '_ {
        self.contexts.handles_at_point(point)
    }

    pub fn context_group_at_point(&self, point: ProgramPoint) -> contexts::FactContextGroup {
        self.contexts.group_at_point(point)
    }

    pub fn context_handles_in_group(
        &self,
        group: contexts::FactContextGroup,
    ) -> impl Iterator<Item = FactContextHandle> + '_ {
        self.contexts.handles_in_group(group)
    }

    pub fn domain_implies(
        &self,
        source_domain_symbol: SymbolHandle,
        target_domain_symbol: SymbolHandle,
    ) -> bool {
        let mut visited = Vec::new();
        self.domain_implies_inner(source_domain_symbol, target_domain_symbol, &mut visited)
    }

    fn domain_implies_inner(
        &self,
        source_domain_symbol: SymbolHandle,
        target_domain_symbol: SymbolHandle,
        visited: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !source_domain_symbol.is_valid() || !target_domain_symbol.is_valid() {
            return false;
        }
        if source_domain_symbol == target_domain_symbol {
            return true;
        }
        if visited.contains(&source_domain_symbol) {
            return false;
        }
        visited.push(source_domain_symbol);

        self.domain_memberships_for_symbol(source_domain_symbol)
            .any(|fact| {
                self.domain_implies_inner(fact.domain_symbol, target_domain_symbol, visited)
            })
    }

    pub fn boolean_facts_for_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> impl Iterator<Item = BooleanFact> + '_ {
        self.facts_for_symbol(symbol).filter_map(|fact| {
            fact.payload
                .boolean_expression()
                .map(|expression| BooleanFact { expression })
        })
    }

    pub fn domain_memberships_for_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> impl Iterator<Item = DomainMembershipFact> + '_ {
        self.facts_for_symbol(symbol)
            .filter_map(|fact| as_domain_membership(fact.payload))
    }

    pub fn fact_place_equals(&self, fact_place: FactPlace, place: PlaceHandle) -> bool {
        self.fact_places_equal(fact_place, FactPlace::Place(place))
    }

    pub fn fact_places_equal(&self, left: FactPlace, right: FactPlace) -> bool {
        match (left, right) {
            (FactPlace::Place(left), FactPlace::Place(right)) => self.places_equal(left, right),
            _ => left == right,
        }
    }

    pub fn fact_place_bucket_hash(&self, place: FactPlace) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        std::mem::discriminant(&place).hash(&mut hash);
        match place {
            FactPlace::Unknown => {}
            FactPlace::Place(place) => {
                let place = self.places.get(place);
                places::place_bucket_hash(
                    place.root,
                    self.place_segments.span_or_empty(place.segments),
                )
                .hash(&mut hash);
            }
            FactPlace::Symbol(symbol) => symbol.hash(&mut hash),
            FactPlace::Expression(expression) => expression.hash(&mut hash),
            FactPlace::TypeReference(reference) => reference.hash(&mut hash),
        }
        hash.finish()
    }

    /// Find the first occurrence of each transferred qualification in one batch.
    /// Place hashes only narrow the search; evidence and payload retain their
    /// full duplicate comparison. Distinct arena spans can name the same path.
    /// The existing prefix is left untouched, including any duplicates in it.
    /// Only supported membership and float payloads suppress a new occurrence.
    pub fn first_owned_qualification_positions(
        &self,
        facts: &[Fact],
        existing: usize,
    ) -> Vec<usize> {
        let buckets = places::PlaceBucketIndex::new(
            facts
                .iter()
                .map(|fact| self.fact_place_bucket_hash(fact.place)),
        );
        let mut first: Vec<_> = (0..facts.len()).collect();
        for position in existing..facts.len() {
            let fact = &facts[position];
            let previous = buckets
                .candidates(self.fact_place_bucket_hash(fact.place))
                .take_while(|prior| *prior < position)
                .filter(|prior| *prior < existing || first[*prior] == *prior)
                .find(|prior| {
                    let prior = &facts[*prior];
                    prior.evidence == fact.evidence
                        && matches!((prior.place, fact.place),
                    (FactPlace::Place(prior_place), FactPlace::Place(place))
                    if self.places_equal(prior_place, place))
                        && match (prior.payload, fact.payload) {
                            (
                                FactPayload::FloatPredicate { predicate: left },
                                FactPayload::FloatPredicate { predicate: right },
                            ) => left == right,
                            (prior, transferred) => prior
                                .domain_membership()
                                .zip(transferred.domain_membership())
                                .is_some_and(
                                    |(
                                        (_, _, left_symbol, left_identity),
                                        (_, _, right_symbol, right_identity),
                                    )| {
                                        left_symbol == right_symbol
                                            && left_identity == right_identity
                                    },
                                ),
                        }
                });
            if let Some(previous) = previous {
                first[position] = previous;
            }
        }
        first
    }

    pub fn group_fact_refs_by_place(&self, references: HandleSpan<FactRef>) -> Vec<Vec<FactRef>> {
        let references = self.refs.span_or_empty(references);
        let subjects: Vec<_> = references
            .iter()
            .map(|reference| self.facts.get(reference.fact).place)
            .collect();
        let buckets = places::PlaceBucketIndex::new(
            subjects
                .iter()
                .map(|place| self.fact_place_bucket_hash(*place)),
        );
        let first = buckets.group_equal_positions(|left, right| {
            self.fact_places_equal(subjects[left], subjects[right])
        });
        let mut groups: Vec<Vec<FactRef>> = (0..references.len()).map(|_| Vec::new()).collect();
        for (position, reference) in references.iter().enumerate() {
            groups[first[position]].push(*reference);
        }
        groups
            .into_iter()
            .filter(|group| !group.is_empty())
            .collect()
    }

    pub fn places_equal(&self, left: PlaceHandle, right: PlaceHandle) -> bool {
        let left_place = self.places.get(left);
        let right_place = self.places.get(right);
        left_place.root == right_place.root
            && self.place_segments.span_or_empty(left_place.segments)
                == self.place_segments.span_or_empty(right_place.segments)
    }

    pub fn places_match(
        &self,
        program: &TypedTrees,
        left: PlaceHandle,
        right: PlaceHandle,
    ) -> bool {
        if self.places_equal(left, right) {
            return true;
        }
        if matches!(
            self.places.get(left).root,
            PlaceRoot::InvocationReferent { .. } | PlaceRoot::InvocationOutput(_)
        ) || matches!(
            self.places.get(right).root,
            PlaceRoot::InvocationReferent { .. } | PlaceRoot::InvocationOutput(_)
        ) {
            return false;
        }
        thread_local! {
            static LABELS: std::cell::RefCell<(String, String)> =
                const { std::cell::RefCell::new((String::new(), String::new())) };
        }
        let write_label = |place: PlaceHandle, label: &mut String| {
            let place = self.places.get(place);
            label.clear();
            crate::places::resolution::write_canonical_place_label(
                program,
                place.root,
                self.place_segments.span_or_empty(place.segments),
                label,
            );
        };
        LABELS.with(|labels| match labels.try_borrow_mut() {
            Ok(mut labels) => {
                let (left_label, right_label) = &mut *labels;
                write_label(left, left_label);
                write_label(right, right_label);
                left_label == right_label
            }
            Err(_) => self.place_label(program, left) == self.place_label(program, right),
        })
    }

    pub fn place_label(&self, program: &TypedTrees, place: PlaceHandle) -> String {
        let place = self.places.get(place);
        crate::canonical_place_label_from_parts(
            program,
            place.root,
            self.place_segments.span_or_empty(place.segments),
        )
    }
}

pub(crate) fn as_domain_membership(payload: FactPayload) -> Option<DomainMembershipFact> {
    payload
        .domain_membership()
        .map(
            |(value, domain, domain_symbol, semantic_domain)| DomainMembershipFact {
                value,
                domain,
                domain_symbol,
                semantic_domain,
            },
        )
}

pub use contexts::view::FactContextView;
pub use contexts::{
    AssignedFactRef, BooleanFact, DomainMembershipFact, FactContext, FactContextGroup,
    FactContexts, FactRef, ProgramPoint, SymbolFactSet, TypeConstraintFact, view,
};
pub use evidence::{
    BoundaryQualificationIssuerRecord, ContractFactKind, ContractMemberProjection,
    DataDefinitionFactDependency, DataDefinitionFactRecord, DomainDefinitionFactDependency,
    DomainDefinitionFactRecord, Fact, FactOrigin, FactPayload, FloatPredicate,
    InstantiatedExpression, InstructionEffectOperation, IntegerRange, ProofObligationKind,
    QualificationCorrespondence, QualificationEvidence, QualificationPayloadIdentity,
    QualificationProvenance, ScalarValue,
};
pub use places::resolution::{
    canonical_place_label_from_parts, effective_member_symbol, payload_variant_for_field,
    resolve_place_member_symbol,
};
pub use places::write_frame::{NormalizedWriteFrame, WriteFrameCompleteness};
pub use places::{
    FactPlace, InvocationOutputIdentity, Place, PlaceRoot, PlaceSegment,
    compose_captured_view_segments, project_captured_view_segments, resolution, write_frame,
};
pub use predicates::{
    BooleanPredicate, BooleanPredicateBinding, BooleanPredicateOperand, BooleanPredicateRole,
    CheckedBooleanExpression, CheckedByteSequenceOperand, CheckedEntryIndexBound,
    CheckedEntryIndexSelector, CheckedIeeeFloatComparisonKind, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedIntegerMathExpression, CheckedIntegerMathTerm,
    CheckedRuntimeIndex, CheckedScalarExpression, CheckedScalarReadProjection,
    CheckedScalarSelector, CheckedStorageRoot, CheckedStructuralCaseRoot,
    CheckedStructuralCaseSubject, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment, u64_integer_literal,
};

pub mod fact_plan {
    pub use crate::*;
}
