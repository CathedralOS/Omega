//! F7 named-float ProviderPlan execution bridge.
//!
//! Checking retains the source operator identity and the exact selected plan
//! on each named use. Execution may then redirect only a compiler-known
//! realization to either an existing builtin or an exact primitive expression.
//! The source expression handle and fact remain unchanged, so proof,
//! result-policy evidence, and diagnostics continue to name the boundary
//! requirement rather than the bootstrap execution form.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::RoundingDirection;
use numerics::literals::{FloatFormat, FloatLiteral};
use provider_planning::plans::{CompilerIntrinsicExecutionIdentity, CompilerNumericType};
use std::sync::Arc;
use symbols::BuiltinFunction;
use typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};

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

/// Closed compiler-owned execution child selected for one exact
/// `CompilerIntrinsic` boundary-operator row.
///
/// Primitive-expression realizations deliberately remain distinct from
/// builtin functions. Only explicitly closed children enter `Closed`; every
/// other compiler path remains `Unsupported` and package review rejects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCompilerIntrinsicExecutionIdentity {
    Closed(CompilerIntrinsicExecutionIdentity),
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedCompilerIntrinsicRealization {
    PrimitiveFloatBinary(CompilerIntrinsicExecutionIdentity),
    NamedFloat(NamedFloatRealization),
    OtherCompilerPath,
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
        symbol: symbols::SymbolHandle,
    },
    Negate(FloatFormat),
    Convert {
        domain: ArithmeticDomain,
        target_type: typed_trees::types::TypeReferenceHandle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StagedNamedFloatRewrite {
    expression: typed_trees::expression::ExpressionHandle,
    realization: NamedFloatRealization,
    execution: StagedNamedFloatExecution,
}

pub fn settle_selected_float_intrinsic_dispatch(
    checked: &mut Arc<CheckedTrees>,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    super::settle_selected_execution_dispatch(checked, selected_provider_plans)
}

pub(super) fn plan_selected_float_intrinsic_rewrites(
    checked: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<StagedNamedFloatRewrite>, Vec<Diagnostic>> {
    let mut rewrites = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, operator_use) in checked.facts.operators.named_uses.iter() {
        if operator_use.provider_plan_report_fingerprint == 0
            && operator_use.provider_plan_commitment.is_empty()
        {
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

    Ok(rewrites)
}

pub(super) fn apply_selected_float_intrinsic_rewrites(
    checked: &mut CheckedTrees,
    rewrites: Vec<StagedNamedFloatRewrite>,
    source_edits: &mut super::source_edits::SourceEditBuilder,
) {
    for rewrite in rewrites {
        source_edits.expression(&checked.typed, rewrite.expression);
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
                call.receiver = typed_trees::expression::ExpressionHandle::invalid();
                call.target = typed_trees::name::Identifier::generated(function.name());
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
            } => ExpressionNode::Cast(typed_trees::expression::TableCastExpression {
                value: arguments[0],
                target_type,
                target_label: arena::HandleSpan::empty(),
                domain,
                semantic_domain: arena::HandleSpan::empty(),
                semantic_domain_arguments: arena::HandleSpan::empty(),
                semantic_domain_symbol: symbols::SymbolHandle::invalid(),
                semantic_domain_id: language_semantics::SemanticDomainId::NULL,
                form: language_core::CastForm::Value,
            }),
        };
        *checked
            .typed
            .expression_table
            .expression_mut(rewrite.expression) = replacement;
    }
}

pub(super) fn selected_ieee_float_fma_unit_applications(
    checked: &CheckedTrees,
    rewrites: &[StagedNamedFloatRewrite],
) -> Result<Vec<typed_trees_to_checked_trees::SelectedIeeeFloatFmaUnitApplication>, Diagnostic> {
    let mut applications = Vec::new();
    for rewrite in rewrites {
        let format = match rewrite.realization {
            NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F32) => {
                semantic_vocabulary::IeeeFloatFormat::Binary32
            }
            NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F64) => {
                semantic_vocabulary::IeeeFloatFormat::Binary64
            }
            _ => continue,
        };
        let uses = checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(_, operator_use)| operator_use)
            .filter(|operator_use| operator_use.expression == rewrite.expression)
            .collect::<Vec<_>>();
        let Some(operator_use) = uses.first().copied() else {
            return Err(Diagnostic::error(format!(
                "selected nearest FMA expression {:?} retains no checked named use",
                rewrite.expression,
            )));
        };
        if uses.iter().any(|candidate| {
            candidate.selected_operator_symbol != operator_use.selected_operator_symbol
                || candidate.policy_adapter != operator_use.policy_adapter
                || candidate.provider_plan_report_fingerprint
                    != operator_use.provider_plan_report_fingerprint
                || candidate.provider_plan_commitment != operator_use.provider_plan_commitment
        }) {
            return Err(Diagnostic::error(format!(
                "selected nearest FMA expression {:?} carries contradictory checked named-use custody",
                rewrite.expression,
            )));
        }
        let mut local_initializer_origins = Vec::new();
        for candidate in &uses {
            if matches!(
                candidate.origin,
                checked_trees::CheckedValueOrigin::StateStatement {
                    role: checked_trees::CheckedValueStatementRole::LocalInitializer,
                    ..
                }
            ) && !local_initializer_origins.contains(&candidate.origin)
            {
                local_initializer_origins.push(candidate.origin);
            }
        }
        let origin = match local_initializer_origins.as_slice() {
            [] => continue,
            [origin] => *origin,
            origins => {
                return Err(Diagnostic::error(format!(
                    "selected nearest FMA expression {:?} is attributed to {} distinct local initializers",
                    rewrite.expression,
                    origins.len(),
                )));
            }
        };
        let ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(rewrite.expression)
        else {
            return Err(Diagnostic::error(format!(
                "selected nearest FMA expression {:?} lost its checked call shape",
                rewrite.expression,
            )));
        };
        applications.push(
            typed_trees_to_checked_trees::SelectedIeeeFloatFmaUnitApplication {
                expression: rewrite.expression,
                origin,
                requirement_operator: operator_use.selected_operator_symbol,
                provider_plan_report_fingerprint: operator_use.provider_plan_report_fingerprint,
                provider_plan_commitment: operator_use.provider_plan_commitment,
                format,
                operands: checked
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec(),
            },
        );
    }
    Ok(applications)
}

