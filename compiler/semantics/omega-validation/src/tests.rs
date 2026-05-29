use super::{validate_effect_plan, validate_program};
use omega_source_files_to_tokens::Lexer;
use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use omega_tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn validates_main_entry_surface_from_source_pipeline() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    assert_eq!(typed.machines().len(), 1);
    assert_eq!(typed.machines()[0].name.as_str(), "Main::main");
    assert_eq!(typed.machine_states(&typed.machines()[0]).len(), 1);
    assert_eq!(
        typed.machine_states(&typed.machines()[0])[0].name.as_str(),
        "main"
    );
    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn validates_local_state_call_arguments_from_source_pipeline() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
        take_non_negative(0);

        state take_non_negative(
            &mut self,
            value: u32[exact, non_negative]
        ) {
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let entry = typed
        .machine_states(&typed.machines()[0])
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("entry state");
    let call_argument_count = typed
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::Call(call) => Some(call.arguments.len()),
            omega_typed_trees::statement::StatementNode::Expression(expression) => {
                let omega_typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(*expression)
                else {
                    return None;
                };
                Some(call.arguments.len())
            }
            _ => None,
        })
        .expect("expected call statement");
    assert_eq!(call_argument_count, 1);
    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn rejects_unknown_trait_machine_effects() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: String)
        effects
            stdoutish;
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject effect");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown effect `stdoutish`")),
        "expected unknown effect diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_unknown_domain_membership_in_domain_body() {
    let source = r#"
    data Player {
    }

    domain Player::Alive {
        self in Player::Valid
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("domain `Player::Alive` references unknown domain `Player::Valid`")),
        "expected unknown domain diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_unknown_domain_membership_in_contract() {
    let source = r#"
    data Player {
    }

    boundary trait Renderer {
        machine draw(player: Player)
        requires
            player in Player::Drawable;
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "trait `Renderer` state `draw` requires contract references unknown domain `Player::Drawable`"
        )),
        "expected unknown contract domain diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_non_boolean_shaped_proof_fact() {
    let source = r#"
    data Player {
    }

    domain Player::Weird {
        1 + 2
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject fact");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("domain `Player::Weird` proof fact `1 + 2` is not boolean-shaped")),
        "expected non-boolean proof fact diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_domain_import_with_different_target_type() {
    let source = r#"
    data Player {
    }

    data Enemy {
    }

    domain Enemy::Valid {
        true
    }

    domain Player::Alive {
        self in Enemy::Valid
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject import");
    let has_target_mismatch = diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "domain `Player::Alive` imports `Enemy::Valid` but they classify different types",
        )
    });
    assert!(
        has_target_mismatch,
        "expected domain target mismatch diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_domain_import_cycles() {
    let source = r#"
    data Player {
    }

    domain Player::Alive {
        self in Player::Valid
    }

    domain Player::Valid {
        self in Player::Alive
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject cycle");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "domain membership cycle: Player::Alive -> Player::Valid -> Player::Alive",
            )
        }),
        "expected domain cycle diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_machine_effects_outside_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: String)
        effects
            stdout_io;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::write_line(text: String) satisfies Console
    effects
        stdout_io, filesystem_io
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject extra effect");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("effect `filesystem_io` is not allowed by the trait requirement")),
        "expected effect ceiling diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_machine_effects_within_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: String)
        effects
            stdout_io;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::write_line(text: String) satisfies Console
    effects
        stdout_io
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn accepts_machine_effects_below_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: String)
        effects
            stdout_io;
    }

    data TestConsole {
    }

    machine TestConsole::write_line(text: String) satisfies Console {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn rejects_declared_machine_effects_below_reached_effects() {
    let source = r#"
    boundary trait Console {
        machine read_line(out: &mut String)
        effects
            stdin_io;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::read_line(out: &mut String) satisfies Console
    effects
        stdin_io
    {
    }

    data Main {
        console: ConsoleImpl;
    }

    machine Main::main(&mut self)
    effects
        stdout_io
    {
        let line: String;
        self.console.read_line(&mut line);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("direct effect validation should pass");
    let effect_plan = omega_effects::infer_effects(&typed);
    let diagnostics =
        validate_effect_plan(&typed, &effect_plan).expect_err("effect ceiling should fail");

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("reaches undeclared effects `stdin_io`")
            && diagnostic.message.contains("call path for `stdin_io`")
            && diagnostic.message.contains("Main::main statement")
            && diagnostic
                .message
                .contains("source: machine `ConsoleImpl::read_line` directly declares the effect")),
        "expected transitive effect ceiling diagnostic, got {diagnostics:#?}"
    );
}
