//! A public receiver-free top-level `boundary requirement` settles one direct-
//! call dispatch row keyed on its owner and entry state, execution settlement
//! redirects the journaled call to the selected checked adapter, and a called
//! requirement with no selected provider rejects at settlement.

use super::{Arc, CheckedTrees, ProviderPlan, settle_selected_boundary_adapter_dispatch};
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
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check requirement fixture");
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
    let mut settled = Arc::clone(&original);
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
    let mut settled = Arc::clone(&original);
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
    assert!(Arc::ptr_eq(&settled, &original));
}

#[test]
fn a_receiver_bearing_requirement_settles_no_direct_call_row() {
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
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("takes a `self` receiver; only a receiver-free requirement settles")),
            "{diagnostics:?}"
        ),
    }
}