pub(super) fn validate_selected_ieee_float_fma_unit_applications(
    checked: &CheckedTrees,
    applications: &[typed_trees_to_checked_trees::SelectedIeeeFloatFmaUnitApplication],
) -> Result<(), Diagnostic> {
    for application in applications {
        let checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index,
            role: checked_trees::CheckedValueStatementRole::LocalInitializer,
        } = application.origin
        else {
            continue;
        };
        let matches = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .filter(|machine| {
                machine.machine == machine_symbol && machine.state == state_symbol
            })
            .flat_map(|machine| &machine.operations)
            .filter(|operation| {
                matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                        coordinate,
                        requirement_operator,
                        provider_plan_report_fingerprint,
                        provider_plan_commitment,
                        format,
                        ..
                    } if usize::try_from(coordinate.statement_index).ok() == Some(statement_index)
                        && *requirement_operator == application.requirement_operator
                        && *provider_plan_report_fingerprint == application.provider_plan_report_fingerprint
                        && *provider_plan_commitment == application.provider_plan_commitment
                        && *format == application.format
                )
            })
            .count();
        if matches > 1 {
            return Err(Diagnostic::error(format!(
                "selected nearest FMA at {:?} retained {matches} exact checked Unit operations; expected at most one",
                application.origin,
            )));
        }
    }
    Ok(())
}

