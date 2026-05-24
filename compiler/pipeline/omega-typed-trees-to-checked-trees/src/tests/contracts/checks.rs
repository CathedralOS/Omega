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
