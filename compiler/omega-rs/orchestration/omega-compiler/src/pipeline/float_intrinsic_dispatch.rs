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
    DirectedFusedMultiplyAdd(FloatFormat, RoundingDirection),
    DirectedSquareRoot(FloatFormat, RoundingDirection),
    DirectedBinary(DirectedFloatBinaryOperation, FloatFormat, RoundingDirection),
    Convert(ArithmeticDomain),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectedFloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StagedNamedFloatExecution {
    Builtin {
        function: BuiltinFunction,
        symbol: psi_symbols::SymbolHandle,
    },
    Negate(FloatFormat),
    Convert {
        domain: ArithmeticDomain,
        target_type: psi_typed_trees::types::TypeReferenceHandle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StagedNamedFloatRewrite {
    expression: psi_typed_trees::expression::ExpressionHandle,
    realization: NamedFloatRealization,
    execution: StagedNamedFloatExecution,
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
        let rewrite = match resolve_selected_float_intrinsic_call(
            checked,
            selected_provider_plans.plans(),
            operator_use,
        ) {
            Ok(Some(rewrite)) => rewrite,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        if let Some(existing) = rewrites
            .iter()
            .find(|existing: &&StagedNamedFloatRewrite| existing.expression == rewrite.expression)
        {
            if existing.realization != rewrite.realization
                || existing.execution != rewrite.execution
            {
                diagnostics.push(Diagnostic::error(format!(
                    "named float expression {:?} carries contradictory selected intrinsic realizations",
                    operator_use.expression
                )));
            }
        } else {
            rewrites.push(rewrite);
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for rewrite in rewrites {
        let ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(rewrite.expression)
            .clone()
        else {
            unreachable!("validated named-float rewrite ceased to be a call before publication");
        };
        let arguments = checked
            .typed
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        let replacement = match rewrite.execution {
            StagedNamedFloatExecution::Builtin { function, symbol } => {
                let mut call = call;
                call.receiver = psi_typed_trees::expression::ExpressionHandle::invalid();
                call.target = psi_typed_trees::name::Identifier::generated(function.name());
                call.target_symbol = symbol;
                ExpressionNode::Call(call)
            }
            StagedNamedFloatExecution::Negate(format) => {
                let negative_one = checked.typed.expression_table.insert(ExpressionNode::Float(
                    FloatLiteral::from_f64(-1.0).with_landing(format),
                ));
                ExpressionNode::Binary(TableBinaryExpression {
                    left: arguments[0],
                    operator: BinaryOperator::Multiply,
                    right: negative_one,
                })
            }
            StagedNamedFloatExecution::Convert {
                domain,
                target_type,
            } => ExpressionNode::Cast(psi_typed_trees::expression::TableCastExpression {
                value: arguments[0],
                target_type,
                target_label: psi_arena::HandleSpan::empty(),
                domain,
                semantic_domain: psi_arena::HandleSpan::empty(),
                semantic_domain_arguments: psi_arena::HandleSpan::empty(),
                semantic_domain_symbol: psi_symbols::SymbolHandle::invalid(),
                semantic_domain_id: psi_language_semantics::SemanticDomainId::NULL,
                form: psi_language_core::CastForm::Value,
            }),
        };
        *checked
            .typed
            .expression_table
            .expression_mut(rewrite.expression) = replacement;
    }

    Ok(())
}

fn resolve_selected_float_intrinsic_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    operator_use: &psi_checked_trees::CheckedNamedOperatorUseFact,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    let plans = selected_provider_plans
        .iter()
        .filter(|plan| plan.identity_fingerprint() == operator_use.provider_plan_identity)
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        return Err(Diagnostic::error(match plans.len() {
            0 => format!(
                "named float operator use carries unknown ProviderPlan identity {:#018x}",
                operator_use.provider_plan_identity,
            ),
            count => format!(
                "named float operator use ProviderPlan identity {:#018x} matches {count} selected plans",
                operator_use.provider_plan_identity,
            ),
        }));
    };

    resolve_float_intrinsic_call(checked, operator_use, plan)
}

fn resolve_float_intrinsic_call(
    checked: &CheckedTrees,
    operator_use: &psi_checked_trees::CheckedNamedOperatorUseFact,
    plan: &omega_effects::provider_plan::ProviderPlan,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    let operators = checked
        .typed
        .operators()
        .iter()
        .filter(|operator| operator.symbol == operator_use.selected_operator_symbol)
        .collect::<Vec<_>>();
    let [operator] = operators.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected named float at expression {:?} resolves symbol {:?} to {} operator definitions",
            operator_use.expression,
            operator_use.selected_operator_symbol,
            operators.len(),
        )));
    };
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected named float at expression {:?} does not name a boundary operator",
            operator_use.expression,
        )));
    }
    let overload_identity =
        psi_typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if overload_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected named float at expression {:?} has an empty canonical overload identity",
            operator_use.expression,
        )));
    }
    let [method] = plan.schema.methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected named-float ProviderPlan `{}` must retain exactly one schema method",
            plan.name,
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected named-float ProviderPlan `{}` must retain exactly one realization row",
            plan.name,
        )));
    };
    if plan.schema.trait_name != overload_identity
        || method.name != "realize"
        || method.requirement_owner != overload_identity
        || method.requirement_identity != overload_identity
        || !plan.schema.row_binds_method(row, method)
    {
        return Err(Diagnostic::error(format!(
            "selected named-float ProviderPlan `{}` does not bind exact overload `{overload_identity}`",
            plan.name,
        )));
    }

    let ProviderBinding::CompilerIntrinsic { machine, .. } = &row.binding else {
        return Ok(None);
    };
    if !crate::pipeline::provider_plans::intrinsic_realization_matches_operator(
        &checked.typed,
        machine,
        operator,
    ) {
        return Err(Diagnostic::error(format!(
            "selected named-float ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact overload `{overload_identity}` as an external leaf",
            plan.name,
        )));
    }
    let expected = crate::pipeline::provider_plans::compiler_intrinsic_diagnostic_label(
        &checked.typed,
        operator,
    )
    .ok_or_else(|| {
        Diagnostic::error(format!(
            "selected named-float overload `{overload_identity}` has no compiler-known intrinsic realization",
        ))
    })?;
    let realization = named_float_realization_from_operator(&checked.typed, operator).ok_or_else(|| {
        Diagnostic::error(format!(
            "selected named-float overload `{overload_identity}` has no execution realization for compiler catalog entry `{expected}`",
        ))
    })?;
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(operator_use.expression)
    else {
        return Err(Diagnostic::error(format!(
            "selected named float intrinsic at expression {:?} is not a call",
            operator_use.expression,
        )));
    };
    let expected_arity = named_float_realization_arity(realization);
    let arguments = checked
        .typed
        .expression_table
        .expression_handles(call.arguments);
    if arguments.len() != expected_arity {
        return Err(Diagnostic::error(format!(
            "selected named float overload `{overload_identity}` requires {expected_arity} runtime argument(s), but its checked call retains {}",
            arguments.len(),
        )));
    }
    let source_names_selected_operator =
        psi_typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
            .map(|resolved| resolved.symbol)
            == Some(operator.symbol);
    let source_is_matching_builtin = match realization {
        NamedFloatRealization::Builtin { function, .. } => {
            checked.typed.symbols.builtin_function_symbol(function) == Some(call.target_symbol)
        }
        _ => false,
    };
    if !source_names_selected_operator && !source_is_matching_builtin {
        return Err(Diagnostic::error(format!(
            "selected named float intrinsic at expression {:?} no longer names its checked operator symbol or normalized builtin",
            operator_use.expression,
        )));
    }

    let execution = preflight_named_float_execution(checked, operator, realization)?;
    Ok(Some(StagedNamedFloatRewrite {
        expression: operator_use.expression,
        realization,
        execution,
    }))
}

