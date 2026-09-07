use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionTargetNode};

fn typed_source(source: &str) -> Result<TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("const value tokens");
    let syntax = parse_syntax_trees(&tokens).expect("const value syntax");
    let syntax = syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax)?;
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)?;
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}

fn accepts(source: &str) -> CheckedTrees {
    let typed = typed_source(source).expect("const value source should type");
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| {
        panic!("const value source should check: {diagnostics:#?}\n{source}")
    })
}

fn rejects(source: &str, fragment: &str) {
    let typed = typed_source(source).expect("const value refusal source should type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("false const value obligation must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}\n{source}"
    );
}

fn returned_expression(program: &TypedTrees, machine_symbol: SymbolHandle) -> ExpressionHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("machine with the retained symbol");
    let [state] = program.machine_states(machine) else {
        panic!("one entry state")
    };
    match program
        .statement_table
        .statements(state.statement_nodes)
        .last()
    {
        Some(StatementNode::Expression(expression)) => *expression,
        Some(StatementNode::Transition(transition)) => {
            let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(transition.target)
            else {
                panic!("a value exit")
            };
            *expression
        }
        statement => panic!("expected a returned expression: {statement:#?}"),
    }
}

#[test]
fn inferred_const_binder_is_an_executable_checked_value() {
    let source = "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64 [0..=3] { N }
        machine main() -> u64 {
            let witness: [u8; 2] = [0, 0];
            endpoint(&witness)
        }";
    let typed = typed_source(source).expect("inferred const body should type");
    let endpoint = &typed.machines()[0];
    let [binder] = typed.machine_type_parameters(endpoint) else {
        panic!("endpoint has one const binder")
    };
    assert!(binder.symbol.is_valid());
    let ExpressionNode::Name(path) = typed
        .expression_table
        .expression(returned_expression(&typed, endpoint.symbol))
    else {
        panic!("unspecialized body retains its binder reference")
    };
    assert_eq!(path.symbol, binder.symbol);
    let endpoint_symbol = endpoint.symbol;
    let checked =
        lower_typed_trees(typed).expect("inferred N must pass checked return-range proof");
    let ExpressionNode::Integer(value) = checked
        .expression_table
        .expression(returned_expression(&checked, endpoint_symbol))
    else {
        panic!("closed body returns an integer value")
    };
    assert_eq!(value.value_u64(), Some(2));
}

#[test]
fn original_and_cloned_instances_return_distinct_closed_values() {
    let checked = accepts(
        "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64 [0..=3] { N }
         machine main() -> u64 {
             let pair: [u8; 2] = [0, 0];
             let triple: [u8; 3] = [0, 0, 0];
             let first: u64 = endpoint(&pair);
             let second: u64 = endpoint(&triple);
             second
         }",
    );
    let [original, cloned] = checked.machine_specializations.as_slice() else {
        panic!("two closed const instances")
    };
    assert_eq!(original.instance, original.template);
    assert_eq!(cloned.template, original.template);
    assert_ne!(original.instance, cloned.instance);
    for (specialization, expected) in [(original, 2), (cloned, 3)] {
        let ExpressionNode::Integer(value) = checked
            .expression_table
            .expression(returned_expression(&checked, specialization.instance))
        else {
            panic!("each checked instance returns its own integer")
        };
        assert_eq!(value.value_u64(), Some(expected));
    }
}

#[test]
fn original_and_cloned_const_values_must_independently_prove_return_ranges() {
    for earlier_call in ["", "let earlier: u64 = endpoint(&pair);"] {
        rejects(
            &format!(
                "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64 [0..=3] {{ N }}
                 machine main() -> u64 {{
                     let pair: [u8; 2] = [0, 0];
                     let oversized: [u8; 5] = [0, 0, 0, 0, 0];
                     {earlier_call}
                     endpoint(&oversized)
                 }}"
            ),
            "not provably within its declared range",
        );
    }
}

#[test]
fn same_spelled_const_binders_in_different_machines_keep_their_symbols() {
    let source = "machine pair<const N: u64>(witness: &[u8; N]) -> u64 [2..=2] { N }
        machine triple<const N: u64>(witness: &[u8; N]) -> u64 [3..=3] { N }
        machine main() -> u64 {
            let pair_witness: [u8; 2] = [0, 0];
            let triple_witness: [u8; 3] = [0, 0, 0];
            let first: u64 = pair(&pair_witness);
            triple(&triple_witness)
        }";
    let typed = typed_source(source).expect("separate const scopes should type");
    let first_binder = typed.machine_type_parameters(&typed.machines()[0])[0].symbol;
    let second_binder = typed.machine_type_parameters(&typed.machines()[1])[0].symbol;
    assert!(first_binder.is_valid());
    assert!(second_binder.is_valid());
    assert_ne!(first_binder, second_binder);
    for (machine, binder) in typed.machines()[..2]
        .iter()
        .zip([first_binder, second_binder])
    {
        let ExpressionNode::Name(path) = typed
            .expression_table
            .expression(returned_expression(&typed, machine.symbol))
        else {
            panic!("body retains a resolved const reference")
        };
        assert_eq!(path.symbol, binder);
    }
    lower_typed_trees(typed).expect("each machine proves its own exact return range");
}

