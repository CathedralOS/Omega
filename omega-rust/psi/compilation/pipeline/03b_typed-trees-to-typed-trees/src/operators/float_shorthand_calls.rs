//! Bind resolved float shorthand to the ordinary requirement-call route.
//!
//! Retaining a selected `F64::minimum` use beside a call to the builtin `min`
//! symbol is not executable supply: ordinary call checking needs the selected
//! declaration's signature, effects and source identity. Normalize that join
//! before validation, including calls synthesized by `abs` and `clamp`. This
//! selects a requirement, never a provider; Omega still installs and replays
//! its selected implementation. Integer intrinsics and authored same-spelled
//! functions are not rewritten.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

pub(super) fn bind(
    program: &mut TypedTrees,
    facts: &checked_trees::CheckedOperatorFacts,
    requirements: &arena::Arena<checked_trees::CheckedNamedRequirementUseFact>,
) -> Result<usize, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    for (expression, selected) in facts
        .named_uses()
        .map(|use_fact| (use_fact.expression, use_fact.selected_operator_symbol))
        .chain(
            requirements
                .iter()
                .map(|(_, use_fact)| (use_fact.expression, use_fact.requirement_symbol)),
        )
    {
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            continue;
        };
        if !matches!(
            program
                .symbols
                .builtin_function_for_symbol(call.target_symbol),
            Some(
                symbols::BuiltinFunction::Min
                    | symbols::BuiltinFunction::Max
                    | symbols::BuiltinFunction::Sqrt
            )
        ) || call.receiver.is_valid()
            || !call.carries_only_positional_arguments()
            || !call.selects_only_nominal_route()
        {
            continue;
        }
        let target = program
            .machine_by_symbol(selected)
            .and_then(|machine| {
                program
                    .machine_states(machine)
                    .first()
                    .map(|entry| entry.symbol)
            })
            .unwrap_or(selected);
        if let Some((_, previous)) = bindings.iter().find(|(bound, _)| *bound == expression) {
            if *previous != target {
                return Err(vec![Diagnostic::error(
                    "a float shorthand occurrence selects conflicting requirement identities",
                )]);
            }
        } else {
            bindings.push((expression, target));
        }
    }
    for &(expression, target) in &bindings {
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) {
            call.target_symbol = target;
        }
    }
    Ok(bindings.len())
}