const fn named_float_realization_arity(realization: NamedFloatRealization) -> usize {
    match realization {
        NamedFloatRealization::Builtin { arity, .. } => arity,
        NamedFloatRealization::Negate(_) | NamedFloatRealization::Convert(_) => 1,
        NamedFloatRealization::MultiplyThenAdd(_)
        | NamedFloatRealization::FusedMultiplyAdd(_)
        | NamedFloatRealization::DirectedFusedMultiplyAdd(_, _) => 3,
        NamedFloatRealization::DirectedSquareRoot(_, _) => 1,
        NamedFloatRealization::DirectedBinary(_, _, _) => 2,
    }
}

fn preflight_named_float_execution(
    checked: &CheckedTrees,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    realization: NamedFloatRealization,
) -> Result<StagedNamedFloatExecution, Diagnostic> {
    if let NamedFloatRealization::Negate(format) = realization {
        return Ok(StagedNamedFloatExecution::Negate(format));
    }
    if let NamedFloatRealization::Convert(domain) = realization {
        if !operator.return_type.is_valid() {
            return Err(Diagnostic::error(
                "selected named conversion intrinsic has no exact return type",
            ));
        }
        return Ok(StagedNamedFloatExecution::Convert {
            domain,
            target_type: operator.return_type,
        });
    }
    let function = named_float_realization_builtin(realization)?;
    let symbol = checked
        .typed
        .symbols
        .builtin_function_symbol(function)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "compiler builtin `{}` is absent while preflighting a selected named float intrinsic",
                function.name(),
            ))
        })?;
    Ok(StagedNamedFloatExecution::Builtin { function, symbol })
}

