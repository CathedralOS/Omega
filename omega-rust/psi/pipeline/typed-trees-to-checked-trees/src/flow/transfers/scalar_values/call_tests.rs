use super::{CallValues, FlowBuildContext, retains_values_across_unit_call};
use checked_trees::CheckedTrees;
use facts::ScalarValue;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

const SOURCE: &str = r#"
    machine observe(value: bool) {}
    machine wrapper(value: u8) -> u8 { observe(false); value }
"#;

fn checked(source: &str) -> CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize call fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse call fixture");
    let resolved = lower_syntax_trees(&syntax).expect("resolve call fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type call fixture");
    crate::lower_typed_trees(typed).expect("check authentic call fixture")
}

fn preserves_values(checked: &CheckedTrees) -> bool {
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wrapper")
        .expect("wrapper machine");
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("wrapper begins with an authored Unit call");
    };
    let symbols = [program.state_parameters(state)[0].symbol];
    let values = CallValues {
        bindings: vec![Some(ScalarValue::Integer(
            numerics::bignum::BigInt::from_u64(65),
        ))],
        storage: Vec::new(),
    };
    let mut context = FlowBuildContext::new(
        &checked.facts.borrow,
        &checked.facts.proof,
        &checked.facts.semantic,
        &checked.facts.values.scalar_expressions,
    );
    retains_values_across_unit_call(
        program,
        &checked.facts.borrow,
        &mut context,
        machine,
        state,
        0,
        call,
        &symbols,
        &values,
    )
    .is_some()
}

#[test]
fn unit_call_preservation_requires_one_exact_borrow_occurrence() {
    let authentic = checked(SOURCE);
    assert!(preserves_values(&authentic));
    let wrapper = authentic
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wrapper")
        .expect("wrapper machine");
    let state_symbol = authentic.machine_states(wrapper)[0].symbol;
    let (state_handle, state) = authentic
        .facts
        .borrow
        .states
        .iter()
        .find(|(_, state)| state.state_symbol == state_symbol)
        .expect("authentic wrapper borrow state");
    let call = authentic.facts.borrow.calls.span_or_empty(state.calls)[0].clone();
    for corruption in [
        "missing state",
        "duplicate state",
        "missing call",
        "duplicate call",
        "wrong target",
        "wrong ordinal",
        "wrong statement",
        "unexpected receiver",
    ] {
        let mut changed = authentic.clone();
        let borrow = &mut changed.facts.borrow;
        match corruption {
            "missing state" => {
                borrow.states.get_mut(state_handle).state_symbol = Default::default();
            }
            "duplicate state" => {
                borrow.states.insert(state.clone());
            }
            "missing call" => {
                borrow.states.get_mut(state_handle).calls = Default::default();
            }
            "duplicate call" => {
                let calls = borrow.calls.insert_many([call.clone(), call.clone()]);
                borrow.states.get_mut(state_handle).calls = calls;
            }
            _ => {
                let mut changed_call = call.clone();
                match corruption {
                    "wrong target" => changed_call.target_symbol = state_symbol,
                    "wrong ordinal" => changed_call.call_ordinal = 1,
                    "wrong statement" => changed_call.statement_index = 1,
                    "unexpected receiver" => changed_call.has_receiver = true,
                    _ => unreachable!("enumerated corruption"),
                }
                let calls = borrow.calls.insert_many([changed_call]);
                borrow.states.get_mut(state_handle).calls = calls;
            }
        }
        assert!(!preserves_values(&changed), "accepted {corruption}");
    }
}

#[test]
fn unit_call_preservation_requires_the_selected_body_and_local_argument() {
    let mut missing_body = checked(SOURCE);
    assert!(preserves_values(&missing_body));
    missing_body
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "observe")
        .expect("observe machine")
        .body_is_present = false;
    assert!(!preserves_values(&missing_body));

    let mut wrong_name = checked(
        "machine observe(value: u8) {} \
         machine wrapper(value: u8) -> u8 { observe(value); value }",
    );
    assert!(preserves_values(&wrong_name));
    let wrapper = wrong_name
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wrapper")
        .expect("wrapper machine");
    let foreign_symbol = wrapper.symbol;
    let state = &wrong_name.machine_states(wrapper)[0];
    let StatementNode::Call(call) =
        &wrong_name.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("authored call");
    };
    let argument = wrong_name
        .statement_table
        .expression_handles(call.arguments)[0];
    let ExpressionNode::Name(path) = wrong_name.typed.expression_table.expression_mut(argument)
    else {
        panic!("authored local argument");
    };
    path.symbol = foreign_symbol;
    path.head_symbol = foreign_symbol;
    assert!(!preserves_values(&wrong_name));
}

#[test]
fn mutable_owned_scalar_formals_do_not_alias_the_callers_local() {
    let checked = checked(
        "machine observe(mut value: u8) { value = 200; } \
         machine wrapper(value: u8) -> u8 { observe(value); value }",
    );
    assert!(preserves_values(&checked));
}

fn range_call_source(body: &str) -> CheckedTrees {
    checked(&format!(
        "machine observe(value: bool) {{}} \
         machine narrow(value: u32) -> u8 {{ {body} }} \
         data Input {{ value: u32 in Wrapping; }} \
         machine wrapper(input: &Input) -> u8 {{ \
             let digit: u32 in Wrapping = input.value % 10 + 48; \
             let byte: u8 = narrow(digit as u32); byte \
         }}"
    ))
}

