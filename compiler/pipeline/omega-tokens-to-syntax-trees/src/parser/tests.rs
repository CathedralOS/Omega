use super::parse_syntax_trees;
use omega_source_files_to_tokens::Lexer;
use omega_syntax_trees::expression::ExpressionNode;
use omega_syntax_trees::statement::StatementNode;
use omega_syntax_trees::types::TypeReferenceNode;

#[test]
fn parses_dungeon_machine_surface() {
    let source = r#"
        machine Game::new() -> Game {
            let game: Game;
            transition {
                _ -> game
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    assert_eq!(parsed.root_item_count(), 1);
}

#[test]
fn parses_dungeon_state_flow() {
    let source = r#"
        data Main {
        }

        machine Main::main(&mut self) -> i32 {
            transition {
                _ -> running()
            }

            state running(&mut self) {
                transition {
                    _ -> 0
                }
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    assert_eq!(parsed.root_item_count(), 2);
}

#[test]
fn parses_attached_main_state_name_as_main() {
    let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {}
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state_handle = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(state_handle);
    assert_eq!(state.name.as_str(), "main");
}

#[test]
fn parses_plain_and_boundary_traits() {
    let source = r#"
        trait Drawable {
            machine draw(&self, canvas: &mut Canvas);
        }

        boundary trait Console {
            machine write_line(text: String)
            effects
                stdout_io;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let traits: Vec<_> = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Trait(trait_definition) => Some(trait_definition),
            _ => None,
        })
        .collect();

    assert_eq!(traits.len(), 2);
    assert_eq!(traits[0].name.as_str(), "Drawable");
    assert!(!traits[0].is_boundary);
    assert_eq!(traits[0].machines.len(), 1);
    assert_eq!(traits[1].name.as_str(), "Console");
    assert!(traits[1].is_boundary);
    assert_eq!(traits[1].machines.len(), 1);
    let signature_handle = parsed.items.state_signatures(traits[1].machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    let effects = parsed.items.identifier_path_members(signature.effects);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].as_str(), "stdout_io");
}

#[test]
fn parses_machine_contract_clauses() {
    let source = r#"
        machine distinct_indices(i: usize, j: usize)
        requires
            i < j
        ensures
            i != j
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let contracts = parsed.items.capability_contracts(machine.contracts);

    assert_eq!(contracts.len(), 2);
    assert!(matches!(
        contracts[0].kind,
        omega_syntax_trees::item::CapabilityContractKind::Requires
    ));
    assert!(matches!(
        contracts[1].kind,
        omega_syntax_trees::item::CapabilityContractKind::Ensures
    ));
    assert!(contracts[0].token_count > 0);
    assert!(contracts[1].token_count > 0);
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(parsed.items.proof_facts(contracts[1].facts).len(), 1);
}

#[test]
fn parses_machine_termination_clauses() {
    let source = r#"
        machine walk(items: &[Item], remaining: usize)
        terminates {
            decreases remaining -> Nat::Descending;
        }
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    assert!(machine.terminates);
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.decreases)
            .len(),
        1
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.decrease_order)
            .len(),
        2
    );
}

#[test]
fn rejects_bare_arrow_transition_in_explicit_state_body() {
    let source = r#"
        machine Main::main(&mut self) {
            transition { _ -> running() }

            state running(&mut self) {
                -> finished();
            }

            state finished(&mut self) {
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens)
        .expect_err("parse should reject bare arrows in explicit state bodies");
    assert!(
        error
            .message
            .contains("explicit state bodies must use the `transition` keyword"),
        "unexpected parse error: {}",
        error.message
    );
}

#[test]
fn parses_slice_range_indexing_into_range_expression() {
    let source = r#"
        data Main {}

        machine Main::main(&mut self) -> usize {
            let values: [usize; 4] = [1, 2, 3, 4];
            let view: &[usize] = values.as_slice();
            let tail: &[usize] = view[1..];
            tail.len
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should accept slice range surface");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state_handle = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(state_handle);
    let statement_handle = parsed
        .items
        .statements(state.statements)
        .get(2)
        .copied()
        .expect("tail local");
    let statement = parsed.statements.statement(statement_handle);
    let StatementNode::LocalData(local) = statement else {
        panic!("expected local data statement");
    };
    let ExpressionNode::Indexed(indexed) = parsed.expressions.expression(local.initial_value)
    else {
        panic!("expected indexed initializer");
    };
    let ExpressionNode::Range(range) = parsed.expressions.expression(indexed.index) else {
        panic!("expected range index expression");
    };
    assert_eq!(
        parsed.expressions.display_name(indexed.index),
        "1..",
        "unexpected range display"
    );
    assert!(range.start.is_valid(), "expected explicit range start");
    assert!(!range.end.is_valid(), "expected open-ended range");
}

#[test]
fn parses_trait_machine_contract_clauses() {
    let source = r#"
        boundary trait Filesystem {
            machine open(path: String)
            requires
                path in String::NonEmpty
            ensures
                handle in FileHandle::Open
            effects
                filesystem_io;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Trait(trait_definition) => Some(trait_definition),
            _ => None,
        })
        .expect("trait root item");
    let signature_handle = parsed.items.state_signatures(trait_definition.machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    let contracts = parsed.items.capability_contracts(signature.contracts);
    let effects = parsed.items.identifier_path_members(signature.effects);

    assert_eq!(contracts.len(), 2);
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(parsed.items.proof_facts(contracts[1].facts).len(), 1);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].as_str(), "filesystem_io");
}