fn named_float_realization_builtin(
    realization: NamedFloatRealization,
) -> Result<BuiltinFunction, Diagnostic> {
    let function = match realization {
        NamedFloatRealization::Builtin { function, .. } => function,
        // Keep multiply-then-add and FMA as distinct unnameable builtins.
        // The first executes two explicit roundings; the second remains one
        // fused operation through instruction selection.
        NamedFloatRealization::MultiplyThenAdd(FloatFormat::F32) => {
            BuiltinFunction::FloatMultiplyThenAddF32
        }
        NamedFloatRealization::MultiplyThenAdd(FloatFormat::F64) => {
            BuiltinFunction::FloatMultiplyThenAddF64
        }
        NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F32) => {
            BuiltinFunction::FloatFusedMultiplyAddF32
        }
        NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F64) => {
            BuiltinFunction::FloatFusedMultiplyAddF64
        }
        NamedFloatRealization::DirectedFusedMultiplyAdd(format, direction) => {
            directed_fused_multiply_add_builtin(format, direction)?
        }
        NamedFloatRealization::DirectedSquareRoot(format, direction) => {
            directed_square_root_builtin(format, direction)?
        }
        NamedFloatRealization::DirectedBinary(operation, format, direction) => {
            directed_binary_builtin(operation, format, direction)?
        }
        NamedFloatRealization::Negate(_) | NamedFloatRealization::Convert(_) => {
            return Err(Diagnostic::error(
                "non-builtin named-float realization reached builtin preflight",
            ));
        }
    };
    Ok(function)
}

/// Resolve execution from the exact checked operator shape. The retained
/// catalog label is diagnostic-only; it is never parsed or used as a dispatch
/// key.
fn named_float_realization_from_operator(
    typed: &psi_typed_trees::TypedTrees,
    operator: &psi_typed_trees::operator::OperatorDefinition,
) -> Option<NamedFloatRealization> {
    let [namespace, requirement] = typed.operator_path_members(operator.name) else {
        return None;
    };
    let requirement = requirement.as_str();
    if matches!(
        namespace.as_str(),
        "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64"
    ) {
        return matches!(requirement, "from_f32" | "from_f64").then(|| {
            NamedFloatRealization::Convert(
                typed
                    .type_reference_table
                    .arithmetic_domain(operator.return_type),
            )
        });
    }
    let format = match namespace.as_str() {
        "F32" => FloatFormat::F32,
        "F64" => FloatFormat::F64,
        _ => return None,
    };
    if requirement.starts_with("from_") {
        return Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact));
    }
    let builtin = |function, arity| NamedFloatRealization::Builtin { function, arity };
    match requirement {
        "minimum" => Some(builtin(BuiltinFunction::Min, 2)),
        "maximum" => Some(builtin(BuiltinFunction::Max, 2)),
        "square_root" => Some(builtin(BuiltinFunction::Sqrt, 1)),
        "negate" => Some(NamedFloatRealization::Negate(format)),
        "multiply_then_add" => Some(NamedFloatRealization::MultiplyThenAdd(format)),
        "fused_multiply_add" => Some(NamedFloatRealization::FusedMultiplyAdd(format)),
        "is_nan" => Some(builtin(BuiltinFunction::FloatIsNan, 1)),
        "is_finite" => Some(builtin(BuiltinFunction::FloatIsFinite, 1)),
        "is_infinite" => Some(builtin(BuiltinFunction::FloatIsInfinite, 1)),
        "is_normal" => Some(builtin(BuiltinFunction::FloatIsNormal, 1)),
        "is_subnormal" => Some(builtin(BuiltinFunction::FloatIsSubnormal, 1)),
        "classify" => Some(builtin(
            match format {
                FloatFormat::F32 => BuiltinFunction::FloatClassifyF32,
                FloatFormat::F64 => BuiltinFunction::FloatClassifyF64,
            },
            1,
        )),
        _ => directed_float_realization(requirement, format),
    }
}

