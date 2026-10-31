//! Entry arithmetic over independently captured input values. Storage capture
//! owns selector bounds, access and load identity; this query owns declaration
//! application formation and membership. Desired rank bounds are never premises.

use super::{BigInt, Engine, Machine, Polynomial, RankValue, RankingRangeMeasure};
use crate::frozen_program::expression_table;
use crate::proof_contracts::contract_entailment::scoped_arithmetic::{
    calls::ScopedCalls, install_hypotheses,
};
use crate::{ScopedArithmeticExpression, ScopedArithmeticHypothesis, ScopedArithmeticValue};
use typed_trees::expression::ExpressionHandle;

/// Prepared by the source observation owner after authenticating every input
/// coordinate and its selector/load receipts. `inputs` carries only inherent
/// bounds of those typed values, never the proposed rank or desired interval.
pub struct CapturedRankEntry {
    pub subject: ScopedArithmeticExpression,
    pub paired_subject: Option<ScopedArithmeticExpression>,
    pub range_start: Option<ScopedArithmeticExpression>,
    pub range_end: Option<ScopedArithmeticExpression>,
    pub hypotheses: Vec<ScopedArithmeticHypothesis>,
    pub inputs: Vec<ScopedArithmeticValue>,
}

pub fn prove_captured_ranking_range_entry(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    observed: &CapturedRankEntry,
) -> bool {
    prove(program, machine, range, measure, observed).unwrap_or(false)
}

fn prove(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    observed: &CapturedRankEntry,
) -> Option<bool> {
    let root = program.machine_states(machine).first()?;
    let (subject, paired) = match measure {
        RankingRangeMeasure::Single(subject) | RankingRangeMeasure::Computed { subject, .. } => {
            (subject, None)
        }
        RankingRangeMeasure::Distance { lower, upper } => (lower, Some(upper)),
        RankingRangeMeasure::IncreasingTo { subject, limit } => (subject, Some(limit)),
        RankingRangeMeasure::Field { .. } | RankingRangeMeasure::SliceLength(_) => return None,
    };
    if observed.subject.owner != root.symbol
        || observed.subject.expression != subject
        || observed
            .paired_subject
            .as_ref()
            .map(|value| value.expression)
            != paired
        || observed
            .paired_subject
            .as_ref()
            .is_some_and(|value| value.owner != root.symbol)
    {
        return None;
    }
    let authored_range = if range.is_valid() {
        Some(expression_table(program).range_node(range)?)
    } else {
        None
    };
    if observed.range_start.as_ref().map(|value| value.expression)
        != authored_range.map(|range| range.start)
        || observed.range_end.as_ref().map(|value| value.expression)
            != authored_range.map(|range| range.end)
        || [&observed.range_start, &observed.range_end]
            .into_iter()
            .flatten()
            .any(|value| value.owner != root.symbol)
    {
        return None;
    }
    let mut calls = ScopedCalls::for_declaration(program, machine.symbol);
    let subject = calls.expression(&observed.subject)?;
    let paired = match &observed.paired_subject {
        Some(value) => Some(calls.expression(value)?),
        None => None,
    };
    let start = match &observed.range_start {
        Some(value) => Some(calls.expression(value)?),
        None => None,
    };
    let end = match &observed.range_end {
        Some(value) => Some(calls.expression(value)?),
        None => None,
    };
    let hypotheses = calls.hypotheses(&observed.hypotheses)?;
    let mut engine = Engine::strict(program, machine, &[]);
    for input in &observed.inputs {
        engine.normalize_scoped_value(input)?;
    }
    if !install_hypotheses(program, &mut engine, &hypotheses)
        || !engine.install_scoped_expression(&subject)
    {
        return None;
    }
    let input = engine.normalize(subject.expression)?;
    let rank = match measure {
        RankingRangeMeasure::Computed {
            subject,
            parameter,
            body,
            ..
        } => RankValue::computed(program, machine, &mut engine, subject, parameter, body)?,
        RankingRangeMeasure::Single(_) => RankValue::scalar(input),
        RankingRangeMeasure::Distance { .. } | RankingRangeMeasure::IncreasingTo { .. } => {
            let paired = paired.as_ref()?;
            if !engine.install_scoped_expression(paired) {
                return None;
            }
            RankValue::scalar(engine.normalize(paired.expression)?.sub(&input))
        }
        _ => return None,
    };
    if engine.requires_unsatisfiable {
        return Some(true);
    }
    if !rank.forms(&mut engine) {
        return Some(false);
    }
    let Some(range) = authored_range else {
        return Some(
            matches!(
                measure,
                RankingRangeMeasure::Distance { .. } | RankingRangeMeasure::IncreasingTo { .. }
            ) || engine.prove_at_least(&engine.substituted(&rank.value), &BigInt::zero()),
        );
    };
    let mut endpoint = |expression: &ScopedArithmeticExpression| -> Option<Polynomial> {
        if !engine.install_scoped_expression(expression) {
            return None;
        }
        let value = engine.normalize(expression.expression)?;
        super::fields::endpoint_lands_under(
            &mut engine,
            program,
            machine,
            root,
            expression.expression,
            &value,
        )
        .then_some(value)
    };
    let floor = endpoint(start.as_ref()?)?;
    let ceiling = endpoint(end.as_ref()?)?;
    Some(super::calls::membership(
        &engine,
        measure,
        &rank.value,
        &floor,
        &ceiling,
        range.end_inclusive,
    ))
}
