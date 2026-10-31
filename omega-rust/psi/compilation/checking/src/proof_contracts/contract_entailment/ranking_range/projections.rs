//! Expressions whose exact metadata/field coordinates enter one range judgment.
use super::{ExpressionHandle, Machine, RankingRangeMeasure, RankingRangePremises, State};
use crate::frozen_program::expression_table;
use typed_trees::expression::{ExpressionNode, TableRangeExpression};

pub(super) fn expressions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    root: &State,
    range: Option<&TableRangeExpression>,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    arguments: Option<&[ExpressionHandle]>,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
) -> Vec<ExpressionHandle> {
    let mut expressions = range.map_or_else(Vec::new, |range| vec![range.start, range.end]);
    super::push_measure_subjects(measure, &mut expressions);
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
/// Depth-bounded projection-expression walk shared by the coordinate
/// installers. The member arm owns each Member node's coordinate work and
/// receiver recursion; every other node uses the shared structural value
/// edges, including already captured call operands. This installs declaration
/// coordinates, not call-result equalities or executable branch hypotheses.
/// `None` ends the walk on a member refusal or a depth/validity breach.
pub(super) fn for_each_projection_expression(
    program: &dyn crate::CheckingInput,
    expressions: &[ExpressionHandle],
    mut member_arm: impl FnMut(
        &typed_trees::expression::TableMemberExpression,
        ExpressionHandle,
        &mut dyn FnMut(ExpressionHandle),
    ) -> Option<()>,
) -> Option<()> {
    let mut pending = expressions
        .iter()
        .map(|expression| (*expression, 0))
        .collect::<Vec<_>>();
    while let Some((expression, depth)) = pending.pop() {
        let node = expression_table(program).expression_at_depth(expression, depth)?;
        let mut visit = |child: ExpressionHandle| {
            if child.is_valid() {
                pending.push((child, depth + 1));
            }
        };
        match node {
            ExpressionNode::Member(member) => member_arm(member, expression, &mut visit)?,
            other => crate::expression_traversal::children(program, other, &mut visit),
        }
    }
    Some(())
}

pub(super) fn entry_expressions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    root: &State,
) -> Vec<ExpressionHandle> {
    let mut expressions = super::required_contract_expressions(program, machine);
    crate::proof_contracts::contract_entailment::ranking_range::constrained_parameter_bound_expressions(
        program,
        root,
        &mut expressions,
        None,
    );
    expressions
}