fn directed_float_realization(
    requirement: &str,
    format: FloatFormat,
) -> Option<NamedFloatRealization> {
    for (suffix, direction) in [
        ("_toward_zero", RoundingDirection::TowardZero),
        ("_toward_positive", RoundingDirection::TowardPositive),
        ("_toward_negative", RoundingDirection::TowardNegative),
    ] {
        let Some(operation) = requirement.strip_suffix(suffix) else {
            continue;
        };
        return match operation {
            "add" => Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Add,
                format,
                direction,
            )),
            "subtract" => Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Subtract,
                format,
                direction,
            )),
            "multiply" => Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Multiply,
                format,
                direction,
            )),
            "divide" => Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Divide,
                format,
                direction,
            )),
            "square_root" => Some(NamedFloatRealization::DirectedSquareRoot(format, direction)),
            "fused_multiply_add" => Some(NamedFloatRealization::DirectedFusedMultiplyAdd(
                format, direction,
            )),
            _ => None,
        };
    }
    None
}

fn directed_fused_multiply_add_builtin(
    format: FloatFormat,
    direction: RoundingDirection,
) -> Result<BuiltinFunction, Diagnostic> {
    match (format, direction) {
        (FloatFormat::F32, RoundingDirection::TowardZero) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardZero) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64)
        }
        (FloatFormat::F32, RoundingDirection::TowardPositive) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardPositive) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64)
        }
        (FloatFormat::F32, RoundingDirection::TowardNegative) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardNegative) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64)
        }
        (_, RoundingDirection::NearestTiesToEven) => Err(Diagnostic::error(
            "directed FMA realization cannot select nearest-even",
        )),
    }
}

