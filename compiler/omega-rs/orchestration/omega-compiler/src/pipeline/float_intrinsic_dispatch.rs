//! F7 named-float ProviderPlan execution bridge.
//!
//! Checking retains the source operator identity and the exact selected plan
//! on each named use. Execution may then redirect only a compiler-known
//! realization to either an existing builtin or an exact primitive expression.
//! The source expression handle and fact remain unchanged, so proof,
//! result-policy evidence, and diagnostics continue to name the boundary
//! requirement rather than the bootstrap execution form.

use omega_checked_trees::CheckedTrees;
use omega_core::{
    diagnostics::Diagnostic,
    literals::{FloatFormat, FloatLiteral},
    symbols::BuiltinFunction,
};
use omega_effects::provider_plan::ProviderBinding;
use omega_typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedFloatRealization {
    Builtin {
        function: BuiltinFunction,
        arity: usize,
    },
    Negate(FloatFormat),
}

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
        let Some(realization) = named_float_realization(name) else {
            continue;
        };
        let ExpressionNode::Call(call) = checked
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
        let arguments = checked
            .typed
            .expression_table
            .expression_handles(call.arguments);
        let expected_arity = match realization {
            NamedFloatRealization::Builtin { arity, .. } => arity,
            NamedFloatRealization::Negate(_) => 1,
        };
        if arguments.len() != expected_arity {
            diagnostics.push(Diagnostic::error(format!(
                "selected named float intrinsic `{name}` requires {expected_arity} runtime argument(s), but its checked call retains {}",
                arguments.len()
            )));
            continue;
        }
        if let Some((_, existing)) = rewrites
            .iter()
            .find(|(expression, _)| *expression == operator_use.expression)
        {
            if *existing != realization {
                diagnostics.push(Diagnostic::error(format!(
                    "named float expression {:?} carries contradictory selected intrinsic realizations",
                    operator_use.expression
                )));
            }
        } else {
            rewrites.push((operator_use.expression, realization));
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for (expression, realization) in rewrites {
        let ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(expression)
            .clone()
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected named float intrinsic at expression {expression:?} is not a call"
            )));
            continue;
        };
        let arguments = checked
            .typed
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        let replacement = match realization {
            NamedFloatRealization::Builtin { function, .. } => {
                let Some(symbol) = checked.typed.symbols.builtin_function_symbol(function) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "compiler builtin `{}` is absent while lowering a selected named float intrinsic",
                        function.name()
                    )));
                    continue;
                };
                let mut call = call;
                call.receiver = omega_typed_trees::expression::ExpressionHandle::invalid();
                call.target = omega_typed_trees::name::Identifier::generated(function.name());
                call.target_symbol = symbol;
                ExpressionNode::Call(call)
            }
            NamedFloatRealization::Negate(format) => {
                let negative_one = checked.typed.expression_table.insert(ExpressionNode::Float(
                    FloatLiteral::from_f64(-1.0).with_landing(format),
                ));
                ExpressionNode::Binary(TableBinaryExpression {
                    left: arguments[0],
                    operator: BinaryOperator::Multiply,
                    right: negative_one,
                })
            }
        };
        *checked.typed.expression_table.expression_mut(expression) = replacement;
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn named_float_realization(intrinsic: &str) -> Option<NamedFloatRealization> {
    match intrinsic {
        "F32::minimum.f32" | "F64::minimum.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::Min,
            arity: 2,
        }),
        "F32::maximum.f32" | "F64::maximum.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::Max,
            arity: 2,
        }),
        "F32::square_root.f32" | "F64::square_root.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::Sqrt,
            arity: 1,
        }),
        "F32::negate.f32" => Some(NamedFloatRealization::Negate(FloatFormat::F32)),
        "F64::negate.f64" => Some(NamedFloatRealization::Negate(FloatFormat::F64)),
        "F32::is_nan.f32" | "F64::is_nan.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::FloatIsNan,
            arity: 1,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_migrated_named_float_cohort_maps_to_execution_forms() {
        assert_eq!(
            named_float_realization("F32::minimum.f32"),
            Some(NamedFloatRealization::Builtin {
                function: BuiltinFunction::Min,
                arity: 2,
            })
        );
        assert_eq!(
            named_float_realization("F64::maximum.f64"),
            Some(NamedFloatRealization::Builtin {
                function: BuiltinFunction::Max,
                arity: 2,
            })
        );
        assert_eq!(
            named_float_realization("F64::square_root.f64"),
            Some(NamedFloatRealization::Builtin {
                function: BuiltinFunction::Sqrt,
                arity: 1,
            })
        );
        assert_eq!(
            named_float_realization("F64::negate.f64"),
            Some(NamedFloatRealization::Negate(FloatFormat::F64))
        );
        assert_eq!(
            named_float_realization("F32::is_nan.f32"),
            Some(NamedFloatRealization::Builtin {
                function: BuiltinFunction::FloatIsNan,
                arity: 1,
            })
        );
        assert_eq!(
            named_float_realization("F32::square_root_toward_positive.f32"),
            None
        );
    }
}