fn resolve_selected_float_intrinsic_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
    operator_use: &checked_trees::CheckedNamedOperatorUseFact,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    if operator_use.provider_plan_commitment.is_empty() {
        return Err(Diagnostic::error(format!(
            "named float operator use carries ProviderPlan report fingerprint {:#018x} without an exact commitment",
            operator_use.provider_plan_report_fingerprint,
        )));
    }
    let report_matches = selected_provider_plans
        .iter()
        .filter(|plan| plan.report_fingerprint() == operator_use.provider_plan_report_fingerprint)
        .collect::<Vec<_>>();
    let plans = report_matches
        .iter()
        .copied()
        .filter(|plan| {
            plan.identity_digest().as_bytes() == operator_use.provider_plan_commitment.as_bytes()
        })
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        return Err(Diagnostic::error(
            match (report_matches.len(), plans.len()) {
                (1, 0) => format!(
                    "named float operator use ProviderPlan report fingerprint {:#018x} has an exact commitment that does not match the selected plan",
                    operator_use.provider_plan_report_fingerprint,
                ),
                (0, _) => format!(
                    "named float operator use carries unknown ProviderPlan report fingerprint {:#018x}",
                    operator_use.provider_plan_report_fingerprint,
                ),
                (_, count) => format!(
                    "named float operator use ProviderPlan report fingerprint {:#018x} and exact commitment match {count} selected plans",
                    operator_use.provider_plan_report_fingerprint,
                ),
            },
        ));
    };

    resolve_float_intrinsic_call(checked, operator_use, plan)
}

fn resolve_float_intrinsic_call(
    checked: &CheckedTrees,
    operator_use: &checked_trees::CheckedNamedOperatorUseFact,
    plan: &effects::provider_plan::ProviderPlan,
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
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
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

    let Some(selected_realization) = selected_compiler_intrinsic_realization(
        checked,
        plan,
        operator_use.selected_operator_symbol,
    )?
    else {
        return Ok(None);
    };
    let SelectedCompilerIntrinsicRealization::NamedFloat(realization) = selected_realization else {
        return Err(Diagnostic::error(format!(
            "selected named-float overload `{overload_identity}` has no named-float execution realization",
        )));
    };
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
        typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
            .map(|resolved| resolved.symbol)
            == Some(operator.symbol);
    let source_is_matching_builtin = matches!(realization, NamedFloatRealization::Builtin { .. })
        && typed_trees_to_checked_trees::resolve_checked_builtin_float_operator_requirement(
            &checked.typed,
            operator_use.expression,
            operator_use.origin,
        ) == Some(operator.symbol);
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

/// Rederive the closed execution child for one exact selected provider row.
///
/// This consumes the selected plan and the checked requirement symbol. The
/// realization-machine string is validated only as an authored-plan join; it
/// is never parsed into compiler identity. A package-authored lookalike name
/// cannot enter the returned closed lane.
pub fn derive_selected_compiler_intrinsic_execution_identity(
    checked: &CheckedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    let Some(selected_realization) =
        selected_compiler_intrinsic_realization(checked, plan, requirement_symbol)?
    else {
        return Ok(None);
    };
    if let SelectedCompilerIntrinsicRealization::PrimitiveFloatBinary(identity) =
        selected_realization
    {
        return Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
            identity,
        )));
    }
    let SelectedCompilerIntrinsicRealization::NamedFloat(realization) = selected_realization else {
        return Ok(Some(
            SelectedCompilerIntrinsicExecutionIdentity::Unsupported,
        ));
    };
    if let NamedFloatRealization::Negate(format) = realization {
        return Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
            CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format),
        )));
    }
    if let NamedFloatRealization::Convert(domain) = realization {
        let operators = checked
            .typed
            .operators()
            .iter()
            .filter(|operator| operator.symbol == requirement_symbol)
            .collect::<Vec<_>>();
        let [operator] = operators.as_slice() else {
            return Err(Diagnostic::error(format!(
                "selected ProviderPlan `{}` resolves conversion requirement symbol {:?} to {} operator declarations",
                plan.name,
                requirement_symbol,
                operators.len(),
            )));
        };
        return named_float_conversion_execution_identity(checked, operator, domain)
            .map(SelectedCompilerIntrinsicExecutionIdentity::Closed)
            .map(Some);
    }
    let function = named_float_realization_builtin(realization)?;
    let symbol = checked
        .typed
        .symbols
        .builtin_function_symbol(function)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "compiler builtin `{}` is absent while deriving selected provider review identity",
                function.name(),
            ))
        })?;
    if checked.typed.symbols.builtin_function_for_symbol(symbol) != Some(function) {
        return Err(Diagnostic::error(format!(
            "compiler builtin `{}` does not round-trip through its exact root slot",
            function.name(),
        )));
    }
    Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function),
    )))
}

