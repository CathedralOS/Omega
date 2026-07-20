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

#[test]
fn static_machine_argument_specializes_body_calls_to_direct_symbols() {
    let source = r#"
        data Card {}
        data Main {}

        machine Card::power(value: &Card) -> u64 {
            7
        }

        machine apply<T, machine F>(value: &T) -> u64
        where machine F(item: &T) -> u64
        {
            F(value)
        }

        machine caller(card: &Card) {
            let score: u64 = apply<Card::power>(card);
        }

        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let power_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Card::power")
        .and_then(|machine| typed.machine_states(machine).first())
        .map(|state| state.symbol)
        .expect("power entry symbol");

    let checked = lower_typed_trees(typed).expect("static specialization should check");
    let apply = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("specialized apply machine");
    assert!(checked.machine_type_parameters(apply).is_empty());
    assert_eq!(checked.machine_specializations.len(), 1);
    assert_eq!(
        checked.machine_specializations[0].machine_arguments,
        vec![power_symbol]
    );
    assert_eq!(
        checked.machine_specializations[0].type_arguments,
        vec!["Card"]
    );
    assert_ne!(checked.machine_specializations[0].fingerprint, 0);

    let direct_call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            omega_typed_trees::expression::ExpressionNode::Call(call)
                if call.target_symbol == power_symbol =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("F(value) should become a direct Card::power call");
    assert_eq!(direct_call.target.as_str(), "power");
    assert!(direct_call.machine_arguments.is_empty());

    assert!(
        checked
            .expression_table
            .iter_expressions()
            .filter_map(|(_, expression)| match expression {
                omega_typed_trees::expression::ExpressionNode::Call(call) => Some(call),
                _ => None,
            })
            .all(|call| call.machine_arguments.is_empty())
    );
}

#[test]
fn static_machine_specialization_identity_is_reproducible() {
    fn fingerprint(source: &str) -> u64 {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        lower_typed_trees(typed)
            .expect("specialization should check")
            .machine_specializations[0]
            .fingerprint
    }

    let source = r#"
        data Card {}
        data Main {}
        machine Card::power(value: &Card) {}
        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        { F(value); }
        machine caller(card: &Card) {
            apply<Card::power>(card);
        }
        machine Main::run(&mut self) {}
    "#;
    assert_eq!(fingerprint(source), fingerprint(source));
}

#[test]
fn value_machine_type_parameter_is_inferred_through_a_borrowed_place() {
    let source = r#"
        data Light [copy] { weight: i32 in Wrapping; }
        data Main { light: Light; }

        machine Main::weigh<T [copy]>(&self, value: &T) -> i32 {
            70
        }

        machine Main::run(&mut self) {
            let result: i32 in Wrapping = self.weigh(&self.light);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("the borrowed place should select and materialize T := Light");

    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "Main::weigh"
            })
        })
        .expect("weigh specialization");
    assert_eq!(specialization.type_arguments, ["Light"]);
}

#[test]
fn distinct_static_machine_specializations_clone_the_template() {
    let source = r#"
        data Card {}
        data Main {}
        machine Card::power(value: &Card) {}
        machine Card::rank(value: &Card) {}
        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        { F(value); }
        machine caller(card: &Card) {
            apply<Card::power>(card);
            apply<Card::rank>(card);
        }
        machine Main::run(&mut self) {}
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("each concrete machine tuple should receive its own specialization");
    let apply_specializations: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "apply"
            })
        })
        .collect();
    assert_eq!(apply_specializations.len(), 2);
    assert_ne!(
        apply_specializations[0].machine_arguments,
        apply_specializations[1].machine_arguments
    );
    assert_eq!(
        checked
            .machines()
            .iter()
            .filter(|machine| {
                machine.name.as_str() == "apply"
                    || machine.name.as_str().starts_with("apply$specialized$")
            })
            .count(),
        2
    );
}

#[test]
fn forwarded_generic_calls_specialize_after_their_caller() {
    let source = r#"
        data Light [copy] { weight: i32; }
        data Main { light: Light; number: i32; }

        machine Main::copy_it<T [copy]>(&self, value: &T) {}
        machine Main::wrap<U [copy]>(&self, value: &U) {
            self.copy_it(value);
        }
        machine Main::run(&mut self) {
            self.copy_it(&self.light);
            self.copy_it(&self.number);
            self.wrap(&self.light);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("specializing the generic caller should expose its forwarded concrete type");

    let specialization_count = |name: &str| {
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template && machine.name.as_str() == name
                })
            })
            .count()
    };
    assert_eq!(specialization_count("Main::copy_it"), 2);
    assert_eq!(specialization_count("Main::wrap"), 1);
}