#[test]
fn local_with_const_binders_spelling_keeps_its_lexical_value() {
    accepts(
        "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64 [7..=7] {
             let N: u64 = 7;
             N
         }
         machine main() -> u64 {
             let pair: [u8; 2] = [0, 0];
             let triple: [u8; 3] = [0, 0, 0];
             let first: u64 = endpoint(&pair);
             endpoint(&triple)
         }",
    );
}

#[test]
fn negative_integer_const_values_check_in_original_and_cloned_bodies() {
    accepts(
        "data Values {}
         const Values::NEGATIVE: i32 = -2;
         const Values::POSITIVE: i32 = 3;
         data Witness<const N: i32> { case Only; }
         data Main { first: Witness<Values::NEGATIVE>; second: Witness<Values::POSITIVE>; }
         machine value<const N: i32>(witness: &Witness<N>) -> i32 [-2..=3] ensures result == N { N }
         machine Main::main(&self) -> i32 {
             let negative: i32 = value(&self.first);
             value(&self.second)
         }",
    );
}

#[test]
fn boolean_const_values_check_in_original_and_cloned_bodies() {
    accepts(
        "const Values::ENABLED: bool = true;
         const Values::DISABLED: bool = false;
         data Witness<const N: bool> { case Only; }
         data Main { first: Witness<Values::ENABLED>; second: Witness<Values::DISABLED>; }
         machine value<const N: bool>(witness: &Witness<N>) -> bool ensures result == N { N }
         machine Main::main(&self) -> bool {
             let enabled: bool = value(&self.first);
             value(&self.second)
         }",
    );
}

#[test]
fn named_structured_const_values_check_as_executable_machine_results() {
    accepts(
        "data Config { count: u8; enabled: bool; }
         data Values {}
         const Values::FIRST: Config = Config { count: 2, enabled: true };
         const Values::SECOND: Config = Config { enabled: false, count: 3 };
         data Witness<const N: Config> { case Only; }
         data Main { first: Witness<Values::FIRST>; second: Witness<Values::SECOND>; }
         machine value<const N: Config>(witness: &Witness<N>) -> Config { N }
         machine Main::main(&self) -> Config {
             let first_value: Config = value(&self.first);
             value(&self.second)
         }",
    );
}

#[test]
fn const_body_values_and_ensures_close_through_forwarded_static_arguments() {
    accepts(
        "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64 [0..=3]
             ensures result == N { N }
         machine forward<const N: u64>(witness: &[u8; N]) -> u64 [0..=3]
             ensures result == N { endpoint<N>(witness) }
         machine main() -> u64 {
             let pair: [u8; 2] = [0, 0];
             let triple: [u8; 3] = [0, 0, 0];
             let first: u64 = forward(&pair);
             forward(&triple)
         }",
    );
}

#[test]
fn a_const_binder_in_ensures_does_not_prove_a_different_body_value() {
    rejects(
        "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64
             ensures result == N { 0 }
         machine main() -> u64 {
             let pair: [u8; 2] = [0, 0];
             endpoint(&pair)
         }",
        "ensures",
    );
}

#[test]
fn selected_const_value_keeps_its_declared_integer_width() {
    rejects(
        "machine value<const N: u8>() -> u64 { N }
         machine main() -> u64 { value<256>() }",
        "does not fit its `u8` suffix",
    );
}

#[test]
fn const_values_close_literal_return_range_endpoints() {
    accepts(
        "machine value<const N: u64>(witness: &[u8; N]) -> u64 [0..=N] { N }
         machine main() -> u64 {
             let pair: [u8; 2] = [0, 0];
             let triple: [u8; 3] = [0, 0, 0];
             let first: u64 = value(&pair);
             let second: u64 = value(&triple);
             second
         }",
    );
}

#[test]
fn array_and_case_const_values_retain_their_declared_shape() {
    accepts(
        "data Choice { case Empty; case Item(value: u8); }
         const Values::PAIR: [u8; 2] = [2, 3];
         const Values::EMPTY: Choice = Choice::Empty;
         const Values::ITEM: Choice = Choice::Item { value: 7 };
         data ArrayWitness<const N: [u8; 2]> { case Only; }
         data ChoiceWitness<const N: Choice> { case Only; }
         data Main {
             pair: ArrayWitness<Values::PAIR>;
             empty: ChoiceWitness<Values::EMPTY>;
             item: ChoiceWitness<Values::ITEM>;
         }
         machine array<const N: [u8; 2]>(witness: &ArrayWitness<N>) -> [u8; 2] { N }
         machine choice<const N: Choice>(witness: &ChoiceWitness<N>) -> Choice { N }
         machine Main::main(&self) -> Choice {
             let pair_value: [u8; 2] = array(&self.pair);
             let empty_value: Choice = choice(&self.empty);
             choice(&self.item)
         }",
    );
}
