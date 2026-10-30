//! Roster the constant positions a deferred continuation leaves symbolic.
//!
//! Deferral marks each position pending and keeps the continuation; the
//! roster names the same positions on the typed program so the shared check
//! can admit exactly them, record obligations against them, and a later
//! closure can read their values back by position.

use crate::build_time_evaluation::EvaluationDomain;
use crate::const_evaluation::{const_applications, const_lengths, range_endpoints};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::constant_recipes::{
    RetainedConstantPosition, RetainedConstantRecipe, retain_constant_recipes,
};
use typed_trees::name::Identifier;

pub(crate) fn retain_deferred_constant_recipes(
    typed: &mut TypedTrees,
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let mut recipes = Vec::new();
    for (type_reference, machine, name, source_span) in
        const_lengths::pending_array_lengths(typed, domain)
    {
        recipes.push(RetainedConstantRecipe {
            position: RetainedConstantPosition::ArrayLength(type_reference),
            contexts: Default::default(),
            invocations: Default::default(),
            machine,
            name: Identifier::generated(name),
            source_span,
        });
    }
    let integers = range_endpoints::integer_root_plan(typed, domain)?;
    for endpoint in integers
        .calls
        .iter()
        .filter(|call| !call.contract_expression)
    {
        recipes.push(RetainedConstantRecipe {
            position: RetainedConstantPosition::RangeEndpoint(endpoint.expression),
            contexts: Default::default(),
            invocations: Default::default(),
            machine: endpoint.machine,
            name: Identifier::generated(typed.symbols.name(endpoint.machine)),
            source_span: typed.expression_table.source_span(endpoint.root),
        });
    }
    // A constant root exists independently of its helper-call roster. Layout
    // observations have no authored callee, but the same target closure owns
    // their value and the contracts that depend on it.
    for root in &integers.roots {
        let range_endpoints::IntegerRootPosition::Contract(context) = root.position else {
            continue;
        };
        let position = RetainedConstantPosition::ContractExpression(root.expression);
        if typed.constant_recipes.iter().any(|(_, recipe)| {
            recipe.position == position
                && typed
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts)
                    .iter()
                    .any(|retained| retained.same_scope(&context))
        }) {
            continue;
        }
        let calls: Vec<_> = integers
            .calls
            .iter()
            .filter(|call| call.contract_expression && call.root == root.expression)
            .collect();
        let machine = calls.first().map(|call| call.machine).unwrap_or_default();
        recipes.push(RetainedConstantRecipe {
            position,
            contexts: typed.constant_expression_contexts.insert_many([context]),
            invocations: typed
                .expression_table
                .insert_expression_handles(calls.iter().map(|call| call.expression)),
            machine,
            name: Identifier::generated(if machine.is_valid() {
                typed.symbols.name(machine)
            } else {
                "constant expression"
            }),
            source_span: typed.expression_table.source_span(root.expression),
        });
    }
    let plan = const_applications::const_application_plan(typed, domain);
    for application in &plan.applications {
        // The application's executable dependency is the first authored call
        // inside its argument expression; a bare operator token argument
        // names no machine and keeps the invalid handle.
        let call = plan.calls.iter().find(|call| call.site == application.site);
        let machine = call.map(|call| call.machine).unwrap_or_default();
        recipes.push(RetainedConstantRecipe {
            position: RetainedConstantPosition::ConstApplication {
                site: application.site,
                root: application.root,
            },
            contexts: Default::default(),
            machine,
            invocations: Default::default(),
            name: Identifier::generated(if machine.is_valid() {
                typed.symbols.name(machine)
            } else {
                "const application"
            }),
            source_span: typed.expression_table.source_span(application.root),
        });
    }
    retain_constant_recipes(typed, recipes);
    Ok(())
}
