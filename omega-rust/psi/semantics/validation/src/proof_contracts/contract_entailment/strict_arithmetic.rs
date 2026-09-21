//! Strict arithmetic symbol and expression bindings and their implication
//! judgments.

use crate::proof_contracts::contract_entailment::arithmetic_judgment::{Engine, Judgment};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;

/// One exact symbol substitution admitted by the strict arithmetic
/// implication adapter. Call-site entailment and quotient correspondence
/// supply exact parameter/theorem atoms or integer constants; display
/// spellings never select a binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrictArithmeticSymbolBinding {
    pub symbol: symbols::SymbolHandle,
    pub value: StrictArithmeticBindingValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictArithmeticBindingValue {
    Atom { identity: String, unsigned: bool },
    Integer(numerics::bignum::BigInt),
}

/// Bind a callee formal to a mathematical argument in the caller's original
/// symbol namespace. The caller must establish immutable operands and exact
/// builtin arithmetic meaning; this query does not establish executable
/// arithmetic formation or replace its overflow obligations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrictArithmeticExpressionBinding {
    pub symbol: symbols::SymbolHandle,
    pub expression: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrictArithmeticImplicationJudgment {
    Proven,
    Refuted,
    Unknown,
}

/// Judge one instantiated integer-expression goal from an exact authored
/// hypothesis roster. This authority-bearing adapter is stricter than the
/// ordinary contract validator: every hypothesis and the goal must be inside
/// the arithmetic engine's language, every name must resolve through a
/// symbol-keyed binding, and only `Proven` succeeds. In particular, the
/// validator's sound stand-down behavior is `Unknown` here, never acceptance.
pub fn strict_arithmetic_expression_implication(
    program: &TypedTrees,
    context_machine: &Machine,
    hypotheses: &[ExpressionHandle],
    goal: ExpressionHandle,
    bindings: &[StrictArithmeticSymbolBinding],
) -> StrictArithmeticImplicationJudgment {
    strict_arithmetic_expression_implication_with_arguments(
        program,
        context_machine,
        hypotheses,
        goal,
        bindings,
        &[],
    )
}

/// Instantiate all arguments simultaneously in the original symbol namespace
/// before judging the goal. Unknown arguments and conflicting formal bindings
/// reject even when the hypotheses are inconsistent or the goal is constant.
/// Argument order cannot give a later argument access to an earlier callee
/// formal. This supplies mathematical substitution, not executable value custody.
pub fn strict_arithmetic_expression_implication_with_arguments(
    program: &TypedTrees,
    context_machine: &Machine,
    hypotheses: &[ExpressionHandle],
    goal: ExpressionHandle,
    bindings: &[StrictArithmeticSymbolBinding],
    arguments: &[StrictArithmeticExpressionBinding],
) -> StrictArithmeticImplicationJudgment {
    let mut engine = Engine::strict_with_symbol_bindings(program, context_machine, bindings);
    if !engine.strict_symbol_bindings_are_valid() || !engine.bind_exact_arguments(arguments) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    let mut comparisons = Vec::new();
    if !engine.collect_comparisons(hypotheses, &mut comparisons)
        || !engine.install_hypotheses(comparisons)
    {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    if engine.requires_unsatisfiable {
        return StrictArithmeticImplicationJudgment::Proven;
    }
    match engine.judge(goal) {
        Judgment::Proven => StrictArithmeticImplicationJudgment::Proven,
        Judgment::ConstantFalse | Judgment::Refuted => StrictArithmeticImplicationJudgment::Refuted,
        Judgment::Unknown { .. } => StrictArithmeticImplicationJudgment::Unknown,
    }
}
