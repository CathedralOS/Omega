//! Arithmetic implication under per-proposition simultaneous substitutions.
//!
//! A cyclic header owes one obligation per arrival, and every arrival reads
//! the same predicate under its own parameter-to-argument substitution. The
//! source spells an invocation formal and its current header binding with one
//! parameter symbol, so a proposition relating the two cannot be a single
//! expression read under one symbol table. This adapter lets each hypothesis
//! and the goal carry its own roster: a parameter symbol, an exact field
//! projection or reserved domain `self` occurrence, or the synthetic guarantee
//! `result`, denotes a
//! private atom or a term that is itself read under a nested roster. Atoms are
//! shared by identity across the rosters of one implication, so the same
//! formal named in two propositions is one
//! mathematical value. Like the strict adapter this is authority-bearing:
//! every name and projection must resolve through its roster, and only
//! `Proven` succeeds. Projection meaning is established by the caller; this
//! adapter never equates member occurrences by their spelling or receiver.

use super::arithmetic_judgment::{Engine, Judgment, Polynomial};
use super::inductive_judgment::negated_comparison;
use super::strict_arithmetic::StrictArithmeticImplicationJudgment;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

#[cfg(test)]
mod tests;

/// What one scoped binding stands for: a resolved binder symbol, an exact
/// member occurrence, or the synthetic `result` name for the returned value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopedArithmeticBinder {
    Symbol(symbols::SymbolHandle),
    /// One exact member-expression occurrence, not an arbitrary term rewrite.
    Projection(ExpressionHandle),
    /// One exact unresolved reserved `self` occurrence in a domain predicate.
    /// The caller must rejoin this occurrence to its owning declaration and
    /// establish the original operator and carrier meaning. This binding only
    /// supplies mathematical substitution; it does not confer that ownership.
    DomainSelf(ExpressionHandle),
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopedArithmeticValue {
    /// A private mathematical unknown. The identity is shared across every
    /// proposition of one implication; an unsigned atom carries `>= 0`.
    Atom { identity: String, unsigned: bool },
    /// An exact mathematical integer supplied by the binding's owner, such as
    /// a checked closed domain index. This does not establish executable
    /// carrier landing or the constant's source custody.
    Integer(numerics::bignum::BigInt),
    /// Mathematical addition, subtraction or multiplication of already
    /// admitted integer values. The caller establishes exact builtin meaning;
    /// this node cannot reinterpret wrapping or trapping source arithmetic.
    IntegerOperation {
        operator: BinaryOperator,
        left: Box<Self>,
        right: Box<Self>,
    },
    /// A term read under its own roster before it stands for the binder.
    Term(ScopedArithmeticExpression),
    /// One exact integer remainder whose caller independently established
    /// selected builtin meaning, fixed-carrier formation and source custody.
    /// This marker grants only the existing mathematical remainder
    /// normalization; positivity and unsigned source bounds remain hypotheses.
    AdmittedIntegerRemainder(ScopedArithmeticExpression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedArithmeticBinding {
    pub binder: ScopedArithmeticBinder,
    pub value: ScopedArithmeticValue,
}

/// One authored expression together with the simultaneous substitution it is
/// read under. A name or projection the roster does not bind is outside the
/// language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedArithmeticExpression {
    pub expression: ExpressionHandle,
    pub bindings: Vec<ScopedArithmeticBinding>,
}

/// A hypothesis proposition, asserted or denied. A denied hypothesis must be
/// one comparison; the denial of a conjunction is a disjunction the engine
/// does not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedArithmeticHypothesis {
    pub proposition: ScopedArithmeticExpression,
    pub holds: bool,
}

/// An integer comparison supplied by the owner of live value facts, without
/// manufacturing an authored expression for an implicit bound or condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedArithmeticComparison {
    pub operator: BinaryOperator,
    pub left: ScopedArithmeticValue,
    pub right: ScopedArithmeticValue,
}

/// Decide whether these simultaneous integer comparisons are contradictory.
/// The caller establishes integer denotation, operand formation and current
/// observation custody. Shared atom identities mean the same mathematical
/// value; no source spelling or previously invalidated fact supplies identity.
/// Unknown terms/operators decline even when another comparison is impossible.
pub fn scoped_arithmetic_comparisons_unsatisfiable(
    program: &TypedTrees,
    context_machine: &Machine,
    comparisons: &[ScopedArithmeticComparison],
) -> bool {
    let mut engine = Engine::strict_with_symbol_bindings(program, context_machine, &[]);
    let mut normalized = Vec::with_capacity(comparisons.len());
    for comparison in comparisons {
        if !matches!(
            comparison.operator,
            BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Equal
                | BinaryOperator::NotEqual
        ) {
            return false;
        }
        let Some(left) = engine.normalize_scoped_value(&comparison.left) else {
            return false;
        };
        let Some(right) = engine.normalize_scoped_value(&comparison.right) else {
            return false;
        };
        normalized.push((comparison.operator, left, right));
    }
    engine.integer_comparisons_unsatisfiable(normalized)
}

