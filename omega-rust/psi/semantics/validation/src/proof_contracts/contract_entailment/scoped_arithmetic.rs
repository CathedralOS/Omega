//! Arithmetic implication under per-proposition simultaneous substitutions.
//!
//! A cyclic header owes one obligation per arrival, and every arrival reads
//! the same predicate under its own parameter-to-argument substitution. The
//! source spells an invocation formal and its current header binding with one
//! parameter symbol, so a proposition relating the two cannot be a single
//! expression read under one symbol table. This adapter lets each hypothesis
//! and the goal carry its own roster: a parameter symbol, an exact field
//! projection occurrence, or the synthetic guarantee `result`, denotes a
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
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
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
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopedArithmeticValue {
    /// A private mathematical unknown. The identity is shared across every
    /// proposition of one implication; an unsigned atom carries `>= 0`.
    Atom { identity: String, unsigned: bool },
    /// A term read under its own roster before it stands for the binder.
    Term(ScopedArithmeticExpression),
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
    let mut comparisons = Vec::new();
    for hypothesis in hypotheses {
        let Some(mut read) = read_comparisons(&mut engine, &hypothesis.proposition) else {
            return StrictArithmeticImplicationJudgment::Unknown;
        };
        if !hypothesis.holds {
            let [(operator, left, right)] = read.as_slice() else {
                return StrictArithmeticImplicationJudgment::Unknown;
            };
            let Some(negated) = negated_comparison(*operator) else {
                return StrictArithmeticImplicationJudgment::Unknown;
            };
            read = vec![(negated, left.clone(), right.clone())];
        }
        comparisons.extend(read);
    }
    if !engine.install_hypotheses(comparisons) {
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
/// language is dropped: a weaker asserted hypothesis is sound, and the caller
/// decides whether a denied one may lose structure.
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
