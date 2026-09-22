//! A public receiver-free top-level `boundary requirement` settles one direct-
//! call dispatch row keyed on its owner and entry state, execution settlement
//! redirects the journaled call to the selected checked adapter, and a called
//! requirement with no selected provider rejects at settlement.

use super::{Arc, CheckedTrees, ProviderPlan, settle_selected_boundary_adapter_dispatch};
use crate::settle_selected_execution_dispatch_with_source_edits;
use provider_planning::ProviderPlanDerivation;
use typed_trees::expression::ExpressionNode;

const REQUIREMENT_SOURCE: &str = r#"
    pub data CheckedMath {}
    pub boundary requirement CheckedMath::offset_zero(value: i32) -> i32;

    data CheckedMathProvider {}
    machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
    satisfies CheckedMath::offset_zero
    {
        transition { _ -> (input + 0) }
    }

    data Client {}
    machine Client::run(&mut self) -> i32 {
        let selected: i32 = CheckedMath::offset_zero(35);
        transition { _ -> (selected) }
    }
"#;

fn requirement_fixture(source: &str) -> (CheckedTrees, Vec<ProviderPlan>) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize requirement fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse requirement fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve requirement fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type requirement fixture");
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check requirement fixture");
    (checked, plans)
}

fn entry_symbol(checked: &CheckedTrees, machine_name: &str) -> symbols::SymbolHandle {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
    checked
        .typed
        .machine_states(machine)
        .first()
        .expect("entry state")
        .symbol
}

/// The symbol of parameter `parameter_name` on `machine_name`'s entry state.
fn entry_parameter_symbol(
    checked: &CheckedTrees,
    machine_name: &str,
    parameter_name: &str,
) -> symbols::SymbolHandle {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
    checked
        .typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| checked.typed.state_parameters(state))
        .find(|parameter| parameter.name.as_str() == parameter_name)
        .unwrap_or_else(|| panic!("missing parameter `{machine_name}`::{parameter_name}"))
        .symbol
}

fn data_symbol(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing data `{name}`"))
        .symbol
}

fn direct_call(
    checked: &CheckedTrees,
) -> (
    typed_trees::expression::ExpressionHandle,
    symbols::SymbolHandle,
) {
    checked
        .typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "offset_zero" => {
                Some((handle, call.target_symbol))
            }
            _ => None,
        })
        .expect("direct requirement call")
}

fn selected_all(plans: &[ProviderPlan]) -> effects::SelectedProviderPlanFacts {
    let names = plans
        .iter()
        .map(|plan| plan.name.clone())
        .collect::<Vec<_>>();
    effects::SelectedProviderPlanFacts::from_selection(plans, &names).expect("select every plan")
}

#[test]
fn public_requirement_settles_one_owner_keyed_direct_call_row() {
    let (checked, plans) = requirement_fixture(REQUIREMENT_SOURCE);
    assert_eq!(plans.len(), 1, "one derived top-level requirement plan");
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "CheckedMath::offset_zero");
    let realization = entry_symbol(&checked, "CheckedMathProvider::offset_zero_impl");
    let owner = data_symbol(&checked, "CheckedMath");
    let (_, call_target) = direct_call(&checked);
    assert_eq!(
        call_target, requirement,
        "the direct call targets the requirement entry"
    );

    let original = Arc::new(checked);
    let mut settled = Arc::new(original.as_ref().clone());
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
    assert_eq!(
        settled.typed, original.typed,
        "association settles no source rewrite"
    );
    let rows = &settled.facts.boundary_adapter_dispatch;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.receiver, owner);
    assert_eq!(row.requirement, requirement);
    assert_eq!(row.realization_state, realization);
    assert!(!row.forward_receiver && row.family_tuple.is_empty());
}

