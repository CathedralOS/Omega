use super::*;

#[test]
fn rejects_unproven_exit_ensures_domain_membership() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.player in Player::Alive
        {
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "exit ensures without a supporting flow fact should fail",
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main at statement 0")
            && diagnostic.message.contains("Player::Alive")
    }));
}

#[test]
fn accepts_exit_ensures_preserved_from_entry_fact() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.player in Player::Alive
        ensures
            self.player in Player::Alive
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit ensures should be provable from preserved entry facts");
}

#[test]
fn does_not_seed_machine_ensures_into_machine_entry_contexts() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.player in Player::Alive
        {
            0
        }
    "#;

    let typed = parse_typed_trees(source);
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");

    assert!(
        semantic
            .contexts_at_point(omega_facts::ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            })
            .next()
            .is_none(),
        "machine ensures should not be treated as entry facts"
    );
}

fn parse_typed_trees(source: &str) -> omega_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn rejects_unproven_exit_ensures_boolean_expression() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.value > 0
        {
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "exit boolean ensures without a supporting flow fact should fail",
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main at statement 0")
            && diagnostic.message.contains("self.value > 0")
    }));
}

#[test]
fn accepts_exit_ensures_preserved_boolean_expression() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.value > 0
        ensures
            self.value > 0
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit boolean ensures should be provable from preserved entry facts");
}

#[test]
fn exit_ensures_requirement_label_resolves_attached_data_members() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.player in Player::Alive
        {
            0
        }
    "#;

    let typed = parse_typed_trees(source);
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let exit_context = semantic
        .contexts_at_point(omega_facts::ProgramPoint::Exit {
            machine_symbol: machine.symbol,
            state_symbol: typed.machine_states(machine)[0].symbol,
            statement_index: 0,
        })
        .next()
        .expect("exit context");
    let fact = exit_context
        .facts()
        .next()
        .expect("exit ensures fact");

    let omega_facts::FactPlace::Place(place_handle) = fact.place else {
        panic!("expected place-backed contract fact");
    };
    let place = semantic.places.get(place_handle);
    let segments = semantic.place_segments.span_or_empty(place.segments);

    let state = &typed.machine_states(machine)[0];
    let self_symbol = typed.state_parameters(state)[0].symbol;
    let value_expression = match fact.payload {
        omega_facts::FactPayload::ContractDomainMembership { value, .. } => value,
        _ => panic!("expected contract domain membership fact"),
    };
    assert_eq!(typed.expression_table.display_name(value_expression), "self.player");
    assert_eq!(place.root, omega_facts::PlaceRoot::Symbol(self_symbol));
    let self_type_symbol = crate::flow::symbol_type_symbol(&typed, self_symbol)
        .expect("self parameter should have a resolvable type symbol");
    assert!(
        typed
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == self_type_symbol)
            .and_then(|candidate| candidate.attached_data.as_ref())
            .is_some()
            || typed
                .data_definitions()
                .iter()
                .any(|definition| definition.symbol == self_type_symbol),
        "self type symbol should resolve to a machine with attached data or a data definition"
    );
    let mut scratch = omega_facts::build_definition_fact_plan(&typed);
    let self_place = scratch.append_symbol_place(self_symbol);
    assert!(
        crate::semantic_places::resolve_place_member_symbol(&typed, &scratch, self_place, "player")
            .is_some(),
        "root self place should resolve attached-data member"
    );
    assert_eq!(segments.len(), 1, "segments: {segments:?}");
    let omega_facts::PlaceSegment::Field { symbol: member_symbol } = segments[0] else {
        panic!("expected field segment: {:?}", segments[0]);
    };
    assert!(member_symbol.is_valid());
    assert_eq!(
        crate::labels::semantic_fact_requirement_label(&typed, &semantic, fact),
        "self.player in Player::Alive"
    );
}
