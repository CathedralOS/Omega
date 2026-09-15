use super::{entry_operand, has_stable_observable_contents};
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionTargetNode};

fn typed_program(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn named_state(
    program: &TypedTrees,
    machine_name: &str,
    state_name: &str,
) -> (SymbolHandle, SymbolHandle) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("machine {machine_name}"));
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == state_name)
        .unwrap_or_else(|| panic!("state {machine_name}::{state_name}"));
    (machine.symbol, state.symbol)
}

fn first_call_argument(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> (usize, ExpressionHandle) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .unwrap();
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .unwrap();
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let argument = match statement {
            StatementNode::Call(call) => program
                .statement_table
                .expression_handles(call.arguments)
                .first()
                .copied(),
            StatementNode::LocalData(local) => {
                match program.expression_table.expression(local.initial_value) {
                    ExpressionNode::Call(call) => program
                        .expression_table
                        .expression_handles(call.arguments)
                        .first()
                        .copied(),
                    _ => None,
                }
            }
            StatementNode::Expression(expression) => {
                match program.expression_table.expression(*expression) {
                    ExpressionNode::Call(call) => program
                        .expression_table
                        .expression_handles(call.arguments)
                        .first()
                        .copied(),
                    _ => None,
                }
            }
            // A state's tail call is its transition's named or value target.
            StatementNode::Transition(transition) => [transition.target, transition.continuation]
                .into_iter()
                .filter(|target| target.is_valid())
                .find_map(
                    |target| match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named { arguments, .. } => program
                            .statement_table
                            .expression_handles(*arguments)
                            .first()
                            .copied(),
                        TransitionTargetNode::Value(value) => {
                            match program.expression_table.expression(*value) {
                                ExpressionNode::Call(call) => program
                                    .expression_table
                                    .expression_handles(call.arguments)
                                    .first()
                                    .copied(),
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                ),
            _ => None,
        };
        if let Some(argument) = argument {
            return (index, argument);
        }
    }
    panic!("expected a call carrying an argument");
}

#[test]
fn state_parameter_arrival_transports_its_named_transition_argument() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             transition flag { true -> next(flag) false -> false }
             state next(input: bool) -> bool { sink(input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
        "the state parameter forwards the invocation actual, not a read of `next`'s storage"
    );
}

#[test]
fn state_parameter_arrival_transports_a_literal_argument() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(input: bool) -> bool { sink(input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Boolean(false)),
    );
}

#[test]
fn divergent_state_arrivals_keep_provenance_unknown() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(take: bool) -> bool {
             transition take { true -> next(false) false -> next(true) }
             state next(input: bool) -> bool { sink(input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        None,
        "two arrivals binding different values are not one saved actual"
    );
}

#[test]
fn unreachable_and_mutable_state_parameters_keep_provenance_unknown() {
    for source in [
        // `next` is never targeted, so its parameter has no arrival at all.
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition { _ -> false }
             state next(input: bool) -> bool { sink(input); input }
         }",
        // A mutable parameter can be rebound after arrival; the binding
        // statement is not the saved actual.
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool { sink(input); input }
         }",
    ] {
        let program = typed_program(source);
        let (machine, next) = named_state(&program, "value", "next");
        let (call_index, argument) = first_call_argument(&program, machine, next);
        assert_eq!(
            entry_operand(&program, machine, next, call_index, argument),
            None,
            "{source}"
        );
    }
}

#[test]
fn a_self_forwarded_arrival_is_tautological() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             transition flag { true -> next(flag) false -> false }
             state next(input: bool) -> bool {
                 sink(input);
                 transition true { true -> next(input) false -> false }
             }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
        "the `-> next(input)` edge binds the parameter to its own arrival value"
    );
}

#[test]
fn a_named_self_cycle_with_a_rebound_argument_stays_unproven() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             transition flag { true -> next(flag) false -> false }
             state next(input: bool) -> bool {
                 sink(input);
                 transition input { true -> next(!input) false -> false }
             }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        None,
        "`-> next(!input)` rebinds the parameter, so no single saved actual exists"
    );
}

const TRIGGER: &str = "machine trigger(input: bool) -> bool
         crashes Trap input
         {
             transition { !input -> false }
             crash Trap;
         }";

#[test]
fn a_state_arrival_actual_discharges_the_selected_crash_route() {
    // `input` arrives only as `false`, so the selected `crashes Trap input`
    // route is false at this invocation. `value` is public: an undischarged
    // route would have to appear on its published ceiling.
    for declaration in ["", "crashes Trap false"] {
        let program = typed_program(&format!(
            "{TRIGGER}
             pub machine value() -> bool
             {declaration}
             {{
                 transition true {{ true -> next(false) false -> false }}
                 state next(input: bool) -> bool {{ let r: bool = trigger(input); r }}
             }}"
        ));
        crate::lower_typed_trees(program)
            .unwrap_or_else(|diagnostics| panic!("{declaration}: {diagnostics:#?}"));
    }
}

#[test]
fn a_state_arrival_actual_retains_the_exact_entry_origin() {
    // `input` arrives as the caller's `flag`: the surviving Trap route is the
    // entry predicate `flag` itself, covered only when `value` publishes a
    // same-cause route for exactly that parameter.
    for (declaration, expect_ok) in [
        ("crashes Trap flag", true),
        ("crashes Trap !flag", false),
        ("", false),
    ] {
        let program = typed_program(&format!(
            "{TRIGGER}
             pub machine value(flag: bool) -> bool
             {declaration}
             {{
                 transition true {{ true -> next(flag) false -> false }}
                 state next(input: bool) -> bool {{ let r: bool = trigger(input); r }}
             }}"
        ));
        match crate::lower_typed_trees(program) {
            Ok(_) => assert!(expect_ok, "{declaration} must not check"),
            Err(diagnostics) => {
                assert!(!expect_ok, "{declaration}: {diagnostics:#?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("uncovered Trap crash route")),
                    "{declaration}: {diagnostics:#?}"
                );
            }
        }
    }
}

#[test]
fn divergent_or_mutable_state_arrivals_still_reject_at_checking() {
    for source in [
        // Arrivals disagree: `input` is not one invocation value.
        "pub machine value(take: bool) -> bool
         crashes Trap take
         {
             transition take { true -> next(false) false -> next(true) }
             state next(input: bool) -> bool { let r: bool = trigger(input); r }
         }",
        // A mutable state parameter's binding is not a saved actual.
        "pub machine value(still: bool) -> bool
         crashes Trap still
         {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool { let r: bool = trigger(input); r }
         }",
    ] {
        let program = typed_program(&format!(
            "{TRIGGER}
{source}"
        ));
        let diagnostics = crate::lower_typed_trees(program)
            .expect_err("unproven state provenance must stay conservative");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("uncovered Trap crash route")),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn structural_entry_identity_requires_plain_contents_through_generic_substitution() {
    for (carrier, stable) in [
        ("Holder<Flag>", true),
        ("Holder<Borrowed>", false),
        ("Holder<Holder<Borrowed>>", false),
        ("&Holder<Flag>", true),
        ("&mut Holder<Flag>", false),
    ] {
        let source = format!(
            "data Flag {{ enabled: bool; }}
             data Borrowed {{ flag: &mut Flag; }}
             data Holder<T> {{ value: T; }}
             machine inspect(holder: {carrier}) {{}}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .unwrap();
        let parameter = &program.state_parameters(&program.machine_states(machine)[0])[0];
        assert_eq!(
            has_stable_observable_contents(&program, parameter.type_reference),
            stable,
            "{carrier}"
        );
    }
}
