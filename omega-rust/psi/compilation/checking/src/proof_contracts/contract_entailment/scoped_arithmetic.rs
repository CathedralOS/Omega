//! Arithmetic implication under per-proposition simultaneous substitutions.
//!
//! A cyclic header owes one obligation per arrival, and every arrival reads
//! the same predicate under its own parameter-to-argument substitution. The
//! source spells an invocation formal and its current header binding with one
//! parameter symbol, so a proposition relating the two cannot be a single
//! expression read under one symbol table. This adapter lets each hypothesis
//! and the goal carry its own roster: a parameter symbol, an exact value
//! occurrence, or the synthetic guarantee
//! `result`, denotes a
//! private atom or a term that is itself read under a nested roster. Atoms are
//! shared by identity across the rosters of one implication, so the same
//! formal named in two propositions is one
//! mathematical value. Like the strict adapter this is authority-bearing:
//! every name and occurrence must resolve through its roster, and only
//! `Proven` succeeds. Occurrence meaning is established by the caller; this
//! adapter never equates value occurrences by their spelling or operands.

pub(super) mod calls;

use super::arithmetic_judgment::{Engine, Judgment, Polynomial};

use super::strict_arithmetic::StrictArithmeticImplicationJudgment;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

/// What one scoped binding stands for: a resolved binder symbol, an exact
/// value occurrence, or the synthetic `result` name for the returned value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopedArithmeticBinder {
    Symbol(symbols::SymbolHandle),
    /// One exact value occurrence whose denotation the binding owner supplies.
    Occurrence(ExpressionHandle),
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopedArithmeticValue {
    /// A private mathematical unknown. The identity is shared across every
    /// proposition of one implication; an unsigned atom carries `>= 0`.
    Atom { identity: String, unsigned: bool },
    /// Independently established bounds for a value, retained while its scoped
    /// identity is transported. These bounds do not form the source operand.
    BoundedInteger {
        value: Box<Self>,
        minimum: Option<numerics::bignum::BigInt>,
        maximum: Option<numerics::bignum::BigInt>,
    },
    /// An exact mathematical integer supplied by the binding's owner, such as
    /// a checked closed domain index. This does not establish executable
    /// carrier landing or the constant's source custody.
    Integer(numerics::bignum::BigInt),
    /// Mathematical integer arithmetic over already
    /// admitted integer values. The caller establishes exact builtin meaning;
    /// this node cannot reinterpret wrapping or trapping source arithmetic.
    IntegerOperation {
        operator: BinaryOperator,
        left: Box<Self>,
        right: Box<Self>,
    },
    /// A term read under its own roster before it stands for the binder.
    Term(ScopedArithmeticExpression),
    /// The length observation of a captured collection result. Its source
    /// roster supplies actual values, not permission to replay a callee body.
    /// Call preparation binds independently published guarantees to a private
    /// observation identity before the integer engine can consume this value.
    CollectionLength(ScopedArithmeticExpression),
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
/// read under. A name or occurrence the roster does not bind is outside the
/// language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedArithmeticExpression {
    /// The exact source declaration whose types and selected operations this
    /// expression retains. Substituting values does not relocate its meaning.
    pub owner: symbols::SymbolHandle,
    pub expression: ExpressionHandle,
    pub bindings: Vec<ScopedArithmeticBinding>,
}

/// A hypothesis proposition, asserted or denied. The denial of a conjunction
/// is a disjunction the engine does not read, so a denied hypothesis whose
/// authored root is not one negatable comparison carries no fact: the
/// installer drops it and judges the goal from the premises that remain.
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
    program: &dyn crate::CheckingInput,
    context_machine: &Machine,
    comparisons: &[ScopedArithmeticComparison],
) -> bool {
    let mut calls = calls::ScopedCalls::new(program, context_machine);
    let mut engine = Engine::strict(program, context_machine, &[]);
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
        let Some(left) = calls
            .value(&comparison.left)
            .and_then(|value| engine.normalize_scoped_value(&value))
        else {
            return false;
        };
        let Some(right) = calls
            .value(&comparison.right)
            .and_then(|value| engine.normalize_scoped_value(&value))
        else {
            return false;
        };
        normalized.push((comparison.operator, left, right));
    }
    let Some(hypotheses) = calls.hypotheses(&[]) else {
        return false;
    };
    if !install_hypotheses(program, &mut engine, &hypotheses) {
        return false;
    }
    engine.integer_comparisons_unsatisfiable(normalized)
}

/// Judge `goal` from `hypotheses`, each read under its own roster. An
/// asserted hypothesis contributes the conjuncts the engine can read; a
/// hypothesis or goal whose roster fails to install answers `Unknown`.
pub fn scoped_arithmetic_implication(
    program: &dyn crate::CheckingInput,
    context_machine: &Machine,
    hypotheses: &[ScopedArithmeticHypothesis],
    goal: &ScopedArithmeticExpression,
) -> StrictArithmeticImplicationJudgment {
    scoped_arithmetic_implication_for_declaration(program, context_machine.symbol, hypotheses, goal)
}