/// Judge `goal` from `hypotheses`, each read under its own roster. An
/// asserted hypothesis contributes the conjuncts the engine can read; a
/// hypothesis or goal whose roster fails to install answers `Unknown`.
pub fn scoped_arithmetic_implication(
    program: &TypedTrees,
    context_machine: &Machine,
    hypotheses: &[ScopedArithmeticHypothesis],
    goal: &ScopedArithmeticExpression,
) -> StrictArithmeticImplicationJudgment {
    let mut engine = Engine::strict_with_symbol_bindings(program, context_machine, &[]);
    // Declare the goal's atoms first so their sign facts seed with the
    // hypotheses; the goal roster is installed again before judging.
    if !engine.install_scoped_bindings(&goal.bindings) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    if !install_hypotheses(program, &mut engine, hypotheses) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    if engine.requires_unsatisfiable {
        return StrictArithmeticImplicationJudgment::Proven;
    }
    if !engine.install_scoped_bindings(&goal.bindings) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    match engine.judge(goal.expression) {
        Judgment::Proven => StrictArithmeticImplicationJudgment::Proven,
        Judgment::ConstantFalse | Judgment::Refuted => StrictArithmeticImplicationJudgment::Refuted,
        Judgment::Unknown { .. } => StrictArithmeticImplicationJudgment::Unknown,
    }
}

/// Read every conjunct of one scoped proposition. A conjunct outside the
/// language is dropped: a weaker asserted hypothesis is sound. The caller
/// admits denied hypotheses only when their authored root is one comparison,
/// and requires exactly one successfully read comparison before negating it.
fn read_comparisons(
    engine: &mut Engine<'_>,
    proposition: &ScopedArithmeticExpression,
) -> Option<Vec<(BinaryOperator, Polynomial, Polynomial)>> {
    if !engine.install_scoped_bindings(&proposition.bindings) {
        return None;
    }
    let mut comparisons = Vec::new();
    engine.collect_comparisons(&[proposition.expression], &mut comparisons);
    Some(comparisons)
}

fn install_hypotheses(
    program: &TypedTrees,
    engine: &mut Engine<'_>,
    hypotheses: &[ScopedArithmeticHypothesis],
) -> bool {
    let mut comparisons = Vec::new();
    for hypothesis in hypotheses {
        // Weakening an asserted conjunction is sound; negating that weakened
        // conjunction is not. Inspect the authored shape before flattening so
        // an unread conjunct cannot turn !(A && B) into !A.
        if !hypothesis.holds
            && !matches!(
                program.expression_table.expression(hypothesis.proposition.expression),
                ExpressionNode::Binary(binary) if negated_comparison(binary.operator).is_some()
            )
        {
            return false;
        }
        let Some(mut read) = read_comparisons(engine, &hypothesis.proposition) else {
            return false;
        };
        if !hypothesis.holds {
            let [(operator, left, right)] = read.as_slice() else {
                return false;
            };
            let Some(negated) = negated_comparison(*operator) else {
                return false;
            };
            read = vec![(negated, left.clone(), right.clone())];
        }
        comparisons.extend(read);
    }
    if !engine.install_hypotheses(comparisons) {
        return false;
    }
    true
}

/// Compare independent scoped values through the existing integer judgment.
/// The caller retains exact source carrier, policy and observation custody.
pub fn scoped_arithmetic_values_relation(
    program: &TypedTrees,
    context_machine: &Machine,
    hypotheses: &[ScopedArithmeticHypothesis],
    operator: BinaryOperator,
    left: &ScopedArithmeticValue,
    right: &ScopedArithmeticValue,
) -> StrictArithmeticImplicationJudgment {
    use numerics::bignum::BigInt;
    // An inconsistent premise set entails propositions, but does not turn an
    // arithmetic or Boolean operator into a comparison judgment.
    if !matches!(
        operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Equal
    ) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    let mut engine = Engine::strict_with_symbol_bindings(program, context_machine, &[]);
    let Some(left) = engine.normalize_scoped_value(left) else {
        return StrictArithmeticImplicationJudgment::Unknown;
    };
    let Some(right) = engine.normalize_scoped_value(right) else {
        return StrictArithmeticImplicationJudgment::Unknown;
    };
    if !install_hypotheses(program, &mut engine, hypotheses) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    let left = engine.substituted(&left);
    let right = engine.substituted(&right);
    let zero = BigInt::from_i64(0);
    let one = BigInt::from_i64(1);
    let proven = engine.requires_unsatisfiable
        || match operator {
            BinaryOperator::Less => engine.prove_at_least(&right.sub(&left), &one),
            BinaryOperator::LessOrEqual => engine.prove_at_least(&right.sub(&left), &zero),
            BinaryOperator::Greater => engine.prove_at_least(&left.sub(&right), &one),
            BinaryOperator::GreaterOrEqual => engine.prove_at_least(&left.sub(&right), &zero),
            BinaryOperator::Equal => {
                engine.prove_at_least(&right.sub(&left), &zero)
                    && engine.prove_at_least(&left.sub(&right), &zero)
            }
            _ => false,
        };
    if proven {
        StrictArithmeticImplicationJudgment::Proven
    } else {
        StrictArithmeticImplicationJudgment::Unknown
    }
}