fn directed_square_root_builtin(
    format: FloatFormat,
    direction: RoundingDirection,
) -> Result<BuiltinFunction, Diagnostic> {
    match (format, direction) {
        (FloatFormat::F32, RoundingDirection::TowardZero) => {
            Ok(BuiltinFunction::FloatSqrtTowardZeroF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardZero) => {
            Ok(BuiltinFunction::FloatSqrtTowardZeroF64)
        }
        (FloatFormat::F32, RoundingDirection::TowardPositive) => {
            Ok(BuiltinFunction::FloatSqrtTowardPositiveF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardPositive) => {
            Ok(BuiltinFunction::FloatSqrtTowardPositiveF64)
        }
        (FloatFormat::F32, RoundingDirection::TowardNegative) => {
            Ok(BuiltinFunction::FloatSqrtTowardNegativeF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardNegative) => {
            Ok(BuiltinFunction::FloatSqrtTowardNegativeF64)
        }
        (_, RoundingDirection::NearestTiesToEven) => Err(Diagnostic::error(
            "directed square-root realization cannot select nearest-even",
        )),
    }
}

fn directed_binary_builtin(
    operation: DirectedFloatBinaryOperation,
    format: FloatFormat,
    direction: RoundingDirection,
) -> Result<BuiltinFunction, Diagnostic> {
    let function = match (operation, format, direction) {
        (DirectedFloatBinaryOperation::Add, FloatFormat::F32, RoundingDirection::TowardZero) => {
            BuiltinFunction::FloatAddTowardZeroF32
        }
        (DirectedFloatBinaryOperation::Add, FloatFormat::F64, RoundingDirection::TowardZero) => {
            BuiltinFunction::FloatAddTowardZeroF64
        }
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
        (DirectedFloatBinaryOperation::Divide, FloatFormat::F32, RoundingDirection::TowardZero) => {
            BuiltinFunction::FloatDivideTowardZeroF32
        }
        (DirectedFloatBinaryOperation::Divide, FloatFormat::F64, RoundingDirection::TowardZero) => {
            BuiltinFunction::FloatDivideTowardZeroF64
        }
        (
            DirectedFloatBinaryOperation::Divide,
            FloatFormat::F32,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatDivideTowardPositiveF32,
        (
            DirectedFloatBinaryOperation::Divide,
            FloatFormat::F64,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatDivideTowardPositiveF64,
        (
            DirectedFloatBinaryOperation::Divide,
            FloatFormat::F32,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatDivideTowardNegativeF32,
        (
            DirectedFloatBinaryOperation::Divide,
            FloatFormat::F64,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatDivideTowardNegativeF64,
        (_, _, RoundingDirection::NearestTiesToEven) => {
            return Err(Diagnostic::error(
                "directed float realization cannot select nearest-even",
            ));
        }
    };
    Ok(function)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
        data F32 {}
        boundary operator F32::minimum(left: f32, right: f32) -> f32;
        boundary operator F32::maximum(left: f32, right: f32) -> f32;

        data FloatProvider {}
        machine FloatProvider::minimum(left: f32, right: f32) -> f32
        satisfies F32::minimum
        via Binding::CompilerIntrinsic;
        machine FloatProvider::maximum(left: f32, right: f32) -> f32
        satisfies F32::maximum
        via Binding::CompilerIntrinsic;

        machine run() -> f32 {
            transition { _ -> (F32::minimum(1.0f32, 2.0f32)) }
        }
    "#;

    struct Fixture {
        checked: CheckedTrees,
        minimum_plan: omega_effects::provider_plan::ProviderPlan,
        maximum_plan: omega_effects::provider_plan::ProviderPlan,
        operator_use: psi_checked_trees::CheckedNamedOperatorUseFact,
    }

    fn fixture() -> Fixture {
        let tokens = psi_source_files_to_tokens::Lexer::new(SOURCE)
            .tokenize()
            .expect("tokenize named-float dispatch fixture");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse named-float dispatch fixture");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve named-float dispatch fixture");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type named-float dispatch fixture");
        let plans = crate::pipeline::provider_plans::derive_satisfies_plans(&typed, None);
        let minimum_plan = plans
            .iter()
            .find(|plan| plan.schema.trait_name.contains("F32::minimum"))
            .expect("F32::minimum provider plan")
            .clone();
        let maximum_plan = plans
            .iter()
            .find(|plan| plan.schema.trait_name.contains("F32::maximum"))
            .expect("F32::maximum provider plan")
            .clone();
        let checked = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect("check named-float dispatch fixture");
        let operator_use = checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(_, operator_use)| *operator_use)
            .find(|operator_use| {
                checked
                    .typed
                    .operators()
                    .iter()
                    .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
                    .is_some_and(|operator| {
                        checked
                            .typed
                            .operator_path_members(operator.name)
                            .iter()
                            .map(|member| member.as_str())
                            .eq(["F32", "minimum"])
                    })
            })
            .expect("checked F32::minimum use");
        Fixture {
            checked,
            minimum_plan,
            maximum_plan,
            operator_use,
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Drift {
        None,
        UnknownPlan,
        DuplicatePlan,
        EmptyOverload,
        CrossOperatorPlan,
        MissingRow,
        DuplicateRow,
        ReadableRow,
        WrongIntrinsic,
        NonCallExpression,
        Arity,
        SourceOperator,
        NormalizedBuiltin,
        CheckedAdapter,
    }

    #[test]
    fn exact_intrinsic_resolver_rejects_every_identity_drift() {
        let cases = [
            (Drift::None, None),
            (Drift::UnknownPlan, Some("unknown ProviderPlan identity")),
            (Drift::DuplicatePlan, Some("matches 2 selected plans")),
            (Drift::EmptyOverload, Some("does not bind exact overload")),
            (
                Drift::CrossOperatorPlan,
                Some("does not bind exact overload"),
            ),
            (Drift::MissingRow, Some("exactly one realization row")),
            (Drift::DuplicateRow, Some("exactly one realization row")),
            (Drift::ReadableRow, Some("does not bind exact overload")),
            (
                Drift::WrongIntrinsic,
                Some("does not satisfy exact overload"),
            ),
            (Drift::NonCallExpression, Some("is not a call")),
            (Drift::Arity, Some("requires 2 runtime argument")),
            (
                Drift::SourceOperator,
                Some("no longer names its checked operator symbol"),
            ),
            (Drift::NormalizedBuiltin, None),
            (Drift::CheckedAdapter, None),
        ];

        for (drift, expected_error) in cases {
            let mut fixture = fixture();
            let mut plan = fixture.minimum_plan.clone();
            let mut plans = vec![plan.clone()];
            match drift {
                Drift::None => {}
                Drift::UnknownPlan => fixture.operator_use.provider_plan_identity = u64::MAX,
                Drift::DuplicatePlan => plans.push(plan.clone()),
                Drift::EmptyOverload => {
                    plan.schema.trait_name.clear();
                    plan.schema.methods[0].requirement_owner.clear();
                    plan.schema.methods[0].requirement_identity.clear();
                    plan.rows[0].requirement_identity.clear();
                    plans = vec![plan.clone()];
                }
                Drift::CrossOperatorPlan => {
                    plan = fixture.maximum_plan.clone();
                    plans = vec![plan.clone()];
                }
                Drift::MissingRow => {
                    plan.rows.clear();
                    plans = vec![plan.clone()];
                }
                Drift::DuplicateRow => {
                    plan.rows.push(plan.rows[0].clone());
                    plans = vec![plan.clone()];
                }
                Drift::ReadableRow => {
                    plan.rows[0].method = "minimum".into();
                    plans = vec![plan.clone()];
                }
                Drift::WrongIntrinsic => {
                    plan.rows[0].binding = ProviderBinding::CompilerIntrinsic {
                        machine: "F32::maximum.f32".into(),
                    };
                    plans = vec![plan.clone()];
                }
                Drift::NonCallExpression => {
                    let ExpressionNode::Call(call) = fixture
                        .checked
                        .typed
                        .expression_table
                        .expression(fixture.operator_use.expression)
                    else {
                        panic!("fixture named-float expression is not a call");
                    };
                    fixture.operator_use.expression = fixture
                        .checked
                        .typed
                        .expression_table
                        .expression_handles(call.arguments)[0];
                }
                Drift::Arity => {
                    let ExpressionNode::Call(mut call) = fixture
                        .checked
                        .typed
                        .expression_table
                        .expression(fixture.operator_use.expression)
                        .clone()
                    else {
                        panic!("fixture named-float expression is not a call");
                    };
                    call.arguments = psi_arena::HandleSpan::empty();
                    *fixture
                        .checked
                        .typed
                        .expression_table
                        .expression_mut(fixture.operator_use.expression) =
                        ExpressionNode::Call(call);
                }
                Drift::SourceOperator => {
                    let maximum = fixture
                        .checked
                        .typed
                        .operators()
                        .iter()
                        .find(|operator| {
                            fixture
                                .checked
                                .typed
                                .operator_path_members(operator.name)
                                .iter()
                                .map(|member| member.as_str())
                                .eq(["F32", "maximum"])
                        })
                        .expect("F32::maximum operator");
                    fixture.operator_use.selected_operator_symbol = maximum.symbol;
                    plan = fixture.maximum_plan.clone();
                    plans = vec![plan.clone()];
                }
                Drift::NormalizedBuiltin => {
                    let symbol = fixture
                        .checked
                        .typed
                        .symbols
                        .builtin_function_symbol(BuiltinFunction::Min)
                        .expect("min builtin symbol");
                    let ExpressionNode::Call(mut call) = fixture
                        .checked
                        .typed
                        .expression_table
                        .expression(fixture.operator_use.expression)
                        .clone()
                    else {
                        panic!("fixture named-float expression is not a call");
                    };
                    call.receiver = psi_typed_trees::expression::ExpressionHandle::invalid();
                    call.target = psi_typed_trees::name::Identifier::generated("min");
                    call.target_symbol = symbol;
                    *fixture
                        .checked
                        .typed
                        .expression_table
                        .expression_mut(fixture.operator_use.expression) =
                        ExpressionNode::Call(call);
                }
                Drift::CheckedAdapter => {
                    plan.rows[0].binding = ProviderBinding::CheckedAdapter {
                        machine: "FloatProvider::minimum".into(),
                    };
                    plans = vec![plan.clone()];
                }
            }
            if !matches!(drift, Drift::UnknownPlan) {
                fixture.operator_use.provider_plan_identity = plan.identity_fingerprint();
            }

            let result = resolve_selected_float_intrinsic_call(
                &fixture.checked,
                &plans,
                &fixture.operator_use,
            );
            match expected_error {
                Some(expected) => {
                    let diagnostic = result.expect_err("drift must fail closed");
                    assert!(
                        diagnostic.message.contains(expected),
                        "{drift:?}: expected `{expected}`, got `{}`",
                        diagnostic.message,
                    );
                }
                None if matches!(drift, Drift::CheckedAdapter) => {
                    assert_eq!(result.expect("checked adapter remains delegated"), None);
                }
                None => {
                    let rewrite = result
                        .expect("exact intrinsic resolves")
                        .expect("compiler intrinsic stages a rewrite");
                    assert_eq!(rewrite.expression, fixture.operator_use.expression);
                    assert!(matches!(
                        rewrite.execution,
                        StagedNamedFloatExecution::Builtin {
                            function: BuiltinFunction::Min,
                            ..
                        }
                    ));
                }
            }
        }
    }

    fn selected_fixture() -> (
        Fixture,
        omega_effects::SelectedProviderPlanFacts,
        psi_arena::Handle<psi_checked_trees::CheckedNamedOperatorUseFact>,
        psi_checked_trees::CheckedNamedOperatorUseFact,
    ) {
        let mut fixture = fixture();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&fixture.minimum_plan),
            std::slice::from_ref(&fixture.minimum_plan.name),
        )
        .expect("select exact named-float plan");
        let (handle, mut retained) = fixture
            .checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(handle, operator_use)| (handle, *operator_use))
            .find(|(_, operator_use)| operator_use.expression == fixture.operator_use.expression)
            .expect("fixture named-float use");
        retained.provider_plan_identity = fixture.minimum_plan.identity_fingerprint();
        *fixture.checked.facts.operators.named_uses.get_mut(handle) = retained;
        (fixture, selected, handle, retained)
    }

    #[test]
    fn exact_intrinsic_rewrite_preserves_handle_and_checked_evidence() {
        let (mut fixture, selected, handle, retained) = selected_fixture();

        rewrite_selected_float_intrinsic_calls(&mut fixture.checked, &selected)
            .expect("exact selected intrinsic rewrites");

        let ExpressionNode::Call(rewritten) = fixture
            .checked
            .typed
            .expression_table
            .expression(retained.expression)
        else {
            panic!("rewritten intrinsic is not a call");
        };
        assert_eq!(rewritten.target.as_str(), "min");
        assert_eq!(
            rewritten.target_symbol,
            fixture
                .checked
                .typed
                .symbols
                .builtin_function_symbol(BuiltinFunction::Min)
                .expect("min builtin symbol"),
        );
        assert_eq!(
            fixture.checked.facts.operators.named_uses.get(handle),
            &retained,
            "execution rewrite must not change exact checked evidence",
        );
    }

    #[test]
    fn non_builtin_execution_forms_preflight_without_publication() {
        let fixture = fixture();
        let operator = fixture
            .checked
            .typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == fixture.operator_use.selected_operator_symbol)
            .expect("selected F32::minimum operator");
        assert_eq!(
            preflight_named_float_execution(
                &fixture.checked,
                operator,
                NamedFloatRealization::Negate(FloatFormat::F32),
            )
            .expect("primitive negate preflights"),
            StagedNamedFloatExecution::Negate(FloatFormat::F32),
        );
        assert_eq!(
            preflight_named_float_execution(
                &fixture.checked,
                operator,
                NamedFloatRealization::Convert(ArithmeticDomain::Exact),
            )
            .expect("exact cast preflights"),
            StagedNamedFloatExecution::Convert {
                domain: ArithmeticDomain::Exact,
                target_type: operator.return_type,
            },
        );
    }

    #[test]
    fn any_invalid_intrinsic_use_prevents_every_staged_rewrite() {
        let (mut fixture, selected, _, retained) = selected_fixture();
        let mut invalid = retained;
        invalid.provider_plan_identity = u64::MAX;
        fixture.checked.facts.operators.named_uses.append(invalid);
        let before = fixture
            .checked
            .typed
            .expression_table
            .expression(retained.expression)
            .clone();

        let diagnostics = rewrite_selected_float_intrinsic_calls(&mut fixture.checked, &selected)
            .expect_err("one invalid intrinsic use rejects the complete rewrite batch");
        assert!(
            diagnostics[0]
                .message
                .contains("unknown ProviderPlan identity")
        );
        assert_eq!(
            fixture
                .checked
                .typed
                .expression_table
                .expression(retained.expression),
            &before,
            "validation failure must not publish any staged rewrite",
        );
    }
}