fn captured_range(checked: &CheckedTrees) -> Option<facts::IntegerRange> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wrapper")
        .expect("wrapper machine");
    let state = &checked.machine_states(machine)[0];
    let (statement_index, statement) = checked.statement_table.statements(state.statement_nodes)
        .iter().enumerate().find(|(_, statement)| {
            matches!(statement, StatementNode::LocalData(local)
                if matches!(checked.expression_table.expression(local.initial_value), ExpressionNode::Call(_)))
        }).expect("actual scalar call initializer");
    let flow_state = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find(|(_, candidate)| candidate.state_symbol == state.symbol)
        .map(|(_, state)| state)
        .expect("wrapper flow state");
    let active = checked
        .facts
        .flow
        .control
        .statements
        .span_or_empty(flow_state.statements)
        .iter()
        .find(|statement| statement.statement_index == statement_index)
        .expect("call initializer entry")
        .entry_semantic_contexts;
    let mut context = FlowBuildContext::new(
        &checked.facts.borrow,
        &checked.facts.proof,
        &checked.facts.semantic,
        &checked.facts.values.scalar_expressions,
    );
    context.contexts = checked.facts.flow.contexts.clone();
    super::capture_bounds(
        &checked.typed,
        &checked.facts.borrow,
        &checked.facts.semantic,
        &mut context,
        state.symbol,
        statement_index,
        statement,
        active,
    )
}

#[test]
fn selected_scalar_calls_capture_live_argument_ranges_and_local_snapshots() {
    for body in [
        "(value as u8 in Wrapping) as u8",
        "observe(false); (value as u8 in Wrapping) as u8",
        "let mut copy: u32 = value; \
         let saved: u8 = (copy as u8 in Wrapping) as u8; copy = 200; saved",
    ] {
        let checked = range_call_source(body);
        assert_eq!(
            captured_range(&checked),
            Some(facts::IntegerRange {
                minimum: numerics::bignum::BigInt::from_u64(48),
                maximum: numerics::bignum::BigInt::from_u64(57),
            }),
            "{body}"
        );
    }
}

#[test]
fn selected_call_ranges_reject_missing_or_substituted_custody() {
    use checked_trees::CheckedScalarExpressionRole;

    let authentic = range_call_source("(value as u8 in Wrapping) as u8");
    assert!(captured_range(&authentic).is_some());
    let wrapper = authentic
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wrapper")
        .expect("wrapper machine");
    let state_symbol = authentic.machine_states(wrapper)[0].symbol;
    let (binding_handle, binding) = authentic
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .find(|(_, binding)| {
            binding.state == state_symbol
                && matches!(
                    binding.role,
                    CheckedScalarExpressionRole::CallArgument { .. }
                )
        })
        .expect("selected ordinary argument binding");
    assert!(
        authentic
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .iter()
            .any(|(_, alternative)| {
                alternative.state == state_symbol
                    && alternative.statement_ordinal == binding.statement_ordinal
                    && alternative.expression == binding.expression
                    && matches!(
                        alternative.role,
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: 0,
                            argument_ordinal: 0,
                        }
                    )
            }),
        "the authentic local retains a separate Unit argument view"
    );
    let (borrow_handle, borrowed_state) = authentic
        .facts
        .borrow
        .states
        .iter()
        .find(|(_, state)| state.state_symbol == state_symbol)
        .expect("wrapper borrow state");
    let call = authentic
        .facts
        .borrow
        .calls
        .span_or_empty(borrowed_state.calls)[0]
        .clone();
    for corruption in [
        "missing argument binding",
        "duplicate argument binding",
        "wrong argument source",
        "wrong argument destination",
        "wrong local binding ordinal",
        "missing call",
        "wrong call target",
        "wrong call ordinal",
        "missing callee body",
        "missing selected return",
    ] {
        let mut changed = authentic.clone();
        match corruption {
            "missing argument binding" => {
                changed
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(binding_handle)
                    .state = Default::default();
            }
            "duplicate argument binding" => {
                changed
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .insert(binding.clone());
            }
            "wrong argument source" => {
                changed
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(binding_handle)
                    .expression = Default::default();
            }
            "wrong argument destination" => {
                changed
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(binding_handle)
                    .destination = state_symbol;
            }
            "wrong local binding ordinal" => {
                changed
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(binding_handle)
                    .role = CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal: 0,
                    argument_ordinal: 0,
                };
            }
            "missing call" => {
                changed.facts.borrow.states.get_mut(borrow_handle).calls = Default::default();
            }
            "wrong call target" | "wrong call ordinal" => {
                let mut call = call.clone();
                if corruption == "wrong call target" {
                    call.target_symbol = state_symbol;
                } else {
                    call.call_ordinal = 1;
                }
                let calls = changed.facts.borrow.calls.insert_many([call]);
                changed.facts.borrow.states.get_mut(borrow_handle).calls = calls;
            }
            "missing callee body" => {
                changed
                    .typed
                    .machines_mut()
                    .iter_mut()
                    .find(|machine| machine.name.as_str() == "narrow")
                    .expect("narrow machine")
                    .body_is_present = false;
            }
            "missing selected return" => {
                changed
                    .facts
                    .values
                    .scalar_expressions
                    .expressions
                    .retain(|expression| expression.role != CheckedScalarExpressionRole::Return);
            }
            _ => unreachable!("enumerated custody mutation"),
        }
        assert_eq!(captured_range(&changed), None, "accepted {corruption}");
    }
}
