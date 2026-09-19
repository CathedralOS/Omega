//! F7 named-float ProviderPlan execution bridge.
//!
//! Checking retains the source operator identity and the exact selected plan
//! on each named use. Execution may then redirect only a compiler-known
//! realization to either an existing builtin or an exact primitive expression.
//! The source expression handle and fact remain unchanged, so proof,
//! result-policy evidence, and diagnostics continue to name the boundary
//! requirement rather than the bootstrap execution form.
//!
//! This file owns the realization vocabulary and the settle, plan and apply
//! entry points. `fma_unit_applications.rs` selects and validates IEEE fused
//! multiply-add unit applications, `intrinsic_resolution.rs` resolves
//! float intrinsic calls, `execution_identities.rs` derives execution
//! identities and realizations, `named_float_realizations.rs` realizes
//! named and directed float builtins and `tests.rs` holds the dispatch
//! tests.

mod execution_identities;
mod fma_unit_applications;
mod intrinsic_resolution;
mod named_float_realizations;
#[cfg(test)]
mod requirement_view_tests;
#[cfg(test)]
mod tests;

pub use execution_identities::{
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution,
};
pub(crate) use fma_unit_applications::{
    selected_ieee_float_fma_unit_applications, validate_selected_ieee_float_fma_unit_applications,
};

use crate::selected_dispatch::float_intrinsic::intrinsic_resolution::{
    SelectedIntrinsicUse, resolve_selected_float_intrinsic_call,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::RoundingDirection;
use numerics::literals::{FloatFormat, FloatLiteral};
use provider_planning::CompilerIntrinsicExecutionIdentity;
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
    PrimitiveIntegerComparison(CompilerIntrinsicExecutionIdentity),
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
    origin: checked_trees::CheckedValueOrigin,
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

    let selected_uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(_, operator_use)| SelectedIntrinsicUse::from(operator_use))
        .chain(
            checked
                .facts
                .operators
                .named_requirement_uses
                .iter()
                .map(|(_, requirement_use)| SelectedIntrinsicUse::from(requirement_use)),
        )
        .collect::<Vec<_>>();
    for selected_use in &selected_uses {
        if selected_use.provider_plan_report_fingerprint == 0
            && selected_use.provider_plan_commitment.is_empty()
        {
            continue;
        }
        let operator_use = selected_use;
        let rewrite = match resolve_selected_float_intrinsic_call(
            checked,
            selected_provider_plans.plans(),
            selected_use,
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
    source_edits: &mut crate::source_edits::SourceEditBuilder,
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
                result_type: typed_trees::types::TypeReferenceHandle::invalid(),
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
        retire_rewritten_call_row(checked, rewrite.origin, rewrite.expression);
    }
}

/// The flow call row captured for the authored call no longer names a call
/// the settled body makes; retire it so execution planning skips the row
/// while the certificates holding its handle stay valid.
fn retire_rewritten_call_row(
    checked: &mut CheckedTrees,
    origin: checked_trees::CheckedValueOrigin,
    expression: typed_trees::expression::ExpressionHandle,
) {
    let checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        ..
    } = origin
    else {
        return;
    };
    let statement_root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| {
            checked
                .typed
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)
                .map(|state| state.statement_nodes)
        })
        .and_then(|statements| {
            checked
                .typed
                .statement_table
                .statements(statements)
                .get(statement_index)
        })
        .is_some_and(|statement| {
            matches!(
                statement,
                typed_trees::statement::StatementNode::Expression(root) if *root == expression
            )
        });
    checked.facts.flow.control.retire_calls(
        machine_symbol,
        state_symbol,
        statement_index,
        expression,
        statement_root,
    );
}