#[test]
fn called_requirement_without_a_selected_provider_rejects_at_settlement() {
    let (checked, _) = requirement_fixture(REQUIREMENT_SOURCE);
    let original = Arc::new(checked);
    let mut settled = Arc::new(original.as_ref().clone());
    let diagnostics = settle_selected_boundary_adapter_dispatch(
        &mut settled,
        &effects::SelectedProviderPlanFacts::default(),
    )
    .expect_err("a direct call to an unselected public requirement rejects");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "direct call `offset_zero` names public boundary requirement `CheckedMath::offset_zero`, which has no selected provider"
        )),
        "{diagnostics:?}"
    );
}

#[test]
fn execution_settlement_redirects_the_journaled_call_to_the_realization() {
    let (checked, plans) = requirement_fixture(REQUIREMENT_SOURCE);
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "CheckedMath::offset_zero");
    let realization = entry_symbol(&checked, "CheckedMathProvider::offset_zero_impl");
    let (expression, _) = direct_call(&checked);
    let settled = Arc::new(checked);
    let (settled, edits) = settle_selected_execution_dispatch_with_source_edits(settled, &selected)
        .expect("execution settles");
    let ExpressionNode::Call(call) = settled.typed.expression_table.expression(expression) else {
        panic!("the journaled call is still a call");
    };
    assert_eq!(call.target_symbol, realization);
    assert_eq!(
        call.target.as_str(),
        "CheckedMathProvider::offset_zero_impl"
    );
    assert!(!call.receiver.is_valid(), "the owner receiver is cleared");
    // The retained flow occurrence follows the call.
    assert!(
        settled
            .facts
            .flow
            .control
            .calls
            .iter()
            .any(|(_, occurrence)| {
                occurrence.authored_expression == expression
                    && occurrence.target_symbol == realization
                    && !occurrence.has_receiver
            })
    );
    // The association row still names the requirement.
    assert!(
        settled
            .facts
            .boundary_adapter_dispatch
            .iter()
            .any(|row| { row.requirement == requirement && row.realization_state == realization })
    );
    // The journal restores the requirement-side occurrence.
    let source = edits
        .source_trees(&settled.typed)
        .expect("restore the journaled source");
    let ExpressionNode::Call(authored) = source.expression_table.expression(expression) else {
        panic!("the restored call is a call");
    };
    assert_eq!(authored.target_symbol, requirement);
    assert_eq!(authored.target.as_str(), "offset_zero");
    assert!(authored.receiver.is_valid());
}

/// The statement call `Owner::name(...);` — including the `_ = call();`
/// explicit discard — is the same settled route as the value-position call:
/// the statement-table node redirects to the adapter entry, its retained flow
/// occurrence follows, and the journal restores the authored requirement call.
#[test]
fn a_statement_position_direct_call_redirects_to_the_selected_adapter() {
    let source = REQUIREMENT_SOURCE.replace(
        "let selected: i32 = CheckedMath::offset_zero(35);\n        transition { _ -> (selected) }",
        "_ = CheckedMath::offset_zero(35);\n        transition { _ -> (35) }",
    );
    assert_ne!(source, REQUIREMENT_SOURCE);
    let (checked, plans) = requirement_fixture(&source);
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "CheckedMath::offset_zero");
    let realization = entry_symbol(&checked, "CheckedMathProvider::offset_zero_impl");
    let statement = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .flat_map(|state| {
            checked
                .typed
                .statement_table
                .iter_statements(state.statement_nodes)
        })
        .find_map(|(handle, statement)| match statement {
            typed_trees::statement::StatementNode::Call(call)
                if call.target.as_str() == "offset_zero" =>
            {
                Some(handle)
            }
            _ => None,
        })
        .expect("statement-position requirement call");

    let settled = Arc::new(checked);
    let (settled, edits) = settle_selected_execution_dispatch_with_source_edits(settled, &selected)
        .expect("a statement-position direct call settles");
    let typed_trees::statement::StatementNode::Call(call) =
        settled.typed.statement_table.statement(statement)
    else {
        panic!("the journaled statement call is still a call");
    };
    assert_eq!(call.target_symbol, realization);
    assert_eq!(
        call.target.as_str(),
        "CheckedMathProvider::offset_zero_impl"
    );
    assert!(
        call.receiver.is_empty() && !call.receiver_symbol.is_valid(),
        "the owner receiver is cleared"
    );
    assert!(
        call.discards_result,
        "the `_ =` explicit discard is retained"
    );
    // The retained flow occurrence follows the call.
    assert!(
        settled
            .facts
            .flow
            .control
            .calls
            .iter()
            .any(|(_, occurrence)| {
                !occurrence.authored_expression.is_valid()
                    && occurrence.target_symbol == realization
                    && !occurrence.has_receiver
            })
    );
    // The journal restores the requirement-side statement call.
    let source = edits
        .source_trees(&settled.typed)
        .expect("restore the journaled source");
    let typed_trees::statement::StatementNode::Call(authored) =
        source.statement_table.statement(statement)
    else {
        panic!("the restored statement is a call");
    };
    assert_eq!(authored.target_symbol, requirement);
    assert_eq!(authored.target.as_str(), "offset_zero");
    assert!(authored.discards_result);
}

