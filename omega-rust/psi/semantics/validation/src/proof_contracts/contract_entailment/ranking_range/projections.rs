//! Expressions whose exact metadata/field coordinates enter one range judgment.
use super::{
    ExpressionHandle, Machine, ProofFact, RankingRangeMeasure, RankingRangePremises,
    SignatureContractKind, State, TypeConstraintNode, TypeReferenceNode, TypedTrees,
    exact_integer_parameter,
};
use typed_trees::expression::TableRangeExpression;

pub(super) fn expressions(
    program: &TypedTrees,
    machine: &Machine,
    root: &State,
    range: &TableRangeExpression,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    arguments: Option<&[ExpressionHandle]>,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
) -> Vec<ExpressionHandle> {
    let mut expressions = vec![range.start, range.end];
    match measure {
        RankingRangeMeasure::Single(subject)
        | RankingRangeMeasure::Computed { subject, .. }
        | RankingRangeMeasure::SliceLength(subject)
        | RankingRangeMeasure::Field { subject, .. } => expressions.push(subject),
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => expressions.extend([lower, upper]),
    }
    expressions.extend(arguments.unwrap_or_default());
    expressions.extend(guards.iter().map(|(expression, _)| *expression));
    expressions.extend(evaluated_prefix);
    if arguments.is_none() || !matches!(premises, RankingRangePremises::RankInvariant) {
        expressions.extend(entry_expressions(program, machine, root));
    }
    expressions
}

/// Requires-fact and constrained-parameter endpoint expressions every range
/// judgment normalizes. The cross-machine call owner installs the same set so
/// a `.len` or other metadata coordinate inside an entry fact binds to the
/// same atom as at a named-state edge.
pub(super) fn entry_expressions(
    program: &TypedTrees,
    machine: &Machine,
    root: &State,
) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    for contract in program
        .machine_contracts(machine)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                expressions.push(*expression);
            }
        }
    }
    for parameter in program
        .state_parameters(root)
        .iter()
        .filter(|parameter| !parameter.is_self)
    {
        if exact_integer_parameter(program, parameter.type_reference).is_none() {
            continue;
        }
        let mut reference = parameter.type_reference;
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        {
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Range {
                    minimum, maximum, ..
                } = constraint
                {
                    expressions.extend([*minimum, *maximum]);
                }
            }
            reference = *base_type;
        }
    }
    expressions
}