fn named_float_conversion_execution_identity(
    checked: &CheckedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
    domain: ArithmeticDomain,
) -> Result<CompilerIntrinsicExecutionIdentity, Diagnostic> {
    let [value] = checked.typed.operator_parameters(operator) else {
        return Err(Diagnostic::error(
            "selected named-float conversion must retain exactly one parameter",
        ));
    };
    let source = checked
        .typed
        .primitive_type_reference(value.type_reference)
        .and_then(CompilerNumericType::from_primitive)
        .ok_or_else(|| {
            Diagnostic::error("selected named-float conversion has no closed numeric source type")
        })?;
    let target = checked
        .typed
        .primitive_type_reference(operator.return_type)
        .and_then(CompilerNumericType::from_primitive)
        .ok_or_else(|| {
            Diagnostic::error("selected named-float conversion has no closed numeric target type")
        })?;
    if !source.is_float() && !target.is_float() {
        return Err(Diagnostic::error(
            "selected named-float conversion does not cross a floating-point type",
        ));
    }
    Ok(CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
        source,
        target,
        domain,
    })
}

fn selected_compiler_intrinsic_realization(
    checked: &CheckedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<Option<SelectedCompilerIntrinsicRealization>, Diagnostic> {
    let operators = checked
        .typed
        .operators()
        .iter()
        .filter(|operator| operator.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [operator] = operators.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` resolves requirement symbol {:?} to {} operator declarations",
            plan.name,
            requirement_symbol,
            operators.len(),
        )));
    };
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` compiler intrinsic does not realize a boundary operator",
            plan.name,
        )));
    }
    let overload_identity =
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if overload_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` compiler intrinsic has an empty canonical overload identity",
            plan.name,
        )));
    }
    let [method] = plan.schema.methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` must retain exactly one schema method",
            plan.name,
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` must retain exactly one realization row",
            plan.name,
        )));
    };
    if plan.schema.trait_name != overload_identity
        || method.name != "realize"
        || method.requirement_owner != overload_identity
        || method.requirement_identity != overload_identity
        || row.requirement_identity != overload_identity
        || !plan.schema.row_binds_method(row, method)
    {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` does not bind exact overload `{overload_identity}`",
            plan.name,
        )));
    }
    let ProviderBinding::CompilerIntrinsic { machine } = &row.binding else {
        return Ok(None);
    };
    if !provider_planning::plans::intrinsic_realization_matches_operator(
        &checked.typed,
        machine,
        operator,
    ) {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact overload `{overload_identity}` as an external leaf",
            plan.name,
        )));
    }
    provider_planning::plans::compiler_intrinsic_diagnostic_label(&checked.typed, operator)
        .ok_or_else(|| {
        Diagnostic::error(format!(
            "selected overload `{overload_identity}` has no compiler-known intrinsic realization",
        ))
    })?;
    if let Some(identity) =
        provider_planning::plans::primitive_float_binary_intrinsic_execution_identity(
            &checked.typed,
            operator,
        )
    {
        return Ok(Some(
            SelectedCompilerIntrinsicRealization::PrimitiveFloatBinary(identity),
        ));
    }
    Ok(Some(
        named_float_realization_from_operator(&checked.typed, operator)
            .map(SelectedCompilerIntrinsicRealization::NamedFloat)
            .unwrap_or(SelectedCompilerIntrinsicRealization::OtherCompilerPath),
    ))
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
    operator: &typed_trees::operator::OperatorDefinition,
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
    typed: &typed_trees::TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
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
        boundary operator F32::negate(value: f32) -> f32;
        boundary operator F32::from_f64(value: f64) -> f32;
        data I32 {}
        boundary operator I32::from_f64(value: f64) -> i32 in Saturating;

        data FloatProvider {}
        machine FloatProvider::minimum(left: f32, right: f32) -> f32
        satisfies F32::minimum
        via Binding::CompilerIntrinsic;
        machine FloatProvider::maximum(left: f32, right: f32) -> f32
        satisfies F32::maximum
        via Binding::CompilerIntrinsic;
        machine FloatProvider::negate(value: f32) -> f32
        satisfies F32::negate
        via Binding::CompilerIntrinsic;
        machine FloatProvider::from_f64(value: f64) -> f32
        satisfies F32::from_f64
        via Binding::CompilerIntrinsic;
        machine FloatProvider::from_f64_saturating(value: f64) -> i32 in Saturating
        satisfies I32::from_f64
        via Binding::CompilerIntrinsic;

        machine run() -> f32 {
            transition { _ -> (F32::minimum(1.0f32, 2.0f32)) }
        }
    "#;

    struct Fixture {
        checked: CheckedTrees,
        minimum_plan: effects::provider_plan::ProviderPlan,
        maximum_plan: effects::provider_plan::ProviderPlan,
        negate_plan: effects::provider_plan::ProviderPlan,
        conversion_plan: effects::provider_plan::ProviderPlan,
        saturating_conversion_plan: effects::provider_plan::ProviderPlan,
        operator_use: checked_trees::CheckedNamedOperatorUseFact,
    }

    fn fixture() -> Fixture {
        let tokens = source_files_to_tokens::Lexer::new(SOURCE)
            .tokenize()
            .expect("tokenize named-float dispatch fixture");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse named-float dispatch fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve named-float dispatch fixture");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type named-float dispatch fixture");
        let plans = provider_planning::plans::derive_satisfies_plans(&typed, None);
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
        let negate_plan = plans
            .iter()
            .find(|plan| plan.schema.trait_name.contains("F32::negate"))
            .expect("F32::negate provider plan")
            .clone();
        let conversion_plan = plans
            .iter()
            .find(|plan| plan.schema.trait_name.contains("F32::from_f64"))
            .expect("F32::from_f64 provider plan")
            .clone();
        let saturating_conversion_plan = plans
            .iter()
            .find(|plan| plan.schema.trait_name.contains("I32::from_f64"))
            .expect("I32::from_f64 provider plan")
            .clone();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
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
            negate_plan,
            conversion_plan,
            saturating_conversion_plan,
            operator_use,
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Drift {
        None,
        UnknownPlan,
        MissingCommitment,
        WrongCommitment,
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
            (
                Drift::UnknownPlan,
                Some("unknown ProviderPlan report fingerprint"),
            ),
            (
                Drift::MissingCommitment,
                Some("without an exact commitment"),
            ),
            (
                Drift::WrongCommitment,
                Some("exact commitment that does not match"),
            ),
            (Drift::DuplicatePlan, Some("match 2 selected plans")),
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
                Drift::UnknownPlan => {
                    fixture.operator_use.provider_plan_report_fingerprint = u64::MAX
                }
                Drift::MissingCommitment => {}
                Drift::WrongCommitment => {}
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
                    call.arguments = arena::HandleSpan::empty();
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
                    call.receiver = typed_trees::expression::ExpressionHandle::invalid();
                    call.target = typed_trees::name::Identifier::generated("min");
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
                        machine_identity: "FloatProvider::minimum".into(),
                        machine_package_identity: None,
                    };
                    plans = vec![plan.clone()];
                }
            }
            if !matches!(drift, Drift::UnknownPlan) {
                fixture.operator_use.provider_plan_report_fingerprint = plan.report_fingerprint();
            }
            fixture.operator_use.provider_plan_commitment =
                if matches!(drift, Drift::MissingCommitment) {
                    checked_trees::CheckedProviderPlanCommitment::default()
                } else {
                    checked_trees::CheckedProviderPlanCommitment::from_digest(
                        if matches!(drift, Drift::WrongCommitment) {
                            [0xa5; 32]
                        } else {
                            *plan.identity_digest().as_bytes()
                        },
                    )
                };

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

    #[test]
    fn review_identity_uses_exact_operator_and_builtin_root_slot() {
        let fixture = fixture();
        assert_eq!(
            derive_selected_compiler_intrinsic_execution_identity(
                &fixture.checked,
                &fixture.minimum_plan,
                fixture.operator_use.selected_operator_symbol,
            )
            .expect("exact selected minimum must rederive")
            .expect("minimum is a compiler intrinsic"),
            SelectedCompilerIntrinsicExecutionIdentity::Closed(
                CompilerIntrinsicExecutionIdentity::BuiltinFunction(BuiltinFunction::Min),
            ),
        );

        let mut spoofed = fixture.minimum_plan.clone();
        spoofed.rows[0].binding = ProviderBinding::CompilerIntrinsic {
            machine: "F32::maximum.f32".into(),
        };
        let diagnostic = derive_selected_compiler_intrinsic_execution_identity(
            &fixture.checked,
            &spoofed,
            fixture.operator_use.selected_operator_symbol,
        )
        .expect_err("a cross-operator realization string must not spoof builtin identity");
        assert!(
            diagnostic
                .message
                .contains("does not satisfy exact overload")
        );

        let mut textual_negation_spoof = fixture.negate_plan.clone();
        textual_negation_spoof.rows[0].binding = ProviderBinding::CompilerIntrinsic {
            machine: "NamedFloatNegation::f32".into(),
        };
        let negate_symbol = fixture
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
                    .eq(["F32", "negate"])
            })
            .expect("F32::negate operator")
            .symbol;
        let diagnostic = derive_selected_compiler_intrinsic_execution_identity(
            &fixture.checked,
            &textual_negation_spoof,
            negate_symbol,
        )
        .expect_err("an authored lookalike name cannot mint compiler negation identity");
        assert!(
            diagnostic
                .message
                .contains("does not satisfy exact overload")
        );

        assert_eq!(
            derive_selected_compiler_intrinsic_execution_identity(
                &fixture.checked,
                &fixture.negate_plan,
                negate_symbol,
            )
            .expect("exact selected negation must rederive")
            .expect("negation is a compiler intrinsic"),
            SelectedCompilerIntrinsicExecutionIdentity::Closed(
                CompilerIntrinsicExecutionIdentity::NamedFloatNegation(FloatFormat::F32),
            ),
        );

        let conversion_symbol = fixture
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
                    .eq(["F32", "from_f64"])
            })
            .expect("F32::from_f64 operator")
            .symbol;
        assert_eq!(
            derive_selected_compiler_intrinsic_execution_identity(
                &fixture.checked,
                &fixture.conversion_plan,
                conversion_symbol,
            )
            .expect("exact selected conversion must rederive")
            .expect("conversion is a compiler intrinsic"),
            SelectedCompilerIntrinsicExecutionIdentity::Closed(
                CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                    source: CompilerNumericType::F64,
                    target: CompilerNumericType::F32,
                    domain: ArithmeticDomain::Exact,
                },
            ),
        );

        let saturating_conversion_symbol = fixture
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
                    .eq(["I32", "from_f64"])
            })
            .expect("I32::from_f64 operator")
            .symbol;
        assert_eq!(
            derive_selected_compiler_intrinsic_execution_identity(
                &fixture.checked,
                &fixture.saturating_conversion_plan,
                saturating_conversion_symbol,
            )
            .expect("exact selected saturating conversion must rederive")
            .expect("saturating conversion is a compiler intrinsic"),
            SelectedCompilerIntrinsicExecutionIdentity::Closed(
                CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                    source: CompilerNumericType::F64,
                    target: CompilerNumericType::I32,
                    domain: ArithmeticDomain::Saturating,
                },
            ),
        );
    }

    fn selected_fixture() -> (
        Fixture,
        effects::SelectedProviderPlanFacts,
        arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
        checked_trees::CheckedNamedOperatorUseFact,
    ) {
        let mut fixture = fixture();
        let selected = effects::SelectedProviderPlanFacts::from_selection(
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
        retained.provider_plan_report_fingerprint = fixture.minimum_plan.report_fingerprint();
        retained.provider_plan_commitment =
            checked_trees::CheckedProviderPlanCommitment::from_digest(
                *fixture.minimum_plan.identity_digest().as_bytes(),
            );
        *fixture.checked.facts.operators.named_uses.get_mut(handle) = retained;
        (fixture, selected, handle, retained)
    }

    #[test]
    fn shared_success_clones_only_after_complete_preflight() {
        let (fixture, selected, handle, retained) = selected_fixture();
        let original_contents = fixture.checked.clone();
        let original = Arc::new(fixture.checked);
        let mut settled = Arc::clone(&original);

        settle_selected_float_intrinsic_dispatch(&mut settled, &selected)
            .expect("exact selected intrinsic rewrites");

        assert!(
            !Arc::ptr_eq(&settled, &original),
            "a shared successful settlement must publish through a fresh Arc"
        );
        assert_eq!(
            original.as_ref(),
            &original_contents,
            "successful settlement must not mutate retained shared custody"
        );
        let ExpressionNode::Call(rewritten) = settled
            .typed
            .expression_table
            .expression(retained.expression)
        else {
            panic!("rewritten intrinsic is not a call");
        };
        assert_eq!(rewritten.target.as_str(), "min");
        assert_eq!(
            rewritten.target_symbol,
            settled
                .typed
                .symbols
                .builtin_function_symbol(BuiltinFunction::Min)
                .expect("min builtin symbol"),
        );
        assert_eq!(
            settled.facts.operators.named_uses.get(handle),
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
    fn negate_publication_appends_only_after_shared_arc_custody_separates() {
        let (fixture, _, handle, retained) = selected_fixture();
        let expression_count = fixture.checked.typed.expression_table.expression_count();
        let original_contents = fixture.checked.clone();
        let original = Arc::new(fixture.checked);
        let mut settled = Arc::clone(&original);

        apply_selected_float_intrinsic_rewrites(
            Arc::make_mut(&mut settled),
            vec![StagedNamedFloatRewrite {
                expression: retained.expression,
                realization: NamedFloatRealization::Negate(FloatFormat::F32),
                execution: StagedNamedFloatExecution::Negate(FloatFormat::F32),
            }],
            &mut crate::source_edits::SourceEditBuilder::default(),
        );

        assert!(!Arc::ptr_eq(&settled, &original));
        assert_eq!(original.as_ref(), &original_contents);
        assert_eq!(
            settled.typed.expression_table.expression_count(),
            expression_count + 1,
            "negate must append exactly one landed -1 literal"
        );
        let ExpressionNode::Binary(binary) = settled
            .typed
            .expression_table
            .expression(retained.expression)
        else {
            panic!("negate publication must replace the selected root with multiplication");
        };
        assert_eq!(binary.operator, BinaryOperator::Multiply);
        assert_eq!(
            binary.right.arena_index() as usize,
            expression_count + 1,
            "the landed -1 literal must retain exact append order"
        );
        let ExpressionNode::Float(negative_one) =
            settled.typed.expression_table.expression(binary.right)
        else {
            panic!("negate publication must append a float literal");
        };
        assert_eq!(negative_one.text(), "-1.0");
        assert_eq!(negative_one.landing(), Some(FloatFormat::F32));
        assert_eq!(
            settled.facts.operators.named_uses.get(handle),
            &retained,
            "arena publication must not change exact checked evidence"
        );
    }

    #[test]
    fn shared_rejection_preserves_arc_identity_and_complete_contents() {
        let (mut fixture, selected, _, retained) = selected_fixture();
        let mut invalid = retained;
        invalid.provider_plan_report_fingerprint = u64::MAX;
        fixture.checked.facts.operators.named_uses.append(invalid);
        let before = fixture.checked.clone();
        let original = Arc::new(fixture.checked);
        let mut rejected = Arc::clone(&original);

        let diagnostics = settle_selected_float_intrinsic_dispatch(&mut rejected, &selected)
            .expect_err("one invalid intrinsic use rejects the complete rewrite batch");
        assert!(
            diagnostics[0]
                .message
                .contains("unknown ProviderPlan report fingerprint")
        );
        assert_eq!(
            rejected.as_ref(),
            &before,
            "validation failure must not publish any staged rewrite",
        );
        assert!(
            Arc::ptr_eq(&rejected, &original),
            "rejection must preserve exact shared program custody"
        );
    }

    #[test]
    fn empty_settlement_preserves_shared_arc_identity_and_contents() {
        let fixture = fixture();
        let original_contents = fixture.checked.clone();
        let original = Arc::new(fixture.checked);
        let mut settled = Arc::clone(&original);

        settle_selected_float_intrinsic_dispatch(
            &mut settled,
            &effects::SelectedProviderPlanFacts::default(),
        )
        .expect("a program without selected float intrinsics is already settled");

        assert!(Arc::ptr_eq(&settled, &original));
        assert_eq!(settled.as_ref(), &original_contents);
    }
}