const SELF_REQUIREMENT_SOURCE: &str = r#"
    pub data Token {}
    pub boundary requirement Token::consume(self) -> i32;

    data TokenProvider {}
    machine TokenProvider::consume_impl(token: Token) -> i32
    satisfies Token::consume
    {
        transition { _ -> (41) }
    }

    data Client {}
    machine Client::run(&mut self, token: Token) -> i32 {
        _ = token.consume();
        transition { _ -> (7) }
    }
"#;

/// A public `self` requirement is called through a member receiver
/// (`token.consume()`). Settlement emits one dispatch row per receiver
/// place — the row keys on the place's own symbol, not the nominal owner —
/// and forwards that place as the adapter's leading argument.
#[test]
fn a_self_requirement_settles_a_receiver_place_keyed_forwarding_row() {
    let (checked, plans) = requirement_fixture(SELF_REQUIREMENT_SOURCE);
    assert_eq!(plans.len(), 1, "one derived self-requirement plan");
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "Token::consume");
    let realization = entry_symbol(&checked, "TokenProvider::consume_impl");
    let token = entry_parameter_symbol(&checked, "Client::run", "token");

    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("a member call on an owned receiver settles");
    let rows = &settled.facts.boundary_adapter_dispatch;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.receiver, token);
    assert_eq!(row.requirement, requirement);
    assert_eq!(row.realization_state, realization);
    assert!(row.forward_receiver && row.family_tuple.is_empty());
}