pub(crate) fn scoped_arithmetic_implication_for_declaration(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    hypotheses: &[ScopedArithmeticHypothesis],
    goal: &ScopedArithmeticExpression,
) -> StrictArithmeticImplicationJudgment {
    let mut calls = calls::ScopedCalls::for_declaration(program, owner);
    let Some(goal) = calls.expression(goal) else {
        return StrictArithmeticImplicationJudgment::Unknown;
    };
    let Some(hypotheses) = calls.hypotheses(hypotheses) else {
        return StrictArithmeticImplicationJudgment::Unknown;
    };
    let mut engine = if let Some(machine) = program.machine(owner) {
        Engine::strict(program, machine, &[])
    } else {
        Engine::strict_for_declaration(program, owner)
    };
    // Declare the goal's atoms first so their sign facts seed with the
    // hypotheses; the goal roster is installed again before judging.
    if !engine.install_scoped_expression(&goal) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    if !install_hypotheses(program, &mut engine, &hypotheses) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    if engine.requires_unsatisfiable {
        return StrictArithmeticImplicationJudgment::Proven;
    }
    if !engine.install_scoped_expression(&goal) {
        return StrictArithmeticImplicationJudgment::Unknown;
    }
    match engine.judge(goal.expression) {
        Judgment::Proven => StrictArithmeticImplicationJudgment::Proven,
        Judgment::ConstantFalse | Judgment::Refuted => StrictArithmeticImplicationJudgment::Refuted,
        Judgment::Unknown { .. } => StrictArithmeticImplicationJudgment::Unknown,
    }
}

/// Read every conjunct of one scoped proposition. A conjunct outside the
/// language is dropped: a weaker asserted hypothesis is sound. A denied
/// hypothesis is negated only after its authored root is known to be one
/// comparison and that comparison read; a roster that fails to install
/// leaves the engine partly bound, which is the one condition that still
/// declines the whole judgment rather than weakening it.
fn read_comparisons(
    engine: &mut Engine<'_>,
    proposition: &ScopedArithmeticExpression,
) -> Option<Vec<(BinaryOperator, Polynomial, Polynomial)>> {
    if !engine.install_scoped_expression(proposition) {
        return None;
    }
    let mut comparisons = Vec::new();
    engine.collect_comparisons(&[proposition.expression], &mut comparisons);
    Some(comparisons)
}

pub(super) fn install_hypotheses(
    program: &dyn crate::CheckingInput,
    engine: &mut Engine<'_>,
    hypotheses: &[ScopedArithmeticHypothesis],
) -> bool {
    let mut comparisons = Vec::new();
    for hypothesis in hypotheses {
        // Weakening an asserted conjunction is sound; negating that weakened
        // conjunction is not. Inspect the authored shape before flattening so
        // an unread conjunct cannot turn !(A && B) into !A. A denial this
        // engine cannot read is not a fact, and dropping one premise leaves a
        // strictly weaker set that proves a subset of what the full set
        // proves. Voiding the whole judgment instead would discard every
        // unrelated premise that arrived with it, so an alternative guarded by
        // a negated conjunction would lose even its own asserted `requires`
        // clauses. The shape is read before this hypothesis installs its
        // roster, so a dropped denial leaves no bindings in the engine.
        if !hypothesis.holds
            && !matches!(
                program.expression_table.expression(hypothesis.proposition.expression),
                ExpressionNode::Binary(binary) if binary.operator.complement().is_some()
            )
        {
            continue;
        }
        let Some(mut read) = read_comparisons(engine, &hypothesis.proposition) else {
            return false;
        };
        if !hypothesis.holds {
            // A root comparison whose operands left the engine's language
            // reads as no comparison, and a denial of several of them is a
            // disjunction. Either way this premise is dropped, exactly as the
            // unreadable root above: its roster is already installed, which
            // is the state every read hypothesis leaves behind.
            let [(operator, left, right)] = read.as_slice() else {
                continue;
            };
            let Some(negated) = operator.complement() else {
                continue;
            };
            read = vec![(negated, left.clone(), right.clone())];
        }
        comparisons.extend(read);
    }
    if !engine.install_hypotheses(comparisons) {
        return false;
    }
    // Captured values are normalized before their occurrence's hypotheses.
    // Revisit their dependent operation bounds now that those facts are live.
    engine.refresh_opaque_intervals();
    true
}

/// Compare independent scoped values through the existing integer judgment.
/// The caller retains exact source carrier, policy and observation custody.
pub fn scoped_arithmetic_values_relation(
    program: &dyn crate::CheckingInput,
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
    let mut calls = calls::ScopedCalls::new(program, context_machine);
    let (Some(left), Some(right)) = (calls.value(left), calls.value(right)) else {
        return StrictArithmeticImplicationJudgment::Unknown;
    };
    let Some(hypotheses) = calls.hypotheses(hypotheses) else {
        return StrictArithmeticImplicationJudgment::Unknown;
    };
    let mut engine = Engine::strict(program, context_machine, &[]);
    let Some(left) = engine.normalize_scoped_value(&left) else {
        return StrictArithmeticImplicationJudgment::Unknown;
    };
    let Some(right) = engine.normalize_scoped_value(&right) else {
        return StrictArithmeticImplicationJudgment::Unknown;
    };
    if !install_hypotheses(program, &mut engine, &hypotheses) {
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

/// Bounds for an independently captured integer value under its exact live
/// hypothesis rosters. Inconsistent premises supply no executable extent.
pub fn scoped_arithmetic_value_bounds(
    program: &dyn crate::CheckingInput,
    context_machine: &Machine,
    hypotheses: &[ScopedArithmeticHypothesis],
    value: &ScopedArithmeticValue,
) -> Option<crate::proof_contracts::arithmetic_domains::BigIntInterval> {
    let mut calls = calls::ScopedCalls::new(program, context_machine);
    let value = calls.value(value)?;
    let hypotheses = calls.hypotheses(hypotheses)?;
    let mut engine = Engine::strict(program, context_machine, &[]);
    let value = engine.normalize_scoped_value(&value)?;
    if !install_hypotheses(program, &mut engine, &hypotheses) || engine.requires_unsatisfiable {
        return None;
    }
    Some(engine.polynomial_interval(&engine.substituted(&value)))
}