#[test]
fn parses_executable_domain_membership_expression() {
    let source = r#"
        data Player {
            health: i32;
        }

        data Main {
            alive: Player;
        }

        machine Main::main(&mut self) {
            transition (self.alive in Player::Alive) {
                (true) -> done()
                _ -> done()
            }

            state done(&mut self) {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("entry transition");
    let omega_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let omega_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
        panic!("transition should lower as a guarded expression");
    };
    assert!(matches!(
        parsed.expressions.expression(subject),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn parses_data_destructure_transition_guard_as_subject_member_guard() {
    let source = r#"
        data Player {
            health: i32;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) {
            match self.player {
                Player::Alive -> done()
                Player { health, .. } if health > 5 -> done()
                _ -> done()
            }

            state done() {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .get(1)
        .copied()
        .expect("data-pattern transition");
    let StatementNode::Transition(transition) = parsed.statements.statement(statement) else {
        panic!("second arm should be a transition")
    };
    let omega_syntax_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        panic!("data-pattern arm should lower to a guard expression");
    };
    let ExpressionNode::Binary(comparison) = parsed.expressions.expression(guard) else {
        panic!("data-pattern guard should be a comparison");
    };
    assert!(matches!(
        parsed.expressions.expression(comparison.left),
        ExpressionNode::Member(_)
    ));
}

#[test]
fn parses_executable_domain_membership_intersection_expression() {
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
        }

        data Main {
            alive: Player;
        }

        machine Main::main(&mut self) {
            transition (self.alive in Player::Alive & Player::Charged) {
                (true) -> done()
                _ -> done()
            }

            state done(&mut self) {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("entry transition");
    let omega_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let omega_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
        panic!("transition should lower as a guarded expression");
    };
    assert!(matches!(
        parsed.expressions.expression(subject),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn parses_executable_domain_membership_union_expression() {
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
        }

        data Main {
            alive: Player;
        }

        machine Main::main(&mut self) {
            transition (self.alive in Player::Alive | Player::Charged) {
                (true) -> done()
                _ -> done()
            }

            state done(&mut self) {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("entry transition");
    let omega_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let omega_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
        panic!("transition should lower as a guarded expression");
    };
    assert!(matches!(
        parsed.expressions.expression(subject),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn parses_export_items_with_optional_alias() {
    let source = r#"
        export internal_regex::Match as Match;
        export Grep::search;
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let exports = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Export(export) => Some(export),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(exports.len(), 2);
    let first_path = parsed.items.identifier_path_members(exports[0].path);
    assert_eq!(first_path.len(), 2);
    assert_eq!(first_path[0].as_str(), "internal_regex");
    assert_eq!(first_path[1].as_str(), "Match");
    assert_eq!(
        exports[0].alias.as_ref().map(|alias| alias.as_str()),
        Some("Match")
    );
    let second_path = parsed.items.identifier_path_members(exports[1].path);
    assert_eq!(second_path.len(), 2);
    assert_eq!(second_path[0].as_str(), "Grep");
    assert_eq!(second_path[1].as_str(), "search");
    assert!(exports[1].alias.is_none());
}

#[test]
fn parses_domain_definition_surface() {
    let source = r#"
        domain Player::Alive {
            self in Player::Valid;
            self.health > 0
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let domains = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name.as_str(), "Player::Alive");
    assert!(domains[0].target_type.is_valid());
    assert_eq!(parsed.items.proof_facts(domains[0].facts).len(), 2);
    assert!(domains[0].body_token_count > 3);

    let facts = parsed.items.proof_facts(domains[0].facts);
    assert!(matches!(
        facts[0],
        omega_syntax_trees::item::ProofFact::Membership(_)
    ));
    assert!(matches!(
        facts[1],
        omega_syntax_trees::item::ProofFact::Expression(_)
    ));
}

#[test]
fn parses_self_parameter_with_dedicated_self_type() {
    let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {}
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state"),
    );
    let parameter = parsed.items.state_parameter(
        parsed
            .items
            .state_parameters(state.parameters)
            .first()
            .copied()
            .expect("self parameter"),
    );

    assert!(parameter.is_self);
    assert!(matches!(
        parsed
            .type_references
            .type_reference(parameter.type_reference),
        TypeReferenceNode::SelfType
    ));
}

#[test]
fn parses_self_expression_as_dedicated_node() {
    let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {
            self;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state"),
    );
    let statement = parsed.statements.statement(
        parsed
            .items
            .statements(state.statements)
            .first()
            .copied()
            .expect("expression statement"),
    );
    let StatementNode::Expression(expression) = statement else {
        panic!("expected expression statement");
    };

    assert!(matches!(
        parsed.expressions.expression(*expression),
        ExpressionNode::SelfValue
    ));
}

#[test]
fn parses_nested_call_arguments_as_contiguous_expression_spans() {
    let source = r#"
        data Player {
            xp: i32;
            level: i32;
        }

        data Main {
            xp_table: Player;
        }

        machine Main::main(&mut self, player: &mut Player) {
            player.xp = max(0, player.xp - self.xp_required(player.level));

            state xp_required(&mut self, level: i32) -> i32 {
                10
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state"),
    );
    let statement = parsed.statements.statement(
        parsed
            .items
            .statements(state.statements)
            .first()
            .copied()
            .expect("assignment statement"),
    );
    let StatementNode::Assignment(assignment) = statement else {
        panic!("expected assignment statement");
    };

    assert_eq!(
        parsed.expressions.display_name(assignment.value),
        "max(0, player.xp - self.xp_required(player.level))"
    );
}

#[test]
fn rejects_self_as_ordinary_declaration_name() {
    let source = r#"
        data self {}
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    assert!(parse_syntax_trees(&tokens).is_err());
}
