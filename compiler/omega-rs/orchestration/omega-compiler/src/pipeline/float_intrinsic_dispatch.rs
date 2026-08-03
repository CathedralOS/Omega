//! F7 named-float ProviderPlan execution bridge.
//!
//! Checking retains the source operator identity and the exact selected plan
//! on each named use. Execution may then redirect only a compiler-known
//! realization to either an existing builtin or an exact primitive expression.
//! The source expression handle and fact remain unchanged, so proof,
//! result-policy evidence, and diagnostics continue to name the boundary
//! requirement rather than the bootstrap execution form.

use omega_effects::provider_plan::ProviderBinding;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::float_semantics::RoundingDirection;
use psi_numerics::literals::{FloatFormat, FloatLiteral};
use psi_symbols::BuiltinFunction;
use psi_typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedFloatRealization {
    Builtin {
        function: BuiltinFunction,
        arity: usize,
    },
    Negate(FloatFormat),
    MultiplyThenAdd(FloatFormat),
    FusedMultiplyAdd(FloatFormat),
    DirectedBinary(DirectedFloatBinaryOperation, FloatFormat, RoundingDirection),
    Convert(ArithmeticDomain),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectedFloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
}

pub(crate) fn rewrite_selected_float_intrinsic_calls(
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
        if psi_typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
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
            NamedFloatRealization::MultiplyThenAdd(_) => 3,
            NamedFloatRealization::FusedMultiplyAdd(_) => 3,
            NamedFloatRealization::DirectedBinary(_, _, _) => 2,
            NamedFloatRealization::Convert(_) => 1,
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
                call.receiver = psi_typed_trees::expression::ExpressionHandle::invalid();
                call.target = psi_typed_trees::name::Identifier::generated(function.name());
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
            NamedFloatRealization::MultiplyThenAdd(format) => {
                // Retain all three operands in an unnameable compiler call.
                // Its format-specific symbol survives the expression-table
                // copies between checking and instruction selection, where a
                // plain nested tree would lose its selected-plan identity.
                // Both engines execute this as two explicit operations:
                // round(round(left * right) + addend), never as FMA.
                let function = match format {
                    FloatFormat::F32 => BuiltinFunction::FloatMultiplyThenAddF32,
                    FloatFormat::F64 => BuiltinFunction::FloatMultiplyThenAddF64,
                };
                let Some(symbol) = checked.typed.symbols.builtin_function_symbol(function) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "compiler builtin `{}` is absent while lowering a selected named float intrinsic",
                        function.name()
                    )));
                    continue;
                };
                let mut call = call;
                call.receiver = psi_typed_trees::expression::ExpressionHandle::invalid();
                call.target = psi_typed_trees::name::Identifier::generated(function.name());
                call.target_symbol = symbol;
                ExpressionNode::Call(call)
            }
            NamedFloatRealization::FusedMultiplyAdd(format) => {
                // Like multiply-then-add, retain all three operands and the
                // format in an unnameable compiler call. The distinct builtin
                // is essential: AArch64 must emit one FMADD rather than the
                // two-rounding FMUL/FADD sequence.
                let function = match format {
                    FloatFormat::F32 => BuiltinFunction::FloatFusedMultiplyAddF32,
                    FloatFormat::F64 => BuiltinFunction::FloatFusedMultiplyAddF64,
                };
                let Some(symbol) = checked.typed.symbols.builtin_function_symbol(function) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "compiler builtin `{}` is absent while lowering a selected named float intrinsic",
                        function.name()
                    )));
                    continue;
                };
                let mut call = call;
                call.receiver = psi_typed_trees::expression::ExpressionHandle::invalid();
                call.target = psi_typed_trees::name::Identifier::generated(function.name());
                call.target_symbol = symbol;
                ExpressionNode::Call(call)
            }
            NamedFloatRealization::DirectedBinary(operation, format, direction) => {
                let function = match (operation, format, direction) {
                    (
                        DirectedFloatBinaryOperation::Add,
                        FloatFormat::F32,
                        RoundingDirection::TowardZero,
                    ) => BuiltinFunction::FloatAddTowardZeroF32,
                    (
                        DirectedFloatBinaryOperation::Add,
                        FloatFormat::F64,
                        RoundingDirection::TowardZero,
                    ) => BuiltinFunction::FloatAddTowardZeroF64,
                    (
                        DirectedFloatBinaryOperation::Add,
                        FloatFormat::F32,
                        RoundingDirection::TowardPositive,
                    ) => BuiltinFunction::FloatAddTowardPositiveF32,
                    (
                        DirectedFloatBinaryOperation::Add,
                        FloatFormat::F64,
                        RoundingDirection::TowardPositive,
                    ) => BuiltinFunction::FloatAddTowardPositiveF64,
                    (
                        DirectedFloatBinaryOperation::Add,
                        FloatFormat::F32,
                        RoundingDirection::TowardNegative,
                    ) => BuiltinFunction::FloatAddTowardNegativeF32,
                    (
                        DirectedFloatBinaryOperation::Add,
                        FloatFormat::F64,
                        RoundingDirection::TowardNegative,
                    ) => BuiltinFunction::FloatAddTowardNegativeF64,
                    (
                        DirectedFloatBinaryOperation::Subtract,
                        FloatFormat::F32,
                        RoundingDirection::TowardZero,
                    ) => BuiltinFunction::FloatSubtractTowardZeroF32,
                    (
                        DirectedFloatBinaryOperation::Subtract,
                        FloatFormat::F64,
                        RoundingDirection::TowardZero,
                    ) => BuiltinFunction::FloatSubtractTowardZeroF64,
                    (
                        DirectedFloatBinaryOperation::Subtract,
                        FloatFormat::F32,
                        RoundingDirection::TowardPositive,
                    ) => BuiltinFunction::FloatSubtractTowardPositiveF32,
                    (
                        DirectedFloatBinaryOperation::Subtract,
                        FloatFormat::F64,
                        RoundingDirection::TowardPositive,
                    ) => BuiltinFunction::FloatSubtractTowardPositiveF64,
                    (
                        DirectedFloatBinaryOperation::Subtract,
                        FloatFormat::F32,
                        RoundingDirection::TowardNegative,
                    ) => BuiltinFunction::FloatSubtractTowardNegativeF32,
                    (
                        DirectedFloatBinaryOperation::Subtract,
                        FloatFormat::F64,
                        RoundingDirection::TowardNegative,
                    ) => BuiltinFunction::FloatSubtractTowardNegativeF64,
                    (
                        DirectedFloatBinaryOperation::Multiply,
                        FloatFormat::F32,
                        RoundingDirection::TowardZero,
                    ) => BuiltinFunction::FloatMultiplyTowardZeroF32,
                    (
                        DirectedFloatBinaryOperation::Multiply,
                        FloatFormat::F64,
                        RoundingDirection::TowardZero,
                    ) => BuiltinFunction::FloatMultiplyTowardZeroF64,
                    (
                        DirectedFloatBinaryOperation::Multiply,
                        FloatFormat::F32,
                        RoundingDirection::TowardPositive,
                    ) => BuiltinFunction::FloatMultiplyTowardPositiveF32,
                    (
                        DirectedFloatBinaryOperation::Multiply,
                        FloatFormat::F64,
                        RoundingDirection::TowardPositive,
                    ) => BuiltinFunction::FloatMultiplyTowardPositiveF64,
                    (
                        DirectedFloatBinaryOperation::Multiply,
                        FloatFormat::F32,
                        RoundingDirection::TowardNegative,
                    ) => BuiltinFunction::FloatMultiplyTowardNegativeF32,
                    (
                        DirectedFloatBinaryOperation::Multiply,
                        FloatFormat::F64,
                        RoundingDirection::TowardNegative,
                    ) => BuiltinFunction::FloatMultiplyTowardNegativeF64,
                    (_, _, RoundingDirection::NearestTiesToEven) => {
                        diagnostics.push(Diagnostic::error(
                            "directed float realization cannot select nearest-even",
                        ));
                        continue;
                    }
                };
                let Some(symbol) = checked.typed.symbols.builtin_function_symbol(function) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "compiler builtin `{}` is absent while lowering a selected directed float intrinsic",
                        function.name()
                    )));
                    continue;
                };
                let mut call = call;
                call.receiver = psi_typed_trees::expression::ExpressionHandle::invalid();
                call.target = psi_typed_trees::name::Identifier::generated(function.name());
                call.target_symbol = symbol;
                ExpressionNode::Call(call)
            }
            NamedFloatRealization::Convert(domain) => {
                let Some(target_type) =
                    psi_typed_trees::operator::resolve_named_expression_call(&checked.typed, &call)
                        .map(|operator| operator.return_type)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected named conversion intrinsic at expression {expression:?} no longer resolves its return type"
                    )));
                    continue;
                };
                ExpressionNode::Cast(psi_typed_trees::expression::TableCastExpression {
                    value: arguments[0],
                    target_type,
                    target_label: psi_arena::HandleSpan::empty(),
                    domain,
                    semantic_domain: psi_arena::HandleSpan::empty(),
                    semantic_domain_arguments: psi_arena::HandleSpan::empty(),
                    semantic_domain_symbol: psi_symbols::SymbolHandle::invalid(),
                    semantic_domain_id: psi_language_semantics::SemanticDomainId::NULL,
                    form: psi_language_core::CastForm::Value,
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
    if let Some(domain) = float_to_integer_intrinsic_domain(intrinsic) {
        return Some(NamedFloatRealization::Convert(domain));
    }
    if integer_to_float_intrinsic(intrinsic) {
        return Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact));
    }
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
        "F32::multiply_then_add.f32" => {
            Some(NamedFloatRealization::MultiplyThenAdd(FloatFormat::F32))
        }
        "F64::multiply_then_add.f64" => {
            Some(NamedFloatRealization::MultiplyThenAdd(FloatFormat::F64))
        }
        "F32::fused_multiply_add.f32" => {
            Some(NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F32))
        }
        "F64::fused_multiply_add.f64" => {
            Some(NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F64))
        }
        "F32::add_toward_zero.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F32,
            RoundingDirection::TowardZero,
        )),
        "F64::add_toward_zero.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F64,
            RoundingDirection::TowardZero,
        )),
        "F32::add_toward_positive.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F32,
            RoundingDirection::TowardPositive,
        )),
        "F64::add_toward_positive.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F64,
            RoundingDirection::TowardPositive,
        )),
        "F32::add_toward_negative.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F32,
            RoundingDirection::TowardNegative,
        )),
        "F64::add_toward_negative.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F64,
            RoundingDirection::TowardNegative,
        )),
        "F32::subtract_toward_zero.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F32,
            RoundingDirection::TowardZero,
        )),
        "F64::subtract_toward_zero.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F64,
            RoundingDirection::TowardZero,
        )),
        "F32::subtract_toward_positive.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F32,
            RoundingDirection::TowardPositive,
        )),
        "F64::subtract_toward_positive.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F64,
            RoundingDirection::TowardPositive,
        )),
        "F32::subtract_toward_negative.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F32,
            RoundingDirection::TowardNegative,
        )),
        "F64::subtract_toward_negative.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F64,
            RoundingDirection::TowardNegative,
        )),
        "F32::multiply_toward_zero.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F32,
            RoundingDirection::TowardZero,
        )),
        "F64::multiply_toward_zero.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F64,
            RoundingDirection::TowardZero,
        )),
        "F32::multiply_toward_positive.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F32,
            RoundingDirection::TowardPositive,
        )),
        "F64::multiply_toward_positive.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F64,
            RoundingDirection::TowardPositive,
        )),
        "F32::multiply_toward_negative.f32" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F32,
            RoundingDirection::TowardNegative,
        )),
        "F64::multiply_toward_negative.f64" => Some(NamedFloatRealization::DirectedBinary(
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F64,
            RoundingDirection::TowardNegative,
        )),
        "F32::is_nan.f32" | "F64::is_nan.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::FloatIsNan,
            arity: 1,
        }),
        "F32::is_finite.f32" | "F64::is_finite.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::FloatIsFinite,
            arity: 1,
        }),
        "F32::is_infinite.f32" | "F64::is_infinite.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::FloatIsInfinite,
            arity: 1,
        }),
        "F32::is_normal.f32" | "F64::is_normal.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::FloatIsNormal,
            arity: 1,
        }),
        "F32::is_subnormal.f32" | "F64::is_subnormal.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::FloatIsSubnormal,
            arity: 1,
        }),
        "F32::classify.f32" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::FloatClassifyF32,
            arity: 1,
        }),
        "F64::classify.f64" => Some(NamedFloatRealization::Builtin {
            function: BuiltinFunction::FloatClassifyF64,
            arity: 1,
        }),
        "F32::from_f64.f64" | "F64::from_f32.f32" => {
            Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact))
        }
        _ => None,
    }
}

