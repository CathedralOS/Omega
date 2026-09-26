//! F7 named-float ProviderPlan realizations.
//!
//! Checking retains the source operator identity and the exact selected plan
//! on each named use, and execution keeps naming the boundary requirement:
//! Omega installs the compiler-known realization. A named use bound to one
//! is found here only so its missing requirement-level route reports
//! `unimplemented:`; the execution identities serve build-time folding and
//! Terminal coverage.
//!
//! `intrinsic_resolution.rs` resolves float intrinsic calls,
//! `execution_identities.rs` derives execution identities and realizations,
//! and `named_float_realizations.rs` realizes named and directed float
//! builtins.

mod execution_identities;
mod intrinsic_resolution;
mod named_float_realizations;

pub use execution_identities::{
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution,
};

use crate::selected_dispatch::float_intrinsic::intrinsic_resolution::{
    SelectedIntrinsicUse, resolve_selected_float_intrinsic_call,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::RoundingDirection;
use numerics::literals::FloatFormat;
use provider_planning::CompilerIntrinsicExecutionIdentity;
use symbols::BuiltinFunction;

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

/// One named float use whose selected row is a compiler-known realization,
/// keyed by the requirement it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StagedNamedFloatRewrite {
    pub(super) expression: typed_trees::expression::ExpressionHandle,
    pub(super) origin: checked_trees::CheckedValueOrigin,
    pub(super) requirement: symbols::SymbolHandle,
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
        if !rewrites
            .iter()
            .any(|existing: &StagedNamedFloatRewrite| existing.expression == rewrite.expression)
        {
            rewrites.push(rewrite);
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(rewrites)
}