/// The statement member call `_ = token.consume();` redirects to the adapter
/// entry with the receiver spliced in as argument 0 — the settled program
/// reads exactly `TokenProvider::consume_impl(token)` — and the journal
/// restores the authored requirement call.
#[test]
fn a_member_call_on_a_self_requirement_forwards_the_receiver_as_argument_zero() {
    let (checked, plans) = requirement_fixture(SELF_REQUIREMENT_SOURCE);
    let selected = selected_all(&plans);
    let requirement = entry_symbol(&checked, "Token::consume");
    let realization = entry_symbol(&checked, "TokenProvider::consume_impl");
    let statement = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .flat_map(|state| {
            checked
                .typed
                .statement_table
                .iter_statements(state.statement_nodes)
        })
        .find_map(|(handle, statement)| match statement {
            typed_trees::statement::StatementNode::Call(call)
                if call.target.as_str() == "consume" =>
            {
                Some(handle)
            }
            _ => None,
        })
        .expect("the statement member call");

    let settled = Arc::new(checked);
    let (settled, edits) = settle_selected_execution_dispatch_with_source_edits(settled, &selected)
        .expect("a member call on a `self` requirement settles");
    let typed_trees::statement::StatementNode::Call(call) =
        settled.typed.statement_table.statement(statement)
    else {
        panic!("the journaled statement call is still a call");
    };
    assert_eq!(call.target_symbol, realization);
    assert_eq!(call.target.as_str(), "TokenProvider::consume_impl");
    assert!(
        call.receiver.is_empty() && !call.receiver_symbol.is_valid(),
        "the receiver moved into the argument list"
    );
    let arguments = settled
        .typed
        .statement_table
        .expression_handles(call.arguments)
        .to_vec();
    assert_eq!(arguments.len(), 1, "the receiver is argument 0");
    let ExpressionNode::Name(name) = settled.typed.expression_table.expression(arguments[0]) else {
        panic!("the forwarded receiver is a place name")
    };
    let token = entry_parameter_symbol(&settled, "Client::run", "token");
    assert_eq!(name.symbol, token);
    assert!(
        settled
            .facts
            .flow
            .control
            .calls
            .iter()
            .any(|(_, occurrence)| {
                !occurrence.authored_expression.is_valid()
                    && occurrence.target_symbol == realization
                    && !occurrence.has_receiver
            })
    );
    let source = edits
        .source_trees(&settled.typed)
        .expect("restore the journaled source");
    let typed_trees::statement::StatementNode::Call(authored) =
        source.statement_table.statement(statement)
    else {
        panic!("the restored statement is a call");
    };
    assert_eq!(authored.target_symbol, requirement);
    assert_eq!(authored.target.as_str(), "consume");
    assert!(authored.receiver_symbol.is_valid());
    assert!(authored.discards_result);
}

#[test]
fn a_borrowed_self_requirement_settles_no_direct_call_row() {
    let source = REQUIREMENT_SOURCE
        .replace(
            "pub boundary requirement CheckedMath::offset_zero(value: i32) -> i32;",
            "pub boundary requirement CheckedMath::offset_zero(&self, value: i32) -> i32;",
        )
        .replace(
            "machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32",
            "machine CheckedMathProvider::offset_zero_impl(math: &CheckedMath, input: i32) -> i32",
        )
        .replace(
            "let selected: i32 = CheckedMath::offset_zero(35);\n        transition { _ -> (selected) }",
            "transition { _ -> (35) }",
        );
    let (checked, plans) = requirement_fixture(&source);
    let selected = selected_all(&plans);
    let mut settled = Arc::new(checked);
    let outcome = settle_selected_boundary_adapter_dispatch(&mut settled, &selected);
    match outcome {
        Ok(()) => assert!(settled.facts.boundary_adapter_dispatch.is_empty()),
        Err(diagnostics) => assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "takes a borrowed `self` receiver; only an owned `self` receiver settles"
                )),
            "{diagnostics:?}"
        ),
    }
}

#[test]
fn a_requirement_realized_by_a_compiler_intrinsic_settles_no_adapter_row_and_is_not_rejected() {
    // The plan for an intrinsic satisfier is derived and selected; the
    // direct call executes through the named-float intrinsic bridge, so
    // association settlement neither emits an adapter row nor rejects it.
    let source = r#"
        pub data Negation [copy] {}
        pub boundary requirement Negation::flip(value: f32) -> f32;

        pub data NegationProvider {}
        machine NegationProvider::flip32(value: f32) -> f32
            satisfies Negation::flip
            via Binding::CompilerIntrinsic;

        data Client {}
        machine Client::run(&mut self) -> f32 {
            let flipped: f32 = Negation::flip(1.0f32);
            transition { _ -> (flipped) }
        }
    "#;
    let (checked, plans) = requirement_fixture(source);
    assert_eq!(
        plans.len(),
        1,
        "the intrinsic satisfier derives one plan: {plans:?}"
    );
    let selected = selected_all(&plans);
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).expect(
        "an intrinsic-realized requirement call is the intrinsic bridge's, not an adapter row",
    );
    assert!(settled.facts.boundary_adapter_dispatch.is_empty());
}
