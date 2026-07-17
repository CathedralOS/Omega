use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;

/// MP1: the machine-parameter requirement is semantic tree data. It is
/// populated once from the declaration and copied through the resolved tree
/// into the typed tree; later rungs consume it for modular checking and
/// specialization.
#[test]
fn machine_parameter_contract_survives_resolved_and_typed_trees() {
    let source = r#"
        data Deck {}

        machine Deck::best<T, machine Key>(&self) -> u64
        where machine Key(value: &T) -> u64
        {
            0
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");

    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let typed_machine = typed
        .machines()
        .iter()
        .find(|machine| !typed.machine_type_parameters(machine).is_empty())
        .expect("typed generic machine");
    let typed_parameters = typed.machine_type_parameters(typed_machine);
    assert_eq!(typed_parameters.len(), 2);
    let omega_typed_trees::data::TypeParameterKind::Machine { contract } =
        &typed_parameters[1].kind
    else {
        panic!("typed Key should remain a machine parameter");
    };
    assert_eq!(contract.name.as_str(), "Key");
    assert_eq!(typed.state_signature_parameters(contract).len(), 1);
    assert!(contract.return_type.is_valid());
}

#[test]
fn call_site_machine_argument_resolves_to_static_entry_symbol() {
    let source = r#"
        data Card {}

        machine Card::power(value: &Card) {
        }

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {
        }

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let call = typed
        .machines()
        .iter()
        .flat_map(|machine| typed.machine_states(machine))
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::Call(call)
                if !call.machine_arguments.is_empty() =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("call carrying a static machine argument");

    assert_eq!(call.machine_arguments.len(), 1);
    assert!(call.machine_arguments[0].symbol.is_valid());
    assert_eq!(
        call.machine_arguments[0]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        vec!["Card", "power"]
    );
}

#[test]
fn generic_body_call_resolves_to_machine_parameter_contract() {
    let source = r#"
        data Card {}

        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        {
            F(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("generic machine");
    let machine_parameter = typed
        .machine_type_parameters(machine)
        .iter()
        .find(|parameter| parameter.name.as_str() == "F")
        .expect("machine parameter");
    let omega_typed_trees::data::TypeParameterKind::Machine { contract } = &machine_parameter.kind
    else {
        panic!("F should be a machine parameter");
    };
    assert_eq!(contract.symbol, machine_parameter.symbol);
    assert!(
        typed
            .state_signature_parameters(contract)
            .iter()
            .all(|parameter| parameter.symbol.is_valid())
    );

    let call = typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::Call(call)
                if call.target.as_str() == "F" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("generic body call");
    assert_eq!(call.target_symbol, machine_parameter.symbol);
}

#[test]
fn generic_body_call_is_accepted_modularly_by_checked_lowering() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        {
            F(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed).expect("generic body should check from F's authored contract");
}

#[test]
fn generic_body_must_discharge_machine_parameter_preconditions() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>(value: i32)
        where machine F(item: i32)
            requires item > 0
        {
            F(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("an unconstrained generic body must not assume F's precondition");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("item > 0")
            || diagnostic.message.contains("contract")
            || diagnostic.message.contains("proof")
    }));
}

#[test]
fn generic_body_can_discharge_machine_parameter_precondition_from_own_contract() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>(value: i32)
        where machine F(item: i32)
            requires item > 0;
        requires value > 0
        {
            F(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed)
        .expect("the generic body's own requires fact should discharge F's precondition");
}

#[test]
fn generic_body_can_discharge_machine_parameter_precondition_from_call_value() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>()
        where machine F(item: i32)
            requires item > 0
        {
            F(1);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed).expect("the call argument should discharge F's precondition");
}

#[test]
fn generic_body_inherits_machine_parameter_effect_ceiling() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>()
        where machine F()
            effects device_io
        {
            F();
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let apply = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("apply machine");
    let effects = omega_effects::infer_effects(&typed);
    let apply_effects = effects
        .machines()
        .iter()
        .find(|entry| entry.symbol == apply.symbol)
        .expect("apply effect summary");
    let device_io = omega_effects::EffectSet::from_name("device_io").expect("known effect");
    assert!(apply_effects.body_transitive.intersects(device_io));
}

#[test]
fn generic_body_can_consume_machine_parameter_ensures() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        domain i32::Positive { self > 0 }

        machine pipeline<machine Establish, machine Consume>(value: &mut i32)
        where machine Establish(item: &mut i32)
            ensures item in i32::Positive;
        where machine Consume(item: &i32)
            requires item in i32::Positive
        {
            Establish(value);
            Consume(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed)
        .expect("Establish's authored ensures should discharge Consume's requires");
}
