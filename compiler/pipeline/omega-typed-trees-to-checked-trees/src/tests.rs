    use super::{
        build_borrow_facts, build_domain_facts, build_flow_facts, build_proof_facts,
        build_semantic_facts, call_mutated_places,
        instantiate_call_contract_place, lower_typed_trees, StateMutationSummaryCache,
    };
    use crate::checks::context_proves_requirement_place_domain;
    use omega_checked_trees::expression::{CallExpression, Expression, NamePath};
    use omega_checked_trees::machine::{Machine, TraitConformance};
    use omega_checked_trees::name::ProgramName;
    use omega_checked_trees::signature::{
        SignatureContract, SignatureContractKind, StateParameter, StateSignature,
    };
    use omega_checked_trees::state::State;
    use omega_checked_trees::statement::{StatementNode, TableCall};
    use omega_checked_trees::trait_definition::TraitDefinition;
    use omega_checked_trees::types::TypeReferenceNode;
    use omega_checked_trees::{BorrowAccessKind, ContractProofFactKind, ContractProofFactOwner};
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;
    use omega_facts::{FactPayload, FactPlace};
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;
    use std::sync::Arc;

    #[test]
    fn carries_machine_contract_facts_into_checked_proof_facts() {
        let machine_symbol = SymbolHandle::from_arena_index(5);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_machine(machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let contract_fact = facts
            .contract_facts
            .iter()
            .next()
            .map(|(_, fact)| fact)
            .expect("checked proof facts should include the machine contract");

        assert_eq!(facts.contract_facts.len(), 1);
        assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
        assert_eq!(contract_fact.fact, fact);
        assert_eq!(
            contract_fact.owner,
            ContractProofFactOwner::Machine { machine_symbol }
        );
    }

    #[test]
    fn centralizes_contract_facts_in_semantic_fact_plan() {
        let machine_symbol = SymbolHandle::from_arena_index(5);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_machine(machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let proof = build_proof_facts(&program, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&program, &proof);

        assert_eq!(semantic.facts.len(), 1);
        assert_eq!(semantic.contexts.len(), 1);
        assert_eq!(semantic.symbol_sets.len(), 0);

        let semantic_fact = semantic
            .facts
            .iter()
            .next()
            .map(|(_, fact)| fact)
            .expect("semantic contract fact");
        let omega_facts::FactPlace::Place(place) = semantic_fact.place else {
            panic!("expected canonical contract fact place");
        };
        assert_eq!(
            semantic.places.get(place).root,
            omega_facts::PlaceRoot::Expression(expression)
        );
        assert_eq!(
            semantic_fact.payload,
            omega_facts::FactPayload::ContractBooleanExpression {
                kind: omega_facts::ContractFactKind::Requires,
                fact,
                expression,
            }
        );
        let context = semantic
            .contexts_at_point(omega_facts::ProgramPoint::Machine { machine_symbol })
            .next()
            .expect("machine contract context");
        assert_eq!(context.boolean_facts().count(), 1);
    }

    #[test]
    fn builds_shared_flow_facts_for_state_and_call_sites() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(40);
        let caller_state_symbol = SymbolHandle::from_arena_index(41);
        let callee_machine_symbol = SymbolHandle::from_arena_index(42);
        let callee_state_symbol = SymbolHandle::from_arena_index(43);

        let mut program = omega_typed_trees::TypedTrees::default();
        let contract_expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let contract_fact =
            program
                .proof_facts
                .append(omega_typed_trees::domain::ProofFact::Expression(
                    contract_expression,
                ));

        let callee_state = State {
            symbol: callee_state_symbol,
            name: ProgramName::generated("run"),
            parameters: Default::default(),
            return_type: Default::default(),
            statement_nodes: Default::default(),
        };
        let mut callee_machine = Machine {
            symbol: callee_machine_symbol,
            name: ProgramName::generated("Worker::run"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_state(&mut callee_machine, callee_state);
        program.push_machine_contract(
            &mut callee_machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(contract_fact, 1),
                token_count: 1,
            },
        );
        program.push_machine(callee_machine);

        let call_arguments = HandleSpan::empty();
        let call_statement_receiver = HandleSpan::empty();
        let call_statement = StatementNode::Call(TableCall {
            receiver: call_statement_receiver,
            receiver_symbol: caller_machine_symbol,
            target: ProgramName::generated("run"),
            target_symbol: callee_state_symbol,
            arguments: call_arguments,
        });
        let caller_statement = program.statement_table.insert(call_statement);
        let caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: Default::default(),
            statement_nodes: HandleSpan::from_parts(caller_statement, 1),
        };
        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let effects = omega_effects::infer_effects(&program);
        let borrow = build_borrow_facts(&program);
        let proof = build_proof_facts(&program, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&program, &proof);
        let domains = build_domain_facts(&program, &semantic);
        let flow = build_flow_facts(&program, &borrow, &proof, &semantic, &domains, &effects);

        let caller_flow = flow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == caller_machine_symbol
                    && state.state_symbol == caller_state_symbol)
                    .then_some(state)
            })
            .expect("caller flow state");
        assert!(caller_flow.entry_semantic_contexts.is_empty());
        assert_eq!(flow.calls.span_or_empty(caller_flow.calls).len(), 1);

        let call_flow = flow.calls.span_or_empty(caller_flow.calls)[0].clone();
        assert_eq!(call_flow.statement_index, 0);
        assert_eq!(call_flow.call_ordinal, 0);
        assert_eq!(call_flow.target_symbol, callee_state_symbol);
        assert!(call_flow.entry_semantic_contexts.is_empty());
        assert!(!call_flow.requires_contexts.is_empty());
        assert!(call_flow.exit_semantic_contexts.is_empty());
        assert_eq!(
            proof
                .contract_fact_refs
                .span_or_empty(call_flow.requires)
                .len(),
            1
        );
    }

    #[test]
    fn invalidates_proved_domain_membership_after_mutating_call() {
        let source = r#"
            data Player {
                health: i32;
            }

            domain Player::Valid {
                self.health >= 0;
                self.health <= 100;
            }

            data Main {
                player: Player;
            }

            machine Main::mark_valid(&mut self, player: &mut Player)
            ensures
                player in Player::Valid
            {
                player.health = 0;
            }

            machine Main::break_valid(&mut self, player: &mut Player) {
                player.health = 200;
            }

            machine Main::heal(&mut self, player: &mut Player)
            requires
                player in Player::Valid
            {
                player.health = 10;
            }

            machine Main::main(&mut self) {
                self.mark_valid(&mut self.player);
                self.break_valid(&mut self.player);
                self.heal(&mut self.player);
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
        let effects = omega_effects::infer_effects(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&typed, &proof);
        let domains = build_domain_facts(&typed, &semantic);
        let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);
        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_state = typed
            .machine_states(main_machine)
            .iter()
            .find(|state| state.name.as_str() == "main")
            .expect("main state");
        let caller_flow = flow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == main_machine.symbol
                    && state.state_symbol == main_state.symbol)
                    .then_some(state)
            })
            .expect("main flow state");
        let calls = flow.calls.span_or_empty(caller_flow.calls);
        assert_eq!(calls.len(), 3);
        assert_eq!(flow.invalidations.span_or_empty(caller_flow.invalidations).len(), 1);
        assert_eq!(flow.invalidations.span_or_empty(calls[1].invalidations).len(), 1);

        let heal_call = &calls[2];
        let (required_place, required_domain) = flow
            .semantic_context_refs
            .span_or_empty(heal_call.requires_contexts)
            .iter()
            .find_map(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                semantic
                    .context_view(context)
                    .facts()
                    .find_map(|fact| match fact.payload {
                        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                            let FactPlace::Place(place) = fact.place else {
                                return None;
                            };
                            Some((place, domain_symbol))
                        }
                        _ => None,
                    })
            })
            .expect("heal requires domain membership");

        let mark_exit_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[0].exit_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });
        let break_entry_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[1].entry_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });
        let heal_entry_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[2].entry_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });

        let diagnostics =
            lower_typed_trees(typed.clone()).expect_err("requires should fail after mutation");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract for call heal from Main::main")
                && diagnostic
                    .message
                    .contains("invalidated by prior mutation of Main::main.player.health")
        }));

        assert!(mark_exit_proves);
        assert!(break_entry_proves);
        assert!(!heal_entry_proves);
    }

    #[test]
    fn invalidates_imported_domain_requires_after_mutating_call() {
        let source = r#"
            data Player {
                health: i32;
            }

            domain Player::Valid {
                self.health >= 0;
                self.health <= 100;
            }

            domain Player::Alive {
                self in Player::Valid;
                self.health > 0;
            }

            data Main {
                player: Player;
            }

            machine Main::mark_valid(&mut self, player: &mut Player)
            ensures
                player in Player::Valid
            {
                player.health = 0;
            }

            machine Main::break_valid(&mut self, player: &mut Player) {
                player.health = 200;
            }

            machine Main::heal(&mut self, player: &mut Player)
            requires
                player in Player::Valid
            ensures
                player in Player::Alive
            {
                player.health = 10;
            }

            machine Main::main(&mut self) {
                self.mark_valid(&mut self.player);
                self.break_valid(&mut self.player);
                self.heal(&mut self.player);
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
        let effects = omega_effects::infer_effects(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&typed, &proof);
        let domains = build_domain_facts(&typed, &semantic);
        let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);
        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_state = typed
            .machine_states(main_machine)
            .iter()
            .find(|state| state.name.as_str() == "main")
            .expect("main state");
        let caller_flow = flow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == main_machine.symbol
                    && state.state_symbol == main_state.symbol)
                    .then_some(state)
            })
            .expect("main flow state");
        let calls = flow.calls.span_or_empty(caller_flow.calls);
        assert_eq!(flow.invalidations.span_or_empty(caller_flow.invalidations).len(), 1);
        assert_eq!(flow.invalidations.span_or_empty(calls[1].invalidations).len(), 1);
        let heal_call = &calls[2];
        let (required_place, required_domain) = flow
            .semantic_context_refs
            .span_or_empty(heal_call.requires_contexts)
            .iter()
            .find_map(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                semantic
                    .context_view(context)
                    .facts()
                    .find_map(|fact| match fact.payload {
                        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                            let FactPlace::Place(place) = fact.place else {
                                return None;
                            };
                            Some((place, domain_symbol))
                        }
                        _ => None,
                    })
            })
            .expect("heal requires domain membership");
        let heal_entry_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[2].entry_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });
        assert!(!heal_entry_proves);

        let diagnostics =
            lower_typed_trees(typed).expect_err("requires should fail after mutation");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract for call heal from Main::main")
                && diagnostic
                    .message
                    .contains("invalidated by prior mutation of Main::main.player.health")
        }));
    }

    #[test]
    fn preserves_imported_domain_requires_across_disjoint_mutating_call() {
        let source = r#"
            data Player {
                health: i32;
                mana: i32;
                stamina: i32;
            }

            domain Player::Valid {
                self.health >= 0;
                self.health <= 100;
            }

            domain Player::Ready {
                self in Player::Valid;
                self.mana >= 0;
            }

            data Main {
                player: Player;
            }

            machine Main::mark_ready(&mut self, player: &mut Player)
            ensures
                player in Player::Ready
            {
                player.health = 40;
                player.mana = 5;
            }

            machine Main::spend_stamina(&mut self, player: &mut Player) {
                player.stamina = 0;
            }

            machine Main::heal(&mut self, player: &mut Player)
            requires
                player in Player::Ready
            {
                player.health = 50;
            }

            machine Main::main(&mut self) {
                self.mark_ready(&mut self.player);
                self.spend_stamina(&mut self.player);
                self.heal(&mut self.player);
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
        let effects = omega_effects::infer_effects(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&typed, &proof);
        let domains = build_domain_facts(&typed, &semantic);
        let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);
        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_state = typed
            .machine_states(main_machine)
            .iter()
            .find(|state| state.name.as_str() == "main")
            .expect("main state");
        let caller_flow = flow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == main_machine.symbol
                    && state.state_symbol == main_state.symbol)
                    .then_some(state)
            })
            .expect("main flow state");
        let calls = flow.calls.span_or_empty(caller_flow.calls);
        assert!(flow.invalidations.span_or_empty(calls[1].invalidations).is_empty());
        let heal_call = &calls[2];
        let (required_place, required_domain) = flow
            .semantic_context_refs
            .span_or_empty(heal_call.requires_contexts)
            .iter()
            .find_map(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                semantic
                    .context_view(context)
                    .facts()
                    .find_map(|fact| match fact.payload {
                        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                            let FactPlace::Place(place) = fact.place else {
                                return None;
                            };
                            Some((place, domain_symbol))
                        }
                        _ => None,
                    })
            })
            .expect("heal requires domain membership");
        let heal_entry_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[2].entry_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });

        assert!(heal_entry_proves);
        lower_typed_trees(typed).expect("disjoint mutation should preserve imported domain fact");
    }

    #[test]
    fn materializes_domain_dependency_facts() {
        let source = r#"
            data Player {
                health: i32;
                mana: i32;
            }

            domain Player::Valid {
                self.health >= 0;
                self.health <= 100;
            }

            domain Player::Ready {
                self in Player::Valid;
                self.mana >= 0;
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&typed, &proof);
        let domains = build_domain_facts(&typed, &semantic);

        let ready_symbol = typed
            .domain_definitions()
            .iter()
            .find(|domain| domain.name.as_str() == "Player::Ready")
            .map(|domain| domain.symbol)
            .expect("ready domain");
        let ready_fact = domains
            .dependencies
            .iter()
            .find_map(|(_, fact)| (fact.domain_symbol == ready_symbol).then_some(fact))
            .expect("ready dependency fact");

        let paths = domains.dependency_paths.span_or_empty(ready_fact.dependencies);
        assert_eq!(paths.len(), 2);

        let mut field_symbols = paths
            .iter()
            .filter_map(|path| {
                let segments = domains.segments.span_or_empty(path.segments);
                match segments {
                    [omega_facts::PlaceSegment::Field { symbol }] => Some(*symbol),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        field_symbols.sort_by_key(|symbol| symbol.arena_index());

        let player = typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Player")
            .expect("player data");
        let mut expected = typed
            .data_members(player)
            .iter()
            .filter_map(|member| match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == "health" || field.name.as_str() == "mana" =>
                {
                    Some(field.symbol)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        expected.sort_by_key(|symbol| symbol.arena_index());

        assert_eq!(field_symbols, expected);
    }

    #[test]
    fn instantiates_call_contract_places_onto_caller_arguments() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(1);
        let caller_state_symbol = SymbolHandle::from_arena_index(2);
        let callee_machine_symbol = SymbolHandle::from_arena_index(3);
        let callee_state_symbol = SymbolHandle::from_arena_index(4);
        let caller_argument_symbol = SymbolHandle::from_arena_index(5);
        let callee_parameter_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let caller_argument_expression =
            program
                .expression_table
                .insert(omega_typed_trees::expression::ExpressionNode::Name(
                    omega_typed_trees::expression::TableNamePath {
                        members: HandleSpan::empty(),
                        member_symbols: HandleSpan::empty(),
                        head_symbol: caller_argument_symbol,
                        symbol: caller_argument_symbol,
                    },
                ));
        let callee_parameter_expression =
            program
                .expression_table
                .insert(omega_typed_trees::expression::ExpressionNode::Name(
                    omega_typed_trees::expression::TableNamePath {
                        members: HandleSpan::empty(),
                        member_symbols: HandleSpan::empty(),
                        head_symbol: callee_parameter_symbol,
                        symbol: callee_parameter_symbol,
                    },
                ));
        let callee_fact =
            program
                .proof_facts
                .append(omega_typed_trees::domain::ProofFact::Expression(
                    callee_parameter_expression,
                ));

        let mut caller_arguments = HandleSpan::empty();
        program
            .statement_table
            .push_expression_handle(&mut caller_arguments, caller_argument_expression);
        let caller_statement = program
            .statement_table
            .insert(StatementNode::Call(TableCall {
                receiver_symbol: SymbolHandle::invalid(),
                target_symbol: callee_state_symbol,
                receiver: HandleSpan::empty(),
                target: ProgramName::generated("run"),
                arguments: caller_arguments,
            }));

        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: HandleSpan::empty(),
            return_type: Default::default(),
            statement_nodes: HandleSpan::from_parts(caller_statement, 1),
        };
        program.push_state_parameter(
            &mut caller_state,
            StateParameter {
                symbol: caller_argument_symbol,
                name: ProgramName::generated("value"),
                type_reference: Default::default(),
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Caller"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let mut callee_state = State {
            symbol: callee_state_symbol,
            name: ProgramName::generated("run"),
            parameters: HandleSpan::empty(),
            return_type: Default::default(),
            statement_nodes: HandleSpan::empty(),
        };
        program.push_state_parameter(
            &mut callee_state,
            StateParameter {
                symbol: callee_parameter_symbol,
                name: ProgramName::generated("amount"),
                type_reference: Default::default(),
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );

        let mut callee_machine = Machine {
            symbol: callee_machine_symbol,
            name: ProgramName::generated("Worker"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_state(&mut callee_machine, callee_state);
        program.push_machine(callee_machine);

        let call = omega_checked_trees::ContractCallFact {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index: 0,
            call_ordinal: 0,
            target_machine_symbol: callee_machine_symbol,
            target_state_symbol: callee_state_symbol,
            requires: HandleSpan::empty(),
            ensures: HandleSpan::empty(),
        };
        let contract = omega_checked_trees::ContractProofFact {
            kind: ContractProofFactKind::Requires,
            owner: ContractProofFactOwner::MachineState {
                machine_symbol: callee_machine_symbol,
                state_symbol: callee_state_symbol,
            },
            fact: callee_fact,
        };

        let mut semantic = omega_facts::FactPlan::default();
        let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
        let omega_facts::FactPlace::Place(place_handle) = place else {
            panic!("expected instantiated call place");
        };

        assert_eq!(
            semantic.places.get(place_handle).root,
            omega_facts::PlaceRoot::Symbol(caller_argument_symbol)
        );
    }

    #[test]
    fn carries_trait_signature_contract_facts_into_checked_proof_facts() {
        let trait_symbol = SymbolHandle::from_arena_index(5);
        let signature_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));

        let mut trait_definition = TraitDefinition {
            symbol: trait_symbol,
            is_boundary: true,
            name: ProgramName::generated("Console"),
            requires: Default::default(),
            machines: Default::default(),
        };
        let mut signature = StateSignature {
            symbol: signature_symbol,
            name: ProgramName::generated("write_line"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            effects: Default::default(),
            contracts: Default::default(),
        };
        program.push_state_signature_contract(
            &mut signature,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_trait_machine_signature(&mut trait_definition, signature);
        program.push_trait_definition(trait_definition);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let contract_fact = facts
            .contract_facts
            .iter()
            .next()
            .map(|(_, fact)| fact)
            .expect("checked proof facts should include the trait signature contract");

        assert_eq!(facts.contract_facts.len(), 1);
        assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
        assert_eq!(contract_fact.fact, fact);
        assert_eq!(
            contract_fact.owner,
            ContractProofFactOwner::StateSignature {
                owner_symbol: trait_symbol,
                state_symbol: signature_symbol,
            }
        );
    }

    #[test]
    fn indexes_call_contract_facts_by_target_machine() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(5);
        let caller_state_symbol = SymbolHandle::from_arena_index(6);
        let target_machine_symbol = SymbolHandle::from_arena_index(7);
        let target_state_symbol = SymbolHandle::from_arena_index(8);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));

        let mut target_machine = Machine {
            symbol: target_machine_symbol,
            name: ProgramName::generated("Target"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut target_machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_machine_state(
            &mut target_machine,
            State {
                symbol: target_state_symbol,
                name: ProgramName::generated("run"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(target_machine);

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Caller"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        let mut receiver = HandleSpan::empty();
        program
            .statement_table
            .push_name_path_member(&mut receiver, ProgramName::generated("target"));
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: target_machine_symbol,
                target_symbol: target_state_symbol,
                receiver,
                target: ProgramName::generated("run"),
                arguments: Default::default(),
            }),
        );
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let contract_call = facts
            .contract_calls
            .iter()
            .next()
            .map(|(_, call)| call)
            .expect("checked proof facts should index the call contract");
        let requires = facts
            .contract_fact_refs
            .span_or_empty(contract_call.requires);

        assert_eq!(facts.contract_calls.len(), 1);
        assert_eq!(contract_call.caller_machine_symbol, caller_machine_symbol);
        assert_eq!(contract_call.caller_state_symbol, caller_state_symbol);
        assert_eq!(contract_call.statement_index, 0);
        assert_eq!(contract_call.call_ordinal, 0);
        assert_eq!(contract_call.target_machine_symbol, target_machine_symbol);
        assert_eq!(contract_call.target_state_symbol, target_state_symbol);
        assert_eq!(requires.len(), 1);
        assert_eq!(facts.contract_facts.get(requires[0].fact).fact, fact);
    }

    #[test]
    fn indexes_inherited_trait_contracts_by_concrete_call_target() {
        let trait_symbol = SymbolHandle::from_arena_index(5);
        let signature_symbol = SymbolHandle::from_arena_index(6);
        let target_machine_symbol = SymbolHandle::from_arena_index(7);
        let target_state_symbol = SymbolHandle::from_arena_index(8);
        let caller_machine_symbol = SymbolHandle::from_arena_index(9);
        let caller_state_symbol = SymbolHandle::from_arena_index(10);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));

        let mut trait_definition = TraitDefinition {
            symbol: trait_symbol,
            is_boundary: true,
            name: ProgramName::generated("Drawable"),
            requires: Default::default(),
            machines: Default::default(),
        };
        let mut signature = StateSignature {
            symbol: signature_symbol,
            name: ProgramName::generated("draw"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            effects: Default::default(),
            contracts: Default::default(),
        };
        program.push_state_signature_contract(
            &mut signature,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_trait_machine_signature(&mut trait_definition, signature);
        program.push_trait_definition(trait_definition);

        let mut target_machine = Machine {
            symbol: target_machine_symbol,
            name: ProgramName::generated("Sprite"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_trait_conformance(
            &mut target_machine,
            TraitConformance {
                symbol: trait_symbol,
                name: ProgramName::generated("Drawable"),
            },
        );
        program.push_machine_state(
            &mut target_machine,
            State {
                symbol: target_state_symbol,
                name: ProgramName::generated("draw"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(target_machine);

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        let mut receiver = HandleSpan::empty();
        program
            .statement_table
            .push_name_path_member(&mut receiver, ProgramName::generated("sprite"));
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: target_machine_symbol,
                target_symbol: target_state_symbol,
                receiver,
                target: ProgramName::generated("draw"),
                arguments: Default::default(),
            }),
        );
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let contract_call = facts
            .contract_calls
            .iter()
            .next()
            .map(|(_, call)| call)
            .expect("checked proof facts should index inherited trait contracts");
        let requires = facts
            .contract_fact_refs
            .span_or_empty(contract_call.requires);

        assert_eq!(facts.contract_calls.len(), 1);
        assert_eq!(requires.len(), 1);
        let inherited_fact = facts.contract_facts.get(requires[0].fact);
        assert_eq!(inherited_fact.kind, ContractProofFactKind::Requires);
        assert_eq!(inherited_fact.fact, fact);
        assert_eq!(
            inherited_fact.owner,
            ContractProofFactOwner::MachineState {
                machine_symbol: target_machine_symbol,
                state_symbol: target_state_symbol,
            }
        );
    }

    #[test]
    fn indexes_terminal_state_contract_ensures() {
        let machine_symbol = SymbolHandle::from_arena_index(5);
        let state_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let fact_expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(
                fact_expression,
            ));
        let return_expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Integer(0));

        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );

        let mut state = State {
            symbol: state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Expression(return_expression),
        );
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let exit = facts
            .contract_exits
            .iter()
            .next()
            .map(|(_, exit)| exit)
            .expect("checked proof facts should index the exit contract");
        let ensures = facts.contract_fact_refs.span_or_empty(exit.ensures);

        assert_eq!(facts.contract_exits.len(), 1);
        assert_eq!(exit.machine_symbol, machine_symbol);
        assert_eq!(exit.state_symbol, state_symbol);
        assert_eq!(exit.statement_index, 0);
        assert_eq!(ensures.len(), 1);
        assert_eq!(facts.contract_facts.get(ensures[0].fact).fact, fact);
    }

    #[test]
    fn collects_nested_state_call_ordinals_for_checked_borrow_facts() {
        let entry_symbol = SymbolHandle::from_arena_index(1);
        let outer_symbol = SymbolHandle::from_arena_index(2);
        let inner_symbol = SymbolHandle::from_arena_index(3);
        let item_symbol = SymbolHandle::from_arena_index(4);
        let machine_symbol = SymbolHandle::from_arena_index(5);

        let item_argument = Expression::Mutable(Box::new(Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("item")],
            item_symbol,
            item_symbol,
        ))));

        let nested_call = Expression::Call(Box::new(CallExpression {
            receiver: None,
            target_symbol: inner_symbol,
            target: ProgramName::generated("inner"),
            arguments: Arc::from(vec![item_argument].into_boxed_slice()),
        }));

        let mut program = omega_typed_trees::TypedTrees::default();
        let unit_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let nested_call = program.expression_table.insert_tree(&nested_call);
        let mut outer_arguments = Default::default();
        program
            .statement_table
            .push_expression_handle(&mut outer_arguments, nested_call);
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Game"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut entry_state = State {
            symbol: entry_symbol,
            name: ProgramName::generated("entry"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut entry_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: SymbolHandle::invalid(),
                target_symbol: outer_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("outer"),
                arguments: outer_arguments,
            }),
        );
        program.push_state_parameter(
            &mut entry_state,
            StateParameter {
                symbol: item_symbol,
                name: ProgramName::generated("item"),
                type_reference: unit_type,
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut machine, entry_state);
        program.push_machine_state(
            &mut machine,
            State {
                symbol: outer_symbol,
                name: ProgramName::generated("outer"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine_state(
            &mut machine,
            State {
                symbol: inner_symbol,
                name: ProgramName::generated("inner"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(machine);

        let facts = build_borrow_facts(&program);
        let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
        let calls = facts.calls.span(state.calls).unwrap();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].statement_index, 0);
        assert_eq!(calls[0].call_ordinal, 0);
        assert_eq!(calls[0].target_symbol, outer_symbol);
        assert_eq!(calls[1].statement_index, 0);
        assert_eq!(calls[1].call_ordinal, 1);
        assert_eq!(calls[1].target_symbol, inner_symbol);
    }

    #[test]
    fn collects_mutable_attached_data_argument_access_roots() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);
        let target_symbol = SymbolHandle::from_arena_index(3);
        let player_symbol = SymbolHandle::from_arena_index(4);

        let mut program = omega_typed_trees::TypedTrees::default();
        let self_name = Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            machine_symbol,
            machine_symbol,
        ));
        let player_member = Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: self_name,
                member_symbol: player_symbol,
                member: ProgramName::generated("player"),
            },
        ));
        let player_argument = Expression::Mutable(Box::new(player_member));
        let player_argument = program.expression_table.insert_tree(&player_argument);

        let mut arguments = HandleSpan::empty();
        program
            .statement_table
            .push_expression_handle(&mut arguments, player_argument);

        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut state = State {
            symbol: state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: machine_symbol,
                target_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("heal"),
                arguments,
            }),
        );
        program.push_machine_state(&mut machine, state);
        program.push_machine_state(
            &mut machine,
            State {
                symbol: target_symbol,
                name: ProgramName::generated("heal"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(machine);

        let facts = build_borrow_facts(&program);
        let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
        let call = facts.calls.span(state.calls).unwrap()[0].clone();
        let accesses = facts.argument_accesses.span(call.accesses).unwrap();

        assert_eq!(accesses.len(), 1);
        assert_eq!(accesses[0].root_symbol, player_symbol);
        assert_eq!(accesses[0].kind, BorrowAccessKind::Mutable);
    }

    #[test]
    fn call_mutated_places_include_mutable_attached_data_arguments() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);
        let target_symbol = SymbolHandle::from_arena_index(3);
        let player_symbol = SymbolHandle::from_arena_index(4);

        let mut program = omega_typed_trees::TypedTrees::default();
        let self_name = Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            machine_symbol,
            machine_symbol,
        ));
        let player_member = Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: self_name,
                member_symbol: player_symbol,
                member: ProgramName::generated("player"),
            },
        ));
        let player_argument = Expression::Mutable(Box::new(player_member));
        let player_argument = program.expression_table.insert_tree(&player_argument);

        let mut arguments = HandleSpan::empty();
        program
            .statement_table
            .push_expression_handle(&mut arguments, player_argument);

        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut state = State {
            symbol: state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: machine_symbol,
                target_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("heal"),
                arguments,
            }),
        );
        program.push_machine_state(&mut machine, state);
        let mut target_state = State {
            symbol: target_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.push_state_parameter(
            &mut target_state,
            StateParameter {
                symbol: SymbolHandle::from_arena_index(5),
                name: ProgramName::generated("self"),
                type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: true,
                is_self: true,
            },
        );
        program.push_state_parameter(
            &mut target_state,
            StateParameter {
                symbol: SymbolHandle::from_arena_index(6),
                name: ProgramName::generated("player"),
                type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut machine, target_state);
        program.push_machine(machine);

        let facts = build_borrow_facts(&program);
        let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
        let call = facts.calls.span(state.calls).unwrap()[0].clone();
        let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
        let places = call_mutated_places(
            &program,
            machine_symbol,
            state_symbol,
            &facts,
            &call,
            &mut state_mutation_summary_cache,
        );

        assert!(places.iter().any(|place| place.root
            == omega_facts::PlaceRoot::Symbol(player_symbol)
            && place.segments.is_empty()));
    }

    #[test]
    fn instantiates_call_contract_places_for_attached_data_arguments() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(1);
        let caller_state_symbol = SymbolHandle::from_arena_index(2);
        let callee_machine_symbol = SymbolHandle::from_arena_index(3);
        let callee_state_symbol = SymbolHandle::from_arena_index(4);
        let caller_player_symbol = SymbolHandle::from_arena_index(5);
        let callee_player_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let player_fact_expression =
            program
                .expression_table
                .insert(omega_typed_trees::expression::ExpressionNode::Name(
                    omega_checked_trees::expression::TableNamePath {
                        members: HandleSpan::empty(),
                        member_symbols: HandleSpan::empty(),
                        head_symbol: callee_player_symbol,
                        symbol: callee_player_symbol,
                    },
                ));
        let callee_fact =
            program
                .proof_facts
                .append(omega_typed_trees::domain::ProofFact::Expression(
                    player_fact_expression,
                ));

        let mut caller_arguments = HandleSpan::empty();
        let self_name = Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            caller_machine_symbol,
            caller_machine_symbol,
        ));
        let player_member = Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: self_name,
                member_symbol: caller_player_symbol,
                member: ProgramName::generated("player"),
            },
        ));
        let player_argument = Expression::Mutable(Box::new(player_member));
        let player_argument = program.expression_table.insert_tree(&player_argument);
        program
            .statement_table
            .push_expression_handle(&mut caller_arguments, player_argument);

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: caller_machine_symbol,
                target_symbol: callee_state_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("heal"),
                arguments: caller_arguments,
            }),
        );
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let mut callee_machine = Machine {
            symbol: callee_machine_symbol,
            name: ProgramName::generated("Game"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut callee_state = State {
            symbol: callee_state_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.push_state_parameter(
            &mut callee_state,
            StateParameter {
                symbol: callee_player_symbol,
                name: ProgramName::generated("player"),
                type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut callee_machine, callee_state);
        program.push_machine(callee_machine);

        let call = omega_checked_trees::ContractCallFact {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index: 0,
            call_ordinal: 0,
            target_machine_symbol: callee_machine_symbol,
            target_state_symbol: callee_state_symbol,
            requires: HandleSpan::empty(),
            ensures: HandleSpan::empty(),
        };
        let contract = omega_checked_trees::ContractProofFact {
            kind: ContractProofFactKind::Requires,
            owner: ContractProofFactOwner::MachineState {
                machine_symbol: callee_machine_symbol,
                state_symbol: callee_state_symbol,
            },
            fact: callee_fact,
        };

        let mut semantic = omega_facts::FactPlan::default();
        let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
        let omega_facts::FactPlace::Place(place_handle) = place else {
            panic!("expected instantiated call place");
        };
        let place = semantic.places.get(place_handle);
        let segments = semantic.place_segments.span_or_empty(place.segments);

        assert_eq!(
            place.root,
            omega_facts::PlaceRoot::Symbol(caller_machine_symbol)
        );
        assert_eq!(segments.len(), 1);
        assert_eq!(
            segments[0],
            omega_facts::PlaceSegment::Field {
                symbol: caller_player_symbol
            }
        );
    }

    #[test]
    fn instantiates_call_contract_places_for_expression_statement_calls() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(1);
        let caller_state_symbol = SymbolHandle::from_arena_index(2);
        let callee_machine_symbol = SymbolHandle::from_arena_index(3);
        let callee_state_symbol = SymbolHandle::from_arena_index(4);
        let caller_player_symbol = SymbolHandle::from_arena_index(5);
        let callee_player_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let player_fact_expression =
            program
                .expression_table
                .insert(omega_typed_trees::expression::ExpressionNode::Name(
                    omega_checked_trees::expression::TableNamePath {
                        members: HandleSpan::empty(),
                        member_symbols: HandleSpan::empty(),
                        head_symbol: callee_player_symbol,
                        symbol: callee_player_symbol,
                    },
                ));
        let callee_fact =
            program
                .proof_facts
                .append(omega_typed_trees::domain::ProofFact::Expression(
                    player_fact_expression,
                ));

        let self_name = Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            caller_machine_symbol,
            caller_machine_symbol,
        ));
        let player_member = Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: self_name,
                member_symbol: caller_player_symbol,
                member: ProgramName::generated("player"),
            },
        ));
        let player_argument = Expression::Mutable(Box::new(player_member));
        let call_expression = Expression::Call(Box::new(CallExpression {
            receiver: Some(Box::new(Expression::Name(NamePath::resolved(
                vec![ProgramName::generated("self")],
                caller_machine_symbol,
                caller_machine_symbol,
            )))),
            target_symbol: callee_state_symbol,
            target: ProgramName::generated("heal"),
            arguments: Arc::from(vec![player_argument].into_boxed_slice()),
        }));
        let call_expression = program.expression_table.insert_tree(&call_expression);

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Expression(call_expression),
        );
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let mut callee_machine = Machine {
            symbol: callee_machine_symbol,
            name: ProgramName::generated("Game"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut callee_state = State {
            symbol: callee_state_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.push_state_parameter(
            &mut callee_state,
            StateParameter {
                symbol: callee_player_symbol,
                name: ProgramName::generated("player"),
                type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut callee_machine, callee_state);
        program.push_machine(callee_machine);

        let call = omega_checked_trees::ContractCallFact {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index: 0,
            call_ordinal: 0,
            target_machine_symbol: callee_machine_symbol,
            target_state_symbol: callee_state_symbol,
            requires: HandleSpan::empty(),
            ensures: HandleSpan::empty(),
        };
        let contract = omega_checked_trees::ContractProofFact {
            kind: ContractProofFactKind::Requires,
            owner: ContractProofFactOwner::MachineState {
                machine_symbol: callee_machine_symbol,
                state_symbol: callee_state_symbol,
            },
            fact: callee_fact,
        };

        let mut semantic = omega_facts::FactPlan::default();
        let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
        let omega_facts::FactPlace::Place(place_handle) = place else {
            panic!("expected instantiated call place");
        };
        let place = semantic.places.get(place_handle);
        let segments = semantic.place_segments.span_or_empty(place.segments);

        assert_eq!(
            place.root,
            omega_facts::PlaceRoot::Symbol(caller_machine_symbol)
        );
        assert_eq!(segments.len(), 1);
        assert_eq!(
            segments[0],
            omega_facts::PlaceSegment::Field {
                symbol: caller_player_symbol
            }
        );
    }
