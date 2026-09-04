//! Source-to-checked-tree fixture entrance shared by native realization tests.

use std::sync::Arc;

use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

pub(crate) fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

pub(crate) fn checked_with_sole_selected_provider(
    source: &str,
) -> Arc<psi_checked_trees::CheckedTrees> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let plans = omega_provider_planning::plans::derive_satisfies_plans(&typed, None);
    let [plan] = plans.as_slice() else {
        panic!("selected-provider fixture requires exactly one ProviderPlan")
    };
    let mut checked = lower_typed_trees(typed).expect("check");
    let named_uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .collect::<Vec<_>>();
    let [(use_handle, operator_use)] = named_uses.as_slice() else {
        panic!("selected-provider fixture requires exactly one named operator use")
    };
    let mut operator_use = *operator_use;
    operator_use.provider_plan_report_fingerprint = plan.report_fingerprint();
    operator_use.provider_plan_commitment =
        psi_checked_trees::CheckedProviderPlanCommitment::from_digest(
            *plan.identity_digest().as_bytes(),
        );
    *checked.facts.operators.named_uses.get_mut(*use_handle) = operator_use;
    let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("select exact checked-provider plan");
    let mut checked = Arc::new(checked);
    omega_selected_dispatch::settle_selected_operator_adapter_dispatch(&mut checked, &selected)
        .expect("settle selected-provider fixture");
    checked
}
