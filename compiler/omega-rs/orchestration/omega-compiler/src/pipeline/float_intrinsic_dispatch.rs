//! F7 named-float ProviderPlan execution bridge.
//!
//! Checking retains the source operator identity and the exact selected plan
//! on each named use.  Execution may then redirect only a compiler-known
//! intrinsic realization to the existing builtin lowering.  The source fact
//! remains unchanged, so proof and diagnostics continue to name the boundary
//! requirement rather than the bootstrap builtin.

use omega_checked_trees::CheckedTrees;
use omega_core::{diagnostics::Diagnostic, symbols::BuiltinFunction};
use omega_effects::provider_plan::ProviderBinding;

pub(crate) fn rewrite_selected_float_intrinsic_calls(
    checked: &mut CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut rewrites = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, operator_use) in checked.facts.operators.named_uses.iter() {
        if operator_use.provider_plan_identity == 0 {
            continue;
        }
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "named float operator use carries unknown ProviderPlan identity {:#018x}",
                operator_use.provider_plan_identity
            )));
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected named-float ProviderPlan `{}` must retain exactly one realization row",
                plan.name
            )));
            continue;
        };
        let ProviderBinding::CompilerIntrinsic { name } = &row.binding else {
            continue;
        };
        let Some(builtin) = named_float_builtin(name) else {
            continue;
        };
        let omega_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected named float intrinsic at expression {:?} is not a call",
                operator_use.expression
            )));
            continue;
        };
        if omega_typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
            .map(|operator| operator.symbol)
            != Some(operator_use.selected_operator_symbol)
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected named float intrinsic at expression {:?} no longer names its checked operator symbol",
                operator_use.expression
            )));
            continue;
        }
        if let Some((_, existing)) = rewrites
            .iter()
            .find(|(expression, _)| *expression == operator_use.expression)
        {
            if *existing != builtin {
                diagnostics.push(Diagnostic::error(format!(
                    "named float expression {:?} carries contradictory selected intrinsic realizations",
                    operator_use.expression
                )));
            }
        } else {
            rewrites.push((operator_use.expression, builtin));
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for (expression, builtin) in rewrites {
        let Some(symbol) = checked.typed.symbols.builtin_function_symbol(builtin) else {
            diagnostics.push(Diagnostic::error(format!(
                "compiler builtin `{}` is absent while lowering a selected named float intrinsic",
                builtin.name()
            )));
            continue;
        };
        let omega_typed_trees::expression::ExpressionNode::Call(call) =
            checked.typed.expression_table.expression_mut(expression)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected named float intrinsic at expression {expression:?} is not a call"
            )));
            continue;
        };
        call.receiver = omega_typed_trees::expression::ExpressionHandle::invalid();
        call.target = omega_typed_trees::name::Identifier::generated(builtin.name());
        call.target_symbol = symbol;
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn named_float_builtin(intrinsic: &str) -> Option<BuiltinFunction> {
    match intrinsic {
        "F32::minimum.f32" | "F64::minimum.f64" => Some(BuiltinFunction::Min),
        "F32::maximum.f32" | "F64::maximum.f64" => Some(BuiltinFunction::Max),
        "F32::square_root.f32" | "F64::square_root.f64" => Some(BuiltinFunction::Sqrt),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_migrated_named_float_cohort_maps_to_builtins() {
        assert_eq!(
            named_float_builtin("F32::minimum.f32"),
            Some(BuiltinFunction::Min)
        );
        assert_eq!(
            named_float_builtin("F64::maximum.f64"),
            Some(BuiltinFunction::Max)
        );
        assert_eq!(
            named_float_builtin("F64::square_root.f64"),
            Some(BuiltinFunction::Sqrt)
        );
        assert_eq!(named_float_builtin("F64::negate.f64"), None);
        assert_eq!(
            named_float_builtin("F32::square_root_toward_positive.f32"),
            None
        );
    }
}
