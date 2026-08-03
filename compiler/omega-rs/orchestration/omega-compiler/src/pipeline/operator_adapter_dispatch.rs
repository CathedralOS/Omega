//! Checked boundary-operator ProviderPlan execution bridge.
//!
//! Semantic checking and retained facts continue to name the public boundary
//! operator. After selection, a named call whose exact plan row is a checked
//! adapter redirects execution to that ordinary Omega machine body. This is
//! the operator analogue of boundary-trait adapter dispatch; compiler
//! intrinsics remain in `float_intrinsic_dispatch`.

use omega_checked_trees::CheckedTrees;
use omega_core::diagnostics::Diagnostic;
use omega_effects::provider_plan::ProviderBinding;
use omega_typed_trees::expression::ExpressionNode;

pub(crate) fn rewrite_selected_operator_adapter_calls(
    checked: &mut CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut rewrites = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, operator_use) in checked.facts.operators.named_uses.iter() {
        if operator_use.provider_plan_identity == 0 {
            continue;
        }
        let Some(plan) =
            selected_provider_plans.plan_by_identity(operator_use.provider_plan_identity)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "named operator use carries unknown ProviderPlan identity {:#018x}",
                operator_use.provider_plan_identity,
            )));
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected checked-operator ProviderPlan `{}` must retain exactly one realization row",
                plan.name,
            )));
            continue;
        };
        let ProviderBinding::CheckedAdapter { machine } = &row.binding else {
            continue;
        };
        let Some(provider) = checked
            .typed
            .machines()
            .iter()
            .find(|candidate| candidate.name.as_str() == machine)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected checked-operator ProviderPlan `{}` names absent adapter `{machine}`",
                plan.name,
            )));
            continue;
        };
        let Some(entry) = checked.typed.machine_states(provider).first() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected checked-operator adapter `{machine}` has no executable entry state",
            )));
            continue;
        };
        let ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected checked operator at expression {:?} is not a named call",
                operator_use.expression,
            )));
            continue;
        };
        if omega_typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
            .map(|operator| operator.symbol)
            != Some(operator_use.selected_operator_symbol)
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected checked operator at expression {:?} no longer names its checked operator symbol",
                operator_use.expression,
            )));
            continue;
        }
        if let Some((_, selected_machine, selected_symbol)) = rewrites
            .iter()
            .find(|(expression, _, _)| *expression == operator_use.expression)
        {
            if selected_machine != machine || *selected_symbol != entry.symbol {
                diagnostics.push(Diagnostic::error(format!(
                    "named operator expression {:?} carries contradictory checked-adapter realizations",
                    operator_use.expression,
                )));
            }
            continue;
        }
        rewrites.push((operator_use.expression, machine.clone(), entry.symbol));
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for (expression, machine, symbol) in rewrites {
        let ExpressionNode::Call(mut call) = checked
            .typed
            .expression_table
            .expression(expression)
            .clone()
        else {
            continue;
        };
        call.receiver = omega_typed_trees::expression::ExpressionHandle::invalid();
        call.target = omega_typed_trees::name::Identifier::generated(machine);
        call.target_symbol = symbol;
        *checked.typed.expression_table.expression_mut(expression) = ExpressionNode::Call(call);
    }

    Ok(())
}