fn integer_to_float_intrinsic(intrinsic: &str) -> bool {
    let Some((namespace, operation)) = intrinsic.split_once("::") else {
        return false;
    };
    let Some((requirement, source_suffix)) = operation.rsplit_once('.') else {
        return false;
    };
    let Some(source) = requirement.strip_prefix("from_") else {
        return false;
    };
    if source != source_suffix
        || !matches!(
            source,
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
        )
    {
        return false;
    }
    matches!(namespace, "F32" | "F64")
}

fn float_to_integer_intrinsic_domain(intrinsic: &str) -> Option<ArithmeticDomain> {
    let (namespace, operation) = intrinsic.split_once("::")?;
    if !matches!(
        namespace,
        "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64"
    ) {
        return None;
    }
    let mut parts = operation.split('.');
    let requirement = parts.next()?;
    let source_suffix = parts.next()?;
    let policy = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let source = requirement.strip_prefix("from_")?;
    if source != source_suffix || !matches!(source, "f32" | "f64") {
        return None;
    }
    match policy {
        "exact" => Some(ArithmeticDomain::Exact),
        "trapping" => Some(ArithmeticDomain::Trapping),
        "saturating" => Some(ArithmeticDomain::Saturating),
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
            named_float_realization("F32::multiply_then_add.f32"),
            Some(NamedFloatRealization::MultiplyThenAdd(FloatFormat::F32))
        );
        assert_eq!(
            named_float_realization("F32::fused_multiply_add.f32"),
            Some(NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F32))
        );
        assert_eq!(
            named_float_realization("F32::classify.f32"),
            Some(NamedFloatRealization::Builtin {
                function: BuiltinFunction::FloatClassifyF32,
                arity: 1,
            })
        );
        assert_eq!(
            named_float_realization("F64::is_subnormal.f64"),
            Some(NamedFloatRealization::Builtin {
                function: BuiltinFunction::FloatIsSubnormal,
                arity: 1,
            })
        );
        assert_eq!(
            named_float_realization("F32::from_f64.f64"),
            Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact))
        );
        assert_eq!(
            named_float_realization("F64::from_f32.f32"),
            Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact))
        );
        assert_eq!(
            named_float_realization("F32::from_i8.i8"),
            Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact))
        );
        assert_eq!(
            named_float_realization("F64::from_u64.u64"),
            Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact))
        );
        assert_eq!(named_float_realization("F32::from_u64.i64"), None);
        assert_eq!(
            named_float_realization("I32::from_f64.f64.trapping"),
            Some(NamedFloatRealization::Convert(ArithmeticDomain::Trapping))
        );
        assert_eq!(
            named_float_realization("I32::from_f64.f64.exact"),
            Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact))
        );
        assert_eq!(
            named_float_realization("U8::from_f32.f32.saturating"),
            Some(NamedFloatRealization::Convert(ArithmeticDomain::Saturating))
        );
        assert_eq!(named_float_realization("U8::from_f32.f32.wrapping"), None);
        assert_eq!(
            named_float_realization("F32::square_root_toward_positive.f32"),
            None
        );
        assert_eq!(
            named_float_realization("F64::add_toward_negative.f64"),
            Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Add,
                FloatFormat::F64,
                RoundingDirection::TowardNegative,
            ))
        );
        assert_eq!(
            named_float_realization("F32::subtract_toward_positive.f32"),
            Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Subtract,
                FloatFormat::F32,
                RoundingDirection::TowardPositive,
            ))
        );
        assert_eq!(
            named_float_realization("F64::multiply_toward_zero.f64"),
            Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Multiply,
                FloatFormat::F64,
                RoundingDirection::TowardZero,
            ))
        );
    }
}
