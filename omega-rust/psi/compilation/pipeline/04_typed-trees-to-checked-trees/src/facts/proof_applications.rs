//! Capture proof-application relationships while checked declarations are live.
//!
//! The shared typed reference visitor supplies formation edges; operational
//! facts retain call/cleanup/static-binding edges. No body or call selection is
//! inferred here, and preserving metadata does not retain executable roots.

use checked_trees::{CheckedTrees, ProofApplicationPlan};
use symbols::SymbolHandle;

pub(crate) fn retain(checked: &CheckedTrees) -> ProofApplicationPlan {
    let typed = &checked.typed;
    let facts = &checked.facts;
    // Contract formation can owe normal completion without a range or a
    // comparison. Its lexical owner must survive source pruning as well.
    let completion_owners = typed
        .constant_recipes
        .iter()
        .filter(|(recipe, declaration)| {
            typed.closed_constant_recipe_value(*recipe).is_none()
                && matches!(
                    declaration.position,
                    typed_trees::constant_recipes::RetainedConstantPosition::ContractExpression(_)
                )
        })
        .flat_map(|(_, declaration)| {
            typed
                .constant_expression_contexts
                .span_or_empty(declaration.contexts)
        })
        .filter(|context| typed.machine_by_symbol(context.evaluation_entry).is_some())
        .map(|context| context.owner)
        .collect::<Vec<_>>();
    let has_obligations = !facts.constant_predicate_obligations.is_empty()
        || !facts.integer_range_obligations.is_empty()
        || !facts.constant_recipe_obligations.is_empty()
        || !completion_owners.is_empty()
        || typed
            .constant_recipes
            .iter()
            .any(|(recipe, _)| typed.closed_constant_recipe_value(recipe).is_none())
        || typed.const_declarations().iter().any(|declaration| {
            declaration.evaluated_value.is_none() && declaration.evaluation_entry.is_valid()
        })
        || !facts
            .flow
            .terminal_unit_effects
            .layout_observations
            .is_empty();
    let mut plan =
        typed_trees::typed_trees::calls::application_graph::source_application_plan(typed);
    plan.has_obligations = has_obligations;
    let symbols = &mut plan.symbols;
    plan.machines.for_each_mut(|_, machine| {
        machine.dynamic_realizations = symbols.insert_many(
            facts
                .dynamic_conformances
                .selections
                .iter()
                .filter(|selection| selection.machine == machine.machine)
                .flat_map(|selection| selection.rows.iter().map(|row| row.realization_machine)),
        );
    });
    let owners: Vec<SymbolHandle> = facts
        .constant_predicate_obligations
        .iter()
        .map(|row| row.caller())
        .chain(
            facts
                .integer_range_obligations
                .iter()
                .map(|row| row.owner()),
        )
        .chain(
            facts
                .constant_recipe_obligations
                .iter()
                .map(|row| row.owner())
                .filter(|owner| owner.is_valid()),
        )
        .chain(completion_owners)
        .collect();
    plan.obligation_owners = plan.symbols.insert_many(owners);
    plan.unscoped_recipe = facts
        .constant_recipe_obligations
        .iter()
        .any(|row| !row.owner().is_valid());
    plan
}
